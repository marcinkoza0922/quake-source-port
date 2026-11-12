use super::*;

impl Bot<'_> {
    pub(super) fn think(&mut self) -> ClientMove {
        let t = self.sv.time();
        let here = self.me_origin();
        let mut mv = ClientMove::default();
        if self.vm().ent_f(self.me, self.o.movetype) as i32 == MOVETYPE_NOCLIP {
            self.sv.command("noclip 0");
        }
        if self.intermission.is_some_and(|g| self.vm().g_f(g) != 0.0) {
            // Exit reached: press fire to leave the intermission.
            mv.buttons = if (t * 4.0) as i64 % 2 == 0 { 1 } else { 0 };
            return mv;
        }

        if let Some(mut mv) = self.jump_move(here, t) {
            // Keep fighting while lining up; flight replays its verified
            // steering without changing view pitch or yaw.
            if self.jump_action.as_ref().is_some_and(|a| a.tick.is_none()) {
                self.aim_approach(&mut mv);
            }
            return mv;
        }

        // Skip ahead when a later path node is already reached (teleporters,
        // lifts and drops move the bot past nodes).
        let close = |nav: &Nav, n: u32| {
            let d = nav.pos[n as usize] - here;
            let reach = if nav.is_fine(n) { 4.0 } else { REACH };
            (d.x * d.x + d.y * d.y) < reach * reach && d.z.abs() < 30.0
        };
        for k in (self.step + 1..self.path.len().min(self.step + 6)).rev() {
            if close(&self.nav, self.path[k]) {
                self.step = k;
                break;
            }
        }

        // Steer along the path.
        let mut wish = Vec3::ZERO;
        self.action_jump = false;
        let riding = self.ride(here, t);
        let mut jump = self.action_jump;
        if let Some(w) = riding {
            wish = w;
            if self.path.is_empty() {
                return mv;
            }
            if close(&self.nav, self.path[self.step]) && self.step + 1 < self.path.len() {
                self.step += 1;
                self.ride_since = None;
            }
        } else {
            while self.step < self.path.len() {
                let p = self.nav.pos[self.path[self.step] as usize];
                let d = p - here;
                if close(&self.nav, self.path[self.step]) && self.step + 1 < self.path.len() {
                    self.step += 1;
                    if self.lifts.contains_key(&(self.path[self.step - 1], self.path[self.step])) {
                        break;
                    }
                    continue;
                }
                wish = Vec3::new(d.x, d.y, 0.0);
                if self.step > 0
                    && let Some(e) = self.nav.edge(self.path[self.step - 1], self.path[self.step])
                {
                    jump = e.kind == EdgeKind::Jump;
                }
                // Jump into teleporters and updrafts (some start above head height).
                if self.step + 1 < self.path.len()
                    && (d.x * d.x + d.y * d.y) < 48.0 * 48.0
                    && self.nav.edge(self.path[self.step], self.path[self.step + 1]).is_some_and(|e| e.kind == EdgeKind::Teleport)
                {
                    jump = true;
                }
                break;
            }
        }
        // Final approach: walk into the goal itself (buttons sit in walls).
        if let Some(i) = self.cur
            && self.step + 1 >= self.path.len()
            && riding.is_none()
        {
            let g = &self.goals[i];
            if matches!(g.kind, Kind::Button | Kind::Secret | Kind::Key | Kind::Pickup | Kind::Exit | Kind::Trigger) {
                let c = self.center(g.ent);
                let d = c - here;
                if (d.x * d.x + d.y * d.y) > 4.0 {
                    wish = Vec3::new(d.x, d.y, 0.0);
                }
                let bottom = self.vm().ent_v(g.ent, self.o.absmin)[2];
                // Trigger centers can be above the player's head even though
                // a jump reaches their lower face.
                if (matches!(g.kind, Kind::Trigger | Kind::Exit) && bottom > here.z + 31.0 && bottom <= here.z + 72.0)
                    || (g.kind == Kind::Secret && d.z > 40.0)
                {
                    jump = true;
                }
            }
        }

        // At the end of the path but the monster is still out of sight (on a
        // ledge the nav graph doesn't reach): try other goals first.
        if let Some(i) = self.cur
            && self.goals[i].kind == Kind::Monster
            && !self.path.is_empty()
            && self.step + 1 >= self.path.len()
            && wish.length() < 24.0
        {
            let (e, c) = (self.goals[i].ent, self.center(self.goals[i].ent));
            if !self.clear_shot(self.eye(), c, e) {
                self.defer(i, "out of sight at the end of its path");
                return mv;
            }
        }

        // Progress towards the goal; stuck → jump and strafe, then block the edge.
        let mut unstick = false;
        if !self.path.is_empty()
            && let Some(gp) = self.goal_point()
        {
            let d = (gp - here).length();
            if d < self.best_dist - 16.0 || d < 32.0 {
                self.best_dist = self.best_dist.min(d);
                self.best_time = t;
                if self.ride_since.is_none() {
                    self.call = None;
                }
            } else if t - self.best_time > STUCK_TIME * 0.5 && t - self.best_time <= STUCK_TIME {
                unstick = true;
                // Up against something that opens when shot (a secret door
                // that closed again): shoot it, as a player would.
                if self.call.is_none() {
                    self.call = self.shootable_ahead(here, wish);
                }
            } else if t - self.best_time > STUCK_TIME {
                if self.opt.verbose {
                    eprintln!("  t={t:.1} stuck at {here:?}{}", self.ents_near(here));
                }
                if self.try_local_recovery(here, t) { return mv; }
                if self.step > 0 && self.step < self.path.len() {
                    self.block_here();
                    self.last_replan = -100.0;
                    self.best_dist = f32::INFINITY;
                    self.best_time = t;
                } else if self.bad_src.len() < 64 && !self.path.is_empty() {
                    // Couldn't get onto the first node: plan from another one.
                    self.bad_src.insert(self.path[0]);
                    self.last_replan = -100.0;
                    self.best_dist = f32::INFINITY;
                    self.best_time = t;
                } else {
                    self.noclip(gp, "stuck before the first node");
                }
                return mv;
            }
        }

        if self.frame.is_multiple_of(2) || self.target_cache.is_some_and(|(_, e)| self.vm().is_free(e) || self.vm().ent_f(e, self.o.health) <= 0.0) {
            self.target_cache = self.target();
        }
        if self.call.is_some_and(|e| self.vm().is_free(e) || self.vm().ent_f(e, self.o.takedamage) == 0.0 || self.vm().ent_f(e, self.o.health) <= 0.0) {
            self.call = None;
        }
        // Aim at where the cached target is now (a lift's button first).
        let target = self.call.map(|e| (self.center(e), e)).or_else(|| self.target_cache.map(|(_, e)| (self.center(e), e)));
        if let Some((tg, e)) = target {
            mv.angles = look_angles(tg - self.eye());
            mv.buttons |= 1;
            // Zombies only die when gibbed: rockets. Otherwise the
            // lightning gun in its range (hitscan: no projectiles for the
            // server to simulate), the super nailgun beyond it.
            let w = if self.vm().classname(e).contains("zombie") {
                7
            } else if (tg - self.eye()).length() < 560.0 {
                8
            } else {
                5
            };
            if w != self.weapon {
                self.weapon = w;
                self.impulses.push(w);
            }
        } else if wish.length_sq() > 1.0 {
            self.yaw = wish.y.atan2(wish.x).to_degrees();
            mv.angles = [0.0, self.yaw, 0.0];
        } else {
            mv.angles = [0.0, self.yaw, 0.0];
        }
        if wish.length_sq() > 1.0 {
            // Wish direction relative to the view yaw; slow down near a lift's middle.
            let fine = self.path.get(self.step).is_some_and(|&n| self.nav.is_fine(n));
            let speed = if riding.is_some() { (wish.length() * 8.0).min(320.0) }
                else if fine { (wish.length() * 8.0).min(160.0) } else { 320.0 };
            let w = wish.normalize() * speed;
            let (s, c) = mv.angles[1].to_radians().sin_cos();
            mv.forward = w.x * c + w.y * s;
            mv.side = w.x * s - w.y * c;
        }
        if unstick && riding.is_none() {
            mv.buttons |= 2;
            mv.side = if (t * 2.0) as i64 % 2 == 0 { 320.0 } else { -320.0 };
        }
        if jump {
            mv.buttons |= 2;
        }
        let waterlevel = self.vm().ent_f(self.me, self.o.waterlevel);
        if waterlevel >= 2.0 || self.cm.point_contents(here) == CONTENTS_WATER {
            // Swim towards the next node's height: up (jump at the surface to
            // climb out), or dive for a node on the floor below.
            let next = self.path.get(self.step).map(|&n| self.nav.pos[n as usize]);
            match next {
                Some(p) if p.z < here.z - 16.0 => mv.up = -300.0,
                _ if next.is_none_or(|p| p.z > here.z - 8.0) => {
                    mv.up = 300.0;
                    mv.buttons |= 2;
                }
                _ => {}
            }
        }
        mv
    }

    pub(super) fn goal_point(&self) -> Option<Vec3> {
        let g = &self.goals[self.cur?];
        match self.path.last() {
            Some(&n) => Some(self.nav.pos[n as usize]),
            None => Some(self.center(g.ent)),
        }
    }
}
