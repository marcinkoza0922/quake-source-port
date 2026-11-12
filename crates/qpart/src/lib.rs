//! FTE scripted particle system, ported from vkQuake `r_part_fte.c`.
//!
//! This crate holds the effect registry: particle types parsed from FTE
//! `r_part` configs (`particles/<name>.cfg`) and DP `effectinfo.txt` files,
//! effect lookup with namespaces / redirects / legacy names, and model
//! effect associations (`r_effect` / `r_trail`).
mod effectinfo;
pub mod num;
mod parse;
pub mod types;

pub use types::*;

/// Supplies game files (COM_LoadFile).
pub trait FileSource {
    fn load(&mut self, path: &str) -> Option<Vec<u8>>;
}

impl<F: FnMut(&str) -> Option<Vec<u8>>> FileSource for F {
    fn load(&mut self, path: &str) -> Option<Vec<u8>> {
        self(path)
    }
}

/// Kind of model effect association.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssocKind {
    Trail,
    Emit,
}

/// MOD_EMITREPLACE: the emit effect replaces the model's own rendering.
pub const EMIT_REPLACE: u32 = 1;
/// MOD_EMITFORWARDS: emit along the entity's forward vector.
pub const EMIT_FORWARDS: u32 = 2;

/// `r_effect` / `r_trail` association of an effect with a model name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssociatedEffect {
    pub model: String,
    pub effect: String,
    pub flags: u32,
    pub kind: AssocKind,
}

/// Effects resolved for one model (PScript_UpdateModelEffects).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ModelEffects {
    pub trail: Option<TypeId>,
    pub emit: Option<TypeId>,
    pub flags: u32,
}

const LEGACY_NAMES: &[(&str, &str)] = &[
    ("t_rocket", "TR_ROCKET"),
    ("t_grenade", "TR_GRENADE"),
    ("t_gib", "TR_BLOOD"),
    ("te_plasma", "TE_TEI_PLASMAHIT"),
    ("te_smoke", "TE_TEI_SMOKE"),
];

fn legacy(name: &str) -> &str {
    LEGACY_NAMES.iter().find(|(o, _)| *o == name).map_or(name, |(_, n)| n)
}

/// Splits `cfg.name` at the first dot.
fn split_ns(name: &str) -> (Option<&str>, &str) {
    match name.find('.') {
        Some(i) => (Some(&name[..i]), &name[i + 1..]),
        None => (None, name),
    }
}

/// Particle effect registry.
#[derive(Debug, Default)]
pub struct Registry {
    pub types: Vec<PartType>,
    /// `r_partredirect` aliases (from, to), newest first.
    pub aliases: Vec<(String, String)>,
    /// `r_effect` / `r_trail` associations, newest first.
    pub associated: Vec<AssociatedEffect>,
    /// Configs already loaded (or attempted) since the last kill.
    pub loaded_configs: Vec<String>,
    /// Sound names referenced by loaded effects, for precaching.
    pub sounds_to_precache: Vec<String>,
    /// 256-entry RGB palette used by `rampindex` / `colorindex`.
    pub palette: Option<Box<[u8; 768]>>,
    pub opts: RenderOptions,
    /// Console messages produced while loading (drained by the caller).
    pub messages: Vec<String>,
    /// Set when effect definitions changed (r_plooksdirty).
    pub dirty: bool,
}

impl Registry {
    pub fn new() -> Self {
        Registry::default()
    }

    pub fn with_palette(palette: &[u8]) -> Self {
        let mut r = Registry::new();
        r.set_palette(palette);
        r
    }

    pub fn set_palette(&mut self, palette: &[u8]) {
        let mut p = Box::new([0u8; 768]);
        let n = palette.len().min(768);
        p[..n].copy_from_slice(&palette[..n]);
        self.palette = Some(p);
    }

    pub(crate) fn palette_rgb(&self, idx: usize) -> [f32; 3] {
        let i = (idx & 0xff) * 3;
        match &self.palette {
            Some(p) => [p[i] as f32 / 255.0, p[i + 1] as f32 / 255.0, p[i + 2] as f32 / 255.0],
            None => [1.0; 3],
        }
    }

    pub(crate) fn msg(&mut self, s: String) {
        self.messages.push(s);
    }

    pub fn get(&self, id: TypeId) -> Option<&PartType> {
        self.types.get(id)
    }

