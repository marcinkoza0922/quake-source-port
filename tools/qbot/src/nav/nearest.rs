use super::*;

impl Nav {
    /// The node nearest to `p` within `radius`, preferring nodes at a similar height.
    pub fn nearest(&self, p: Vec3, radius: f32) -> Option<u32> {
        let (cx, cy) = self.cell(p);
        let r = (radius / GRID).ceil() as i32;
        let mut best: Option<(f32, u32)> = None;
        for x in cx - r..=cx + r {
            for y in cy - r..=cy + r {
                for &n in self.cells.get(&(x, y)).into_iter().flatten() {
                    let d = self.pos[n as usize] - p;
                    let score = d.x * d.x + d.y * d.y + 4.0 * d.z * d.z;
                    if score <= radius * radius * 4.0 && best.is_none_or(|b| score < b.0) {
                        best = Some((score, n));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The nearest node within `radius` that a player can move to in a
    /// straight line from `p` (so not one behind a wall).
    pub fn nearest_clear(&self, cm: &Geo, p: Vec3, radius: f32) -> Option<u32> {
        self.nearest_clear_candidates(cm, p, radius, &HashSet::default(), usize::MAX)
    }

    /// Best-effort local route start, skipping nodes in `skip`. Tests the
    /// closest twelve candidates before falling back to the nearest node;
    /// keep this recovery policy separate from exhaustive goal lookup.
    pub fn nearest_clear_except(&self, cm: &Geo, p: Vec3, radius: f32, skip: &HashSet<u32, FxBuild>) -> Option<u32> {
        self.nearest_clear_candidates(cm, p, radius, skip, 12)
    }

    pub(super) fn nearest_clear_candidates(&self, cm: &Geo, p: Vec3, radius: f32, skip: &HashSet<u32, FxBuild>, limit: usize) -> Option<u32> {
        let (cx, cy) = self.cell(p);
        let r = (radius / GRID).ceil() as i32;
        let mut cands: Vec<(f32, u32)> = Vec::new();
        for x in cx - r..=cx + r {
            for y in cy - r..=cy + r {
                for &n in self.cells.get(&(x, y)).into_iter().flatten() {
                    let d = self.pos[n as usize] - p;
                    let score = d.x * d.x + d.y * d.y + 4.0 * d.z * d.z;
                    if score <= radius * radius * 4.0 && !skip.contains(&n) {
                        cands.push((score, n));
                    }
                }
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let up = Vec3::new(0.0, 0.0, 1.0);
        // Goal lookup tries every candidate: the closest dozen can all be
        // across a wall while a usable approach lies farther away.
        cands.iter().take(limit).find(|&&(_, n)| {
            let q = self.pos[n as usize];
            let tr = cm.trace(1, p + up, Vec3::new(q.x, q.y, p.z.max(q.z) + 1.0));
            !tr.startsolid && !tr.allsolid && tr.fraction >= 1.0
        }).or(cands.first()).map(|c| c.1)
    }

    /// The closest node anywhere (linear scan).
    pub fn nearest_any(&self, p: Vec3) -> Option<u32> {
        (0..self.pos.len() as u32).min_by(|&a, &b| {
            let da = (self.pos[a as usize] - p).length_sq();
            let db = (self.pos[b as usize] - p).length_sq();
            da.total_cmp(&db)
        })
    }
}
