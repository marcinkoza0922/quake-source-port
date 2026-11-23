use super::*;

impl World {
    /// Opaque-water VIS can omit the entire other side of a liquid boundary.
    pub(crate) fn set_transparent_liquids(&mut self, enabled: bool) {
        if self.transparent_liquids != enabled {
            self.transparent_liquids = enabled;
            self.vis_leaf = usize::MAX;
            self.portal_cache.clear();
        }
    }

    /// True if any translucent liquid type exists in a leaf whose PVS omits
    /// non-matching contents, requiring `novis` to render across the surface.
    pub(crate) fn needs_transparent_novis(&self, alphas: [f32; 4]) -> bool {
        alphas
            .iter()
            .enumerate()
            .any(|(i, &a)| a < 1.0 && (self.water_vised & (1 << i)) == 0)
    }

    /// The map's VIS sees through liquid type `i` (0 water, 1 lava, 2 slime,
    /// 3 tele), or the map has no VIS at all: ironwail's
    /// `contentstransparent`, which decides whether `r_wateralpha` applies.
    pub(crate) fn liquid_seethrough(&self, i: usize) -> bool {
        self.visdata.is_empty() || self.water_vised & (1 << i) != 0
    }

    /// Port of ironwail's `Mod_CheckWaterVis`: detects which liquid types
    /// (bits 0..4 = water, lava, slime, tele) have VIS seeing across liquid
    /// boundaries. Absent liquid types are treated as safe.
    pub(super) fn check_water_vis(&self) -> u8 {
        use qformats::bsp::{CONTENTS_LAVA, CONTENTS_SLIME, CONTENTS_WATER};
        if self.visdata.is_empty() || self.numleafs == 0 {
            return 0;
        }
        let numclusters = self.numleafs.min(self.leafs.len().saturating_sub(1));
        let row = numclusters.div_ceil(8);
        let mut found = 0u8;
        let mut vised = 0u8;
        for li in 1..=numclusters {
            let leaf = &self.leafs[li];
            let mask = match leaf.contents {
                CONTENTS_WATER => {
                    let first = leaf.first_marksurface as usize;
                    let end = (first + leaf.num_marksurfaces as usize).min(self.marksurfaces.len());
                    let mut m = 0u8;
                    for &si in &self.marksurfaces[first..end] {
                        if let Some(s) = self.surfaces.get(si as usize)
                            && s.kind == TextureKind::Liquid
                            && let Some(name) = self.texture_names.get(s.texture as usize)
                        {
                            let ty = liquid_type(name);
                            if (1..=4).contains(&ty) {
                                m |= 1 << (ty - 1);
                            }
                        }
                    }
                    if m == 0 {
                        continue;
                    }
                    m
                }
                CONTENTS_LAVA => 1 << 1,
                CONTENTS_SLIME => 1 << 2,
                _ => continue,
            };
            found |= mask;
            if (vised & mask) == mask || leaf.visofs < 0 {
                continue;
            }
            let mut i = leaf.visofs as usize;
            let mut byte_idx = 0usize;
            let mut saw_other = false;
            'vis: while byte_idx < row {
                let Some(&b) = self.visdata.get(i) else { break };
                i += 1;
                if b == 0 {
                    let Some(&c) = self.visdata.get(i) else { break };
                    i += 1;
                    byte_idx += c as usize;
                    continue;
                }
                for k in 0..8usize {
                    if b & (1 << k) != 0 {
                        let other = (byte_idx << 3) + k + 1;
                        if other <= numclusters && self.leafs[other].contents != leaf.contents {
                            saw_other = true;
                            break 'vis;
                        }
                    }
                }
                byte_idx += 1;
            }
            if saw_other {
                vised |= mask;
            }
        }
        vised | (!found & 0b1111)
    }

    /// Texture a surface is batched under: its own texture, or for random
    /// animations (`-0..-9`) the frame picked for that surface.
    pub fn slot_texture(&self, si: u32) -> u32 {
        let t = self.surfaces[si as usize].texture;
        match self.anims.get(t as usize) {
            Some((_, _, true)) => self.resolve_texture(t, false, 0.0, si),
            _ => t,
        }
    }

    /// Static inputs of cull.comp: (leaf, surface) pairs for every world leaf
    /// and one record per surface. `cmd_of(si)` is the draw command index a
    /// surface's triangles go to.
    pub fn gpu_cull(&self, cmd_of: impl Fn(u32) -> u32) -> (Vec<[u32; 2]>, Vec<GpuCullSurf>) {
        let surfs = self
            .surfaces
            .iter()
            .enumerate()
            .map(|(si, s)| {
                let (center, extent) = center_extent(s.mins, s.maxs);
                let plane = if s.kind == TextureKind::Sky {
                    [0.0, 0.0, 0.0, -1.0] // never back-face culled
                } else if s.flipped {
                    [-s.normal[0], -s.normal[1], -s.normal[2], -s.dist]
                } else {
                    [s.normal[0], s.normal[1], s.normal[2], s.dist]
                };
                let cmd = if s.num_indices > 0 { cmd_of(si as u32) } else { 0 };
                GpuCullSurf {
                    plane,
                    center: [center[0], center[1], center[2], f32::from_bits(cmd)],
                    extent: [extent[0], extent[1], extent[2], f32::from_bits(s.lm)],
                    idx: [s.first_index, s.num_indices, 0, 0],
                }
            })
            .collect();
        let mut marks = Vec::new();
        for li in 1..=self.numleafs.min(self.leafs.len().saturating_sub(1)) {
            let l = &self.leafs[li];
            let ms = l.first_marksurface as usize;
            for k in ms..ms + l.num_marksurfaces as usize {
                let Some(&si) = self.marksurfaces.get(k) else { break };
                if self.surfaces.get(si as usize).is_some_and(|s| s.num_indices > 0) {
                    marks.push([li as u32, si]);
                }
            }
        }
        (marks, surfs)
    }

    /// R_MarkLeaves: list the PVS leaves of `leaf` that have surfaces, build
    /// 16-leaf frustum culling blocks, and populate the flat PVS leaf bitset
    /// for static entity culling.
    /// All-visible queries share key 0, since their candidate list does not
    /// depend on the camera leaf. Frustum clipping still runs for each view.
    pub(super) fn mark_leaves(&mut self, leaf: usize) {
        let novis = self.transparent_liquids || leaf == 0
            || self.leafs.get(leaf).is_none_or(|l| l.visofs < 0)
            || self.visdata.is_empty();
        let key = if novis { 0 } else { leaf };
        if key == self.vis_leaf {
            return;
        }
        self.vis_leaf = key;
        self.vis_leaves.clear();
        self.vis_blocks.clear();
        self.vis_pvs_bits.clear();
        let pvs = if novis {
            Vec::new()
        } else {
            self.vis_pvs_bits.resize(self.leafs.len().div_ceil(32), 0);
            qformats::bsp::decompress_vis(&self.visdata, self.leafs[leaf].visofs, self.numleafs)
        };
        let nclusters = self.numleafs.min(self.leafs.len().saturating_sub(1));
        for i in 0..nclusters {
            if !novis {
                if pvs.get(i >> 3).is_none_or(|b| b & (1 << (i & 7)) == 0) {
                    continue;
                }
                let li = i + 1;
                self.vis_pvs_bits[li >> 5] |= 1 << (li & 31);
            }
            let l = &self.leafs[i + 1];
            if l.num_marksurfaces > 0 {
                let (center, extent) = center_extent(l.mins, l.maxs);
                self.vis_leaves.push(VisLeaf { leaf: i as u32 + 1, center, extent });
            }
        }
        const BLOCK: usize = 16;
        let mut start = 0usize;
        while start < self.vis_leaves.len() {
            let end = (start + BLOCK).min(self.vis_leaves.len());
            let mut mins = [f32::INFINITY; 3];
            let mut maxs = [f32::NEG_INFINITY; 3];
            for l in &self.vis_leaves[start..end] {
                for k in 0..3 {
                    mins[k] = mins[k].min(l.center[k] - l.extent[k]);
                    maxs[k] = maxs[k].max(l.center[k] + l.extent[k]);
                }
            }
            let (center, extent) = center_extent(mins, maxs);
            self.vis_blocks.push(VisBlock {
                center,
                extent,
                start: start as u32,
                count: (end - start) as u32,
            });
            start = end;
        }
    }

    /// R_SplitEntityOnNode (ironwail `gl_refrag.c`): appends the 1-based BSP
    /// leaf indices (excluding `CONTENTS_SOLID`) touched by `[mins, maxs]`.
    pub fn box_leaves(&self, mins: [f32; 3], maxs: [f32; 3], out: &mut Vec<u32>) {
        if self.bsp_nodes.is_empty() {
            return;
        }
        let mut stack = [0i32; 128];
        let mut sp = 1usize;
        while sp > 0 {
            sp -= 1;
            let n = stack[sp];
            if n < 0 {
                let idx = (-(n + 1)) as usize;
                if idx >= 1
                    && self
                        .leafs
                        .get(idx)
                        .is_some_and(|l| l.contents != qformats::bsp::CONTENTS_SOLID)
                {
                    out.push(idx as u32);
                }
                continue;
            }
            let Some(node) = self.bsp_nodes.get(n as usize) else { continue };
            let Some(pl) = self.planes.get(node.plane.max(0) as usize) else { continue };
            let mut dmin = -pl.dist;
            let mut dmax = -pl.dist;
            for k in 0..3 {
                let a = pl.normal[k] * mins[k];
                let b = pl.normal[k] * maxs[k];
                if a < b {
                    dmin += a;
                    dmax += b;
                } else {
                    dmin += b;
                    dmax += a;
                }
            }
            if dmax >= 0.0 && sp < stack.len() {
                stack[sp] = node.children[0];
                sp += 1;
            }
            if dmin < 0.0 && sp < stack.len() {
                stack[sp] = node.children[1];
                sp += 1;
            }
        }
    }

    /// Flat PVS leaf bitset (1 bit per 1-based leaf index) for `eye`'s view
    /// leaf, or `None` when PVS is disabled / unavailable (`novis`).
    pub fn pvs_bits(&mut self, eye: [f32; 3]) -> Option<&[u32]> {
        let leaf = self.view_leaf(eye);
        self.mark_leaves(leaf);
        (self.vis_leaf != 0).then_some(self.vis_pvs_bits.as_slice())
    }
}
