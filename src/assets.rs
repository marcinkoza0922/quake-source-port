//! Per-level assets: models/sounds from the client precache lists loaded
//! into qrender/qsound, plus the parsed BSP (ambient levels, worldspawn keys).

use std::collections::HashMap;
use std::sync::Arc;

use qclient::ClientState;
use qcore::fs::Fs;
use qformats::{Bsp, Mdl, Spr, parse_entities};
use qrender::{Fog, LevelBsp, ModelId, Renderer};
use qsound::{Sfx, SoundSystem};

mod external_textures;
pub(crate) mod md3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Brush,
    Alias,
    Sprite,
}

#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub id: ModelId,
    pub kind: Kind,
    pub flags: u32,
    pub md3: bool,
}

#[derive(Default)]
pub struct Level {
    pub mapname: String,
    pub enhanced_models: bool,
    /// Indexed by client model index.
    pub models: Vec<Option<Model>>,
    /// Indexed by client sound index.
    pub sounds: Vec<Option<Arc<Sfx>>>,
    /// Models by name (temp entities, beams).
    pub by_name: HashMap<String, Option<Model>>,
    pub bsp: Option<Bsp>,
    /// Lazily initialized BSP collision model shared across client FX systems.
    pub cm: std::sync::OnceLock<qworld::CollisionModel>,
    pub world_fog: Fog,
    pub world_sky: String,
    /// Skybox in use (worldspawn, svc_skybox or the `sky` command).
    pub sky: String,
    /// The skybox's wind (`gfx/env/<sky>wind.cfg`, `skywind` commands).
    pub sky_wind: crate::skywind::SkyWind,
    /// Worldspawn `[_]wateralpha`/`lavaalpha`/`slimealpha`/`telealpha`
    /// (R_ParseWorldspawn), in that order.
    pub liquid_alpha: [Option<f32>; 4],
    /// Visible-weapon rig and models.
    pub vwep: crate::vwep::Cache,
}

fn load_model(fs: &Fs, r: &mut Renderer, name: &str, enhanced_models: bool) -> Option<Model> {
    let data = fs.open(name)?;
    if data.len() >= 4 && &data[..4] == b"IDPO" {
        let m = Mdl::parse(&data).ok()?;
        let flags = m.flags as u32;
        let replacement = enhanced_models
            .then(|| md3::load_replacement(fs, r, name, &m).or_else(|| md3::load_md5_replacement(fs, r, name, &m)))
            .flatten();
        if let Some(id) = replacement {
            return Some(Model { id, kind: Kind::Alias, flags, md3: true });
        }
        let id = r.load_alias(&m).ok()?;
        return Some(Model { id, kind: Kind::Alias, flags, md3: false });
    }
    if data.len() >= 4 && &data[..4] == b"IDP3" {
        let m = qformats::Md3::parse(&data).ok()?;
        let id = md3::load_md3(fs, r, name, &m, false)?;
        return Some(Model { id, kind: Kind::Alias, flags: 0, md3: true });
    }
    if data.len() >= 4 && &data[..4] == b"IDSP" {
        let s = Spr::parse(&data).ok()?;
        let id = r.load_sprite(&s).ok()?;
        return Some(Model { id, kind: Kind::Sprite, flags: 0, md3: false });
    }
    None
}

pub struct LevelLoad<'a> {
    pub colored: bool,
    pub enhanced_models: bool,
    pub external_entities: Option<&'a str>,
    pub reload_bsp: Option<Bsp>,
}

impl Level {
    /// Lazily builds and caches the BSP collision model for this level.
    pub fn collision_model(&self) -> Option<&qworld::CollisionModel> {
        self.bsp
            .as_ref()
            .map(|bsp| self.cm.get_or_init(|| qworld::CollisionModel::from_bsp(bsp)))
    }

