//! Savegames (Host_Savegame_f / Host_Loadgame_f): ironwail-compatible
//! version 5 text format.

use qcore::cmd::com_parse;
use qvm::{EType, Vm};

use crate::ents::Ents;
use crate::server::Server;

const SAVEGAME_VERSION: i32 = 5;
/// 2021 re-release (KEX) saves: version 5 plus a game-dir line.
const SAVEGAME_VERSION_KEX: i32 = 6;
const NUM_SPAWN_PARMS: usize = 16;
const MAX_LIGHTSTYLES: usize = 64;

/// PR_UglyValueString for savegames.
fn ugly_value(vm: &Vm, ty: EType, v: &[u32]) -> String {
    match ty {
        // Raw like PR_UglyValueString ("%s"); loaders read newlines inside
        // quotes. A '"' would end the token early, so (unlike C) it becomes '\''.
        EType::String => qcore::text::decode(vm.strings.get(v[0] as i32).unwrap_or(b"")).replace('"', "'"),
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

/// The precache lines of a vkQuake-style extended save ([`Server::precache_text`]).
fn parse_precaches(text: &str) -> Vec<(bool, usize, String)> {
    let Some(at) = text.find("\n/*\n").or_else(|| text.find("\n/*\r\n")) else { return Vec::new() };
    let line = |l: &str| {
        let (key, rest) = com_parse(l)?;
        let model = match key {
            "sv.model_precache" => true,
            "sv.sound_precache" => false,
            _ => return None,
        };
        let (i, rest) = com_parse(rest)?;
        let (name, _) = com_parse(rest)?;
        Some((model, i.parse().ok()?, name.to_string()))
    };
    text[at + 3..].lines().take_while(|l| !l.starts_with("*/")).filter_map(line).collect()
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
        f += &format!("{}\n{}\n{:.6}\n", c.sv.skill, c.sv.name, vm.time);
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
        f += &self.precache_text();
        self.wrap_save(f)
    }

    /// Models and sounds precached during play, as vkQuake saves them: a
    /// comment to engines that don't read it. Saved model indices refer to them.
    fn precache_text(&self) -> String {
        let sv = &self.core.sv;
        let [models, sounds] = sv.spawn_precaches;
        let mut t = String::new();
        for (key, list, from) in [("sv.model_precache", &sv.model_precache, models), ("sv.sound_precache", &sv.sound_precache, sounds)] {
            for (i, n) in list.iter().enumerate().skip(from) {
                // (a name that would end the line or the comment is lost)
                let ok = !n.contains(['"', '\n', '\r']) && !n.contains("*/");
                t += &format!("{key} {i} \"{}\"\n", if ok { n } else { "" });
            }
        }
        if t.is_empty() { t } else { format!("/*\n// QuakeSpasm extended savegame\n{t}*/\n") }
    }

    /// Reads a save without touching the running game, so a damaged or foreign
    /// file costs nothing. Decode disk bytes with `qcore::text::decode` first.
    pub fn parse_save(&self, text: &str) -> Result<ParsedSave, String> {
        let (addons, text) = crate::addons::save::split(text)?;
        let mut lines = text.split_inclusive('\n');
        let mut next = || lines.next().map(|l| l.trim_end_matches(['\n', '\r']).to_string()).ok_or("unexpected end of savegame");
        let version: i32 = next()?.trim().parse().map_err(|_| "bad version")?;
        if version == SAVEGAME_VERSION_KEX {
            // 2021 re-release: the game dir follows the version, then the
            // version 5 layout (ironwail loads these too).
            let game = next()?.trim().to_string();
            let current = self.core.fs.game_dir().unwrap_or("id1");
            if !game.eq_ignore_ascii_case(current) {
                return Err(format!("savegame is for \"{game}\"; switch with `game {game}` first"));
            }
        } else if version != SAVEGAME_VERSION {
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
        let header = 5 + NUM_SPAWN_PARMS + MAX_LIGHTSTYLES + usize::from(version == SAVEGAME_VERSION_KEX);
        let consumed: usize = text.split_inclusive('\n').take(header).map(str::len).sum();
        let blocks = parse_blocks(&text[consumed..])?;
        let precaches = parse_precaches(&text[consumed..]);
        if !self.core.fs.exists(&format!("maps/{mapname}.bsp")) {
            return Err(format!("maps/{mapname}.bsp not found"));
        }
        Ok(ParsedSave { addons, parms, skill, mapname, time, styles, blocks, precaches })
    }

    /// [`Self::parse_save`] and [`Self::load_parsed`].
    pub fn load_game(&mut self, text: &str) -> Result<(), String> {
        let save = self.parse_save(text)?;
        self.load_parsed(save)
    }

    /// Host_Loadgame_f: spawns the saved map and restores globals/edicts. The
    /// local client must be (re)connected afterwards.
    pub fn load_parsed(&mut self, save: ParsedSave) -> Result<(), String> {
        let ParsedSave { addons, parms, skill, mapname, time, styles, blocks, precaches } = save;
        for i in 0..self.core.maxclients() {
            self.drop_client(i, true);
        }
        // a split-screen / co-op save brings back its player slots and rules;
        // the host rejoins the local players from `cl_splitscreen`
        let players = addons.as_ref().and_then(|h| h.players);
        let slots = players.map_or(1, |p| p.slots);
        if slots != self.core.maxclients() {
            self.set_maxclients(slots);
        }
        let cv = &mut self.core.cvars;
        if let Some(p) = players {
            cv.set_value("coop", p.coop);
            cv.set_value("deathmatch", p.deathmatch);
            cv.set_value("teamplay", p.teamplay);
            cv.set_value("cl_splitscreen", p.local.clamp(1, slots) as f32);
        } else {
            // other saves are single player
            cv.set_value("coop", 0.0);
            cv.set_value("deathmatch", 0.0);
        }
        // the saved models and entities are those of a map with monsters
        self.core.cvars.set("nomonsters", "0");
        self.core.cvars.set_value("skill", skill.floor().clamp(0.0, 3.0));
        self.spawn_server(&mapname)?;
        // A save from other progs fails part way through its edicts: Host_Error
        // in C. Never leave the half-restored level running (it could be saved).
        self.restore_precaches(&precaches);
        let restored = self.restore(parms, time, styles, blocks, addons);
        if restored.is_err() {
            self.level = None;
        }
        restored
    }

    /// Late precaches of a save, at the indices they had.
    fn restore_precaches(&mut self, list: &[(bool, usize, String)]) {
        let Some(l) = &mut self.level else { return };
        let mut h = crate::host::Ctx::new(&mut self.core, &mut l.world);
        for (model, i, name) in list {
            let len = if *model { h.c.sv.model_precache.len() } else { h.c.sv.sound_precache.len() };
            // earlier ones are the level's own (vkQuake lists them all)
            if *i != len {
                continue;
            }
            if *model {
                let _ = h.late_precache_model(&l.vm, name.clone());
            } else {
                h.c.sv.sound_precache.push(name.clone());
            }
        }
    }

    fn restore(
        &mut self,
        parms: [f32; NUM_SPAWN_PARMS],
        time: f64,
        styles: Vec<Vec<u8>>,
        blocks: Vec<Vec<(String, String)>>,
        addons: Option<crate::addons::save::Header>,
    ) -> Result<(), String> {
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
        c.sv.loadgame_players = vec![false; c.clients.len()];
        for (e, pairs) in it.enumerate() {
            let e = e as u32;
            l.vm.ed_alloc_at(e).map_err(|e| e.to_string())?;
            let mut h = crate::host::Ctx::new(&mut *c, &mut l.world);
            let p = l.vm.parse_edict(&mut h, e, &pairs).map_err(|e| e.to_string())?;
            if p.init {
                live.push(e);
                // an unused slot's edict has the odd field too, but was never
                // put in the game
                if let Some(slot) = (e as usize).checked_sub(1).and_then(|i| c.sv.loadgame_players.get_mut(i)) {
                    *slot = !l.vm.classname(e).is_empty();
                }
            }
        }
        l.vm.time = time;
        for e in live {
            if e != 0 {
                l.world.link_edict(&mut Ents { vm: &mut l.vm, c: &mut *c }, e, false);
            }
        }
        c.clients[0].spawn_parms = parms;
        // a game's customization comes from its save: none for a plain .sav
        c.set_custom(addons.as_ref().is_some_and(|h| !h.vanilla));
        if let Some(h) = addons.filter(|h| !h.vanilla) {
            for t in crate::addons::Toy::ALL {
                c.cvars.set(t.cvar(), if h.toys.contains(&t) { "1" } else { "0" });
            }
            crate::addons::settings::load(&mut c.cvars, &h.extra);
            crate::modifiers::load_text(&mut c.cvars, &h.extra);
            l.portals.load_text(&h.portals);
            l.portals.sync_links(&mut c.linked_portals);
            l.stickies.load_text(&h.extra);
            l.gel.load_text(&h.extra);
            l.akimbo.load_text(&h.extra);
            l.rifle.load_text(&h.extra);
        }
        Ok(())
    }
}

/// A save file's contents ([`Server::parse_save`]).
pub struct ParsedSave {
    addons: Option<crate::addons::save::Header>,
    parms: [f32; NUM_SPAWN_PARMS],
    skill: f32,
    mapname: String,
    time: f64,
    styles: Vec<Vec<u8>>,
    blocks: Vec<Vec<(String, String)>>,
    /// (model else sound, index, name)
    precaches: Vec<(bool, usize, String)>,
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

    /// A save whose edicts don't fit the progs (ironwail: Host_Error) must not
    /// leave a half-restored level running.
    #[test]
    fn failed_restore_shuts_the_level_down() {
        let Some(base) = basedir() else { return };
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        sv.connect_local().unwrap();
        sv.spawn_server("e1m1").unwrap();
        let text = sv.save_game().replacen("\"classname\" \"light\"", "\"think\" \"no_such_function\"", 1);
        assert!(text.contains("no_such_function"));
        assert!(sv.load_game(&text).is_err());
        assert!(sv.level.is_none());
        assert!(sv.save_game().is_empty());
    }

    /// The level's skill is saved, not the cvar the next level will use: the
    /// model indices in the save only fit the entities of that skill.
    #[test]
    fn skill_changed_during_the_level() {
        let Some(base) = basedir() else { return };
        for nomonsters in ["0", "1"] {
            let mut fs = Fs::new(base.clone());
            fs.add_game_dir("id1");
            let mut sv = Server::new(fs, Cvars::new(), 1);
            sv.core.cvars.set("skill", "0");
            sv.spawn_server("e1m1").unwrap();
            sv.connect_local().unwrap();
            sv.frame(1.0 / 72.0);
            sv.core.cvars.set("skill", "3");
            let text = sv.save_game();
            sv.core.cvars.set("nomonsters", nomonsters);
            sv.load_game(&text).unwrap();
            assert_eq!(sv.core.sv.skill, 0);
            let (model, index) = (sv.core.sv.fields.model, sv.core.sv.fields.modelindex);
            let vm = &sv.level.as_ref().unwrap().vm;
            let mut seen = 0;
            for e in (2..vm.num_edicts() as u32).filter(|&e| !vm.is_free(e)) {
                let name = String::from_utf8_lossy(vm.ent_str(e, model).unwrap()).into_owned();
                let i = vm.ent_f(e, index) as usize;
                if i != 0 {
                    assert_eq!(sv.core.sv.model_precache[i], name, "edict {e}");
                    seen += 1;
                }
            }
            assert!(seen > 50);
        }
    }

    /// A model set during play is there again after a load.
    #[test]
    fn late_precaches_survive_a_load() {
        let Some(base) = basedir() else { return };
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        sv.connect_local().unwrap();
        assert!(!sv.save_game().contains("/*"), "vanilla saves stay as they are");
        let (n, sounds) = (sv.core.sv.model_precache.len(), sv.core.sv.sound_precache.len());
        sv.restore_precaches(&[(true, n, "progs/shambler.mdl".into()), (false, sounds, "shambler/sattck1.wav".into())]);
        let l = sv.level.as_mut().unwrap();
        l.vm.set_ent_f(1, sv.core.sv.fields.modelindex, n as f32);
        let text = sv.save_game();
        assert!(text.contains(&format!("sv.model_precache {n} \"progs/shambler.mdl\"\n")));
        let text = format!("{text}// qsp_level_damage 0\n");
        sv.load_game(&text).unwrap();
        sv.connect_local().unwrap();
        sv.frame(1.0 / 72.0);
        assert!(sv.error().is_none());
        assert_eq!(sv.core.sv.model_precache[n], "progs/shambler.mdl");
        assert!(sv.core.sv.models[n].is_some());
        assert_eq!(sv.core.sv.sound_precache[sounds], "shambler/sattck1.wav");
        // and again: saved from the loaded game
        assert!(sv.save_game().contains("progs/shambler.mdl\"\n"));
        // a vkQuake list names everything; only what is missing is added
        let all: String = sv.core.sv.model_precache.iter().enumerate().skip(1).map(|(i, m)| format!("sv.model_precache {i} \"{m}\"\n")).collect();
        let body = &text[..text.find("/*").unwrap()];
        sv.load_game(&format!("{body}/*\n{all}sv.model_precache 9999 \"x\"\n*/\n")).unwrap();
        assert_eq!(sv.core.sv.model_precache.len(), n + 1);
    }

    /// Co-op: who wasn't in the save starts like in a new game.
    #[test]
    fn a_player_who_joins_a_loaded_game_spawns() {
        let Some(base) = basedir() else { return };
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 4);
        sv.core.cvars.set("coop", "1");
        sv.spawn_server("e1m1").unwrap();
        sv.connect_local().unwrap();
        for _ in 0..10 {
            sv.frame(1.0 / 72.0);
        }
        let health = |sv: &Server, e: u32| sv.level.as_ref().unwrap().vm.ent_f(e, sv.core.sv.fields.health);
        let l = sv.level.as_mut().unwrap();
        l.vm.set_ent_f(1, sv.core.sv.fields.health, 57.0);
        let text = sv.save_game();
        sv.load_game(&text).unwrap();
        let (a, b) = (sv.connect_local().unwrap(), sv.connect_local().unwrap());
        for _ in 0..10 {
            sv.frame(1.0 / 72.0);
        }
        assert!(sv.error().is_none());
        assert_eq!((a, b), (0, 1));
        assert_eq!((health(&sv, 1), health(&sv, 2)), (57.0, 100.0));
        assert_eq!(sv.level.as_ref().unwrap().vm.classname(2), "player");
        assert!(!sv.any_client_dead());
    }

    #[test]
    fn a_damaged_save_leaves_the_game_running() {
        let Some(base) = basedir() else { return };
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        let p = sv.connect_local().unwrap();
        for _ in 0..20 {
            sv.frame(1.0 / 72.0);
        }
        let text = sv.save_game();
        let spawns = sv.core.spawns;
        for bad in [&text[..text.len() / 2], "6\nnotthisgame\n", "4\n", "", &text.replacen("e1m1", "nosuchmap", 1)] {
            assert!(sv.load_game(bad).is_err());
            assert!(sv.active() && sv.core.spawns == spawns && sv.core.clients[p].spawned);
        }
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
