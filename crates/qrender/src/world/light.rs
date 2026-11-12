use super::*;

impl World {
    /// R_LightPoint: returns RGB light (0..255+ scale) at `p`, or None if nothing hit.
    pub fn light_point(&self, p: [f32; 3], styles: &[i32]) -> Option<[f32; 3]> {
        if !self.has_light || self.bsp_nodes.is_empty() {
            return Some([255.0; 3]);
        }
        let key = p.map(|v| v.floor() as i32);
        let cached = self.light_cache.lock().ok().and_then(|c| c.get(&key).copied());
        let hit = cached.unwrap_or_else(|| {
            let end = [p[0], p[1], p[2] - 8192.0];
            let mut maxdist = 8192.0f32;
            let h = self.recursive_light(0, p, p, end, &mut maxdist);
            if let Ok(mut c) = self.light_cache.lock() {
                // Moving entities add a new key each frame; keep it bounded.
                if c.len() >= 8192 {
                    c.clear();
                }
                c.insert(key, h);
            }
            h
        });
        match hit {
            LightHit::Miss => None,
            LightHit::Black => Some([0.0; 3]),
            LightHit::Surf { surf, ds, dt } => Some(self.interpolate_light(surf as usize, ds, dt, styles)),
        }
    }

    /// InterpolateLightmap: bilinear between the four nearest luxels.
    pub(super) fn interpolate_light(&self, si: usize, ds: i32, dt: i32, styles: &[i32]) -> [f32; 3] {
        let Some(s) = self.surfaces.get(si) else { return [0.0; 3] };
        let w = ((s.extents[0] >> 4) + 1) as usize;
        let h = ((s.extents[1] >> 4) + 1) as usize;
        let (x, y) = ((ds >> 4) as usize, (dt >> 4) as usize);
        let (x1, y1) = ((x + 1).min(w - 1), (y + 1).min(h - 1));
        let (fs, ft) = (ds & 15, dt & 15);
        let mut acc = [[0i32; 3]; 4];
        let mut base = s.samples as usize;
        for &st in s.styles.iter().take_while(|&&st| st != 255) {
            let scale = styles.get(st as usize).copied().unwrap_or(256);
            for (a, (cx, cy)) in acc.iter_mut().zip([(x, y), (x1, y), (x, y1), (x1, y1)]) {
                let v = self.samples.get(base + cy * w + cx).copied().unwrap_or(0);
                for (k, ak) in a.iter_mut().enumerate() {
                    *ak += ((v >> (k * 8)) & 0xff) as i32 * scale;
                }
            }
            base += w * h;
        }
        let [c00, c01, c10, c11] = acc;
        std::array::from_fn(|k| {
            let top = (((c01[k] - c00[k]) * fs) >> 4) + c00[k];
            let bot = (((c11[k] - c10[k]) * fs) >> 4) + c10[k];
            ((((bot - top) * ft) >> 4) + top) as f32 / 256.0
        })
    }

    pub(super) fn recursive_light(
        &self,
        node: i32,
        rayorg: [f32; 3],
        start: [f32; 3],
        end: [f32; 3],
        maxdist: &mut f32,
    ) -> LightHit {
        let mut node = node;
        let mut start = start;
        loop {
            if node < 0 {
                return LightHit::Miss;
            }
            let Some(n) = self.bsp_nodes.get(node as usize) else { return LightHit::Miss };
            let Some(pl) = self.planes.get(n.plane.max(0) as usize) else { return LightHit::Miss };
            let front = dot(start, pl.normal) - pl.dist;
            let back = dot(end, pl.normal) - pl.dist;
            let side = (front < 0.0) as usize;
            if (back < 0.0) == (front < 0.0) {
                node = n.children[side];
                continue;
            }
            let frac = front / (front - back);
            let mid = [0, 1, 2].map(|i| start[i] + (end[i] - start[i]) * frac);
            let r = self.recursive_light(n.children[side], rayorg, start, mid, maxdist);
            if r != LightHit::Miss {
                return r;
            }
            for si in n.first_face..n.first_face + n.num_faces {
                let Some(s) = self.surfaces.get(si as usize) else {
                    break;
                };
                if s.lm == NO_LM {
                    continue;
                }
                let ds = (dot64(mid, s.vecs[0]) + s.vecs[0][3] as f64) as i32;
                let dt = (dot64(mid, s.vecs[1]) + s.vecs[1][3] as f64) as i32;
                if ds < s.texturemins[0] || dt < s.texturemins[1] {
                    continue;
                }
                let (ds, dt) = (ds - s.texturemins[0], dt - s.texturemins[1]);
                if ds > s.extents[0] || dt > s.extents[1] {
                    continue;
                }
                let sfront = dot(rayorg, s.normal) - s.dist;
                let sback = dot(end, s.normal) - s.dist;
                let len = (0..3).map(|i| (end[i] - rayorg[i]).powi(2)).sum::<f32>().sqrt();
                let dist = sfront / (sfront - sback) * len;
                if s.samples == NO_LM {
                    // Lightmapped flag but no data: keep looking nearby (DOTM fix).
                    *maxdist = maxdist.min(dist + 8.0);
                    continue;
                }
                if dist >= *maxdist {
                    return LightHit::Black;
                }
                return LightHit::Surf { surf: si, ds, dt };
            }
            node = n.children[1 - side];
            start = mid;
        }
    }
}
