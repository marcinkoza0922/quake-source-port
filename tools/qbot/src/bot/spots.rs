use super::*;

impl Bot<'_> {
    /// A touch goal needs actual hull overlap after fitting around current
    /// doors and bodies. A generic nearest free node can land outside it.
    pub(super) fn trigger_spot(&self, e: u32, preferred: Option<Vec3>) -> Option<Vec3> {
        let vm = self.vm();
        let lo = v(vm.ent_v(e, self.o.absmin));
        let hi = v(vm.ent_v(e, self.o.absmax));
        let c = (lo + hi) * 0.5;
        let inside = |p: Vec3| p.x + 16.0 > lo.x && p.x - 16.0 < hi.x
            && p.y + 16.0 > lo.y && p.y - 16.0 < hi.y
            && p.z + 32.0 > lo.z && p.z - 24.0 < hi.z;
        if let Some(p) = preferred.filter(|&p| inside(p) && self.fits(p)) { return Some(p); }
        let mut candidates = self.nav.around(lo, hi, 16.0, c.z, (hi.z - lo.z) * 0.5 + 32.0)
            .into_iter().map(|n| self.nav.pos[n as usize]).filter(|&p| inside(p)).collect::<Vec<_>>();
        for x in [c.x, lo.x + 1.0, hi.x - 1.0, lo.x - 15.0, hi.x + 15.0] {
            for y in [c.y, lo.y + 1.0, hi.y - 1.0, lo.y - 15.0, hi.y + 15.0] {
                for z in [c.z, lo.z - 31.0, hi.z + 23.0] {
                    let p = Vec3::new(x, y, z);
                    if let Some(g) = nav::ground(&self.geo, p + Vec3::new(0.0, 0.0, 24.0), 128.0)
                        && inside(g) { candidates.push(g); }
                    if inside(p) { candidates.push(p); }
                }
            }
        }
        candidates.sort_by(|a, b| (*a - c).length_sq().total_cmp(&(*b - c).length_sq()));
        candidates.into_iter().find(|&p| self.fits(p))
    }

    /// Where a player stands pressing a brush entity off the graph (a button
    /// on a high wall): just off one of its faces, body overlapping it in
    /// height, standing on a floor if any face has one.
    pub(super) fn touch_spot(&self, e: u32) -> Option<Vec3> {
        let vm = self.vm();
        let (lo, hi) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
        let c = (lo + hi) * 0.5;
        let sides = [
            Vec3::new(lo.x - 17.0, c.y, c.z),
            Vec3::new(hi.x + 17.0, c.y, c.z),
            Vec3::new(c.x, lo.y - 17.0, c.z),
            Vec3::new(c.x, hi.y + 17.0, c.z),
        ];
        let mut floating = None;
        for p in sides {
            if !self.fits(p) {
                continue;
            }
            // Lowest origin whose body still reaches the button (body -24..+32).
            let tr = self.geo.trace(1, p, Vec3::new(p.x, p.y, lo.z - 31.0));
            let ground = tr.fraction < 1.0 && !tr.startsolid;
            if ground && self.fits(tr.endpos) {
                return Some(tr.endpos);
            }
            floating.get_or_insert(p);
        }
        floating
    }

    /// A nav node with a view of the monster, else a spot 72 units from it.
    pub(super) fn monster_spot(&self, e: u32) -> Vec3 {
        let c = self.center(e);
        let r = Vec3::splat(384.0);
        let mut ns = self.nav.around(c - r, c + r, 0.0, c.z, 384.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - c).length_sq().total_cmp(&(self.nav.pos[b as usize] - c).length_sq()));
        for &n in &ns {
            let p = self.nav.pos[n as usize];
            let eye = p + Vec3::new(0.0, 0.0, 22.0);
            if self.fits(p) && self.clear_shot(eye, c, e) {
                return p;
            }
        }
        // A moving monster may have entered a region absent from the graph.
        // Find a supported, unobstructed firing position near its live origin
        // instead of repeatedly assisting to a distant node behind a door.
        for radius in [72.0, 128.0, 192.0, 256.0] {
            for i in 0..16 {
                let angle = i as f32 * std::f32::consts::TAU / 16.0;
                let point = c + Vec3::new(angle.cos() * radius, angle.sin() * radius, 32.0);
                if let Some(p) = nav::ground(&self.geo, point, 256.0)
                    && self.fits(p) && self.clear_shot(p + Vec3::new(0.0, 0.0, 22.0), c, e) {
                    return p;
                }
            }
        }
        let here = self.me_origin();
        let back = Vec3::new(here.x - c.x, here.y - c.y, 0.0);
        let p = if back.length() > 1.0 { c + back.normalize() * 72.0 } else { c };
        if nav::clear(&self.geo, p, p + Vec3::new(0.0, 0.0, 1.0)) {
            return p;
        }
        self.nav.nearest_any(c).map_or(p, |n| self.nav.pos[n as usize])
    }

    /// Whether a player fits at `p` right now: world, brush entities where
    /// they are now (closed doors, cages) and other bodies.
    pub(super) fn fits(&self, p: Vec3) -> bool {
        if self.geo.trace(1, p, p).startsolid {
            return false;
        }
        let l = self.sv.level.as_ref().unwrap();
        let vm = &l.vm;
        let modelindex = vm.field_ofs("modelindex");
        let (pmin, pmax) = (p + Vec3::new(-16.0, -16.0, -24.0), p + Vec3::new(16.0, 16.0, 32.0));
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || e == self.me {
                continue;
            }
            let solid = vm.ent_f(e, self.o.solid) as i32;
            if solid == SOLID_NOT || solid == SOLID_TRIGGER {
                continue;
            }
            let (a, b) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            if a.x > pmax.x || b.x < pmin.x || a.y > pmax.y || b.y < pmin.y || a.z > pmax.z || b.z < pmin.z {
                continue;
            }
            if solid != SOLID_BSP {
                return false;
            }
            if let Some(mi) = modelindex
                && let Some((m, sub)) = l.world.model(vm.ent_f(e, mi) as usize)
            {
                let o = v(vm.ent_v(e, self.o.origin));
                if m.hull(*sub, 1).trace(p - o, p - o).startsolid {
                    return false;
                }
            }
        }
        true
    }

    /// Brush entities within 64 units (for the verbose log).
    pub(super) fn ents_near(&self, p: Vec3) -> String {
        let vm = self.vm();
        let mut out = String::new();
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP {
                continue;
            }
            let (a, b) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            if a.x < p.x + 128.0 && b.x > p.x - 128.0 && a.y < p.y + 128.0 && b.y > p.y - 128.0 && a.z < p.z + 128.0 && b.z > p.z - 128.0 {
                out += &format!(" [#{e} {} {a:?}-{b:?}]", vm.classname(e));
            }
        }
        out
    }

    /// `to`, or the nearest node around it where a player fits.
    pub(super) fn free_spot(&self, to: Vec3) -> Vec3 {
        if self.fits(to) {
            return to;
        }
        let r = Vec3::splat(256.0);
        let mut ns = self.nav.around(to - r, to + r, 0.0, to.z, 256.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - to).length_sq().total_cmp(&(self.nav.pos[b as usize] - to).length_sq()));
        ns.iter().take(48).map(|&n| self.nav.pos[n as usize]).find(|&p| self.fits(p)).unwrap_or(to)
    }

    /// Last resort: jump straight to `to` (a soft failure in the report).
    pub(super) fn noclip(&mut self, to: Vec3, why: &str) {
        let t = self.sv.time();
        if why != "off the nav graph" && let Some(i) = self.cur {
            self.goals[i].noclips += 1;
            if self.goals[i].noclips > 3 {
                self.fail(i, "given up after 3 noclips");
                return;
            }
        }
        let gates = self.gates();
        let src = if why == "off the nav graph" { None } else {
            self.nav.nearest_clear_except(&self.geo, self.me_origin(), 96.0, &self.bad_src)
        };
        let targets: Vec<u32> = self.cur.map(|i| &self.goals[i]).into_iter()
            .flat_map(|g| self.goal_node(g).into_iter().chain(g.views.iter().copied())).collect();
        let (cause, edge) = crate::diagnostics::classify(&self.nav, src, &targets, &gates.edges, &self.blocked);
        let to = self.free_spot(to);
        self.rep.assists.push(crate::diagnostics::Assist {
            time: t, cause, goal: self.cur.map(|i| self.goals[i].ent),
            kind: self.cur.map_or_else(|| "none".into(), |i| format!("{:?}", self.goals[i].kind)),
            classname: self.cur.map_or_else(String::new, |i| self.vm().classname(self.goals[i].ent)),
            reason: why.into(),
            from: self.me_origin(), to, door: edge.and_then(|e| gates.door.get(&e).copied()),
        });
        self.rep.noclip_uses += 1;
        if self.cur.is_some_and(|i| self.goals[i].kind == Kind::Monster) {
            self.rep.monster_noclips += 1;
        }
        if why != "off the nav graph" && self.cur.is_some_and(|i| self.goals[i].kind == Kind::Exit) {
            self.rep.exit_noclips += 1;
        }
        if self.opt.verbose {
            let g = self.cur.map(|i| (self.goals[i].kind, self.goals[i].ent, self.vm().classname(self.goals[i].ent)));
            eprintln!("  t={t:.1} NOCLIP ({why}) {:?} -> {to:?} (goal {g:?})", self.me_origin());
        }
        self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
        self.noclip_at = t;
        self.noclip_to_goal = why != "off the nav graph";
        self.best_dist = f32::INFINITY;
        self.best_time = t;
        self.ride_since = None;
        self.lift_action = None;
        self.jump_action = None;
    }

    /// Block the edge being walked and its parallel lanes (a doorway is
    /// several grid edges wide).
    pub(super) fn block_here(&mut self) {
        if self.step == 0 || self.step >= self.path.len() {
            return;
        }
        let (a, b) = (self.path[self.step - 1], self.path[self.step]);
        let (pa, pb) = (self.nav.pos[a as usize], self.nav.pos[b as usize]);
        let dir = pb - pa;
        let r = Vec3::splat(64.0);
        for n in self.nav.around(pa - r, pa + r, 0.0, pa.z, 40.0) {
            for e in &self.nav.edges[n as usize] {
                let q = self.nav.pos[e.to as usize];
                if (q - pb).length() < 96.0 && (q - self.nav.pos[n as usize]).dot(dir) > 0.0 {
                    self.blocked.insert((n, e.to));
                }
            }
        }
        self.blocked.insert((a, b));
        if self.opt.verbose {
            eprintln!("  t={:.1} blocked {pa:?} -> {pb:?} ({} edges blocked)", self.sv.time(), self.blocked.len());
        }
    }
}
