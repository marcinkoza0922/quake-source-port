//! CPU-side world preparation: surfaces, lightmap atlas packing, texture
//! animation tables, PVS/frustum marking and light sampling.

use qformats::bsp::{Bsp, TEX_SPECIAL, TextureKind};

pub const ATLAS: u32 = 2048;
pub const NO_LM: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct WorldVertex {
    pub pos: [f32; 3],
    pub st: [f32; 2],
    pub lm: [f32; 3],
}

/// GPU layout of `Surf` in lightmap.comp (std430, 96 bytes).
#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct LmSurf {
    pub sample_ofs: u32,
    pub styles: u32,
    pub size: u32,
    pub atlas: u32,
    pub vecs0: [f32; 4],
    pub vecs1: [f32; 4],
    pub plane: [f32; 4],
    pub tmins: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct Surface {
    pub first_index: u32,
    pub num_indices: u32,
    pub texture: u32,
    pub kind: TextureKind,
    pub normal: [f32; 3],
    pub dist: f32,
    pub flipped: bool,
    pub lm: u32,
    pub vecs: [[f32; 4]; 2],
    pub texturemins: [i32; 2],
    pub extents: [i32; 2],
    /// Offset into `samples` (CPU light sampling) or NO_LM.
    pub samples: u32,
    pub styles: [u8; 4],
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

#[derive(Clone, Debug)]
pub struct SubModel {
    pub first_surface: u32,
    pub num_surfaces: u32,
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
}

pub struct World {
    /// R_LightPoint cache (ironwail's per-entity lightcache_t): sample point
    /// floored to whole units -> trace result. Entities that don't move skip
    /// the BSP trace; the colour is re-read from the lightmap each call so
    /// lightstyles still animate.
    light_cache: std::sync::Mutex<std::collections::HashMap<[i32; 3], LightHit, PointHash>>,
    pub vertices: Vec<WorldVertex>,
    pub indices: Vec<u32>,
    pub surfaces: Vec<Surface>,
    pub lm_surfs: Vec<LmSurf>,
    pub samples: Vec<u32>,
    pub atlas_layers: u32,
    /// Coloured (.lit) lighting was loaded for the world model.
    pub lit: bool,
    /// Per texture: primary and alternate animation sequences (texture indices).
    pub anims: Vec<(Vec<u32>, Vec<u32>, bool)>,
    pub submodels: Vec<SubModel>,
    bsp_nodes: Vec<qformats::bsp::Node>,
    planes: Vec<qformats::bsp::Plane>,
    leafs: Vec<qformats::bsp::Leaf>,
    marksurfaces: Vec<u32>,
    visdata: Vec<u8>,
    numleafs: usize,
    /// R_MarkLeaves state: the PVS leaves of `vis_leaf` that have surfaces.
    vis_leaves: Vec<VisLeaf>,
    vis_leaf: usize,
    pub has_light: bool,
    packer: Packer,
}

struct Packer {
    allocated: Vec<u32>,
    layer: u32,
}

impl Packer {
    /// AllocBlock from gl_rsurf.c on a skyline of column heights.
    fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32, u32)> {
        if w > ATLAS || h > ATLAS {
            return None;
        }
        loop {
            let mut best = ATLAS;
            let mut bx = 0;
            let mut i = 0;
            while i + w <= ATLAS {
                let mut best2 = 0;
                let mut j = 0;
                while j < w {
                    let a = self.allocated[(i + j) as usize];
                    if a >= best {
                        break;
                    }
                    best2 = best2.max(a);
                    j += 1;
                }
                if j == w {
                    bx = i;
                    best = best2;
                }
                i += 1;
            }
            if best + h <= ATLAS {
                for k in 0..w {
                    self.allocated[(bx + k) as usize] = best + h;
                }
                return Some((bx, best, self.layer));
            }
            self.layer += 1;
            self.allocated.iter_mut().for_each(|a| *a = 0);
        }
    }
}

fn liquid_type(name: &str) -> u32 {
    let n = name.to_ascii_lowercase();
    if n.starts_with("*lava") {
        2
    } else if n.starts_with("*slime") {
        3
    } else if n.starts_with("*tele") {
        4
    } else {
        1
    }
}

/// Material flags (low 4 bits liquid type, 0x10 fence, 0x20 no lightmap).
pub fn material_flags(name: &str, kind: TextureKind) -> u32 {
    match kind {
        TextureKind::Liquid => liquid_type(name),
        TextureKind::Fence => 0x10,
        _ => 0,
    }
}

