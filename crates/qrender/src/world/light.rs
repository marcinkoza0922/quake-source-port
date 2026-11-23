use super::*;

impl World {
    /// R_LightPoint: returns RGB light (0..255+ scale) at `p`, or None if nothing hit.
    /// Uses BSPX `LIGHTGRID_OCTREE` when present (falling back to the classic
    /// downward BSP trace if `p` is outside the grid or fully occluded).
    pub fn light_point(&self, p: [f32; 3], styles: &[i32]) -> Option<[f32; 3]> {
        self.light_point_dir(p, styles).map(|(c, _)| c)
    }

    /// R_LightPoint plus optional world-space baked light direction (from `.dlit` / `LIGHTINGDIR`).
    pub fn light_point_dir(&self, p: [f32; 3], styles: &[i32]) -> Option<([f32; 3], Option<[f32; 3]>)> {
        if let Some(grid) = &self.lightgrid {
            let mut sh = 0xcbf2_9ce4_8422_2325u64;
            for &s in styles.iter().take(64) {
                sh = (sh ^ s as u64).wrapping_mul(0x0100_0000_01b3);
            }
            let key = p.map(|v| v.to_bits() as i32);
            let cached = self.lightgrid_cache.borrow().get(&key).copied();
            let rgb = match cached {
                Some((h, rgb)) if h == sh => rgb,
                _ => {
                    let rgb = grid.sample(p, styles);
                    let mut c = self.lightgrid_cache.borrow_mut();
                    if c.len() >= 8192 {
                        c.clear();
                    }
                    c.insert(key, (sh, rgb));
                    rgb
                }
            };
            if let Some(rgb) = rgb {
                return Some((rgb, None));
            }
        }
        if !self.has_light || self.bsp_nodes.is_empty() {
            return Some(([255.0; 3], None));
        }
        let key = p.map(|v| v.floor() as i32);
        let style_vals_for = |si: usize| -> [i32; 4] {
            let Some(s) = self.surfaces.get(si) else { return [0; 4] };
            let mut vals = [0i32; 4];
            for (dst, &st) in vals.iter_mut().zip(&s.styles) {
                if st == 255 {
                    break;
                }
                *dst = styles.get(st as usize).copied().unwrap_or(256);
            }
            vals
        };
        let cached = self.light_cache.borrow().get(&key).copied();
        if let Some(entry) = cached {
            return match entry.hit {
                LightHit::Miss => None,
                LightHit::Black => Some(([0.0; 3], None)),
                LightHit::Surf { surf, ds, dt } => {
                    let cur = style_vals_for(surf as usize);
                    if cur == entry.style_vals {
                        return Some((entry.rgb, entry.dir));
                    }
                    let (rgb, dir) = self.interpolate_light_dir(surf as usize, ds, dt, styles);
                    if let Some(e) = self.light_cache.borrow_mut().get_mut(&key) {
                        e.style_vals = cur;
                        e.rgb = rgb;
                        e.dir = dir;
                    }
                    Some((rgb, dir))
                }
            };
        }
        let end = [p[0], p[1], p[2] - 8192.0];
        let mut maxdist = 8192.0f32;
        let hit = self.recursive_light(0, p, p, end, &mut maxdist);
        let (style_vals, rgb, dir, result) = match hit {
            LightHit::Miss => ([0; 4], [0.0; 3], None, None),
            LightHit::Black => ([0; 4], [0.0; 3], None, Some(([0.0; 3], None))),
            LightHit::Surf { surf, ds, dt } => {
                let sv = style_vals_for(surf as usize);
                let (rgb, dir) = self.interpolate_light_dir(surf as usize, ds, dt, styles);
                (sv, rgb, dir, Some((rgb, dir)))
            }
        };
        let mut c = self.light_cache.borrow_mut();
        // Moving entities add a new key each frame; keep it bounded.
        if c.len() >= 8192 {
            c.clear();
        }
        c.insert(key, LightCacheEntry { hit, style_vals, rgb, dir });
        result
    }

