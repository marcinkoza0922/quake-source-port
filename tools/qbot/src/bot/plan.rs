use super::*;

impl Bot<'_> {
    /// Pick the cheapest reachable goal; when nothing is reachable, lift
    /// blocks, and only then noclip. Returns false when there is nothing left to do.
    pub(super) fn replan(&mut self) -> bool {
        self.last_replan = self.sv.time();
        let here = self.me_origin();
        if self.nav.nearest(here, 96.0).is_none() && self.nav.extend(&self.geo, here) && self.opt.verbose {
            eprintln!("  t={:.1} nav extended from {here:?}: {} nodes", self.sv.time(), self.nav.pos.len());
        }
        // Standing on a brush entity (lift, platform) above a known floor:
        // path from the floor node below.
        let below = || {
            let tr = self.geo.trace(1, here, here - Vec3::new(0.0, 0.0, 1024.0));
            (!tr.startsolid && tr.fraction < 1.0).then_some(tr.endpos).and_then(|g| self.nav.nearest(g, 48.0))
        };
        let Some(src) = self.nav.nearest_clear_except(&self.geo, here, 96.0, &self.bad_src).or_else(below) else {
            // Off the graph (after a noclip or a fall): back to the nearest
            // node, and don't walk the edge it fell from again.
            if self.sv.time() - self.noclip_at > 1.0 {
                self.block_here();
            }
            self.path.clear();
            self.returns += 1;
            if self.returns > 8 {
                if self.opt.verbose {
                    eprintln!("  stranded at {here:?}");
                }
                self.rep.stranded = true;
                self.stranded = true;
                return false;
            }
            if let Some(n) = self.landing(here).or_else(|| self.nav.nearest(here, 2048.0)).or_else(|| self.nav.nearest_any(here)) {
                let to = self.nav.pos[n as usize];
                self.noclip(to, "off the nav graph");
            } else if let Some(i) = self.cur {
                self.fail(i, "no nav graph");
            }
            return true;
        };
        let others_left = self.goals.iter().any(|g| g.kind != Kind::Exit && Self::pending(g) && !self.dormant(g));
        let prev_goal = self.cur;
        let mut choice = None;
        let mut opener = None;
        self.update_views();
        let gates = self.gates();
        if gates.edges.len() != self.gated && self.opt.verbose {
            let mut doors: Vec<u32> = gates.door.values().copied().collect();
            doors.sort_unstable();
            doors.dedup();
            let ds: Vec<String> = doors.iter().map(|&d| format!("#{d} {} opened by {:?}", self.vm().classname(d), gates.openers.get(&d).map(|o| o.iter().map(|&i| self.goals[i].ent).collect::<Vec<_>>()))).collect();
            eprintln!("  t={:.1} {} edges gated by locked doors: {}", self.sv.time(), gates.edges.len(), ds.join(", "));
        }
        self.gated = gates.edges.len();
        // Gates are a preference: with them first, then without (the opener
        // may have fired already), then with blocks lifted.
        let gated = (!gates.edges.is_empty()).then(|| self.blocked.union(&gates.edges).copied().collect::<Blocked>());
        for pass in 0..3 {
            let blocked = match (&gated, pass) {
                (Some(g), 0) => g,
                (None, 2) => break,
                _ => &self.blocked,
            };
            let (dist, prev) = self.nav.dijkstra(src, blocked, self.opt.seed);
            let mut best: Option<(f32, usize, u32)> = None;
            for (i, g) in self.goals.iter().enumerate() {
                if !Self::pending(g) || g.deferred || (g.kind == Kind::Exit && others_left) || self.dormant(g) {
                    continue;
                }
                let n = self.goal_node(g).into_iter().chain(g.views.iter().copied()).filter(|&n| dist[n as usize].is_finite()).min_by(|&a, &b| dist[a as usize].total_cmp(&dist[b as usize]));
                if let Some(n) = n {
                    // Prefer finishing the current goal unless something is much closer.
                    let mut bias = if Some(i) == self.cur { 0.7 } else { 1.0 };
                    if self.opt.seed != 0 {
                        // A fixed preference per goal and variant: another goal order.
                        bias *= 1.0 + 0.6 * nav::unit_hash(self.opt.seed, 1, g.ent as u64);
                    }
                    let d = dist[n as usize] * bias;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, i, n));
                    }
                }
            }
            if let Some((_, i, n)) = best {
                let path = path_to(&prev, n);
                // Ungated plan through a locked door: go for its opener
                // instead (walking there if possible, else by noclip).
                if pass > 0 && gated.is_some()
                    && let Some(m) = path.windows(2).find_map(|w| gates.door.get(&(w[0], w[1])))
                    && let Some(&o) = gates.openers.get(m).into_iter().flatten().find(|&&o| Self::pending(&self.goals[o]) && self.goals[o].noclips < 3)
                {
                    // Search all alternatives and prerequisite chains under
                    // actual gates before choosing a walking action.
                    let approaches: Vec<Approach> = self.goals.iter().map(|g| {
                        if !Self::pending(g) || g.noclips >= 3 || self.dormant(g) { return Approach { touch: None, views: Vec::new() }; }
                        let mut views = Vec::new();
                        if g.kind == Kind::Button && self.shootable(g) && !g.deferred {
                            views.extend(g.views.iter().copied().filter(|&n| self.clear_shot(
                                self.nav.pos[n as usize] + Vec3::new(0.0, 0.0, 22.0), self.center(g.ent), g.ent,
                            )));
                        }
                        Approach { touch: self.goal_node(g), views }
                    }).collect();
                    let planner = crate::actions::Prerequisites {
                        nav: &self.nav, gates: &gates.edges, doors: &gates.door,
                        openers: &gates.openers, approaches: &approaches, blocked: &self.blocked,
                    };
                    if let Some(action) = planner.first_step(src, *m, self.opt.seed) {
                        if self.cur != Some(action.goal) { self.rep.prerequisite_actions += 1; }
                        if self.opt.verbose && self.cur != Some(action.goal) {
                            eprintln!("  t={:.1} prerequisites {:?}: opener #{} first", self.sv.time(), action.doors, self.goals[action.goal].ent);
                        }
                        choice = Some((action.goal, action.path));
                    } else {
                        // Gate inference is incomplete for scripted maps.
                        // Preserve the existing ungated recovery if no verified
                        // prerequisite leaf exists, rather than forcing assists.
                        let og = &self.goals[o];
                        let reach = crate::actions::opener_node(self.goal_node(og), &approaches[o].views, &dist);
                        match reach {
                            Some(n) => choice = Some((o, path_to(&prev, n))),
                            None => opener = Some(o),
                        }
                    }
                    break;
                }
                choice = Some((i, path));
                break;
            }
            if pass == 0 && gated.is_some() {
                continue;
            }
            let sig = self.mover_sig;
            if !self.reset_blocks() {
                break;
            }
            if self.mover_sig != sig {
                // New graph: node numbers changed, plan again next frame.
                self.cur = None;
                return true;
            }
        }
        if let Some(o) = opener {
            if self.cur != Some(o) {
                self.returns = 0;
            }
            self.cur = Some(o);
            self.noclip_goal(o);
        } else if let Some((i, path)) = choice {
            if self.cur != Some(i) {
                self.returns = 0;
            }
            self.cur = Some(i);
            self.path = path;
            self.step = 0;
            self.lift_action = None;
        } else {
            // Nothing reachable by game means. Triggers are not worth a noclip
            // (many are meant for monsters); noclip to anything else,
            // non-deferred goals first.
            for i in 0..self.goals.len() {
                if self.goals[i].kind == Kind::Trigger && Self::pending(&self.goals[i]) {
                    self.goals[i].failed = true;
                    self.rep.triggers_missed += 1;
                }
            }
            // In entity order, which tends to follow the level's layout;
            // goals not deferred first.
            let pick = |deferred: bool| {
                self.goals
                    .iter()
                    .position(|g| Self::pending(g) && !self.dormant(g) && g.deferred == deferred && (g.kind != Kind::Exit || !others_left))
            };
            // Keys first: a key door can cut off much of the level.
            let key = self.goals.iter().position(|g| g.kind == Kind::Key && Self::pending(g));
            let pick = key.or_else(|| pick(false)).or_else(|| pick(true));
            match pick {
                Some(i) => {
                    if self.goals[i].noclips >= 3 {
                        self.fail(i, "given up after 3 noclips");
                        return true;
                    }
                    self.cur = Some(i);
                    self.noclip_goal(i);
                }
                None => self.cur = None,
            }
        }
        let active = self.cur.is_some() || self.goals.iter().any(|g| Self::pending(g) && !self.dormant(g));
        if self.cur != prev_goal {
            self.goal_start = self.sv.time();
            self.best_dist = f32::INFINITY;
            self.best_time = self.sv.time();
            if let Some(i) = self.cur {
                let g = &self.goals[i];
                if g.kind == Kind::Exit {
                    let (e, touch) = (g.ent, g.touch);
                    self.sv.level.as_mut().unwrap().vm.set_ent_i(e, self.o.touch, touch);
                    self.rec.exits.push((self.frame, e));
                }
                if self.opt.verbose {
                    let g = &self.goals[i];
                    eprintln!("  t={:.1} goal {:?} #{} ({})", self.sv.time(), g.kind, g.ent, self.vm().classname(g.ent));
                }
            }
        }
        active
    }

    /// Edges (walk, jump, lift rides) through closed doors the bot can't open by walking into them:
    /// key doors while it lacks the key, and doors opened only by a button or
    /// trigger (the master door has a targetname and no health) that a
    /// pending goal still fires, directly or through relays. The key, button
    /// or trigger is an ordinary goal, so the bot fetches it first; the edges
    /// open again once the key is held or the door has moved. Doors nothing
    /// pending opens (delayed, opened by a monster's death) stay walkable.
    pub(super) fn gates(&self) -> Gates {
        let mut out = Gates::default();
        let vm = self.vm();
        let (Some(owner), Some(items), Some(pos1)) = (self.o.owner, self.o.items, self.o.pos1) else { return out };
        let have = vm.ent_f(self.me, items) as i32;
        let velocity = vm.field_ofs("velocity");
        // Names each pending goal fires, through any chain of relays.
        let relays: Vec<(Vec<u8>, Vec<u8>)> = (1..vm.num_edicts() as u32)
            .filter(|&e| !vm.is_free(e))
            .map(|e| (ent_str(vm, e, self.o.targetname), ent_str(vm, e, self.o.target)))
            .filter(|(n, t)| !n.is_empty() && !t.is_empty())
            .collect();
        let mut fires: Vec<(usize, std::collections::HashSet<Vec<u8>>)> = Vec::new();
        for (i, g) in self.goals.iter().enumerate() {
            if !Self::pending(g) || !matches!(g.kind, Kind::Button | Kind::Trigger | Kind::Shoot | Kind::Secret | Kind::Pickup) {
                continue;
            }
            let t = ent_str(vm, g.ent, self.o.target);
            if t.is_empty() {
                continue;
            }
            let mut names = std::collections::HashSet::from([t]);
            let mut grew = true;
            while grew {
                grew = false;
                for (n, t) in &relays {
                    if names.contains(n) && !names.contains(t) {
                        names.insert(t.clone());
                        grew = true;
                    }
                }
            }
            fires.push((i, names));
        }
        let oldorigin = vm.field_ofs("oldorigin");
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP {
                continue;
            }
            let secret = match vm.classname(e).as_str() {
                "door" | "func_door" => false,
                "func_door_secret" => true,
                _ => continue,
            };
            if velocity.is_some_and(|f| v(vm.ent_v(e, f)).length_sq() > 0.0) {
                continue;
            }
            let m = if secret {
                // Closed: where it spawned (oldorigin); opened by its
                // trigger, or by shooting it (then it is its own goal).
                if oldorigin.is_none_or(|f| (v(vm.ent_v(e, self.o.origin)) - v(vm.ent_v(e, f))).length() > 1.0) {
                    continue;
                }
                let name = ent_str(vm, e, self.o.targetname);
                let shoot = self.goals.iter().position(|g| g.ent == e && g.kind == Kind::Shoot && Self::pending(g));
                let remote: Vec<usize> = if name.is_empty() { Vec::new() } else { fires.iter().filter(|f| f.1.contains(&name)).map(|f| f.0).collect() };
                if shoot.is_none() && remote.is_empty() {
                    continue;
                }
                out.openers.entry(e).or_insert_with(|| shoot.into_iter().chain(remote).collect());
                e
            } else {
                // Closed: resting where it spawned.
                if (v(vm.ent_v(e, self.o.origin)) - v(vm.ent_v(e, pos1))).length() > 1.0 {
                    continue;
                }
                let m = match vm.ent_i(e, owner) as u32 {
                    0 => e,
                    m if vm.is_free(m) => e,
                    m => m,
                };
                let need = vm.ent_f(m, items) as i32 & (IT_KEY1 | IT_KEY2) & !have;
                let name = ent_str(vm, m, self.o.targetname);
                let remote = vm.ent_f(m, self.o.health) <= 0.0 && fires.iter().any(|f| f.1.contains(&name));
                if need == 0 && !remote {
                    continue;
                }
                out.openers.entry(m).or_insert_with(|| {
                    let mut o: Vec<usize> = fires.iter().filter(|f| remote && f.1.contains(&name)).map(|f| f.0).collect();
                    o.extend(self.goals.iter().enumerate().filter(|(_, g)| {
                        g.kind == Kind::Key && Self::pending(g) && vm.ent_f(g.ent, items) as i32 & need != 0
                    }).map(|(i, _)| i));
                    o
                });
                m
            };
            let (lo, hi) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            let (zc, dz) = ((lo.z + hi.z) * 0.5, (hi.z - lo.z) * 0.5 + 64.0);
            // Slightly under the player's half width: edges grazing the frame stay.
            let (bl, bh) = (lo - Vec3::new(14.0, 14.0, 0.0), hi + Vec3::new(14.0, 14.0, 0.0));
            for n in self.nav.around(lo, hi, 64.0, zc, dz) {
                let pa = self.nav.pos[n as usize];
                for ed in &self.nav.edges[n as usize] {
                    if ed.kind == EdgeKind::Teleport {
                        continue;
                    }
                    let pb = self.nav.pos[ed.to as usize];
                    // The body spans origin -24..+32; standing on top of the door is fine.
                    if pa.z.min(pb.z) - 23.0 >= hi.z || pa.z.max(pb.z) + 32.0 <= lo.z {
                        continue;
                    }
                    if seg_hits_box_xy(pa, pb, bl, bh) {
                        out.edges.insert((n, ed.to));
                        out.edges.insert((ed.to, n));
                        out.door.insert((n, ed.to), m);
                        out.door.insert((ed.to, n), m);
                    }
                }
            }
        }
        out
    }

    /// The nearest node within 2048 units with room to stand and a floor
    /// under it now (nodes built on a plat float once it has moved away), or
    /// in water.
    pub(super) fn landing(&self, here: Vec3) -> Option<u32> {
        let ok = |n: u32| {
            let p = self.nav.pos[n as usize];
            self.fits(p) && (self.geo.point_contents(p) == CONTENTS_WATER || !self.fits(p - Vec3::new(0.0, 0.0, 4.0)))
        };
        if let Some(n) = self.nav.nearest(here, 2048.0).filter(|&n| ok(n)) {
            return Some(n);
        }
        let r = Vec3::splat(2048.0);
        let mut ns = self.nav.around(here - r, here + r, 0.0, here.z, 2048.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - here).length_sq().total_cmp(&(self.nav.pos[b as usize] - here).length_sq()));
        ns.into_iter().take(64).find(|&n| ok(n))
    }

    /// Noclip to goal `i` (the current goal).
    pub(super) fn noclip_goal(&mut self, i: usize) {
        if let Some(path) = self.jump_route(i) {
            self.path = path;
            self.step = 0;
            self.best_time = self.sv.time();
            self.best_dist = f32::INFINITY;
            self.last_replan = self.sv.time();
            return;
        }
        self.path.clear();
        let g = &self.goals[i];
        let to = match (g.kind, self.goal_node(g)) {
            (Kind::Monster, _) => self.monster_spot(g.ent),
            (Kind::Secret | Kind::Key | Kind::Pickup | Kind::Trigger | Kind::Exit, node) => self.trigger_spot(g.ent, node.map(|n| self.nav.pos[n as usize])).unwrap_or_else(|| self.center(g.ent)),
            (Kind::Button, None) => self.touch_spot(g.ent).unwrap_or_else(|| self.center(g.ent)),
            (_, Some(n)) => self.nav.pos[n as usize],
            (_, None) => self.center(g.ent),
        };
        self.goals[i].deferred = false;
        if self.opt.verbose {
            eprintln!("    near goal:{}", self.ents_near(to));
        }
        self.noclip(to, "unreachable");
    }
}