impl World {
    pub fn build(bsp: &Bsp, lit: Option<&[u8]>) -> World {
        let numleafs = bsp.models.first().map_or(0, |m| m.visleafs.max(0) as usize);
        let has_light = !bsp.lighting.is_empty();
        let mut w = World {
            light_cache: Default::default(),
            vertices: Vec::new(),
            indices: Vec::new(),
            surfaces: Vec::with_capacity(bsp.faces.len()),
            lm_surfs: Vec::new(),
            samples: Vec::new(),
            atlas_layers: 1,
            lit: lit.is_some_and(|l| !l.is_empty() && l.len() == bsp.lighting.len() * 3),
            anims: Vec::new(),
            submodels: Vec::new(),
            bsp_nodes: bsp.nodes.clone(),
            planes: bsp.planes.clone(),
            leafs: bsp.leafs.clone(),
            marksurfaces: bsp.marksurfaces.clone(),
            visdata: bsp.visdata.clone(),
            numleafs,
            vis_leaves: Vec::new(),
            vis_leaf: usize::MAX,
            has_light,
            packer: Packer {
                allocated: vec![0; ATLAS as usize],
                layer: 0,
            },
        };
        w.append(bsp, lit, 0);
        w
    }

    /// Appends an external brush model (`maps/b_*.bsp`) whose textures start
    /// at `tex_base` in the material table. Its faces share the world's
    /// vertex buffer and lightmap atlas. Returns the index of its model 0 in
    /// `submodels`.
    pub fn add_brush(&mut self, bsp: &Bsp, lit: Option<&[u8]>, tex_base: u32) -> u32 {
        let first = self.submodels.len() as u32;
        self.append(bsp, lit, tex_base);
        first
    }

