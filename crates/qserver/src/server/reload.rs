//! Same-map rebuilds with a debounced watch on the effective loose map files.
use super::*;
use qcore::time::SystemTime;
use std::path::PathBuf;

#[derive(PartialEq, Eq)]
struct MapFiles {
    bsp: Option<(PathBuf, SystemTime)>,
    ent: Option<(PathBuf, SystemTime)>,
}

impl MapFiles {
    fn read(c: &Core, map: &str) -> Self {
        let bsp = format!("maps/{map}.bsp");
        let stamp = |path: Option<PathBuf>| {
            let path = path?;
            let modified = qcore::vfs::modified(&path).ok()?;
            Some((path, modified))
        };
        Self {
            bsp: stamp(c.fs.loose_path(&bsp, None)),
            ent: if c.cvars.value_of("external_ents") != 0.0 {
                stamp(c.fs.loose_path(&format!("maps/{map}.ent"), Some(&bsp)))
            } else {
                None
            },
        }
    }
}

#[derive(Default)]
pub(super) struct MapWatch {
    files: Option<MapFiles>,
    next_poll: f64,
    reload_after: Option<f64>,
}

impl Server {
    pub(crate) fn map_reload_command(&mut self, args: &qcore::cmd::Args) {
        if self.core.cmd_client.is_some() {
            return;
        }
        match (args.argc(), args.argv(1)) {
            (1, _) => {
                self.reset_map_watch(&self.core.sv.name.clone());
                self.reload_current_map();
            }
            (2, "watch") => {
                self.map_watch = Some(MapWatch::default());
                self.reset_map_watch(&self.core.sv.name.clone());
                self.core
                    .print("map_reload: watching the current map for changes\n");
            }
            (2, "off") => {
                self.map_watch = None;
                self.core.print("map_reload: stopped watching map files\n");
            }
            _ => self.core.print("usage: map_reload [watch|off]\n"),
        }
    }

    pub(super) fn reset_map_watch(&mut self, map: &str) {
        if let Some(watch) = &mut self.map_watch {
            watch.files = Some(MapFiles::read(&self.core, map));
            watch.reload_after = None;
        }
    }

    /// Check the watcher from the host frame before applying player input.
    pub fn poll_map_reload(&mut self, now: f64) {
        if !self.active() || self.error().is_some() {
            return;
        }
        let Some(watch) = &mut self.map_watch else {
            return;
        };
        if now < watch.next_poll {
            return;
        }
        watch.next_poll = now + 0.25;
        let files = MapFiles::read(&self.core, &self.core.sv.name);
        if watch.files.as_ref() != Some(&files) {
            watch.files = Some(files);
            watch.reload_after = Some(now + 0.5);
        } else if watch.reload_after.is_some_and(|due| now >= due) {
            watch.reload_after = None;
            self.reload_current_map();
        }
    }

    fn reload_current_map(&mut self) {
        if !self.active() || self.error().is_some() {
            self.core.print("map_reload: no active map\n");
            return;
        }
        let c = &mut self.core;
        let name = c.sv.name.clone();
        let level = self.level.take().unwrap();
        let sv = std::mem::take(&mut c.sv);
        let clients = c.clients.clone();
        let cvars = c.cvars.clone();
        let cbuf = std::mem::take(&mut c.cbuf);
        let log = std::mem::take(&mut c.log);
        let queued = std::mem::take(&mut c.queued_client_cmds);
        let portals = std::mem::take(&mut c.linked_portals);
        let crossed = std::mem::take(&mut c.portal_crossed);
        let changelevel = c.changelevel_issued;
        let serverflags = c.serverflags;

        let players = clients
            .iter()
            .enumerate()
            .map(|(i, cl)| {
                if let Some(pending) = sv.reload_players.get(i).and_then(Option::as_ref) {
                    return Some(pending.clone());
                }
                (cl.active && cl.spawned)
                    .then(|| PlayerState::capture(&level.vm, &sv.fields, i as u32 + 1))
                    .flatten()
            })
            .collect();
        c.serverflags = level.vm.g_f(sv.globals.serverflags);
        for cl in &mut c.clients {
            cl.message.clear();
            cl.reliable_out.clear();
            cl.datagrams.clear();
            cl.stringcmds.clear();
            cl.cmd = ClientMove::default();
            cl.new_move = false;
            cl.extra_view = None;
        }

        let result = self
            .spawn_inner(&name, Some(players))
            .and_then(|()| self.error().map_or(Ok(()), |e| Err(e.to_string())));
        let c = &mut self.core;
        match result {
            Ok(()) => {
                c.log.splice(..0, log);
                c.spawns += 1;
                c.print(&format!("map_reload: reloaded {name}\n"));
            }
            Err(err) => {
                self.level = Some(level);
                c.sv = sv;
                c.clients = clients;
                c.cvars = cvars;
                c.cbuf = cbuf;
                c.log = log;
                c.queued_client_cmds = queued;
                c.linked_portals = portals;
                c.portal_crossed = crossed;
                c.changelevel_issued = changelevel;
                c.serverflags = serverflags;
                c.print(&format!("map_reload: couldn't reload {name}: {err}\n"));
            }
        }
    }
}