    /// InterpolateLightmap: bilinear between the four nearest luxels (supports RGB8,
    /// E5BGR9 HDR lightmaps, and world-space deluxemap direction vectors).
    pub(super) fn interpolate_light_dir(
        &self,
        si: usize,
        ds: i32,
        dt: i32,
        styles: &[i32],
    ) -> ([f32; 3], Option<[f32; 3]>) {
        let Some(s) = self.surfaces.get(si) else { return ([0.0; 3], None) };
        let (w, h) = (s.lm_size[0] as usize, s.lm_size[1] as usize);
        if w == 0 || h == 0 {
            return ([0.0; 3], None);
        }
        let x = ((ds >> 4).max(0) as usize).min(w - 1);
        let y = ((dt >> 4).max(0) as usize).min(h - 1);
        let (x1, y1) = ((x + 1).min(w - 1), (y + 1).min(h - 1));
        let (fs, ft) = (ds & 15, dt & 15);
        let corners = [(x, y), (x1, y), (x, y1), (x1, y1)];

        let color = if s.hdr_samples {
            let mut acc = [[0.0f32; 3]; 4];
            let mut base = s.samples as usize;
            for &st in s.styles.iter().take_while(|&&st| st != 255) {
                let scale = styles.get(st as usize).copied().unwrap_or(256) as f32 / 256.0;
                for (a, (cx, cy)) in acc.iter_mut().zip(corners) {
                    let v = self.samples.get(base + cy * w + cx).copied().unwrap_or(0);
                    let rgb = qformats::bsp::decode_e5bgr9(v);
                    for (ak, rk) in a.iter_mut().zip(rgb) {
                        *ak += rk * 128.0 * scale;
                    }
                }
                base += w * h;
            }
            let (u, v) = (fs as f32 / 16.0, ft as f32 / 16.0);
            let [c00, c01, c10, c11] = acc;
            std::array::from_fn(|k| {
                let top = c00[k] + (c01[k] - c00[k]) * u;
                let bot = c10[k] + (c11[k] - c10[k]) * u;
                top + (bot - top) * v
            })
        } else {
            let mut acc = [[0i32; 3]; 4];
            let mut base = s.samples as usize;
            for &st in s.styles.iter().take_while(|&&st| st != 255) {
                let scale = styles.get(st as usize).copied().unwrap_or(256);
                for (a, (cx, cy)) in acc.iter_mut().zip(corners) {
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
        };

        let dir = if s.has_deluxe {
            let mut acc = [[0.0f32; 3]; 4];
            let mut base = s.samples as usize;
            for &st in s.styles.iter().take_while(|&&st| st != 255) {
                let scale = (styles.get(st as usize).copied().unwrap_or(256) as f32 / 256.0).max(0.0);
                if scale > 0.0 {
                    for (a, (cx, cy)) in acc.iter_mut().zip(corners) {
                        let d = self.deluxe_samples.get(base + cy * w + cx).copied().unwrap_or(0x0080_8080);
                        for (k, ak) in a.iter_mut().enumerate() {
                            let c = ((d >> (k * 8)) & 0xff) as f32 / 255.0 * 2.0 - 1.0;
                            *ak += c * scale;
                        }
                    }
                }
                base += w * h;
            }
            let (u, v) = (fs as f32 / 16.0, ft as f32 / 16.0);
            let [d00, d01, d10, d11] = acc;
            let raw: [f32; 3] = std::array::from_fn(|k| {
                let top = d00[k] + (d01[k] - d00[k]) * u;
                let bot = d10[k] + (d11[k] - d10[k]) * u;
                top + (bot - top) * v
            });
            let len2 = dot(raw, raw);
            (len2 > 1e-4).then(|| {
                let inv = 1.0 / len2.sqrt();
                raw.map(|c| c * inv)
            })
        } else {
            None
        };

        (color, dir)
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
                if s.lm == NO_LM || s.lm_size[0] == 0 || s.lm_size[1] == 0 {
                    continue;
                }
                let ls = dot64(mid, s.lm_vecs[0]) + s.lm_vecs[0][3] as f64;
                let lt = dot64(mid, s.lm_vecs[1]) + s.lm_vecs[1][3] as f64;
                if ls < 0.0
                    || lt < 0.0
                    || ls > (s.lm_size[0] - 1) as f64
                    || lt > (s.lm_size[1] - 1) as f64
                {
                    continue;
                }
                let (ds, dt) = ((ls * 16.0) as i32, (lt * 16.0) as i32);
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