    fn append(&mut self, bsp: &Bsp, lit: Option<&[u8]>, tex_base: u32) {
        let w = self;
        let surf_base = w.surfaces.len() as u32;
        let has_light = !bsp.lighting.is_empty();
        let packer = &mut w.packer;
        let lit = lit.filter(|l| l.len() == bsp.lighting.len() * 3);
        let ntex = bsp.textures.len().max(1);
        for f in &bsp.faces {
            let mut s = Surface {
                first_index: w.indices.len() as u32,
                num_indices: 0,
                texture: 0,
                kind: TextureKind::Normal,
                normal: [0.0, 0.0, 1.0],
                dist: 0.0,
                flipped: f.side != 0,
                lm: NO_LM,
                vecs: [[0.0; 4]; 2],
                texturemins: [0; 2],
                extents: [0; 2],
                samples: NO_LM,
                styles: f.styles,
                mins: [f32::MAX; 3],
                maxs: [f32::MIN; 3],
            };
            let (Some(ti), Some(pl)) = (
                bsp.texinfo.get(f.texinfo as usize),
                bsp.planes.get(f.plane as usize),
            ) else {
                w.surfaces.push(s);
                continue;
            };
            s.normal = pl.normal;
            s.dist = pl.dist;
            s.vecs = ti.vecs;
            let ltex = (ti.miptex.max(0) as usize).min(ntex - 1);
            s.texture = ltex as u32 + tex_base;
            let tex = bsp.textures.get(ltex).and_then(|t| t.as_ref());
            s.kind = tex.map_or(TextureKind::Normal, |t| t.kind);
            let mut verts = Vec::with_capacity(f.num_edges as usize);
            for e in 0..f.num_edges {
                let Some(&se) = bsp.surfedges.get((f.first_edge + e) as usize) else {
                    break;
                };
                let edge = bsp
                    .edges
                    .get(se.unsigned_abs() as usize)
                    .copied()
                    .unwrap_or([0, 0]);
                let vi = if se >= 0 { edge[0] } else { edge[1] };
                verts.push(bsp.vertexes.get(vi as usize).copied().unwrap_or([0.0; 3]));
            }
            if verts.len() < 3 {
                w.surfaces.push(s);
                continue;
            }
            // CalcSurfaceExtents (double precision as in ironwail)
            let mut mins = [f64::MAX; 2];
            let mut maxs = [f64::MIN; 2];
            for v in &verts {
                for j in 0..2 {
                    let t = &ti.vecs[j];
                    let val = v[0] as f64 * t[0] as f64
                        + v[1] as f64 * t[1] as f64
                        + v[2] as f64 * t[2] as f64
                        + t[3] as f64;
                    mins[j] = mins[j].min(val);
                    maxs[j] = maxs[j].max(val);
                }
                for (k, &c) in v.iter().enumerate() {
                    s.mins[k] = s.mins[k].min(c);
                    s.maxs[k] = s.maxs[k].max(c);
                }
            }
            let mut lmsize = [0u32; 2];
            for j in 0..2 {
                let bmin = (mins[j] / 16.0).floor() as i32;
                let bmax = (maxs[j] / 16.0).ceil() as i32;
                s.texturemins[j] = bmin * 16;
                s.extents[j] = (bmax - bmin) * 16;
                lmsize[j] = ((bmax - bmin) + 1).max(1) as u32;
            }
            let drawtiled = matches!(s.kind, TextureKind::Sky | TextureKind::Liquid)
                || ti.flags & TEX_SPECIAL != 0;
            let mut atlas_pos = (0u32, 0u32, 0u32);
            if !drawtiled && let Some(p) = packer.alloc(lmsize[0], lmsize[1]) {
                atlas_pos = p;
                let nstyles = f.styles.iter().take_while(|&&st| st != 255).count();
                let n = (lmsize[0] * lmsize[1]) as usize;
                let mut sample_ofs = NO_LM;
                if has_light && f.lightofs >= 0 && nstyles > 0 {
                    let base = f.lightofs as usize;
                    let ok = base + n * nstyles <= bsp.lighting.len();
                    if ok {
                        sample_ofs = w.samples.len() as u32;
                        for i in 0..n * nstyles {
                            let (r, g, b) = match lit {
                                Some(l) => (
                                    l[(base + i) * 3],
                                    l[(base + i) * 3 + 1],
                                    l[(base + i) * 3 + 2],
                                ),
                                None => {
                                    let v = bsp.lighting[base + i];
                                    (v, v, v)
                                }
                            };
                            w.samples
                                .push(r as u32 | (g as u32) << 8 | (b as u32) << 16);
                        }
                    }
                }
                s.samples = sample_ofs;
                s.lm = w.lm_surfs.len() as u32;
                w.lm_surfs.push(LmSurf {
                    sample_ofs,
                    styles: u32::from_le_bytes(f.styles),
                    size: lmsize[0] | lmsize[1] << 16,
                    atlas: p.0 | p.1 << 16,
                    vecs0: ti.vecs[0],
                    vecs1: ti.vecs[1],
                    plane: [pl.normal[0], pl.normal[1], pl.normal[2], pl.dist],
                    tmins: [
                        s.texturemins[0] as f32,
                        s.texturemins[1] as f32,
                        p.2 as f32,
                        0.0,
                    ],
                });
            }
            let base = w.vertices.len() as u32;
            // Quake64 re-release texture shift (not for sky/liquids).
            let tex_div = match bsp.textures.get(ti.miptex.max(0) as usize) {
                Some(Some(t)) if t.shift > 0 && matches!(t.kind, TextureKind::Normal | TextureKind::Fence) => {
                    2.0 * t.shift as f32
                }
                _ => 1.0,
            };
            for v in &verts {
                let st = [0, 1].map(|j| {
                    let t = &ti.vecs[j];
                    v[0] * t[0] + v[1] * t[1] + v[2] * t[2] + t[3]
                });
                let lm = if s.lm != NO_LM {
                    [
                        ((st[0] - s.texturemins[0] as f32 + 8.0) / 16.0 + atlas_pos.0 as f32)
                            / ATLAS as f32,
                        ((st[1] - s.texturemins[1] as f32 + 8.0) / 16.0 + atlas_pos.1 as f32)
                            / ATLAS as f32,
                        atlas_pos.2 as f32,
                    ]
                } else {
                    [0.0; 3]
                };
                let st = st.map(|c| c / tex_div);
                w.vertices.push(WorldVertex { pos: *v, st, lm });
            }
            for i in 1..verts.len() as u32 - 1 {
                w.indices.extend_from_slice(&[base, base + i, base + i + 1]);
            }
            s.num_indices = (verts.len() as u32 - 2) * 3;
            w.surfaces.push(s);
        }
        w.atlas_layers = packer.layer + 1;
        let anims = build_anims(bsp);
        let tex_count = bsp.textures.len().max(1);
        w.anims.resize(tex_base as usize, (Vec::new(), Vec::new(), false));
        for i in 0..tex_count {
            let (p, a, r) = anims.get(i).cloned().unwrap_or_default();
            let off = |v: Vec<u32>| v.into_iter().map(|x| x + tex_base).collect();
            w.anims.push((off(p), off(a), r));
        }
        for m in &bsp.models {
            w.submodels.push(SubModel {
                first_surface: m.first_face.max(0) as u32 + surf_base,
                num_surfaces: m.num_faces.max(0) as u32,
                mins: m.mins,
                maxs: m.maxs,
            });
        }
    }

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
    fn mark_leaves(&mut self, leaf: usize) {
        self.vis_leaf = leaf;
        self.vis_leaves.clear();
        let novis = leaf == 0
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
    fn interpolate_light(&self, si: usize, ds: i32, dt: i32, styles: &[i32]) -> [f32; 3] {
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

    fn recursive_light(
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

/// cull.comp surface record.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct GpuCullSurf {
    pub plane: [f32; 4],
    pub center: [f32; 4],
    pub extent: [f32; 4],
    pub idx: [u32; 4],
}

/// A PVS leaf with surfaces, copied out so the per-frame loop is linear.
#[derive(Clone, Copy, Debug)]
struct VisLeaf {
    leaf: u32,
    center: [f32; 3],
    extent: [f32; 3],
}

/// Multiplicative hash for integer points (the default SipHash showed up in
/// profiles; these keys need no DoS resistance).
#[derive(Default, Clone, Copy)]
pub struct PointHash;

impl std::hash::BuildHasher for PointHash {
    type Hasher = PointHasher;
    fn build_hasher(&self) -> PointHasher {
        PointHasher(0)
    }
}

pub struct PointHasher(u64);

impl std::hash::Hasher for PointHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u32(b as u32);
        }
    }
    fn write_u32(&mut self, v: u32) {
        self.0 = (self.0.rotate_left(5) ^ v as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_i32(&mut self, v: i32) {
        self.write_u32(v as u32);
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u32(v as u32);
    }
}

/// Result of a light trace (ironwail's lightcache_t.surfidx/ds/dt).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LightHit {
    Miss,
    Black,
    Surf { surf: u32, ds: i32, dt: i32 },
}

fn build_anims(bsp: &Bsp) -> Vec<(Vec<u32>, Vec<u32>, bool)> {
    let n = bsp.textures.len();
    let mut out = vec![(Vec::new(), Vec::new(), false); n];
    for i in 0..n {
        let Some(t) = &bsp.textures[i] else { continue };
        let b = t.name.as_bytes();
        if b.len() < 2 || (b[0] != b'+' && b[0] != b'-') || !out[i].0.is_empty() {
            continue;
        }
        let random = b[0] == b'-';
        let mut prim: [Option<u32>; 10] = [None; 10];
        let mut alt: [Option<u32>; 10] = [None; 10];
        for (j, t2) in bsp.textures.iter().enumerate() {
            let Some(t2) = t2 else { continue };
            let b2 = t2.name.as_bytes();
            if b2.len() < 2 || b2[0] != b[0] || !b2[2..].eq_ignore_ascii_case(&b[2..]) {
                continue;
            }
            match b2[1].to_ascii_lowercase() {
                c @ b'0'..=b'9' => prim[(c - b'0') as usize] = Some(j as u32),
                c @ b'a'..=b'j' => alt[(c - b'a') as usize] = Some(j as u32),
                _ => {}
            }
        }
        // sequences stop at the first gap (Mod_LoadTextures errors on gaps)
        let p: Vec<u32> = prim.iter().map_while(|x| *x).collect();
        let a: Vec<u32> = alt.iter().map_while(|x| *x).collect();
        for &m in p.iter().chain(a.iter()) {
            out[m as usize] = (p.clone(), a.clone(), random);
        }
    }
    out
}

pub fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn dot64(a: [f32; 3], b: [f32; 4]) -> f64 {
    a[0] as f64 * b[0] as f64 + a[1] as f64 * b[1] as f64 + a[2] as f64 * b[2] as f64
}

/// Frustum planes prepared for center/extent box tests.
struct Planes {
    p: [[f32; 4]; 4],
    abs: [[f32; 3]; 4],
}

impl Planes {
    fn new(f: &[[f32; 4]; 4]) -> Self {
        Planes { p: *f, abs: f.map(|p| [p[0].abs(), p[1].abs(), p[2].abs()]) }
    }