/// Same-level inventory snapshots retain keys, ammo and armor exactly.
/// The normal SetChangeParms path intentionally strips or clamps these.
#[derive(Clone, Debug)]
pub(super) struct PlayerState {
    origin: [f32; 3],
    angles: [f32; 3],
    cheat_flags: i32,
    flight: Option<i32>,
    stats: Vec<(String, f32)>,
    strings: Vec<(String, Vec<u8>)>,
    time: f32,
}

const CHEATS: i32 = 64 | 128; // FL_GODMODE | FL_NOTARGET
const POWERUP_TIMES: &[&str] = &[
    "super_damage_finished",
    "invisible_finished",
    "invincible_finished",
    "radsuit_finished",
    "super_time",
    "invisible_time",
    "invincible_time",
    "rad_time",
];

impl PlayerState {
    fn capture(vm: &Vm, f: &Fields, ent: u32) -> Option<Self> {
        if vm.ent_f(ent, f.health) <= 0.0 {
            return None;
        }
        let mt = vm.ent_f(ent, f.movetype) as i32;
        let mut state = Self {
            origin: vm.ent_v(ent, f.origin),
            angles: vm.ent_v(
                ent,
                if vm.ent_f(ent, f.fixangle) != 0.0 {
                    f.angles
                } else {
                    f.v_angle
                },
            ),
            cheat_flags: vm.ent_f(ent, f.flags) as i32 & CHEATS,
            flight: matches!(mt, qworld::MOVETYPE_NOCLIP | qworld::MOVETYPE_FLY).then_some(mt),
            stats: Vec::new(),
            strings: Vec::new(),
            time: vm.time as f32,
        };
        let counted_keys = f.items_qbj.is_some();
        for def in &vm.progs.fielddefs {
            let Ok(name) = std::str::from_utf8(vm.progs.string(def.name)) else {
                continue;
            };
            let ofs = def.ofs as usize;
            match def.etype() {
                qvm::EType::Float
                    if name.starts_with("ammo_")
                        || name.starts_with("ckeyskin")
                        || POWERUP_TIMES.contains(&name)
                        || (counted_keys && name == "worldtype")
                        || matches!(
                            name,
                            "health"
                                | "items"
                                | "items2"
                                | "moditems"
                                | "items_qbj"
                                | "armorvalue"
                                | "armortype"
                                | "weapon"
                                | "weaponismoditems"
                                | "currentammo"
                        ) =>
                {
                    state.stats.push((name.into(), vm.ent_f(ent, ofs)));
                }
                qvm::EType::String if name == "weaponmodel" || name.starts_with("ckeyname") => {
                    if let Ok(value) = vm.ent_str(ent, ofs) {
                        state.strings.push((name.into(), value.to_vec()));
                    }
                }
                _ => {}
            }
        }
        Some(state)
    }

    pub(super) fn restore(self, vm: &mut Vm, c: &mut Core, world: &mut World, slot: usize) {
        let ent = slot as u32 + 1;
        let f = c.sv.fields.clone();
        for (name, mut value) in self.stats {
            if let Some(def) = vm
                .progs
                .find_field(&name)
                .filter(|d| d.etype() == qvm::EType::Float)
            {
                if POWERUP_TIMES.contains(&name.as_str()) && value > 1.0 {
                    value = (vm.time as f32 + value - self.time).max(f32::EPSILON);
                }
                vm.set_ent_f(ent, def.ofs as usize, value);
            }
        }
        for (name, value) in self.strings {
            if let Some(def) = vm
                .progs
                .find_field(&name)
                .filter(|d| d.etype() == qvm::EType::String)
            {
                let ofs = def.ofs as usize;
                let value = vm.strings.engine(&value);
                vm.set_ent_i(ent, ofs, value);
            }
        }

        let spawn = vm.ent_v(ent, f.origin);
        let flags = vm.ent_f(ent, f.flags) as i32 | self.cheat_flags;
        vm.set_ent_f(ent, f.flags, (flags & !qworld::FL_ONGROUND) as f32);
        vm.set_ent_i(ent, f.groundentity, 0);
        vm.set_ent_v(ent, f.origin, self.origin);
        vm.set_ent_v(ent, f.velocity, [0.0; 3]);
        vm.set_ent_v(ent, f.v_angle, self.angles);
        vm.set_ent_v(ent, f.angles, self.angles);
        vm.set_ent_f(ent, f.fixangle, 1.0);
        if let Some(mt) = self.flight {
            vm.set_ent_f(ent, f.movetype, mt as f32);
        }
        c.clients[slot].cmd.angles = self.angles;
        let mut ents = Ents { vm, c };
        if self.flight != Some(qworld::MOVETYPE_NOCLIP)
            && world.test_entity_position(&ents, ent).is_some()
        {
            ents.vm.set_ent_v(ent, f.origin, spawn);
            ents.c
                .print("map_reload: previous position is blocked; using the player spawn\n");
        }
        world.link_edict(&mut ents, ent, false);
    }
}
