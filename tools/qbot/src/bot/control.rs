use super::*;

impl Bot<'_> {
    pub(super) fn main_loop(&mut self, map: &str) {
        let mut last_refill = -100.0;
        let mut intermission_since: Option<f64> = None;
        loop {
            let t = self.sv.time();
            if t > self.max_time {
                break;
            }
            if self.sv.core.changelevel_issued || !self.sv.core.sv.name.eq_ignore_ascii_case(map) {
                self.rep.exited = true;
                break;
            }
            if let Some(e) = self.sv.error() {
                self.rep.errors.push(format!("t={t:.2}: {}", e.to_string().replace('\n', " | ")));
                break;
            }
            for i in 0..self.goals.len() {
                if matches!(self.goals[i].kind, Kind::Key | Kind::Pickup) && !self.goals[i].armed && self.vm().ent_f(self.goals[i].ent, self.o.solid) as i32 == SOLID_TRIGGER {
                    self.goals[i].armed = true;
                }
                if !self.goals[i].done && self.goal_done(&self.goals[i]) {
                    self.goals[i].done = true;
                    self.progress = true;
                    if self.opt.verbose && self.goals[i].kind == Kind::Key {
                        eprintln!("  t={t:.1} got key #{}", self.goals[i].ent);
                    }
                    if self.cur == Some(i) {
                        self.cur = None;
                    }
                }
            }
            let in_intermission = self.intermission.is_some_and(|g| self.vm().g_f(g) != 0.0);
            // The level is finished once its intermission runs (finales have
            // no changelevel to wait for).
            if in_intermission {
                let since = *intermission_since.get_or_insert(t);
                if t - since > 8.0 {
                    self.rep.exited = true;
                    break;
                }
            }
            let mv = match self.replay_move() {
                Some(mv) => mv,
                None => match self.live_move(t, in_intermission, &mut last_refill) {
                    Some(mv) => mv,
                    None => break,
                },
            };
            if self.frame.is_multiple_of(FPS) {
                let p = self.me_origin();
                self.rec.checkpoints.push((self.frame, p));
            }
            self.rec.moves.push(mv);
            self.frame += 1;
            // impulse 9 (ammo) also hands out every key in id1: strip the
            // keys it added, so key doors stay locked until the key is taken.
            let keys_before = (mv.impulse == 9).then(|| self.o.items.map(|it| (it, self.vm().ent_f(self.me, it) as i32))).flatten();
            let jump_from = (mv.buttons & 2 != 0 && self.vm().ent_f(self.me, self.o.flags) as i32 & qworld::FL_ONGROUND != 0
                && self.vm().ent_f(self.me, self.o.waterlevel) == 0.0).then(|| self.me_origin());
            self.sv.set_move(self.c, mv);
            self.sv.frame(DT);
            if let Some(from) = jump_from {
                let fields = &self.sv.core.sv.fields;
                let velocity = v(self.vm().ent_v(self.me, fields.velocity));
                let delta = self.me_origin() - from;
                if delta.z > 1.0 && delta.length() < 48.0 && velocity.z > 100.0
                    && self.vm().ent_f(self.me, self.o.waterlevel) == 0.0 {
                    let scale = self.vm().field_ofs("gravity").map_or(0.0, |f| self.vm().ent_f(self.me, f));
                    let gravity = self.sv.core.sv.phys.gravity * if scale == 0.0 { 1.0 } else { scale };
                    let impulse = velocity.z + gravity * DT as f32;
                    if (100.0..=600.0).contains(&impulse) { self.jump_speed = Some(impulse); }
                }
            }
            if let Some((it, before)) = keys_before {
                let vm = &mut self.sv.level.as_mut().unwrap().vm;
                let now = vm.ent_f(self.me, it) as i32;
                let added = now & (IT_KEY1 | IT_KEY2) & !before;
                if added != 0 {
                    vm.set_ent_f(self.me, it, (now & !added) as f32);
                }
            }
            let _ = self.sv.take_reliable(self.c);
            let _ = self.sv.take_datagrams(self.c);
        }
        if self.opt.verbose {
            for g in self.goals.iter().filter(|g| !g.done) {
                eprintln!("  left: {:?} #{} ({}) at {:?}", g.kind, g.ent, self.vm().classname(g.ent), self.center(g.ent));
            }
        }
    }

    /// The next move of the cached route, or `None` (no route, or it no
    /// longer matches what the engine does: then the bot takes over).
    pub(super) fn replay_move(&mut self) -> Option<ClientMove> {
        let r = self.replay.as_ref()?;
        let f = self.frame;
        let here = self.me_origin();
        let off = r.checkpoints.binary_search_by_key(&f, |c| c.0).ok().map(|i| (r.checkpoints[i].1 - here).length());
        if off.is_some_and(|d| d > 16.0) || f as usize >= r.moves.len() {
            let t = self.sv.time();
            if self.opt.verbose {
                eprintln!("  t={t:.1} route diverged ({} units off), taking over", off.unwrap_or(0.0));
            }
            self.rep.route = format!("diverged t={t:.0}");
            self.replay = None;
            self.rebuild_nav();
            return None;
        }
        let mv = r.moves[f as usize];
        let exits: Vec<u32> = r.exits.iter().filter(|x| x.0 == f).map(|x| x.1).collect();
        for e in exits {
            if let Some(g) = self.goals.iter().find(|g| g.ent == e && g.kind == Kind::Exit) {
                let touch = g.touch;
                self.sv.level.as_mut().unwrap().vm.set_ent_i(e, self.o.touch, touch);
                self.rec.exits.push((f, e));
            }
        }
        Some(mv)
    }

    /// The bot's own move; `None` when there is nothing left to do.
    pub(super) fn live_move(&mut self, t: f64, in_intermission: bool, last_refill: &mut f64) -> Option<ClientMove> {
        if t - *last_refill > 5.0 {
            // All weapons and ammo, then the super nailgun (no splash).
            self.impulses = vec![self.weapon, 9];
            *last_refill = t;
        }
        // Just noclipped onto an unreachable goal: give it time.
        let settling = self.path.is_empty() && self.cur.is_some() && t - self.noclip_at < 5.0;
        if let Some(i) = self.cur
            && !in_intermission
        {
            let g = &self.goals[i];
            if t - self.goal_start > GOAL_TIMEOUT && self.ride_since.is_none() && self.jump_action.is_none() {
                self.goal_start = t;
                self.defer(i, "timed out");
            } else if self.path.is_empty() && g.kind != Kind::Monster && g.kind != Kind::Exit && self.noclip_to_goal && t - self.noclip_at > 4.0 && t - self.noclip_at < 5.0 {
                // Noclipped onto a goal that still isn't done: it won't be.
                self.fail(i, "not done after noclip");
            }
        }
        // QC can complete the goal and teleport the player during a jump.
        // The successful interaction supersedes the remaining input replay.
        if self.cur.is_none() && self.jump_action.is_some() {
            self.jump_action = None;
            self.path.clear();
            self.last_replan = -100.0;
        }
        let monster_goal = self.cur.is_some_and(|i| self.goals[i].kind == Kind::Monster);
        if !in_intermission
            && !settling
            && (self.cur.is_none() || t - self.last_replan > if monster_goal { 1.0 } else { 3.0 })
            && (self.ride_since.is_none() || self.cur.is_none())
            && self.jump_action.is_none()
            && !self.replan()
        {
            return None;
        }
        let mut mv = self.think();
        if let Some(i) = self.impulses.pop() {
            mv.impulse = i;
        }
        if trace() && self.frame.is_multiple_of(FPS / 2) {
            let next = self.path.get(self.step).map(|&n| self.nav.pos[n as usize]);
            eprintln!(
                "    t={t:.1} at {:?} wl {} step {}/{} next {next:?} mv f{:.0} s{:.0} u{:.0} b{}",
                self.me_origin(),
                self.vm().ent_f(self.me, self.o.waterlevel),
                self.step,
                self.path.len(),
                mv.forward,
                mv.side,
                mv.up,
                mv.buttons
            );
        }
        Some(mv)
    }

    pub(super) fn aim_approach(&mut self, mv: &mut ClientMove) {
        if self.frame.is_multiple_of(2) { self.target_cache = self.target(); }
        let Some((_, ent)) = self.target_cache else { return };
        if self.vm().is_free(ent) || self.vm().ent_f(ent, self.o.health) <= 0.0 { return; }
        let (sin, cos) = mv.angles[1].to_radians().sin_cos();
        let wish = Vec3::new(mv.forward * cos + mv.side * sin, mv.forward * sin - mv.side * cos, 0.0);
        let target = self.center(ent);
        mv.angles = look_angles(target - self.eye());
        let (sin, cos) = mv.angles[1].to_radians().sin_cos();
        mv.forward = wish.x * cos + wish.y * sin;
        mv.side = wish.x * sin - wish.y * cos;
        mv.buttons |= 1;
        let weapon = if self.vm().classname(ent).contains("zombie") { 7 } else if (target - self.eye()).length() < 560.0 { 8 } else { 5 };
        if weapon != self.weapon { self.weapon = weapon; self.impulses.push(weapon); }
    }

    pub(super) fn refine_local(&mut self, center: Vec3) -> bool {
        let changed = self.nav.refine(&self.geo, center);
        if changed {
            hints::reapply_blocks(&mut self.nav, &self.hints);
            self.rep.nav_nodes = self.rep.nav_nodes.max(self.nav.pos.len());
        }
        changed
    }
}