    /// Loads everything in `cl`'s precache lists; sets `cl.model_flags`.
    pub fn load(
        fs: &Fs,
        r: &mut Renderer,
        snd: Option<&mut SoundSystem>,
        cl: &mut ClientState,
        options: LevelLoad<'_>,
        print: &mut dyn FnMut(&str),
    ) -> Level {
        let LevelLoad { colored, enhanced_models, external_entities, reload_bsp } = options;
        let mut lv = Level { mapname: cl.mapname.clone(), enhanced_models, ..Default::default() };
        let world = cl.model_precache.get(1).cloned().unwrap_or_default();
        let mut bsp = reload_bsp.or_else(|| {
            fs.open(&world).and_then(|d| Bsp::parse(&d).map_err(|e| print(&format!("{world}: {e}\n"))).ok())
        });
        if let (Some(bsp), Some(ents)) = (&mut bsp, external_entities) {
            bsp.entities = ents.to_owned();
        }
        let mut subs = Vec::new();
        // External brush models (maps/b_*.bsp) load together with the world.
        let mut brush_names = Vec::new();
        let mut brushes = Vec::new();
        for name in cl.model_precache.iter().skip(2) {
            if name.starts_with('*') || !name.ends_with(".bsp") {
                continue;
            }
            let Some(d) = fs.open(name) else { continue };
            if d.starts_with(b"IDPO") || d.starts_with(b"IDSP") || d.starts_with(b"IDP3") {
                continue;
            }
            match Bsp::parse(&d) {
                Ok(b) => {
                    let lit = load_lit(fs, name, &b, colored);
                    let ext = external_textures::load(fs, name, &b, false);
                    brush_names.push(name.clone());
                    brushes.push((b, lit, ext));
                }
                Err(e) => print(&format!("{name}: {e}\n")),
            }
        }
        let mut brush_models = HashMap::new();
        if let Some(b) = &bsp {
            let lit = load_lit(fs, &world, b, colored);
            let ext = external_textures::load(fs, &world, b, true);
            let level = LevelBsp { bsp: b, lit: lit.as_deref(), external: &ext };
            let refs: Vec<_> =
                brushes.iter().map(|(b, l, e)| LevelBsp { bsp: b, lit: l.as_deref(), external: e }).collect();
            match r.load_level(level, &refs) {
                Ok((s, ext)) => {
                    subs = s;
                    brush_models = brush_names.into_iter().zip(ext).collect();
                }
                Err(e) => print(&format!("load_world: {e:?}\n")),
            }
            if let Ok(ents) = parse_entities(&b.entities)
                && let Some(w) = ents.first()
            {
                if let Some(f) = w.get("fog") {
                    lv.world_fog = qrender::parse_fog(f);
                }
                for (i, k) in ["wateralpha", "lavaalpha", "slimealpha", "telealpha"].iter().enumerate() {
                    if let Some(v) = w.get(k).or_else(|| w.get(&format!("_{k}"))) {
                        lv.liquid_alpha[i] = Some(qcore::cvar::q_atof(v));
                    }
                }
                if let Some(s) = w.get("sky").or(w.get("skyname")) {
                    lv.world_sky = s.to_string();
                }
            }
        }
        lv.bsp = bsp;
        let sky = if cl.skybox.is_empty() { lv.world_sky.clone() } else { cl.skybox.clone() };
        lv.set_sky(fs, r, &sky);
        lv.models = vec![None; cl.model_precache.len()];
        for (i, name) in cl.model_precache.iter().enumerate().skip(2) {
            let m = if let Some(n) = name.strip_prefix('*') {
                n.parse::<usize>().ok().and_then(|n| subs.get(n.wrapping_sub(1))).map(|&id| Model {
                    id,
                    kind: Kind::Brush,
                    flags: 0,
                    md3: false,
                })
            } else if let Some(&id) = brush_models.get(name) {
                Some(Model { id, kind: Kind::Brush, flags: 0, md3: false })
            } else {
                let m = load_model(fs, r, name, enhanced_models);
                if m.is_none() && !name.ends_with(".bsp") {
                    print(&format!("couldn't load model {name}\n"));
                }
                m
            };
            if let Some(m) = m {
                cl.model_flags[i] = m.flags;
            }
            lv.models[i] = m;
            lv.by_name.insert(name.clone(), m);
        }
        if let Some(s) = snd {
            lv.sounds = cl.sound_precache.iter().map(|n| if n.is_empty() { None } else { s.precache(n) }).collect();
        }
        lv
    }

    pub fn set_sky(&mut self, fs: &Fs, r: &mut Renderer, name: &str) {
        let faces = if name.is_empty() { None } else { qrender::load_skybox(fs, name) };
        let _ = r.set_skybox(faces.as_ref());
        let sky = if faces.is_some() { name.to_string() } else { String::new() };
        // Sky_LoadSkyBox keeps a loaded box's (possibly tuned) wind
        if sky != self.sky || sky.is_empty() {
            self.sky_wind = self.load_sky_wind(fs, &sky);
        }
        self.sky = sky;
    }

    /// Skywind_Load_f: the skybox's wind config, or still air.
    pub fn load_sky_wind(&self, fs: &Fs, sky: &str) -> crate::skywind::SkyWind {
        if sky.is_empty() {
            return Default::default();
        }
        fs.open(&crate::skywind::cfg_path(sky))
            .and_then(|d| crate::skywind::SkyWind::parse(&String::from_utf8_lossy(&d)))
            .unwrap_or_default()
    }

    /// Model by name, loading on first use (temp entity models).
    pub fn named(&mut self, fs: &Fs, r: &mut Renderer, name: &str) -> Option<Model> {
        if let Some(m) = self.by_name.get(name) {
            return *m;
        }
        let m = load_model(fs, r, name, self.enhanced_models);
        self.by_name.insert(name.to_string(), m);
        m
    }
}

/// Mod_LoadLighting: a `.lit` only applies when it comes from the same or a
/// higher-priority game dir than the map (so id1's start.lit never lights
/// another game's start.bsp). Falls back to embedded RGB (Quake64, BSPX
/// RGBLIGHTING). `colored` false (r_coloredlight 0) keeps the mono lightmap.
fn load_lit(fs: &qcore::fs::Fs, bsp_path: &str, b: &Bsp, colored: bool) -> Option<Vec<u8>> {
    if !colored {
        return None;
    }
    fs.open_companion(bsp_path, &format!("{}.lit", bsp_path.trim_end_matches(".bsp")))
        .and_then(|d| qformats::bsp::parse_lit(&d).ok())
        .filter(|l| l.len() == b.lighting.len() * 3)
        .or_else(|| b.rgb_lighting.clone())
}
