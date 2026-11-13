//! Goal bookkeeping: finding goals in the edicts, telling when one is done,
//! and choosing the next.

use super::*;

/// hipnotic func_spawn keeps dormant templates (with `spawnfunction` set)
/// and only ever spawns copies of them.
fn spawn_template(vm: &Vm, o: &Ofs, e: u32) -> bool {
    vm.ent_f(e, o.solid) as i32 == SOLID_NOT
        && vm.ent_f(e, o.takedamage) == 0.0
        && o.spawnfunction.is_some_and(|f| vm.ent_i(e, f) != 0)
}

/// A crucified zombie (spawnflags 1, id1 and hipnotic alike) is scenery:
/// monster_zombie skips walkmonster_start, so it never takes damage and
/// is never counted in total_monsters.
fn crucified(vm: &Vm, o: &Ofs, e: u32, cls: &str) -> bool {
    cls == "monster_zombie" && vm.ent_f(e, o.spawnflags) as i32 & 1 != 0 && vm.ent_f(e, o.takedamage) == 0.0
}

impl Bot<'_> {
    /// Add goals for edicts not seen yet: run at the start and every couple
    /// of seconds, since triggers spawn monsters and items as the level runs.
    pub(super) fn scan(&mut self) {
        let vm = &self.sv.level.as_ref().unwrap().vm;
        let o = &self.o;
        let mut new = Vec::new();
        let first = self.known.is_empty();
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || e == self.me || self.known.contains(&e) {
                continue;
            }
            let cls = vm.classname(e);
            let solid = vm.ent_f(e, o.solid) as i32;
            let shootable = vm.ent_f(e, o.takedamage) != 0.0 && vm.ent_f(e, o.health) > 0.0;
            let kind = match cls.as_str() {
                "trigger_changelevel" => Kind::Exit,
                "trigger_secret" => Kind::Secret,
                "func_button" => Kind::Button,
                // Doors opened by touch (key doors) that fire targets when
                // they open: noclip never touches them, so press like a button.
                "door" | "func_door" if !shootable && self.has_str(e, o.target) && !self.has_str(e, o.targetname) => Kind::Button,
                // Chthon never gets FL_MONSTER (mgend's can be killed); rogue's
                // item_time_machine is a monster.
                c if vm.ent_f(e, o.flags) as i32 & FL_MONSTER != 0 || c.starts_with("monster_") => {
                    if vm.ent_f(e, o.health) <= 0.0 || spawn_template(vm, o, e) || crucified(vm, o, e, c) {
                        continue;
                    }
                    Kind::Monster
                }
                c if c.starts_with("item_") && c.contains("key") || c == "item_sigil" || c == "item_hornofconjuring" => Kind::Pickup,
                // Every item the map places: weapons, ammo, health, armor,
                // powerups. Some fire a target when taken (mgend's thunderbolt
                // raises Chthon); impulse 9 alone never touches them. Later
                // ones (backpacks, horde waves) only when they fire a target.
                c if (c.starts_with("item_") || c.starts_with("weapon_")) && (first || self.has_str(e, o.target)) => Kind::Pickup,
                c if c.starts_with("trigger_") && solid == SOLID_TRIGGER && vm.ent_i(e, o.touch) != 0 => Kind::Trigger,
                _ if shootable && (solid == SOLID_BSP || solid == SOLID_BBOX) => Kind::Shoot,
                _ => continue,
            };
            new.push(Goal {
                ent: e,
                kind,
                class: cls,
                done: false,
                failed: None,
                tries: 0,
                armed: false,
                initial_state: o.state.map_or(0.0, |s| vm.ent_f(e, s)),
                initial_absmin: vm.ent_v(e, o.absmin),
                touch: vm.ent_i(e, o.touch),
                again: false,
                late: kind == Kind::Pickup && self.has_str(e, o.target),
            });
        }
        for g in new {
            self.known.insert(g.ent);
            if g.kind == Kind::Exit {
                // Exits stay inert until everything else is done.
                self.sv.level.as_mut().unwrap().vm.set_ent_i(g.ent, self.o.touch, 0);
            }
            self.goals.push(g);
        }
    }

    /// Explain kills short of `total_monsters` that no failed goal covers:
    /// live monsters that never became goals (a horn's charmed ally, one
    /// spawned after the last scan), else counted ones that left no edict
    /// (removed or never spawned).
    pub(super) fn unaccounted_monsters(&mut self) {
        let st = self.sv.stats();
        let short = st.total_monsters - st.killed_monsters;
        let failed = self.goals.iter().filter(|g| g.kind == Kind::Monster && !g.done).count() as i32;
        if short <= failed {
            return;
        }
        let vm = self.vm();
        let o = &self.o;
        let alive: Vec<String> = (1..vm.num_edicts() as u32)
            .filter(|&e| !vm.is_free(e) && vm.ent_f(e, o.flags) as i32 & FL_MONSTER != 0 && vm.ent_f(e, o.health) > 0.0)
            .filter(|&e| !spawn_template(vm, o, e))
            .filter(|e| !self.goals.iter().any(|g| g.ent == *e && !g.done))
            .map(|e| {
                let c = self.center(e);
                format!("Monster {} #{e} at ({:.0} {:.0} {:.0}): alive but never a goal", vm.classname(e), c.x, c.y, c.z)
            })
            .collect();
        let gone = short - failed - alive.len() as i32;
        self.rep.failed.extend(alive);
        if gone > 0 {
            self.rep.failed.push(format!("{gone} counted monster(s) neither killed nor alive (removed, or counted but never spawned)"));
        }
    }

    /// Whether string field `f` of edict `e` is set and not empty.
    fn has_str(&self, e: u32, f: Option<usize>) -> bool {
        f.is_some_and(|f| self.vm().ent_str(e, f).is_ok_and(|s| !s.is_empty()))
    }

    /// Whether the player's box overlaps edict `e`'s bounds.
    pub(super) fn inside(&self, e: u32) -> bool {
        let (a, b) = self.bounds(e);
        let p = self.me_origin();
        p.x + 16.0 > a.x && p.x - 16.0 < b.x && p.y + 16.0 > a.y && p.y - 16.0 < b.y && p.z + 32.0 > a.z && p.z - 24.0 < b.z
    }

    pub(super) fn goal_done(&self, g: &Goal) -> bool {
        let vm = self.vm();
        let e = g.ent;
        if vm.is_free(e) {
            return g.kind != Kind::Exit;
        }
        let o = &self.o;
        match g.kind {
            Kind::Monster => {
                vm.ent_f(e, o.health) <= 0.0 || vm.ent_f(e, o.flags) as i32 & FL_MONSTER == 0 && !vm.classname(e).starts_with("monster_")
            }
            Kind::Secret | Kind::Trigger => vm.ent_f(e, o.solid) as i32 != SOLID_TRIGGER || self.inside(e),
            // Keys spawn SOLID_NOT until PlaceItem runs: taken once placed
            // and not solid again.
            Kind::Pickup => g.armed && vm.ent_f(e, o.solid) as i32 == SOLID_NOT,
            Kind::Button => o.state.is_some_and(|s| vm.ent_f(e, s) != g.initial_state),
            // func_door_secret with SECRET_NO_SHOOT (8) stays shootable and
            // resets health in its th_pain: done once it has moved.
            Kind::Shoot => {
                vm.ent_f(e, o.takedamage) == 0.0 || vm.ent_f(e, o.health) <= 0.0 || matches!(g.class.as_str(), "door" | "func_door_secret") && vm.ent_v(e, o.absmin) != g.initial_absmin
            }
            Kind::Exit => false,
        }
    }

    /// Monsters that can't be hurt yet: waiting to be spawned by a trigger
    /// (hipnotic, AD; not solid) or rogue statues (solid stone until their
    /// targetname fires). Chasing them only burns the goal timeout.
    /// A lock tried before its key was in hand has failed for good: give
    /// failed buttons and key doors another go whenever a key is taken.
    fn reopen_locks(&mut self) {
        let Some(it) = self.o.items else { return };
        const KEYS: i32 = 0x20000 | 0x40000; // IT_KEY1 | IT_KEY2
        let keys = self.vm().ent_f(self.me, it) as i32 & KEYS;
        let gained = keys & !self.keys_held != 0;
        self.keys_held = keys;
        if gained {
            for g in self.goals.iter_mut().filter(|g| g.kind == Kind::Button && !g.done && g.failed.is_some()) {
                (g.failed, g.tries) = (None, 0);
            }
        }
    }

    /// Items never yet touchable: waiting for a trigger to enable them
    /// (mg3 map5).
    pub(super) fn dormant(&self, g: &Goal) -> bool {
        let vm = self.vm();
        match g.kind {
            Kind::Monster => vm.ent_f(g.ent, self.o.takedamage) == 0.0,
            Kind::Pickup => {
                // A key the bot holds refuses a second copy (mg3 map5 has
                // two of each for two locks of each): wait until it's used.
                let held = g.class.contains("key")
                    && self.o.items.is_some_and(|it| vm.ent_f(self.me, it) as i32 & vm.ent_f(g.ent, it) as i32 != 0);
                held || !g.armed && vm.ent_f(g.ent, self.o.solid) as i32 == SOLID_NOT
            }
            _ => false,
        }
    }

    pub(super) fn update_goals(&mut self) {
        self.reopen_locks();
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if g.done {
                // A corpse removed later frees its edict, which a spawner
                // can reuse (ED_Alloc waits 0.5 s): let the scan see it anew.
                if self.vm().is_free(g.ent) {
                    let e = g.ent;
                    self.known.remove(&e);
                }
                continue;
            }
            if g.kind == Kind::Pickup && !g.armed && self.vm().ent_f(g.ent, self.o.solid) as i32 == SOLID_TRIGGER {
                self.goals[i].armed = true;
            }
            if self.goal_done(&self.goals[i]) {
                let freed = self.vm().is_free(self.goals[i].ent);
                if self.goals[i].late {
                    self.late_until = self.sv.time() + 5.0;
                }
                let g = &self.goals[i];
                let msg = format!("done {:?} {} #{}", g.kind, g.class, g.ent);
                self.note(&msg);
                let g = &mut self.goals[i];
                g.done = true;
                if freed {
                    self.known.remove(&g.ent);
                }
                if self.cur == Some(i) {
                    self.cur = None;
                }
            }
        }
    }

    pub(super) fn fail(&mut self, i: usize, why: &str) {
        self.note(&format!("FAILED {}: {why}", self.describe(i)));
        let g = &mut self.goals[i];
        g.failed = Some(why.into());
        g.tries += 1;
        self.cur = None;
    }

    /// Everything is done but monsters are still alive that aren't goals
    /// (e1m7's Chthon: undamageable, killed by `event_lightning` only while
    /// two doors raised by other buttons are up, three times): press every
    /// button that has popped back out again, for a few rounds.
    fn repress(&mut self) -> bool {
        let st = self.sv.stats();
        if st.killed_monsters >= st.total_monsters || self.repressed >= 12 {
            return false;
        }
        self.repressed += 1;
        let Some(state) = self.o.state else { return false };
        let mut any = false;
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if g.kind == Kind::Button && g.done && self.vm().ent_f(g.ent, state) == g.initial_state {
                let g = &mut self.goals[i];
                (g.done, g.again, g.failed, g.tries) = (false, true, None, 0);
                any = true;
            }
        }
        if any {
            self.note(&format!("re-pressing buttons (round {})", self.repressed));
        }
        any
    }

    /// Doors, plats and trains that edict `e` targets.
    fn movers(&self, e: u32) -> Vec<u32> {
        let vm = self.vm();
        let (Some(t), Some(tn)) = (self.o.target, self.o.targetname) else { return Vec::new() };
        let Ok(target) = vm.ent_str(e, t) else { return Vec::new() };
        if target.is_empty() {
            return Vec::new();
        }
        (1..vm.num_edicts() as u32)
            .filter(|&o| {
                !vm.is_free(o)
                    && vm.ent_str(o, tn).is_ok_and(|n| n == target)
                    && matches!(vm.classname(o).as_str(), "door" | "func_door" | "plat" | "func_plat" | "train" | "func_train")
            })
            .collect()
    }

    pub(super) fn fires_mover(&self, e: u32) -> bool {
        !self.movers(e).is_empty()
    }

    /// Re-pressing: a door moved by a re-pressed button is still on its
    /// way (`state` 2 up, 3 down); the other buttons wait for it.
    pub(super) fn movers_moving(&self) -> bool {
        let Some(st) = self.o.state else { return false };
        self.goals
            .iter()
            .filter(|g| g.again && g.done)
            .flat_map(|g| self.movers(g.ent))
            .any(|m| matches!(self.vm().ent_f(m, st) as i32, 2 | 3))
    }

    /// Nearest open goal (the exit only once nothing else is left).
    pub(super) fn pick(&mut self) -> Option<usize> {
        let idle = self.goals.iter().all(|g| g.kind == Kind::Exit || g.done || self.dormant(g) || g.failed.is_some() && g.tries >= 2);
        if idle && self.repress() {
            return self.pick();
        }
        let here = self.me_origin();
        // Re-pressing: buttons that move doors first (e1m7's electrodes must
        // be up before the lightning button), then the rest.
        let movers_first = self.goals.iter().any(|g| g.again && !g.done && g.failed.is_none() && self.fires_mover(g.ent));
        let open = |g: &Goal| !g.done && g.failed.is_none() && !self.dormant(g) && (!movers_first || g.again && self.fires_mover(g.ent));
        // Pickups that fire targets go last: mgend's keys each raise Chthon
        // for another stage, which only works once the stage before is over.
        let late = |g: &Goal| g.late;
        let any_early = self.goals.iter().any(|g| open(g) && g.kind != Kind::Exit && !late(g));
        // Nothing to do but late pickups or the exit: let the last pickup's
        // script run (spawn its monsters) first.
        if !any_early && self.sv.time() < self.late_until {
            return None;
        }
        let best = |exit: bool| {
            self.goals
                .iter()
                .enumerate()
                .filter(|(_, g)| open(g) && (g.kind == Kind::Exit) == exit && !(any_early && late(g)))
                .min_by(|a, b| (self.center(a.1.ent) - here).length_sq().total_cmp(&(self.center(b.1.ent) - here).length_sq()))
                .map(|(i, _)| i)
        };
        if let Some(i) = best(false) {
            return Some(i);
        }
        if self.goals.iter().any(|g| !g.done && g.failed.is_some() && g.tries < 2) {
            for g in self.goals.iter_mut().filter(|g| !g.done && g.tries < 2) {
                g.failed = None;
            }
            return self.pick();
        }
        // Dormant monsters may still wake up; give scripts a moment first.
        let i = best(true)?;
        for g in self.goals.iter().filter(|g| g.kind == Kind::Exit) {
            let (e, t) = (g.ent, g.touch);
            self.sv.level.as_mut().unwrap().vm.set_ent_i(e, self.o.touch, t);
        }
        Some(i)
    }
}
