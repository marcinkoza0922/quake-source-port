//! Savegames (Host_Savegame_f / Host_Loadgame_f): ironwail-compatible
//! version 5 text format.

use qcore::cmd::com_parse;
use qvm::{EType, Vm};

use crate::ents::Ents;
use crate::server::Server;

const SAVEGAME_VERSION: i32 = 5;
const NUM_SPAWN_PARMS: usize = 16;
const MAX_LIGHTSTYLES: usize = 64;

/// PR_UglyValueString for savegames.
fn ugly_value(vm: &Vm, ty: EType, v: &[u32]) -> String {
    match ty {
        EType::String => qcore::text::decode(vm.strings.get(v[0] as i32).unwrap_or(b"")).replace('\n', "\\n").replace('"', "'"),
        EType::Entity => format!("{}", v[0] as i32),
        EType::Function => vm.progs.function_name(v[0] as usize),
        EType::Field => {
            let n = vm.progs.field_at_ofs(v[0] as usize).map(|d| d.name).unwrap_or(0);
            String::from_utf8_lossy(vm.progs.string(n)).into_owned()
        }
        EType::Float => format!("{:.6}", f32::from_bits(v[0])),
        EType::Vector => format!("{:.6} {:.6} {:.6}", f32::from_bits(v[0]), f32::from_bits(v[1]), f32::from_bits(v[2])),
        _ => "void".into(),
    }
}

/// Parses `{ "k" "v" ... }` blocks.
fn parse_blocks(mut data: &str) -> Result<Vec<Vec<(String, String)>>, String> {
    let mut out = Vec::new();
    while let Some((tok, rest)) = com_parse(data) {
        data = rest;
        if tok != "{" {
            return Err(format!("expected {{, found {tok}"));
        }
        let mut pairs = Vec::new();
        loop {
            let (k, rest) = com_parse(data).ok_or("EOF without closing brace")?;
            data = rest;
            if k == "}" {
                break;
            }
            let (v, rest) = com_parse(data).ok_or("EOF without closing brace")?;
            data = rest;
            pairs.push((k.to_string(), v.to_string()));
        }
        out.push(pairs);
    }
    Ok(out)
}

impl Server {
    /// Savegame comment: level name padded to 22 chars + kill count, spaces as '_'.
    fn savegame_comment(&self) -> String {
        let Some(l) = &self.level else { return String::new() };
        let msg = l.vm.field_ofs("message").and_then(|o| l.vm.ent_str(0, o).ok()).unwrap_or(b"");
        let mut s: String = qcore::text::decode(msg).chars().take(22).collect();
        while s.chars().count() < 22 {
            s.push(' ');
        }
        let st = self.stats();
        s += &format!("kills:{:3}/{:3}", st.killed_monsters, st.total_monsters);
        s.chars().map(|c| if c == ' ' || c == '\n' { '_' } else { c }).take(39).collect()
    }

    /// Host_Savegame_f contents. Encode with `qcore::text::encode` for disk.
    pub fn save_game(&self) -> String {
        let Some(l) = &self.level else { return String::new() };
        let vm = &l.vm;
        let c = &self.core;
        let mut f = format!("{SAVEGAME_VERSION}\n{}\n", self.savegame_comment());
        for k in 0..NUM_SPAWN_PARMS {
            f += &format!("{:.6}\n", c.clients.first().map_or(0.0, |cl| cl.spawn_parms[k]));
        }
        f += &format!("{}\n{}\n{:.6}\n", c.cvars.value_of("skill") as i32, c.sv.name, vm.time);
        for i in 0..MAX_LIGHTSTYLES {
            match c.sv.lightstyles.get(i).filter(|s| !s.is_empty()) {
                Some(s) => f += &format!("{}\n", String::from_utf8_lossy(s)),
                None => f += "m\n",
            }
        }
        f += "{\n";
        for d in &vm.progs.globaldefs {
            if !d.save_global() {
                continue;
            }
            let ty = EType::from_raw(d.ty & !(1 << 15));
            if !matches!(ty, EType::String | EType::Float | EType::Entity) {
                continue;
            }
            let o = d.ofs as usize;
            let name = String::from_utf8_lossy(vm.progs.string(d.name));
            f += &format!("\"{name}\" \"{}\"\n", ugly_value(vm, ty, &vm.globals[o..o + ty.size()]));
        }
        f += "}\n";
        for e in 0..vm.num_edicts() as u32 {
            f += "{\n";
            if !vm.is_free(e) {
                let ed = vm.edict(e);
                for d in vm.progs.fielddefs.iter().skip(1) {
                    let name = vm.progs.string(d.name);
                    if name.len() > 1 && name[name.len() - 2] == b'_' {
                        continue;
                    }
                    let ty = EType::from_raw(d.ty & !(1 << 15));
                    let o = d.ofs as usize;
                    let Some(v) = ed.get(o..o + ty.size()) else { continue };
                    if v.iter().all(|&x| x == 0) {
                        continue;
                    }
                    f += &format!("\"{}\" \"{}\"\n", String::from_utf8_lossy(name), ugly_value(vm, ty, v));
                }
            }
            f += "}\n";
        }
        self.wrap_save(f)
    }

