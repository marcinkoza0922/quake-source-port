//! ED_ParseEpair / ED_ParseEdict / ED_ParseGlobals / ED_LoadFromFile.

use crate::builtins::atof;
use crate::{Def, EType, Host, Result, Vm, VmError};

/// Spawnflag filtering of ED_LoadFromFile.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpawnFilter {
    pub deathmatch: bool,
    /// 0..=3 (current_skill).
    pub skill: i32,
    pub nomonsters: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SpawnStats {
    pub spawned: usize,
    pub inhibited: usize,
    pub no_classname: usize,
    pub no_spawn_fn: usize,
}

/// Result of [`Vm::parse_edict`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ParsedEdict {
    /// False if there were no key/value pairs (the edict was freed).
    pub init: bool,
    /// `"alpha"` key (johnfitz hack; the engine stores it outside QC fields).
    pub alpha: Option<f32>,
}

const SPAWNFLAG_NOT_EASY: i32 = 256;
const SPAWNFLAG_NOT_MEDIUM: i32 = 512;
const SPAWNFLAG_NOT_HARD: i32 = 1024;
const SPAWNFLAG_NOT_DEATHMATCH: i32 = 2048;

enum Base {
    Global,
    Edict(u32),
}

impl Vm {
    fn base_slot(&mut self, base: &Base, ofs: usize) -> &mut u32 {
        match *base {
            Base::Global => &mut self.globals[ofs],
            Base::Edict(e) => &mut self.fields[e as usize * self.progs.entityfields + ofs],
        }
    }

    /// ED_ParseEpair: returns false on unknown field/function names.
    fn parse_epair(&mut self, base: Base, key: Def, s: &str) -> bool {
        let o = key.ofs as usize;
        let set = |vm: &mut Vm, i: usize, v: u32| *vm.base_slot(&base, o + i) = v;
        match key.etype() {
            EType::String => {
                let h = self.strings.new_string(s.as_bytes());
                set(self, 0, h as u32);
            }
            EType::Float => set(self, 0, atof(s.as_bytes()).to_bits()),
            EType::Vector => {
                let mut it = s.split(' ');
                for i in 0..3 {
                    let v = it.next().map_or(0.0, |w| atof(w.as_bytes()));
                    set(self, i, v.to_bits());
                }
            }
            EType::Entity => set(self, 0, atof(s.as_bytes()) as i32 as u32),
            EType::Field => match self.progs.find_field(s) {
                Some(d) => {
                    let v = self.globals[d.ofs as usize];
                    set(self, 0, v);
                }
                None => return false,
            },
            EType::Function => match self.progs.find_function(s) {
                Some(f) => set(self, 0, f as u32),
                None => return false,
            },
            _ => {}
        }
        true
    }

    /// ED_ParseEdict on already-tokenised pairs (from `qformats::parse_entities`).
    pub fn parse_edict(&mut self, host: &mut dyn Host, ent: u32, pairs: &[(String, String)]) -> Result<ParsedEdict> {
        if ent != 0 {
            self.edict_mut(ent).fill(0);
        }
        let mut out = ParsedEdict::default();
        for (k, v) in pairs {
            let anglehack = k == "angle";
            let key = match k.as_str() {
                "angle" => "angles",
                "light" => "light_lev",
                k => k.trim_end_matches(' '),
            };
            out.init = true;
            if key.starts_with('_') {
                continue;
            }
            if key == "alpha" {
                out.alpha = Some(atof(v.as_bytes()));
            }
            let Some(&def) = self.progs.find_field(key) else {
                if !key.starts_with("sky") && key != "fog" && key != "alpha" {
                    host.dprint(&format!("\"{key}\" is not a field\n"));
                }
                continue;
            };
            let val = if anglehack { format!("0 {v} 0") } else { v.clone() };
            if !self.parse_epair(Base::Edict(ent), def, &val) {
                return Err(VmError::new("ED_ParseEdict: parse error"));
            }
        }
        if !out.init {
            self.ed_free(ent);
        }
        Ok(out)
    }

    /// ED_ParseGlobals (savegames).
    pub fn parse_globals(&mut self, host: &mut dyn Host, pairs: &[(String, String)]) -> Result<()> {
        for (k, v) in pairs {
            let Some(&def) = self.progs.find_global(k) else {
                host.print(&format!("'{k}' is not a global\n"));
                continue;
            };
            if !self.parse_epair(Base::Global, def, v) {
                return Err(VmError::new("ED_ParseGlobals: parse error"));
            }
        }
        Ok(())
    }

    /// ED_LoadFromFile: parse the entity lump, filter, and call spawn functions.
    pub fn load_from_file(&mut self, host: &mut dyn Host, entities: &str, filter: SpawnFilter) -> Result<SpawnStats> {
        let ents = qformats::parse_entities(entities).map_err(|e| VmError::new(format!("ED_LoadFromFile: {e:?}")))?;
        let t = self.time as f32;
        self.set_g_f(self.sys.g_time, t);
        let sf = self.field_ofs("spawnflags");
        let mut st = SpawnStats::default();
        for (i, e) in ents.iter().enumerate() {
            let ent = if i == 0 { 0 } else { self.ed_alloc()? };
            self.parse_edict(host, ent, &e.pairs)?;
            if self.ent_i(ent, self.sys.f_classname) == 0 {
                host.print(&format!("No classname for:{}", self.ed_print(ent)));
                self.ed_free(ent);
                st.no_classname += 1;
                continue;
            }
            let classname = self.classname(ent);
            let flags = sf.map_or(0, |o| self.ent_f(ent, o) as i32);
            let inhibit = if filter.deathmatch {
                flags & SPAWNFLAG_NOT_DEATHMATCH != 0
            } else {
                (filter.skill == 0 && flags & SPAWNFLAG_NOT_EASY != 0)
                    || (filter.skill == 1 && flags & SPAWNFLAG_NOT_MEDIUM != 0)
                    || (filter.skill >= 2 && flags & SPAWNFLAG_NOT_HARD != 0)
            } || (filter.nomonsters && classname.starts_with("monster_"));
            if inhibit {
                self.ed_free(ent);
                st.inhibited += 1;
                continue;
            }
            let Some(func) = self.progs.find_function(&classname) else {
                host.print(&format!("No spawn function for:{}", self.ed_print(ent)));
                self.ed_free(ent);
                st.no_spawn_fn += 1;
                continue;
            };
            self.set_self(ent);
            self.execute(host, func)?;
            st.spawned += 1;
        }
        host.dprint(&format!("{} entities inhibited\n", st.inhibited));
        Ok(st)
    }
}
