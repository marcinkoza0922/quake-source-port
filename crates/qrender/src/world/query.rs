use super::*;

impl World {
    /// R_TextureAnimation (+ random `-` tiling textures picked per surface).
    pub fn resolve_texture(&self, tex: u32, alt: bool, time: f64, surf: u32) -> u32 {
        let Some((prim, alts, random)) = self.anims.get(tex as usize) else {
            return tex;
        };
        let seq = if alt && !alts.is_empty() { alts } else { prim };
        if seq.is_empty() {
            return tex;
        }
        let i = if *random {
            (surf.wrapping_mul(2654435761) >> 16) as usize % seq.len()
        } else {
            ((time * 5.0) as i64).rem_euclid(seq.len() as i64) as usize
        };
        seq[i]
    }

    /// Leaf contents at a point (world model hull 0).
    pub fn point_contents(&self, p: [f32; 3]) -> i32 {
        self.leafs.get(self.point_leaf(p)).map_or(qformats::bsp::CONTENTS_SOLID, |l| l.contents)
    }

    /// Liquid at `p`: (surface height above it, kind 1 water/2 lava/3 slime,
    /// +4 if something solid caps the column instead of open air).
    pub fn liquid_surface(&self, p: [f32; 3]) -> Option<(f32, u32)> {
        use qformats::bsp::{CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_SOLID, CONTENTS_WATER};
        let c = self.point_contents(p);
        let kind = match c {
            CONTENTS_WATER => 1,
            CONTENTS_LAVA => 2,
            CONTENTS_SLIME => 3,
            _ => return None,
        };
        let at = |z: f32| self.point_contents([p[0], p[1], z]);
        let (mut lo, mut hi) = (p[2], p[2]);
        loop {
            hi += 16.0;
            if hi - p[2] > 4096.0 {
                return Some((hi, kind));
            }
            if at(hi) != c {
                break;
            }
            lo = hi;
        }
        for _ in 0..6 {
            let mid = 0.5 * (lo + hi);
            if at(mid) == c { lo = mid } else { hi = mid }
        }
        let capped = at(hi + 1.0) == CONTENTS_SOLID;
        Some((0.5 * (lo + hi), kind + if capped { 4 } else { 0 }))
    }

    /// Stores the liquid surface above each submerged world face in its
    /// vertices (caustics). Probes just off the face at its corners and centre.
    pub(super) fn bake_liquid(&mut self, first: usize, count: usize) {
        for si in first..(first + count).min(self.surfaces.len()) {
            let s = &self.surfaces[si];
            if s.num_indices == 0 || !matches!(s.kind, TextureKind::Normal | TextureKind::Fence) {
                continue;
            }
            let base = self.indices[s.first_index as usize] as usize;
            let n = s.num_indices as usize / 3 + 2;
            let nrm = if s.flipped { s.normal.map(|c| -c) } else { s.normal };
            let mut cen = [0.0f32; 3];
            for v in &self.vertices[base..base + n] {
                for (c, p) in cen.iter_mut().zip(v.pos) {
                    *c += p / n as f32;
                }
            }
            let mut best: Option<(f32, u32)> = None;
            let probes = self.vertices[base..base + n].iter().map(|v| v.pos).chain(std::iter::once(cen));
            for v in probes {
                let d = [cen[0] - v[0], cen[1] - v[1], cen[2] - v[2]];
                let l = dot(d, d).sqrt();
                let inward = if l > 1.0 { 1.0 / l } else { 0.0 };
                let p = [0, 1, 2].map(|k| v[k] + d[k] * inward + nrm[k]);
                if let Some(h) = self.liquid_surface(p)
                    && best.is_none_or(|b| h.0 > b.0)
                {
                    best = Some(h);
                }
            }
            if let Some((z, k)) = best {
                for v in &mut self.vertices[base..base + n] {
                    v.liquid = [z, k as f32];
                }
            }
        }
    }

    pub fn point_leaf(&self, p: [f32; 3]) -> usize {
        let mut n = 0i32;
        let Some(head) = self.bsp_nodes.first() else {
            return 0;
        };
        let _ = head;
        loop {
            if n < 0 {
                return (-(n + 1)) as usize;
            }
            let Some(node) = self.bsp_nodes.get(n as usize) else {
                return 0;
            };
            let pl = &self.planes[node.plane.max(0) as usize];
            let d = dot(p, pl.normal) - pl.dist;
            n = node.children[if d >= 0.0 { 0 } else { 1 }];
        }
    }

    /// Per-frame leaf bitset for GPU culling: PVS leaves of the eye's leaf
    /// (with surfaces) whose box touches the frustum. `out` gets one bit per
    /// leaf index.
    pub fn leaf_bits(&mut self, eye: [f32; 3], frustum: &[[f32; 4]; 4], out: &mut Vec<u32>) {
        let leaf = self.point_leaf(eye);
        if leaf != self.vis_leaf {
            self.mark_leaves(leaf);
        }
        out.clear();
        out.resize(self.leafs.len().div_ceil(32), 0);
        let planes = Planes::new(frustum);
        for l in &self.vis_leaves {
            if planes.clip(0b1111, l.center, l.extent).is_some() {
                out[(l.leaf >> 5) as usize] |= 1 << (l.leaf & 31);
            }
        }
    }

    pub fn leafs_len(&self) -> usize {
        self.leafs.len()
    }
}
