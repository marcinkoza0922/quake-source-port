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
    /// Rebuilt only when the view leaf changes.
    pub(super) fn mark_leaves(&mut self, leaf: usize) {
        self.vis_leaf = leaf;
        self.vis_leaves.clear();
        let novis = self.transparent_liquids || leaf == 0
            || self.leafs.get(leaf).is_none_or(|l| l.visofs < 0)
            || self.visdata.is_empty();
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
