use super::*;

impl Bot<'_> {
    /// Plan a walk/jump detour to a caller without using the lift being called,
    /// other movers, teleporters or locked doors. Return along that same route.
    pub(super) fn plan_lift_button(&self, lift: u32, here: Vec3, edge: (u32, u32)) -> Option<ButtonTrip> {
        let src = self.nav.nearest_clear(&self.geo, here, 96.0)?;
        let mut blocked = self.blocked.clone();
        blocked.extend(self.gates().edges);
        blocked.extend(self.jump_links.keys().copied());
        blocked.insert(edge);
        blocked.insert((edge.1, edge.0));
        let mut candidates = Vec::new();
        for b in self.lift_callers(lift) {
            let shoot = self.vm().ent_f(b, self.o.health) > 0.0 && self.vm().ent_f(b, self.o.takedamage) != 0.0;
            let Some(g) = self.goals.iter().find(|g| g.ent == b) else { continue };
            let nodes = self.goal_node(g).into_iter().chain(if shoot { g.views.as_slice() } else { &[] }.iter().copied());
            for n in nodes {
                if shoot && !self.clear_shot(self.nav.pos[n as usize] + Vec3::new(0.0, 0.0, 22.0), self.center(b), b) {
                    continue;
                }
                candidates.push((b, n, shoot));
            }
        }
        let trip = crate::actions::button_trip(&self.nav, src, &candidates, &blocked, self.opt.seed)?;
        let ent = trip.ent;
        if self.opt.verbose {
            eprintln!("  t={:.1} lift #{lift}: detour to caller #{ent}", self.sv.time());
        }
        Some(trip)
    }

    pub(super) fn lift_button_trip(&mut self, here: Vec3, t: f64) -> Vec3 {
        let mut a = self.lift_action.take().unwrap();
        let trip = a.trip.as_mut().unwrap();
        let activated = self.vm().is_free(trip.ent)
            || self.o.state.is_some_and(|s| self.vm().ent_f(trip.ent, s) != 1.0);
        if activated && !trip.returning {
            trip.returning = true;
            trip.path.reverse();
            trip.step = 0;
            a.called = true;
            a.stage = LiftStage::ReturnToLift;
            a.since = t;
            self.call = None;
        }
        while trip.step + 1 < trip.path.len() {
            let d = self.nav.pos[trip.path[trip.step] as usize] - here;
            if d.x.hypot(d.y) >= REACH || d.z.abs() >= 30.0 { break; }
            trip.step += 1;
        }
        let p = self.nav.pos[trip.path[trip.step] as usize];
        let d = p - here;
        let at_end = trip.step + 1 == trip.path.len() && d.x.hypot(d.y) < REACH && d.z.abs() < 30.0;
        let wish = if at_end && trip.returning {
            a.trip = None;
            a.enter(LiftStage::Wait, t);
            // Restart the lift wait after returning from the button.
            self.ride_since = Some((self.step, t));
            Vec3::ZERO
        } else if at_end {
            if a.stage != LiftStage::ActivateButton {
                a.stage = LiftStage::ActivateButton;
                a.since = t;
            }
            if trip.shoot {
                if self.clear_shot(self.eye(), self.center(trip.ent), trip.ent) {
                    self.call = Some(trip.ent);
                }
                Vec3::ZERO
            } else {
                let c = self.center(trip.ent);
                let bottom = self.vm().ent_v(trip.ent, self.o.absmin)[2];
                self.action_jump = bottom > here.z + 31.0 && bottom <= here.z + 72.0;
                Vec3::new(c.x - here.x, c.y - here.y, 0.0)
            }
        } else {
            self.action_jump = trip.step > 0 && self.nav.edge(trip.path[trip.step - 1], trip.path[trip.step])
                .is_some_and(|e| e.kind == EdgeKind::Jump);
            Vec3::new(d.x, d.y, 0.0)
        };
        self.lift_action = Some(a);
        wish
    }

    /// A shootable brush entity (secret door, button) right in front of the
    /// bot in the direction it wants to go.
    pub(super) fn shootable_ahead(&self, here: Vec3, wish: Vec3) -> Option<u32> {
        if wish.length_sq() < 1.0 {
            return None;
        }
        let ahead = here + wish.normalize() * 48.0;
        let vm = self.vm();
        (1..vm.num_edicts() as u32).find(|&e| {
            if vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP || vm.ent_f(e, self.o.takedamage) == 0.0 || vm.ent_f(e, self.o.health) <= 0.0 {
                return false;
            }
            let (lo, hi) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            ahead.x + 16.0 > lo.x && ahead.x - 16.0 < hi.x && ahead.y + 16.0 > lo.y && ahead.y - 16.0 < hi.y && here.z + 32.0 > lo.z && here.z - 24.0 < hi.z
        })
    }

    /// Unpressed buttons that fire `lift` (directly or through relays).
    pub(super) fn lift_callers(&self, lift: u32) -> Vec<u32> {
        let vm = self.vm();
        let owner = self.o.owner.map_or(0, |f| vm.ent_i(lift, f) as u32);
        let m = if owner != 0 && !vm.is_free(owner) { owner } else { lift };
        let mut names = vec![ent_str(vm, m, self.o.targetname)];
        if names[0].is_empty() {
            return Vec::new();
        }
        // Walk relays backwards: anything targeting a name we want, with a name itself.
        let mut i = 0;
        while i < names.len() && names.len() < 16 {
            for e in 1..vm.num_edicts() as u32 {
                if !vm.is_free(e) && ent_str(vm, e, self.o.target) == names[i] && vm.classname(e) != "func_button" {
                    let n = ent_str(vm, e, self.o.targetname);
                    if !n.is_empty() && !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            i += 1;
        }
        (1..vm.num_edicts() as u32)
            .filter(|&e| !vm.is_free(e) && vm.classname(e) == "func_button" && names.contains(&ent_str(vm, e, self.o.target)))
            // STATE_BOTTOM: out, ready to press.
            .filter(|&e| self.o.state.is_none_or(|s| vm.ent_f(e, s) == 1.0))
            .collect()
    }

    /// Move (or aim) to press a button calling `lift`; `None` when there is none in reach.
    pub(super) fn call_lift(&mut self, lift: u32, here: Vec3) -> Option<Vec3> {
        let eye = self.eye();
        let mut best: Option<(f32, u32)> = None;
        for b in self.lift_callers(lift) {
            let d = (self.center(b) - here).length();
            if d < 640.0 && best.is_none_or(|x| d < x.0) {
                best = Some((d, b));
            }
        }
        let (_, b) = best?;
        let c = self.center(b);
        let vm = self.vm();
        if vm.ent_f(b, self.o.health) > 0.0 && vm.ent_f(b, self.o.takedamage) != 0.0 && self.clear_shot(eye, c, b) {
            self.call = Some(b);
            return Some(Vec3::ZERO);
        }
        // Walk into it if it is within a jump of the floor and nothing is in between.
        let spot = self.touch_spot(b).unwrap_or(c);
        if (spot.z - here.z).abs() > 48.0 || !nav::clear(&self.geo, here, Vec3::new(spot.x, spot.y, here.z)) {
            return None;
        }
        if self.opt.verbose && self.frame.is_multiple_of(20) {
            eprintln!("  t={:.1} calling lift #{lift} with button #{b}", self.sv.time());
        }
        Some(Vec3::new(c.x - here.x, c.y - here.y, 0.0))
    }
}
