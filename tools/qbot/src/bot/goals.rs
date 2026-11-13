//! Goal bookkeeping: finding goals in the edicts, telling when one is done,
//! and choosing the next.

use super::*;

impl Bot<'_> {
    /// Add goals for edicts not seen yet: run at the start and every couple
    /// of seconds, since triggers spawn monsters and items as the level runs.
    pub(super) fn scan(&mut self) {
        let vm = &self.sv.level.as_ref().unwrap().vm;
        let o = &self.o;
        let mut new = Vec::new();
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
                c if c.starts_with("item_") && c.contains("key") || c == "item_sigil" || c == "item_hornofconjuring" => Kind::Pickup,
                _ if vm.ent_f(e, o.flags) as i32 & FL_MONSTER != 0 => {
                    // hipnotic func_spawn keeps dormant templates (with
                    // `spawnfunction` set) and only ever spawns copies.
                    let template = solid == SOLID_NOT
                        && vm.ent_f(e, o.takedamage) == 0.0
                        && o.spawnfunction.is_some_and(|f| vm.ent_i(e, f) != 0);
                    if vm.ent_f(e, o.health) <= 0.0 || template {
                        continue;
                    }
                    Kind::Monster
                }
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
                touch: vm.ent_i(e, o.touch),
                again: false,
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
            Kind::Monster => vm.ent_f(e, o.health) <= 0.0 || vm.ent_f(e, o.flags) as i32 & FL_MONSTER == 0,
            Kind::Secret | Kind::Trigger => vm.ent_f(e, o.solid) as i32 != SOLID_TRIGGER || self.inside(e),
            // Keys spawn SOLID_NOT until PlaceItem runs: taken once placed
            // and not solid again.
            Kind::Pickup => g.armed && vm.ent_f(e, o.solid) as i32 == SOLID_NOT,
            Kind::Button => o.state.is_some_and(|s| vm.ent_f(e, s) != g.initial_state),
            Kind::Shoot => vm.ent_f(e, o.takedamage) == 0.0 || vm.ent_f(e, o.health) <= 0.0,
            Kind::Exit => false,
        }
    }

    /// Monsters that can't be hurt yet: waiting to be spawned by a trigger
    /// (hipnotic, AD; not solid) or rogue statues (solid stone until their
    /// targetname fires). Chasing them only burns the goal timeout.
    pub(super) fn dormant(&self, g: &Goal) -> bool {
        g.kind == Kind::Monster && self.vm().ent_f(g.ent, self.o.takedamage) == 0.0
    }

    pub(super) fn update_goals(&mut self) {
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if g.done {
                continue;
            }
            if g.kind == Kind::Pickup && !g.armed && self.vm().ent_f(g.ent, self.o.solid) as i32 == SOLID_TRIGGER {
                self.goals[i].armed = true;
            }
            if self.goal_done(&self.goals[i]) {
                let freed = self.vm().is_free(self.goals[i].ent);
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
        let best = |exit: bool| {
            self.goals
                .iter()
                .enumerate()
                .filter(|(_, g)| open(g) && (g.kind == Kind::Exit) == exit)
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
