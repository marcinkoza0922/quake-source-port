use super::*;

impl Bot<'_> {
    pub(super) fn vm(&self) -> &Vm {
        &self.sv.level.as_ref().unwrap().vm
    }

    pub(super) fn me_origin(&self) -> Vec3 {
        v(self.vm().ent_v(self.me, self.o.origin))
    }

    pub(super) fn center(&self, e: u32) -> Vec3 {
        let vm = self.vm();
        (v(vm.ent_v(e, self.o.absmin)) + v(vm.ent_v(e, self.o.absmax))) * 0.5
    }

    pub(super) fn goal_done(&self, g: &Goal) -> bool {
        let vm = self.vm();
        let e = g.ent;
        if vm.is_free(e) {
            return g.kind != Kind::Exit;
        }
        match g.kind {
            Kind::Monster => vm.ent_f(e, self.o.health) <= 0.0 || vm.ent_f(e, self.o.flags) as i32 & FL_MONSTER == 0,
            Kind::Secret => vm.ent_i(e, self.o.touch) == 0 || vm.ent_f(e, self.o.solid) as i32 != SOLID_TRIGGER,
            // Keys spawn SOLID_NOT until PlaceItem runs: taken once placed
            // and not solid again.
            Kind::Key | Kind::Pickup => g.armed && vm.ent_f(e, self.o.solid) as i32 == SOLID_NOT,
            Kind::Button => self.o.state.is_some_and(|s| vm.ent_f(e, s) != g.initial_state),
            Kind::Shoot => vm.ent_f(e, self.o.takedamage) == 0.0 || vm.ent_f(e, self.o.health) <= 0.0,
            Kind::Trigger => {
                vm.ent_i(e, self.o.touch) == 0 || vm.ent_f(e, self.o.solid) as i32 != SOLID_TRIGGER || {
                    // trigger_multiple stays: done once we stand in it.
                    let (a, b) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
                    let p = self.me_origin();
                    p.x + 16.0 > a.x && p.x - 16.0 < b.x && p.y + 16.0 > a.y && p.y - 16.0 < b.y && p.z + 32.0 > a.z && p.z - 24.0 < b.z
                }
            }
            Kind::Exit => false,
        }
    }

    pub(super) fn visible(&self, from: Vec3, to: Vec3) -> bool {
        self.cm.hull(0, 0).trace(from, to).fraction >= 1.0
    }

    pub(super) fn eye(&self) -> Vec3 {
        self.me_origin() + v(self.vm().ent_v(self.me, self.o.view_ofs))
    }

    /// Whether a shot from `a` reaches `b`: world, static solids and every
    /// other brush entity where it is now (bars, glass, closed doors).
    pub(super) fn clear_shot(&self, a: Vec3, b: Vec3, target: u32) -> bool {
        if self.geo.trace(0, a, b).fraction < 1.0 {
            return false;
        }
        let l = self.sv.level.as_ref().unwrap();
        let vm = &l.vm;
        let Some(mi) = vm.field_ofs("modelindex") else { return true };
        let (lo, hi) = (a.min(b), a.max(b));
        for e in 1..vm.num_edicts() as u32 {
            if e == target || vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP {
                continue;
            }
            let (emin, emax) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            if emin.x > hi.x || emax.x < lo.x || emin.y > hi.y || emax.y < lo.y || emin.z > hi.z || emax.z < lo.z {
                continue;
            }
            if let Some((m, sub)) = l.world.model(vm.ent_f(e, mi) as usize) {
                let o = v(vm.ent_v(e, self.o.origin));
                if m.hull(*sub, 0).trace(a - o, b - o).fraction < 1.0 {
                    return false;
                }
            }
        }
        true
    }

    /// Nearest live monster or shootable target with a clear shot, within range.
    pub(super) fn target(&self) -> Option<(Vec3, u32)> {
        let eye = self.eye();
        let vm = self.vm();
        let mut cands: Vec<(f32, Vec3, u32)> = Vec::new();
        for g in &self.goals {
            if g.done {
                continue;
            }
            let shoot = match g.kind {
                Kind::Monster => !self.dormant(g),
                Kind::Button | Kind::Shoot => vm.ent_f(g.ent, self.o.health) > 0.0 && vm.ent_f(g.ent, self.o.takedamage) != 0.0,
                _ => false,
            };
            if !shoot {
                continue;
            }
            let c = self.center(g.ent);
            let d = (c - eye).length_sq();
            if d < 2000.0 * 2000.0 {
                cands.push((d, c, g.ent));
            }
        }
        // Nearest first; traces only until a clear shot turns up.
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        cands.iter().take(12).filter(|c| self.visible(eye, c.1)).take(4).find(|c| self.clear_shot(eye, c.1, c.2)).map(|c| (c.1, c.2))
    }

    /// Node to reach for a goal.
    pub(super) fn goal_node(&self, g: &Goal) -> Option<u32> {
        let vm = self.vm();
        let (mins, maxs) = (v(vm.ent_v(g.ent, self.o.absmin)), v(vm.ent_v(g.ent, self.o.absmax)));
        match g.kind {
            Kind::Secret | Kind::Key | Kind::Pickup | Kind::Trigger => self.nav.inside(mins, maxs),
            Kind::Monster => self.nav.nearest(v(vm.ent_v(g.ent, self.o.origin)), 256.0),
            Kind::Button => self.nav.inside(mins - Vec3::splat(24.0), maxs + Vec3::splat(24.0)),
            // Anywhere in the open within 128 units, shooting it from there.
            Kind::Shoot => self.nav.nearest_clear(&self.geo, (mins + maxs) * 0.5, 160.0),
            Kind::Exit => g.node,
        }
    }

    pub(super) fn shootable(&self, g: &Goal) -> bool {
        let vm = self.vm();
        match g.kind {
            Kind::Monster | Kind::Shoot => true,
            Kind::Button => vm.ent_f(g.ent, self.o.health) > 0.0 && vm.ent_f(g.ent, self.o.takedamage) != 0.0,
            _ => false,
        }
    }

    /// Refresh the view nodes of shootable goals whose target moved.
    pub(super) fn update_views(&mut self) {
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if !Self::pending(g) || !self.shootable(g) {
                continue;
            }
            let c = self.center(g.ent);
            if g.views_at.is_some_and(|p| (p - c).length() < 64.0) {
                continue;
            }
            let r = Vec3::splat(768.0);
            let mut ns = self.nav.around(c - r, c + r, 0.0, c.z, 768.0);
            ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - c).length_sq().total_cmp(&(self.nav.pos[b as usize] - c).length_sq()));
            let mut views = Vec::new();
            for &n in ns.iter().take(256) {
                if self.geo.trace(0, self.nav.pos[n as usize] + Vec3::new(0.0, 0.0, 22.0), c).fraction >= 1.0 {
                    views.push(n);
                    if views.len() >= 8 {
                        break;
                    }
                }
            }
            let g = &mut self.goals[i];
            g.views = views;
            g.views_at = Some(c);
        }
    }

    /// Monsters waiting to be spawned by a trigger (hipnotic, AD): not solid
    /// and not damageable yet.
    pub(super) fn dormant(&self, g: &Goal) -> bool {
        let vm = self.vm();
        g.kind == Kind::Monster && vm.ent_f(g.ent, self.o.solid) as i32 == SOLID_NOT && vm.ent_f(g.ent, self.o.takedamage) == 0.0
    }

    pub(super) fn pending(g: &Goal) -> bool {
        !g.done && !g.failed
    }

    pub(super) fn fail(&mut self, i: usize, why: &str) {
        if !self.goals[i].failed {
            self.goals[i].failed = true;
            self.rep.goals_failed += 1;
            if self.opt.verbose {
                eprintln!("  t={:.1} goal #{} failed: {why}", self.sv.time(), self.goals[i].ent);
            }
        }
        if self.cur == Some(i) {
            self.cur = None;
        }
    }

    pub(super) fn defer(&mut self, i: usize, why: &str) {
        let g = &mut self.goals[i];
        g.deferred = true;
        g.defers += 1;
        if self.opt.verbose {
            eprintln!("  t={:.1} goal #{} deferred: {why}", self.sv.time(), g.ent);
        }
        // A failed walking/firing approach is not an exhausted monster.
        // Keep its bounded assist budget available after other goals finish.
        if g.defers > 3 && (g.kind != Kind::Monster || g.noclips >= 3) {
            self.fail(i, "deferred too often");
        }
        if self.cur == Some(i) {
            self.cur = None;
        }
    }

    /// Rebuild the graph when movers have moved since it was built (a bridge
    /// came out, a lift is somewhere else).
    pub(super) fn rebuild_nav(&mut self) {
        let l = self.sv.level.as_ref().unwrap();
        if mover_sig(&l.vm, &self.o) == self.mover_sig && !self.nav.pos.is_empty() {
            return;
        }
        let exits: Vec<u32> = self.goals.iter().filter(|g| g.kind == Kind::Exit).map(|g| g.ent).collect();
        let b = build_nav(&l.vm, &l.world, &self.o, &self.cm, self.start, &self.forbid, &exits, &self.hints);
        for e in b.bad_hints {
            if !self.rep.errors.contains(&e) {
                self.rep.errors.push(e);
            }
        }
        // Seed from where the bot is too, in case the start is cut off now.
        let mut nav = b.nav;
        nav.extend(&b.geo, self.me_origin());
        self.geo = b.geo;
        self.nav = nav;
        self.lifts = b.lifts;
        self.mover_sig = b.sig;
        let mut nodes = b.exit_nodes.into_iter();
        for g in &mut self.goals {
            g.views.clear();
            g.views_at = None;
            if g.kind == Kind::Exit {
                g.node = nodes.next().flatten();
            }
        }
        self.path.clear();
        self.step = 0;
        self.ride_since = None;
        self.lift_action = None;
        self.bad_src.clear();
        self.jump_links.clear();
        self.jump_tried.clear();
        self.jump_action = None;
        self.rep.nav_nodes = self.rep.nav_nodes.max(self.nav.pos.len());
        if self.opt.verbose {
            eprintln!("  t={:.1} nav rebuilt: {} nodes", self.sv.time(), self.nav.pos.len());
        }
    }

    /// Lift every block and deferral when something changed since last time.
    pub(super) fn reset_blocks(&mut self) -> bool {
        let any = !self.blocked.is_empty() || self.goals.iter().any(|g| Self::pending(g) && g.deferred);
        if !any || !self.progress {
            return false;
        }
        if self.opt.verbose {
            eprintln!("  t={:.1} lifting {} blocked edges and deferrals", self.sv.time(), self.blocked.len());
        }
        self.blocked.clear();
        self.bad_src.clear();
        for g in &mut self.goals {
            g.deferred = false;
        }
        self.progress = false;
        self.rebuild_nav();
        true
    }
}