    /// P_GetParticleType / P_AllocateParticleType: find or create a type.
    pub fn allocate(&mut self, config: &str, name: &str) -> TypeId {
        let (ns, n) = split_ns(name);
        let config = ns.unwrap_or(config);
        let name = legacy(n);
        if let Some(i) = self
            .types
            .iter()
            .position(|t| t.name.eq_ignore_ascii_case(name) && t.config.eq_ignore_ascii_case(config))
        {
            return i;
        }
        self.types.push(PartType::empty(config, name));
        self.dirty = true;
        self.types.len() - 1
    }

    /// PScript_FindParticleType: public lookup, loading the namespace config
    /// on demand. Returns None when the effect does not exist.
    pub fn find(&mut self, fullname: &str, files: &mut dyn FileSource) -> Option<TypeId> {
        self.find_depth(fullname, files, 0)
    }

    fn resolve_alias<'a>(&'a self, mut name: &'a str) -> Option<&'a str> {
        let mut limit = 5;
        'outer: loop {
            for (from, to) in &self.aliases {
                if from.eq_ignore_ascii_case(name) {
                    if limit == 0 {
                        return None;
                    }
                    limit -= 1;
                    name = to;
                    continue 'outer;
                }
            }
            return Some(name);
        }
    }

    fn find_depth(&mut self, fullname: &str, files: &mut dyn FileSource, depth: u32) -> Option<TypeId> {
        let name = self.resolve_alias(fullname)?.to_string();
        let (cfg, n) = split_ns(&name);
        let cfg = cfg.unwrap_or("").to_string();
        let n = legacy(n).to_string();

        let mut found = None;
        if !cfg.is_empty() {
            found = self
                .types
                .iter()
                .position(|t| t.name.eq_ignore_ascii_case(&n) && t.config.eq_ignore_ascii_case(&cfg));
        } else {
            for (i, t) in self.types.iter().enumerate() {
                if t.name.eq_ignore_ascii_case(&n) {
                    found = Some(i);
                    if t.is_loaded() {
                        break;
                    }
                }
            }
        }
        if let Some(i) = found
            && self.types[i].is_loaded()
        {
            return Some(i);
        }
        if depth > 8 {
            return None;
        }
        if n.len() > 14 && n[..14].eq_ignore_ascii_case("te_explosion2_") {
            let from = self.find_depth(&format!("{cfg}.te_explosion2"), files, depth + 1);
            if let Some(from) = from {
                let to = self.allocate(&cfg, &n);
                self.retint(to, from, &n[14..]);
                return Some(to);
            }
        }
        if !cfg.is_empty() && self.load_set(&cfg, true, true, files) {
            return self.find_depth(fullname, files, depth + 1);
        }
        None
    }

    /// PScript_RetintEffect: copy `from` into `to` with a new palette colour.
    fn retint(&mut self, to: TypeId, from: TypeId, codes: &str) {
        let (name, config) = (self.types[to].name.clone(), self.types[to].config.clone());
        let mut t = self.types[from].clone();
        t.name = name;
        t.config = config;
        let (ci, rest) = num::strtoul(codes, 10);
        let rest = rest.strip_prefix('_').unwrap_or(rest);
        t.colorindex = ci as i32;
        t.colorrand = num::strtoul(rest, 10).0 as i32;
        self.types[to] = t;
        self.dirty = true;
    }

    /// CheckAssosiation: allocate `name`, refusing chains that loop back.
    pub(crate) fn check_association(&mut self, config: &str, name: &str, from: TypeId) -> Option<TypeId> {
        let orig = self.allocate(config, name);
        let mut to = Some(orig);
        let mut guard = 0;
        while let Some(t) = to {
            if t == from || guard > self.types.len() {
                self.msg(format!("Assosiation of {name} would cause infinate loop"));
                return None;
            }
            guard += 1;
            to = self.types[t].assoc;
        }
        Some(orig)
    }

    /// P_LoadParticleSet. `implicit` loads weakly. Returns true when
    /// something was loaded (or "classic" was requested).
    pub fn load_set(&mut self, name: &str, implicit: bool, showwarning: bool, files: &mut dyn FileSource) -> bool {
        if name.is_empty() || self.loaded_configs.iter().any(|c| c == name) {
            return false;
        }
        self.loaded_configs.push(name.to_string());
        if name == "classic" {
            return true;
        }
        let data = files.load(&format!("particles/{name}.cfg")).or_else(|| files.load(&format!("{name}.cfg")));
        if let Some(data) = data {
            self.parse_fte(name, implicit, &String::from_utf8_lossy(&data));
            return true;
        }
        if name == "effectinfo" || name.starts_with("effectinfo_") {
            let font = files.load("particles/particlefont.txt");
            match files.load(&format!("{name}.txt")) {
                Some(data) => {
                    let font = font.map(|f| String::from_utf8_lossy(&f).into_owned());
                    self.import_effectinfo(name, &String::from_utf8_lossy(&data), false, font.as_deref());
                }
                None => self.msg(format!("{name}.txt not found")),
            }
            return true;
        }
        if showwarning {
            self.msg(format!("Warning: Couldn't find particle description {name}"));
        }
        false
    }

    /// R_Particles_KillAllEffects.
    pub fn kill_all(&mut self) {
        for t in &mut self.types {
            t.texname.clear();
            t.scale = 0.0;
            t.loaded = Loaded::No;
        }
        // C clears only part_type[0]'s ramp (a bug); clearing all is harmless
        // since every reload resets the type first.
        for t in &mut self.types {
            t.ramp.clear();
            t.rampmode = RampMode::None;
        }
        self.loaded_configs.clear();
        self.dirty = true;
    }

    /// R_ParticleDesc_Callback: reload from an `r_particledesc` value, then
    /// the per-map `map_<mapname>` config when a map is given.
    pub fn set_desc(&mut self, desc: &str, map: Option<&str>, files: &mut dyn FileSource) {
        self.kill_all();
        let mut s = desc;
        while let Some((tok, rest)) = qcore::cmd::com_parse(s) {
            if !tok.is_empty() {
                self.load_set(tok, false, true, files);
            }
            s = rest;
        }
        if let Some(map) = map {
            self.load_set(&format!("map_{map}"), false, false, files);
        }
    }

    /// `r_effect` / `r_trail` (PScript_AssociateEffect_f).
    pub fn associate_effect(&mut self, args: &qcore::cmd::Args) {
        let model = args.argv(1).to_string();
        let effect = args.argv(2).to_string();
        let mut flags = 0;
        let kind = if args.argv(0) == "r_trail" {
            AssocKind::Trail
        } else {
            for i in 3..args.argc() {
                match args.argv(i) {
                    "replace" | "1" => flags |= EMIT_REPLACE,
                    "forwards" | "forward" => flags |= EMIT_FORWARDS,
                    "0" => {}
                    f => self.msg(format!("{} {model}: unknown flag {f}", args.argv(0))),
                }
            }
            AssocKind::Emit
        };
        if ["player", "eyes", "flag", "tf_stan", ".bsp", "turr"].iter().any(|s| model.contains(s)) {
            self.msg(format!("Sorry: Not allowed to attach effects to model \"{model}\""));
            return;
        }
        if model.len() >= 64 || effect.len() >= 64 {
            return;
        }
        let ae = AssociatedEffect { model, effect, flags, kind };
        match self.associated.iter_mut().find(|a| a.model == ae.model && a.kind == ae.kind) {
            Some(a) => *a = ae,
            None => self.associated.insert(0, ae),
        }
        self.dirty = true;
    }

    /// `r_partredirect from [to]` (P_PartRedirect_f). Returns console output.
    pub fn redirect(&mut self, args: &qcore::cmd::Args) -> Vec<String> {
        let (from, to) = (args.argv(1), args.argv(2));
        if from.is_empty() {
            return self.aliases.iter().map(|(f, t)| format!("{f} -> {t}")).collect();
        }
        if let Some(i) = self.aliases.iter().position(|(f, _)| f.eq_ignore_ascii_case(from)) {
            if args.argc() == 2 {
                let (f, t) = &self.aliases[i];
                return vec![format!("particle {f} is currently remapped to {t}")];
            }
            self.aliases.remove(i);
        }
        if !to.is_empty() && !from.eq_ignore_ascii_case(to) {
            self.aliases.insert(0, (from.to_string(), to.to_string()));
        }
        self.dirty = true;
        Vec::new()
    }

    /// PScript_UpdateModelEffects for one model name.
    pub fn model_effects(&mut self, model: &str, files: &mut dyn FileSource) -> ModelEffects {
        let mut me = ModelEffects::default();
        let assoc: Vec<_> = self.associated.iter().filter(|a| a.model == model).cloned().collect();
        for a in assoc {
            match a.kind {
                AssocKind::Trail => me.trail = self.find(&a.effect, files),
                AssocKind::Emit => {
                    me.emit = self.find(&a.effect, files);
                    me.flags = a.flags;
                }
            }
        }
        me
    }
}

#[cfg(test)]
mod tests;
