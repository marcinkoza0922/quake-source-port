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

    /// R_MarkLeaves: list the PVS leaves of `leaf` that have surfaces.
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
        let pvs = if novis {
            Vec::new()
        } else {
            qformats::bsp::decompress_vis(&self.visdata, self.leafs[leaf].visofs, self.numleafs)
        };
        for i in 0..self.numleafs.min(self.leafs.len().saturating_sub(1)) {
            if !novis && pvs.get(i >> 3).is_none_or(|b| b & (1 << (i & 7)) == 0) {
                continue;
            }
            let l = &self.leafs[i + 1];
            if l.num_marksurfaces > 0 {
                let (center, extent) = center_extent(l.mins, l.maxs);
                self.vis_leaves.push(VisLeaf { leaf: i as u32 + 1, center, extent });
            }
        }
    }
}