    /// Like [`clip_box`] for a box given as center `c` and half-size `e`.
    #[inline(always)]
    fn clip(&self, clip: u8, c: [f32; 3], e: [f32; 3]) -> Option<u8> {
        let mut out = clip;
        for j in 0..4 {
            if clip & (1 << j) == 0 {
                continue;
            }
            let p = &self.p[j];
            let a = &self.abs[j];
            let d = p[0] * c[0] + p[1] * c[1] + p[2] * c[2] + p[3];
            let r = a[0] * e[0] + a[1] * e[1] + a[2] * e[2];
            if d + r < 0.0 {
                return None;
            }
            if d - r >= 0.0 {
                out &= !(1 << j);
            }
        }
        Some(out)
    }
}

fn center_extent(mins: [f32; 3], maxs: [f32; 3]) -> ([f32; 3], [f32; 3]) {
    (std::array::from_fn(|i| (mins[i] + maxs[i]) * 0.5), std::array::from_fn(|i| (maxs[i] - mins[i]) * 0.5))
}

pub fn box_in_frustum(f: &[[f32; 4]; 4], mins: [f32; 3], maxs: [f32; 3]) -> bool {
    f.iter().all(|p| {
        let v = [0, 1, 2].map(|i| if p[i] >= 0.0 { maxs[i] } else { mins[i] });
        p[0] * v[0] + p[1] * v[1] + p[2] * v[2] + p[3] >= 0.0
    })
}
