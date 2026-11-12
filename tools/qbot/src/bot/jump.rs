use super::*;

impl Bot<'_> {
    /// Last walking recovery before an assist: bridge a nearby graph gap only
    /// after the real movement code lands the player on its far side.
    pub(super) fn jump_route(&mut self, goal: usize) -> Option<Vec<u32>> {
        if self.jump_failed_goals.contains(&self.goals[goal].ent) { return None; }
        let checkpoint = self.nav.clone();
        let route = self.jump_route_inner(goal);
        if route.is_none() {
            let count = checkpoint.pos.len() as u32;
            self.nav.rollback(checkpoint);
            self.jump_tried.retain(|&(a, b)| a < count && b < count);
        }
        route
    }

    pub(super) fn jump_route_inner(&mut self, goal: usize) -> Option<Vec<u32>> {
        let here = self.me_origin();
        let goal_center = self.center(self.goals[goal].ent);
        // A reachable goal that failed its interaction needs the existing
        // fallback, not more samples or another identical walking route.
        if let (Some(src), Some(dst)) = (self.nav.nearest_clear(&self.geo, here, 96.0), self.goal_node(&self.goals[goal])) {
            let blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
            if self.nav.dijkstra(src, &blocked, self.opt.seed).0[dst as usize].is_finite() { return None; }
        }
        let refined = self.refine_local(here) | self.refine_local(goal_center);
        let vm = self.vm();
        let fields = &self.sv.core.sv.fields;
        if v(vm.ent_v(self.me, fields.mins)) != qworld::hull::HULL_MINS[1]
            || v(vm.ent_v(self.me, fields.maxs)) != qworld::hull::HULL_MAXS[1]
            || vm.ent_f(self.me, self.o.waterlevel) >= 2.0 {
            return None;
        }
        let src = self.nav.nearest_clear(&self.geo, self.me_origin(), 96.0)?;
        if self.goal_node(&self.goals[goal]).is_none() {
            let c = self.center(self.goals[goal].ent);
            if let Some(g) = nav::ground(&self.geo, c + Vec3::new(0.0, 0.0, 32.0), 192.0) {
                self.nav.extend(&self.geo, g);
            }
        }
        let dst = self.goal_node(&self.goals[goal])?;
        let blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
        let (dist, prev) = self.nav.dijkstra(src, &blocked, self.opt.seed);
        if dist[dst as usize].is_finite() {
            return refined.then(|| path_to(&prev, dst));
        }
        let basin = self.nav.reaching(dst, &blocked);
        let mut candidates = Vec::new();
        for (a, &d) in dist.iter().enumerate() {
            if !d.is_finite() { continue; }
            let p = self.nav.pos[a];
            for b in self.nav.around(p, p, 192.0, p.z, 48.0) {
                if !basin[b as usize] || self.jump_tried.contains(&(a as u32, b)) || self.nav.edge(a as u32, b).is_some() { continue; }
                let q = self.nav.pos[b as usize];
                let delta = q - p;
                let xy = delta.x.hypot(delta.y);
                if !(40.0..=192.0).contains(&xy) || delta.z > 40.0 { continue; }
                let cost = d + delta.length() + (self.nav.pos[dst as usize] - q).length();
                candidates.push((cost, a as u32, b));
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1, a.2).cmp(&(b.1, b.2))));
        if candidates.is_empty() { return None; }
        // The missing passage may be far from both endpoints of the overall
        // route. Refine the best two reachable/basin boundary pairs as well.
        let mut added = false;
        for &(_, a, b) in candidates.iter().take(2) {
            let center = (self.nav.pos[a as usize] + self.nav.pos[b as usize]) * 0.5;
            added |= self.refine_local(center);
        }
        if added {
            let refined_blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
            let (cost, prev) = self.nav.dijkstra(src, &refined_blocked, self.opt.seed);
            if cost[dst as usize].is_finite() { return Some(path_to(&prev, dst)); }
        }
        let blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
        let mut probe = Probe::snapshot(&self.sv, self.jump_speed?)?;
        for (_, a, b) in candidates.into_iter().take(24) {
            self.jump_tried.insert((a, b));
            let (from, to) = (self.nav.pos[a as usize], self.nav.pos[b as usize]);
            if let Some(jump) = probe.jump(from, to, &self.forbid).or_else(|| probe.recover(from, to, &self.forbid))
                && hints::allows_motion(&jump.positions, &self.hints) {
                let cost = jump.moves.len() as f32 * 16.0 + 64.0;
                self.nav.link(a, b, EdgeKind::Jump, cost);
                self.jump_links.insert((a, b), jump);
                self.rep.jump_links += 1;
                let (dist, prev) = self.nav.dijkstra(src, &blocked, self.opt.seed);
                if dist[dst as usize].is_finite() {
                    if self.opt.verbose {
                        eprintln!("  t={:.1} physics-tested jump {from:?} -> {to:?} for #{}", self.sv.time(), self.goals[goal].ent);
                    }
                    self.rep.nav_nodes = self.rep.nav_nodes.max(self.nav.pos.len());
                    return Some(path_to(&prev, dst));
                }
            }
        }
        None
    }

    /// Try a verified short maneuver to the current waypoint before
    /// blocking it. Use the same alignment, revalidation and replay checks
    /// as gap recovery, and never bypass a gate or special mover edge.
    pub(super) fn try_local_recovery(&mut self, here: Vec3, t: f64) -> bool {
        let Some(i) = self.cur else { return false; };
        let goal = self.goals[i].ent;
        if self.local_recovery_goals.contains(&goal) { return false; }
        let Some(&dst) = self.path.get(self.step) else { return false; };
        if self.step == 0 || self.jump_failed_goals.contains(&goal) { return false; }
        let edge = (self.path[self.step - 1], dst);
        if self.gates().edges.contains(&edge) || self.jump_tried.contains(&edge)
            || self.nav.edge(edge.0, edge.1).is_none_or(|e| !matches!(e.kind, EdgeKind::Walk | EdgeKind::Jump)) {
            return false;
        }
        let to = self.nav.pos[dst as usize];
        let velocity = v(self.vm().ent_v(self.me, self.sv.core.sv.fields.velocity));
        if velocity.x.hypot(velocity.y) > 16.0 || nav::clear(&self.geo, here, to) { return false; }
        if (to - here).length() > 192.0 || self.vm().ent_f(self.me, self.o.waterlevel) >= 2.0 { return false; }
        let fields = &self.sv.core.sv.fields;
        if v(self.vm().ent_v(self.me, fields.mins)) != qworld::hull::HULL_MINS[1]
            || v(self.vm().ent_v(self.me, fields.maxs)) != qworld::hull::HULL_MAXS[1] { return false; }
        let Some(mut probe) = self.jump_speed.and_then(|speed| Probe::snapshot(&self.sv, speed)) else { return false; };
        self.jump_tried.insert(edge);
        self.local_recovery_goals.insert(goal);
        let Some(plan) = probe.detour(here, to, &self.forbid) else { return false; };
        if !hints::allows_motion(&plan.positions, &self.hints) { return false; }
        let src = self.nav.add(here);
        self.nav.link(src, dst, EdgeKind::Jump, plan.moves.len() as f32 * 16.0);
        self.jump_links.insert((src, dst), plan);
        self.rep.jump_links += 1;
        self.path = std::iter::once(src).chain(self.path[self.step..].iter().copied()).collect();
        self.step = 1;
        self.best_time = t;
        self.jump_action = Some(JumpAction { edge: (src, dst), since: t, tick: None, released: false });
        true
    }

    /// Align and stop before replaying a verified jump. Abort on divergence;
    /// the ordinary route planner then chooses another way.
    pub(super) fn jump_move(&mut self, here: Vec3, t: f64) -> Option<ClientMove> {
        if self.jump_action.is_none() {
            let edge = if self.step > 0 && self.step < self.path.len()
                && self.jump_links.contains_key(&(self.path[self.step - 1], self.path[self.step])) {
                Some((self.path[self.step - 1], self.path[self.step]))
            } else if self.step + 1 < self.path.len()
                && (self.nav.pos[self.path[self.step] as usize] - here).length() < 32.0
                && self.jump_links.contains_key(&(self.path[self.step], self.path[self.step + 1])) {
                self.step += 1;
                Some((self.path[self.step - 1], self.path[self.step]))
            } else { None };
            self.jump_action = edge.map(|edge| JumpAction { edge, since: t, tick: None, released: false });
        }
        let mut a = self.jump_action.take()?;
        let jump = self.jump_links.get(&a.edge)?;
        let from = self.nav.pos[a.edge.0 as usize];
        let mut failed = t - a.since > 4.0;
        let mut refreshed = None;
        let mv = match a.tick {
            None => {
                let d = from - here;
                let velocity = v(self.vm().ent_v(self.me, self.sv.core.sv.fields.velocity));
                if d.z.abs() > 24.0 { failed = true; }
                if d.x.hypot(d.y) < 2.0 && d.z.abs() < 4.0 && velocity.length() < 8.0 {
                    if a.released {
                        // Doors, monsters and movers may have changed while
                        // approaching. Verify again from the actual launch.
                        refreshed = self.jump_speed.and_then(|speed| Probe::snapshot(&self.sv, speed))
                            .and_then(|mut probe| if jump.recovery {
                                probe.recover(here, self.nav.pos[a.edge.1 as usize], &self.forbid)
                            } else { probe.jump(here, self.nav.pos[a.edge.1 as usize], &self.forbid) })
                            .filter(|plan| hints::allows_motion(&plan.positions, &self.hints));
                        if refreshed.is_some() { a.tick = Some(0); } else { failed = true; }
                    }
                    a.released = true;
                    ClientMove::default()
                } else {
                    ClientMove { angles: [0.0, d.y.atan2(d.x).to_degrees(), 0.0], forward: if d.x.hypot(d.y) < 2.0 { 0.0 } else { (d.x.hypot(d.y) * 4.0).min(80.0) }, ..Default::default() }
                }
            }
            Some(tick) => {
                if (jump.positions[tick] - here).length() > 20.0 { failed = true; }
                if tick == jump.moves.len() {
                    if self.vm().ent_f(self.me, self.o.flags) as i32 & qworld::FL_ONGROUND != 0 && !failed {
                        self.best_time = t;
                        self.last_replan = t;
                        self.step += 1;
                        self.rep.jumps_completed += 1;
                        return None;
                    }
                    failed = true;
                    ClientMove::default()
                } else {
                    a.tick = Some(tick + 1);
                    jump.moves[tick]
                }
            }
        };
        if let Some(jump) = refreshed { self.jump_links.insert(a.edge, jump); }
        self.best_time = t;
        if failed {
            self.rep.jump_failures += 1;
            if let Some(i) = self.cur { self.jump_failed_goals.insert(self.goals[i].ent); }
            self.blocked.insert(a.edge);
            self.nav.unlink(a.edge.0, a.edge.1);
            self.jump_links.remove(&a.edge);
            self.path.clear();
            self.last_replan = -100.0;
            if self.opt.verbose { eprintln!("  t={t:.1} jump {:?} diverged at {here:?} (tick {:?})", a.edge, a.tick); }
            Some(ClientMove::default())
        } else {
            self.jump_action = Some(a);
            Some(mv)
        }
    }

    /// Steering on a lift edge; `None` when not on one.
    pub(super) fn ride(&mut self, here: Vec3, t: f64) -> Option<Vec3> {
        if self.step == 0 || self.step >= self.path.len() {
            self.ride_since = None;
            self.lift_action = None;
            return None;
        }
        let key = (self.path[self.step - 1], self.path[self.step]);
        let Some(&lift) = self.lifts.get(&key) else {
            self.ride_since = None;
            self.lift_action = None;
            return None;
        };
        if self.lift_action.as_ref().is_none_or(|a| a.edge != key) {
            self.lift_action = Some(LiftAction::new(key, t));
        }
        let since = match self.ride_since {
            Some((s, since)) if s == self.step => since,
            _ => {
                self.ride_since = Some((self.step, t));
                t
            }
        };
        // Progress is measured by the lift arriving, not by distance.
        self.best_time = t;
        let patience = if self.opt.seed == 0 { RIDE_TIME } else { RIDE_TIME * (0.6 + 0.9 * nav::unit_hash(self.opt.seed, 2, 0) as f64) };
        if self.lift_action.as_ref().is_some_and(|a| a.expired(t, patience)
            || (a.trip.is_none() && !a.called && t - since > patience)) {
            if self.opt.verbose {
                eprintln!("  t={t:.1} lift #{} never came", lift.ent);
            }
            self.block_here();
            self.ride_since = None;
            self.last_replan = -100.0;
            self.path.clear();
            return Some(Vec3::ZERO);
        }
        self.call = None;
        if self.lift_action.as_ref().is_some_and(|a| a.trip.is_some()) {
            return Some(self.lift_button_trip(here, t));
        }
        let surf = self.vm().ent_v(lift.ent, self.o.absmax)[2];
        let c = (lift.mins + lift.maxs) * 0.5;
        let still = self.vm().field_ofs("velocity").is_none_or(|f| v(self.vm().ent_v(lift.ent, f)).length_sq() == 0.0);
        let (up0, up1) = (self.nav.pos[key.0 as usize].z, self.nav.pos[key.1 as usize].z);
        let arrived = (surf - if up1 > up0 { lift.hi } else { lift.lo }).abs() < 4.0;
        // A lift that only moves when called: press (or shoot) a button that
        // fires it, once it has kept still for a moment.
        if still && !arrived && t - since > 1.5
            && let Some(w) = self.call_lift(lift.ent, here)
        {
            return Some(w);
        }
        if still && !arrived && t - since > 1.5
            && self.lift_action.as_ref().is_some_and(|a| !a.called)
            && let Some(trip) = self.plan_lift_button(lift.ent, here, key)
        {
            let a = self.lift_action.as_mut().unwrap();
            a.enter(LiftStage::ApproachButton, t);
            a.trip = Some(trip);
            self.rep.lift_detours += 1;
            return Some(self.lift_button_trip(here, t));
        }
        let inside = here.x > lift.mins.x - 16.0 && here.x < lift.maxs.x + 16.0 && here.y > lift.mins.y - 16.0 && here.y < lift.maxs.y + 16.0;
        let on = inside && (here.z - 24.0 - surf).abs() < 6.0;
        let up = self.nav.pos[key.1 as usize].z > self.nav.pos[key.0 as usize].z;
        let to_center = Vec3::new(c.x - here.x, c.y - here.y, 0.0);
        let at = |z: f32| (surf - z).abs() < 4.0;
        // Going up: board it at the bottom. Going down: drop onto it when it
        // is at the bottom; ride it down only if it isn't a plat (a plat stays
        // up while someone stands on it).
        let board = if up { at(lift.lo) } else { at(lift.lo) || (at(lift.hi) && !lift.plat) };
        let stage = if arrived && on { LiftStage::Leave } else if on { LiftStage::Ride } else if board { LiftStage::Board } else { LiftStage::Wait };
        self.lift_action.as_mut().unwrap().enter(stage, t);
        if on || board {
            Some(if to_center.length() > 8.0 { to_center } else { Vec3::ZERO })
        } else if inside && surf > here.z {
            // Under it while it is away: step out so it can come down.
            Some(-to_center)
        } else {
            Some(Vec3::ZERO)
        }
    }
}