    /// Host_Loadgame_f: spawns the saved map and restores globals/edicts.
    /// Decode disk bytes with `qcore::text::decode` first. The local client
    /// must be (re)connected afterwards.
    pub fn load_game(&mut self, text: &str) -> Result<(), String> {
        let (addons, text) = crate::addons::save::split(text)?;
        let mut lines = text.split_inclusive('\n');
        let mut next = || lines.next().map(|l| l.trim_end_matches(['\n', '\r']).to_string()).ok_or("unexpected end of savegame");
        let version: i32 = next()?.trim().parse().map_err(|_| "bad version")?;
        if version != SAVEGAME_VERSION {
            return Err(format!("savegame is version {version}, not {SAVEGAME_VERSION}"));
        }
        let _comment = next()?;
        let mut parms = [0f32; NUM_SPAWN_PARMS];
        for p in &mut parms {
            *p = next()?.trim().parse().unwrap_or(0.0);
        }
        let skill: f32 = next()?.trim().parse().unwrap_or(1.0);
        let mapname = next()?.trim().to_string();
        let time: f64 = next()?.trim().parse().unwrap_or(0.0);
        let mut styles = Vec::new();
        for _ in 0..MAX_LIGHTSTYLES {
            styles.push(next()?.trim().as_bytes().to_vec());
        }
        let consumed: usize = text.split_inclusive('\n').take(5 + NUM_SPAWN_PARMS + MAX_LIGHTSTYLES).map(str::len).sum();
        let blocks = parse_blocks(&text[consumed..])?;

        for i in 0..self.core.maxclients() {
            self.drop_client(i, true);
        }
        self.core.cvars.set_value("skill", skill.floor().clamp(0.0, 3.0));
        self.spawn_server(&mapname)?;
        let Some(l) = &mut self.level else { return Err("no level".into()) };
        let c = &mut self.core;
        c.sv.paused = true;
        c.sv.loadgame = true;
        c.sv.loadgame_parms = parms;
        for (i, s) in styles.into_iter().enumerate() {
            if i < c.sv.lightstyles.len() {
                c.sv.lightstyles[i] = s;
            }
        }
        for e in 0..l.vm.num_edicts() as u32 {
            l.world.unlink(e);
        }
        l.vm.clear_edicts();
        let mut it = blocks.into_iter();
        if let Some(g) = it.next() {
            let mut h = crate::host::Ctx::new(&mut *c, &mut l.world);
            l.vm.parse_globals(&mut h, &g).map_err(|e| e.to_string())?;
        }
        let mut live = Vec::new();
        for (e, pairs) in it.enumerate() {
            let e = e as u32;
            l.vm.ed_alloc_at(e).map_err(|e| e.to_string())?;
            let mut h = crate::host::Ctx::new(&mut *c, &mut l.world);
            let p = l.vm.parse_edict(&mut h, e, &pairs).map_err(|e| e.to_string())?;
            if p.init {
                live.push(e);
            }
        }
        l.vm.time = time;
        for e in live {
            if e != 0 {
                l.world.link_edict(&mut Ents { vm: &mut l.vm, c: &mut *c }, e, false);
            }
        }
        c.clients[0].spawn_parms = parms;
        // a game's add-ons come from its save: none for a plain .sav
        c.set_addons(addons.is_some());
        if let Some(h) = addons {
            for t in crate::addons::Toy::ALL {
                c.cvars.set(t.cvar(), if h.toys.contains(&t) { "1" } else { "0" });
            }
            crate::addons::settings::load(&mut c.cvars, &h.extra);
            l.portals.load_text(&h.portals);
            l.stickies.load_text(&h.extra);
            l.gel.load_text(&h.extra);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use qcore::cvar::Cvars;
    use qcore::fs::Fs;

    use crate::Server;

    fn basedir() -> Option<PathBuf> {
        let d = std::env::var_os("QUAKE_BASEDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
        d.join("id1").is_dir().then_some(d)
    }

    #[test]
    fn save_load_roundtrip() {
        let Some(base) = basedir() else { return };
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        sv.connect_local().unwrap();
        for _ in 0..20 {
            sv.frame(1.0 / 72.0);
        }
        let message = b"\xa0\xd0\xe5\xee\xe4\xf2\xe1\xe7\xef\xee\xa0\n\x9e\x9d\x9d\xc3\xa9";
        let l = sv.level.as_mut().unwrap();
        let ofs = l.vm.field_ofs("message").unwrap();
        // Exercise the entity parser as well as save serialization, as with
        // a colored label from a BSP or an external .ent override.
        let pairs = vec![("message".into(), qcore::text::decode(message).replace('\n', "\\n"))];
        let mut h = crate::host::Ctx::new(&mut sv.core, &mut l.world);
        l.vm.parse_edict(&mut h, 0, &pairs).unwrap();
        assert_eq!(l.vm.ent_str(0, ofs).unwrap(), message);
        let text = sv.save_game();
        assert!(text.starts_with("5\n"));
        let n = sv.level.as_ref().unwrap().vm.num_edicts();
        let bytes = qcore::text::encode(&text); // actual v5 on-disk bytes
        assert!(bytes.windows(11).any(|w| w == &message[..11]));
        sv.load_game(&qcore::text::decode(&bytes)).unwrap();
        sv.connect_local().unwrap();
        sv.frame(1.0 / 72.0);
        assert!(sv.error().is_none());
        let l = sv.level.as_ref().unwrap();
        assert_eq!(l.vm.ent_str(0, ofs).unwrap(), message);
        assert_eq!(l.vm.num_edicts(), n);
        assert_eq!(sv.save_game().lines().count(), text.lines().count());
    }
}
