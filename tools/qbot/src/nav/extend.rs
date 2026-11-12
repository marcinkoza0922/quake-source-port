use super::*;

impl Nav {
    /// Flood fill from a new seed at `start` (snapped to the grid). Returns
    /// whether any node was added.
    pub fn extend(&mut self, cm: &Geo, start: Vec3) -> bool {
        let nav = self;
        let (cx, cy) = nav.cell(start);
        let snapped = Vec3::new(nav.origin.x + cx as f32 * GRID, nav.origin.y + cy as f32 * GRID, start.z + 1.0);
        let seed = [snapped, start + Vec3::new(0.0, 0.0, 1.0)]
            .into_iter()
            .filter(|&p| clear(cm, start, p) || p == start + Vec3::new(0.0, 0.0, 1.0))
            .find_map(|p| ground(cm, p, 2048.0).filter(|&g| !hazard(cm, g)));
        let Some(s) = seed else { return false };
        if nav.find_exact(s).is_some() {
            return false;
        }
        let first = nav.pos.len();
        nav.add(s);
        let dirs: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
        let mut i = first;
        while i < nav.pos.len() && nav.pos.len() < MAX_NODES {
            let p = nav.pos[i];
            let (cx, cy) = nav.cell(p);
            // In water a player can swim up to the surface and jump out.
            let swim = swim_rise(cm, p);
            for (dx, dy) in dirs {
                let tx = nav.origin.x + (cx + dx) as f32 * GRID;
                let ty = nav.origin.y + (cy + dy) as f32 * GRID;
                // Walk, jump, then (in water) swim up in growing steps as far
                // as the surface: the first height that gets over is used.
                // Low ceilings (crawlways, door frames): no room to probe a
                // stair step up, so walk level.
                let step = if clear(cm, p, p + Vec3::new(0.0, 0.0, STEP)) { STEP } else { 0.5 };
                let mut tries = vec![(EdgeKind::Walk, step), (EdgeKind::Jump, JUMP)];
                if swim > JUMP {
                    tries.extend([64.0, 128.0, 256.0, 512.0, 1024.0].into_iter().filter(|&h| h < swim).map(|h| (EdgeKind::Jump, h)));
                    tries.push((EdgeKind::Jump, swim));
                }
                for (kind, rise) in tries {
                    let up = p + Vec3::new(0.0, 0.0, rise);
                    if kind == EdgeKind::Jump && !clear(cm, p, up) {
                        break;
                    }
                    let t = Vec3::new(tx, ty, up.z);
                    if !clear(cm, up, t) {
                        continue;
                    }
                    let Some(g) = ground(cm, t, MAX_DROP) else { break };
                    if g.z - p.z > rise + 0.5 && swim > rise {
                        continue;
                    }
                    if g.z - p.z > rise + 0.5 || hazard(cm, g) || nav.forbid.iter().any(|f| f.contains(g)) {
                        break;
                    }
                    if kind == EdgeKind::Walk && g.z - p.z > STEP + 0.5 {
                        continue;
                    }
                    let to = match nav.find_exact(g) {
                        Some(n) => n,
                        None => nav.add(g),
                    };
                    let mut cost = (g - p).length();
                    if kind == EdgeKind::Jump {
                        cost += 32.0;
                    }
                    if p.z - g.z > 64.0 {
                        cost += 16.0;
                    }
                    nav.edges[i].push(Edge { to, cost, kind });
                    break;
                }
            }
            i += 1;
        }
        true
    }

    /// Refine only a requested 192-unit patch on an 8-unit lattice. Regions
    /// are attempted once per graph; all patches together add at most 8192
    /// nodes. New walk links require support throughout, never bridge gaps.
    pub fn refine(&mut self, cm: &Geo, center: Vec3) -> bool {
        let key = ((center.x / 64.0).floor() as i32, (center.y / 64.0).floor() as i32,
            (center.z / 64.0).floor() as i32);
        if self.refined_nodes >= 8192 || !self.refined.insert(key) { return false; }
        let first = self.pos.len();
        let mut local = Vec::new();
        for x in -12..=12 {
            for y in -12..=12 {
                if self.refined_nodes >= 8192 || self.pos.len() >= MAX_NODES { break; }
                let p = Vec3::new((center.x / 8.0).round() * 8.0 + x as f32 * 8.0,
                    (center.y / 8.0).round() * 8.0 + y as f32 * 8.0, center.z + STEP);
                let Some(g) = static_ground(cm, p, 2.0 * STEP) else { continue; };
                if hazard(cm, g) || self.forbid.iter().any(|f| f.contains(g)) { continue; }
                let n = match self.cells.get(&self.cell(g)).into_iter().flatten().copied().find(|&n| (self.pos[n as usize] - g).length() < 1.0) {
                    Some(n) => n,
                    None => { self.refined_nodes += 1; let n = self.add(g); self.fine.insert(n); n }
                };
                local.push(n);
            }
        }
        for a in local {
            let p = self.pos[a as usize];
            for b in self.around(p, p, 25.0, p.z, STEP) {
                if a == b || ((a as usize) < first && (b as usize) < first) { continue; }
                let q = self.pos[b as usize];
                if (q - p).length() > 32.0 { continue; }
                for (u, v) in [(a, b), (b, a)] {
                    if self.edge(u, v).is_none()
                        && !(0..=8).any(|i| {
                            let p = self.pos[u as usize] + (self.pos[v as usize] - self.pos[u as usize]) * (i as f32 / 8.0);
                            self.forbid.iter().any(|f| f.contains(p))
                        }) && supported_walk(cm, self.pos[u as usize], self.pos[v as usize]) {
                        self.link(u, v, EdgeKind::Walk, (q - p).length());
                    }
                }
            }
        }
        self.pos.len() > first
    }

    /// Discard a failed tentative patch while remembering its work budget.
    pub fn rollback(&mut self, mut checkpoint: Nav) {
        checkpoint.refined.extend(self.refined.iter().copied());
        checkpoint.refined_nodes = self.refined_nodes;
        *self = checkpoint;
    }
}
