use super::*;
use std::sync::Arc;

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
        use qformats::bsp::{CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_WATER};
        if self.bsp_nodes.is_empty() {
            return;
        }
        let is_liquid_leaf = |leaf: usize| -> bool {
            self.leafs.get(leaf).is_some_and(|l| {
                matches!(l.contents, CONTENTS_WATER | CONTENTS_LAVA | CONTENTS_SLIME)
            })
        };
        // Mark every BSP node whose subtree contains at least one liquid leaf
        // so dry faces skip probing in a few plane tests.
        let mut node_has_liquid = vec![false; self.bsp_nodes.len()];
        let mut mark_stack: Vec<(usize, bool)> = Vec::with_capacity(64);
        mark_stack.push((0, false));
        while let Some((ni, visited)) = mark_stack.pop() {
            let Some(node) = self.bsp_nodes.get(ni) else { continue };
            if visited {
                let mut any = false;
                for c in node.children {
                    if c < 0 {
                        any |= is_liquid_leaf((-(c + 1)) as usize);
                    } else if let Some(&has) = node_has_liquid.get(c as usize) {
                        any |= has;
                    }
                }
                node_has_liquid[ni] = any;
            } else {
                mark_stack.push((ni, true));
                for c in node.children {
                    if c >= 0 && (c as usize) < self.bsp_nodes.len() {
                        mark_stack.push((c as usize, false));
                    }
                }
            }
        }
        if !node_has_liquid[0] {
            return;
        }
        let mut box_stack: Vec<i32> = Vec::with_capacity(64);
        for si in first..(first + count).min(self.surfaces.len()) {
            let s = &self.surfaces[si];
            if s.num_indices == 0 || !matches!(s.kind, TextureKind::Normal | TextureKind::Fence) {
                continue;
            }
            // All probes lie within 2 units of the face's AABB; skip faces
            // whose expanded AABB cannot intersect any liquid leaf.
            let bmin = s.mins.map(|c| c - 2.0);
            let bmax = s.maxs.map(|c| c + 2.0);
            box_stack.clear();
            box_stack.push(0);
            let mut touches_liquid = false;
            while let Some(n) = box_stack.pop() {
                if n < 0 {
                    if is_liquid_leaf((-(n + 1)) as usize) {
                        touches_liquid = true;
                        break;
                    }
                    continue;
                }
                let ni = n as usize;
                if !node_has_liquid.get(ni).copied().unwrap_or(false) {
                    continue;
                }
                let node = &self.bsp_nodes[ni];
                let pl = &self.planes[node.plane.max(0) as usize];
                let mut dmin = -pl.dist;
                let mut dmax = -pl.dist;
                for k in 0..3 {
                    let a = pl.normal[k] * bmin[k];
                    let b = pl.normal[k] * bmax[k];
                    if a < b {
                        dmin += a;
                        dmax += b;
                    } else {
                        dmin += b;
                        dmax += a;
                    }
                }
                if dmax >= 0.0 {
                    box_stack.push(node.children[0]);
                }
                if dmin < 0.0 {
                    box_stack.push(node.children[1]);
                }
            }
            if !touches_liquid {
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

    #[cfg(test)]
    pub fn leaf_bits(&mut self, eye: [f32; 3], frustum: &[[f32; 4]; 4], out: &mut Vec<u32>) {
        let leaf = self.view_leaf(eye);
        self.leaf_bits_for_leaf(leaf, frustum, out);
    }

    /// Per-frame leaf bitset for GPU culling: PVS leaves of an explicit leaf
    /// (with surfaces) whose box touches the frustum. `out` gets one bit per
    /// leaf index.
    pub fn leaf_bits_for_leaf(&mut self, leaf: usize, frustum: &[[f32; 4]; 4], out: &mut Vec<u32>) {
        self.mark_leaves(leaf);
        out.clear();
        out.resize(self.leafs.len().div_ceil(32), 0);
        let planes = Planes::new(frustum);
        for l in &self.vis_leaves {
            if planes.clip(0b1111, l.center, l.extent).is_some() {
                out[(l.leaf >> 5) as usize] |= 1 << (l.leaf & 31);
            }
        }
    }

    /// Lock the primary render PVS at the current camera leaf. Portal queries
    /// continue using their own leaves, and frustum tests still use the live camera.
    pub(crate) fn lock_pvs(&mut self, lock: bool, eye: Option<[f32; 3]>) {
        if !lock {
            self.locked_pvs = None;
        } else if self.locked_pvs.is_none() && let Some(eye) = eye {
            self.locked_pvs = Some(self.point_leaf(eye));
        }
    }

    pub(crate) fn view_leaf(&self, eye: [f32; 3]) -> usize {
        self.locked_pvs.unwrap_or_else(|| self.point_leaf(eye))
    }

    pub(crate) fn view_surfaces(&mut self, eye: [f32; 3], frustum: &[[f32; 4]; 4]) -> (Vec<u32>, u32) {
        let leaf = self.view_leaf(eye);
        self.mark_leaves(leaf);
        let planes = Planes::new(frustum);
        let leaves = self.vis_leaves.iter()
            .filter(|l| planes.clip(0b1111, l.center, l.extent).is_some())
            .count() as u32;
        let pvs = self.surfaces_for_leaf(leaf);
        let surfaces = pvs.iter().copied().filter(|&i| {
            let s = &self.surfaces[i as usize];
            s.num_indices != 0 && !self.hidden_surfaces.contains(&i)
                && (s.kind == TextureKind::Sky || ((dot(eye, s.normal) - s.dist < 0.0) == s.flipped))
                && super::box_in_frustum(frustum, s.mins, s.maxs)
        }).collect();
        (surfaces, leaves)
    }

    pub fn leafs_len(&self) -> usize {
        self.leafs.len()
    }
}

impl World {
    /// Surfaces in the PVS of `eye`'s leaf, each once. Cached per leaf, so
    /// static portal/window destinations and split views cost a lookup.
    pub(crate) fn portal_surfaces(&mut self, eye: [f32; 3]) -> Arc<[u32]> {
        self.surfaces_for_leaf(self.point_leaf(eye))
    }

    pub(crate) fn surfaces_for_leaf(&mut self, leaf: usize) -> Arc<[u32]> {
        const CACHE: usize = 16;
        if let Some(i) = self.portal_cache.iter().position(|(l, _)| *l == leaf) {
            let hit = self.portal_cache.remove(i);
            self.portal_cache.push(hit.clone());
            return hit.1;
        }
        self.mark_leaves(leaf);
        self.surf_stamp.resize(self.surfaces.len(), 0);
        self.stamp = self.stamp.wrapping_add(1);
        if self.stamp == 0 {
            self.surf_stamp.fill(0);
            self.stamp = 1;
        }
        let mut out = Vec::new();
        for vl in &self.vis_leaves {
            let l = &self.leafs[vl.leaf as usize];
            let first = l.first_marksurface as usize;
            for &si in &self.marksurfaces[first..first + l.num_marksurfaces as usize] {
                let s = &mut self.surf_stamp[si as usize];
                if *s != self.stamp {
                    *s = self.stamp;
                    out.push(si);
                }
            }
        }
        let out: Arc<[u32]> = out.into();
        if self.portal_cache.len() == CACHE {
            self.portal_cache.remove(0);
        }
        self.portal_cache.push((leaf, out.clone()));
        out
    }
}
