//! BSPX extension lumps (Mod_LoadBSPX): a "BSPX" directory appended after the
//! last standard lump, 4-byte aligned, holding named extra lumps:
//! - `RGBLIGHTING`: coloured lightmap (3 bytes per mono sample).
//! - `DECOUPLED_LM`: arbitrary per-face lightmap size, offset and projection.
//! - `LMSHIFT` (+ optional `LMOFFSET`, `LMSTYLE`): per-face power-of-two luxel scale.
//! - `LIGHTGRID_OCTREE`: volumetric 3D lighting octree for alias models.
//! - `BRUSHLIST`: per-model convex brushes (only drawn by the collision hull view;
//!   like ironwail and vkQuake, collision uses the hulls alone).
//! - `FACENORMALS`: per-face, per-vertex smooth Phong normals/tangents/bitangents.

use crate::bsp::{Face, HEADER_LUMPS};
use crate::{Reader, Vec3};

#[cfg(test)]
mod tests;

/// Find a BSPX lump by name in a whole `.bsp` file. Returns `None` for files
/// without a BSPX directory or with a malformed one.
pub fn find_lump<'a>(data: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let u32_at = |o: usize| data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
    // Directory starts after the furthest standard lump.
    let mut end = 0usize;
    for i in 0..HEADER_LUMPS {
        let ofs = u32_at(4 + i * 8)?;
        let len = u32_at(8 + i * 8)?;
        end = end.max(ofs.checked_add(len)?);
    }
    let dir = end.checked_add(3)? & !3;
    if data.get(dir..dir + 4)? != b"BSPX" {
        return None;
    }
    let count = u32_at(dir + 4)?;
    (0..count.min(1024)).find_map(|i| {
        let e = dir + 8 + i * 32;
        let raw = data.get(e..e + 24)?;
        let lname = raw.split(|&b| b == 0).next()?;
        if !lname.eq_ignore_ascii_case(name.as_bytes()) {
            return None;
        }
        let (ofs, len) = (u32_at(e + 24)?, u32_at(e + 28)?);
        data.get(ofs..ofs.checked_add(len)?)
    })
}

fn finite3(v: Vec3) -> bool {
    v.iter().all(|c| c.is_finite())
}

fn finite4(v: [f32; 4]) -> bool {
    v.iter().all(|c| c.is_finite())
}

// ---- DECOUPLED_LM / LMSHIFT / LMOFFSET / LMSTYLE ----

/// Per-face entry of the `DECOUPLED_LM` BSPX lump (ericw-tools / FTEQW).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DecoupledLm {
    pub width: u16,
    pub height: u16,
    /// Sample offset into `LUMP_LIGHTING` (RGB is `offset * 3`), `-1` = none.
    pub offset: i32,
    /// World -> lightmap luxel coordinates (2x4 row-major: `dot(p, vecs[i].xyz) + vecs[i][3]`).
    pub vecs: [[f32; 4]; 2],
}

/// Parse `DECOUPLED_LM`: 40 bytes per face (`u16 w, u16 h, i32 offset, [[f32; 4]; 2]`).
pub fn parse_decoupled_lm(data: &[u8], num_faces: usize) -> Option<Vec<DecoupledLm>> {
    if data.len() != num_faces.checked_mul(40)? {
        return None;
    }
    let mut r = Reader::new(data, "bspx");
    let mut out = Vec::with_capacity(num_faces);
    for _ in 0..num_faces {
        let width = r.u16().ok()?;
        let height = r.u16().ok()?;
        let offset = r.i32().ok()?;
        let mut vecs = [[0.0f32; 4]; 2];
        for v in vecs.iter_mut().flatten() {
            *v = r.f32().ok()?;
        }
        out.push(DecoupledLm { width, height, offset, vecs });
    }
    Some(out)
}

/// Parse `LMSHIFT`: 1 byte per face giving `1 << shift` texels per luxel (default 4).
pub fn parse_lm_shift(data: &[u8], num_faces: usize) -> Option<Vec<u8>> {
    (data.len() == num_faces).then(|| data.to_vec())
}

/// Parse `LMOFFSET`: 4-byte `i32` lightmap offset per face (companion to `LMSHIFT`).
pub fn parse_lm_offset(data: &[u8], num_faces: usize) -> Option<Vec<i32>> {
    if data.len() != num_faces.checked_mul(4)? {
        return None;
    }
    let mut r = Reader::new(data, "bspx");
    let mut out = Vec::with_capacity(num_faces);
    for _ in 0..num_faces {
        out.push(r.i32().ok()?);
    }
    Some(out)
}

/// Parse `LMSTYLE`: 4 lightstyle bytes per face (companion to `LMSHIFT`/`DECOUPLED_LM`).
pub fn parse_lm_style(data: &[u8], num_faces: usize) -> Option<Vec<[u8; 4]>> {
    if data.len() != num_faces.checked_mul(4)? {
        return None;
    }
    Some(data.as_chunks::<4>().0.to_vec())
}

// ---- LIGHTGRID_OCTREE ----

/// Child flag in [`LightGridNode::children`]: index points into [`LightGridOctree::leafs`].
pub const LIGHTGRID_LEAF: u32 = 1 << 31;
/// Child flag in [`LightGridNode::children`]: octant is solid/occluded (no lighting data).
pub const LIGHTGRID_OCCLUDED: u32 = 1 << 30;

/// Single lightstyle contribution at a lightgrid sample point.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LightGridSample {
    pub style: u8,
    pub rgb: [u8; 3],
}

/// Lighting data at one grid cell: up to 4 styles, or occluded (`count == 0xff`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LightGridCell {
    pub samples: [LightGridSample; 4],
    /// Number of valid entries in `samples` (`0..=4`), or `0xff` when occluded.
    pub count: u8,
}

impl LightGridCell {
    pub const OCCLUDED: u8 = 0xff;

    #[inline]
    pub fn is_occluded(&self) -> bool {
        self.count == Self::OCCLUDED
    }
}

/// Interior split node of a `LIGHTGRID_OCTREE`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightGridNode {
    pub division_point: [i32; 3],
    pub children: [u32; 8],
}

/// Dense 3D leaf block of a `LIGHTGRID_OCTREE`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightGridLeaf {
    pub mins: [i32; 3],
    pub size: [i32; 3],
    /// Samples in `[z][y][x]` order (`size[0] * size[1] * size[2]`).
    pub cells: Vec<LightGridCell>,
}

/// Volumetric 3D lighting octree (`LIGHTGRID_OCTREE`, ericw-tools / FTEQW).
#[derive(Debug, Clone, PartialEq)]
pub struct LightGridOctree {
    pub grid_dist: Vec3,
    pub grid_size: [i32; 3],
    pub grid_mins: Vec3,
    pub num_styles: u8,
    pub root_node: u32,
    pub nodes: Vec<LightGridNode>,
    pub leafs: Vec<LightGridLeaf>,
}

impl LightGridOctree {
    /// Look up a single integer grid coordinate; returns `None` if out of bounds,
    /// missing, or occluded.
    pub fn lookup_cell(&self, pt: [i32; 3]) -> Option<&LightGridCell> {
        if pt.iter().zip(self.grid_size).any(|(&p, s)| p < 0 || p >= s) {
            return None;
        }
        let mut idx = self.root_node;
        // Depth cap prevents infinite loops on malformed cyclic octrees.
        for _ in 0..64 {
            if idx & LIGHTGRID_OCCLUDED != 0 {
                return None;
            }
            if idx & LIGHTGRID_LEAF != 0 {
                let leaf = self.leafs.get((idx & !LIGHTGRID_LEAF) as usize)?;
                let lx = pt[0].checked_sub(leaf.mins[0])?;
                let ly = pt[1].checked_sub(leaf.mins[1])?;
                let lz = pt[2].checked_sub(leaf.mins[2])?;
                if lx < 0 || ly < 0 || lz < 0 || lx >= leaf.size[0] || ly >= leaf.size[1] || lz >= leaf.size[2] {
                    return None;
                }
                let sx = leaf.size[0] as usize;
                let sy = leaf.size[1] as usize;
                let cell_idx = (sx * sy * lz as usize) + (sx * ly as usize) + lx as usize;
                let cell = leaf.cells.get(cell_idx)?;
                return (!cell.is_occluded()).then_some(cell);
            }
            let node = self.nodes.get(idx as usize)?;
            let octant = ((pt[0] >= node.division_point[0]) as usize * 4)
                | ((pt[1] >= node.division_point[1]) as usize * 2)
                | ((pt[2] >= node.division_point[2]) as usize);
            idx = node.children[octant];
        }
        None
    }

    /// Trilinearly sample the octree at world position `p` with `styles`
    /// (`256` = 1.0x, matching Quake's `d_lightstylevalue`). Returns RGB in
    /// `0..255+` scale, or `None` if all 8 surrounding cells are occluded/outside.
    pub fn sample(&self, p: Vec3, styles: &[i32]) -> Option<Vec3> {
        let mut tile = [0i32; 3];
        let mut frac = [0.0f32; 3];
        for axis in 0..3 {
            let g = (p[axis] - self.grid_mins[axis]) / self.grid_dist[axis];
            if !g.is_finite() {
                return None;
            }
            let fl = g.floor();
            tile[axis] = fl as i32;
            frac[axis] = g - fl;
        }
        let mut accum = [0.0f32; 3];
        let mut weight_sum = 0.0f32;
        let mut fallback = [0.0f32; 3];
        let mut fallback_count = 0u32;
        for corner in 0..8usize {
            let dx = (corner & 1) as i32;
            let dy = ((corner >> 1) & 1) as i32;
            let dz = ((corner >> 2) & 1) as i32;
            let Some(cell) = self.lookup_cell([tile[0] + dx, tile[1] + dy, tile[2] + dz]) else {
                continue;
            };
            let mut c = [0.0f32; 3];
            for s in &cell.samples[..cell.count.min(4) as usize] {
                let scale = styles.get(s.style as usize).copied().unwrap_or(256) as f32 / 256.0;
                for (dst, &src) in c.iter_mut().zip(&s.rgb) {
                    *dst += src as f32 * scale;
                }
            }
            let w = if dx != 0 { frac[0] } else { 1.0 - frac[0] }
                * if dy != 0 { frac[1] } else { 1.0 - frac[1] }
                * if dz != 0 { frac[2] } else { 1.0 - frac[2] };
            if w > 0.0 {
                for (a, &cv) in accum.iter_mut().zip(&c) {
                    *a += cv * w;
                }
                weight_sum += w;
            }
            for (f, &cv) in fallback.iter_mut().zip(&c) {
                *f += cv;
            }
            fallback_count += 1;
        }
        if weight_sum > 0.0 {
            Some(accum.map(|v| v / weight_sum))
        } else if fallback_count > 0 {
            let inv = 1.0 / fallback_count as f32;
            Some(fallback.map(|v| v * inv))
        } else {
            None
        }
    }
}

/// Parse `LIGHTGRID_OCTREE` (ericw-tools / FTEQW binary layout).
pub fn parse_lightgrid_octree(data: &[u8]) -> Option<LightGridOctree> {
    let mut r = Reader::new(data, "bspx");
    let grid_dist = r.vec3().ok()?;
    let grid_size = [r.i32().ok()?, r.i32().ok()?, r.i32().ok()?];
    let grid_mins = r.vec3().ok()?;
    if !finite3(grid_dist) || !finite3(grid_mins) || grid_dist.iter().any(|&d| d <= 0.0) {
        return None;
    }
    if grid_size.iter().any(|&s| s < 0) {
        return None;
    }
    // Single byte misaligns subsequent reads per the BSPX spec.
    let num_styles = r.u8().ok()?;
    let root_node = r.u32().ok()?;

    let num_nodes = r.u32().ok()? as usize;
    if num_nodes.checked_mul(44)? > r.remaining() {
        return None;
    }
    let mut nodes = Vec::with_capacity(num_nodes);
    for _ in 0..num_nodes {
        let division_point = [r.i32().ok()?, r.i32().ok()?, r.i32().ok()?];
        let mut children = [0u32; 8];
        for c in &mut children {
            *c = r.u32().ok()?;
        }
        nodes.push(LightGridNode { division_point, children });
    }

    let num_leafs = r.u32().ok()? as usize;
    if num_leafs.checked_mul(24)? > r.remaining() {
        return None;
    }
    let mut leafs = Vec::with_capacity(num_leafs);
    for _ in 0..num_leafs {
        let mins = [r.i32().ok()?, r.i32().ok()?, r.i32().ok()?];
        let size = [r.i32().ok()?, r.i32().ok()?, r.i32().ok()?];
        if size.iter().any(|&s| s < 0) {
            return None;
        }
        let total = (size[0] as usize).checked_mul(size[1] as usize)?.checked_mul(size[2] as usize)?;
        if total > r.remaining() {
            return None;
        }
        let mut cells = Vec::with_capacity(total);
        for _ in 0..total {
            let count = r.u8().ok()?;
            if count == LightGridCell::OCCLUDED {
                cells.push(LightGridCell { samples: [LightGridSample::default(); 4], count });
                continue;
            }
            let mut samples = [LightGridSample::default(); 4];
            let mut stored = 0u8;
            for _ in 0..count {
                let style = r.u8().ok()?;
                let rgb = r.arr::<3>().ok()?;
                if (stored as usize) < 4 {
                    samples[stored as usize] = LightGridSample { style, rgb };
                    stored += 1;
                }
            }
            cells.push(LightGridCell { samples, count: stored });
        }
        leafs.push(LightGridLeaf { mins, size, cells });
    }
    Some(LightGridOctree { grid_dist, grid_size, grid_mins, num_styles, root_node, nodes, leafs })
}

// ---- BRUSHLIST ----

/// Outward-facing non-axial plane of a [`BspxBrush`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BrushPlane {
    pub normal: Vec3,
    pub dist: f32,
}

/// Single convex brush in a `BRUSHLIST` model entry.
/// Axial bounding planes are implied by `mins` and `maxs`.
#[derive(Debug, Clone, PartialEq)]
pub struct BspxBrush {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub contents: i16,
    pub planes: Vec<BrushPlane>,
}

/// All brushes belonging to one inline BSP submodel (`0` = world).
#[derive(Debug, Clone, PartialEq)]
pub struct BspxModelBrushes {
    pub model_num: u32,
    pub brushes: Vec<BspxBrush>,
}

/// Parse `BRUSHLIST` (ericw-tools / FTEQW version 1).
pub fn parse_brushlist(data: &[u8]) -> Option<Vec<BspxModelBrushes>> {
    let mut r = Reader::new(data, "bspx");
    let mut models = Vec::new();
    while r.remaining() > 0 {
        let ver = r.i32().ok()?;
        if ver != 1 {
            return None;
        }
        let model_num = r.i32().ok()?;
        let num_brushes = r.i32().ok()?;
        let expected_planes = r.i32().ok()?;
        if model_num < 0 || num_brushes < 0 || expected_planes < 0 {
            return None;
        }
        let nb = num_brushes as usize;
        let ep = expected_planes as usize;
        if nb.checked_mul(28)?.checked_add(ep.checked_mul(16)?)? > r.remaining() {
            return None;
        }
        let mut brushes = Vec::with_capacity(nb);
        let mut total_planes = 0usize;
        for _ in 0..nb {
            let mins = r.vec3().ok()?;
            let maxs = r.vec3().ok()?;
            if !finite3(mins) || !finite3(maxs) {
                return None;
            }
            let contents = r.i16().ok()?;
            let np = r.u16().ok()? as usize;
            if np.checked_mul(16)? > r.remaining() {
                return None;
            }
            total_planes = total_planes.checked_add(np)?;
            let mut planes = Vec::with_capacity(np);
            for _ in 0..np {
                let normal = r.vec3().ok()?;
                let dist = r.f32().ok()?;
                if !finite4([normal[0], normal[1], normal[2], dist]) {
                    return None;
                }
                planes.push(BrushPlane { normal, dist });
            }
            brushes.push(BspxBrush { mins, maxs, contents, planes });
        }
        if total_planes != ep {
            return None;
        }
        models.push(BspxModelBrushes { model_num: model_num as u32, brushes });
    }
    Some(models)
}

// ---- FACENORMALS ----

/// Per-face-vertex indices into [`FaceNormals::vecs`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FaceNormalIndices {
    pub normal: u32,
    pub tangent: u32,
    pub bitangent: u32,
}

/// Per-face, per-vertex smooth normals, tangents and bitangents (`FACENORMALS`).
#[derive(Debug, Clone, PartialEq)]
pub struct FaceNormals {
    /// Deduplicated 3D vectors referenced by `per_vert`.
    pub vecs: Vec<Vec3>,
    /// Flattened per-face, per-edge indices matching `Bsp::faces`.
    pub per_vert: Vec<FaceNormalIndices>,
    /// Starting offset into `per_vert` for each face `i`.
    pub face_offsets: Vec<u32>,
}

impl FaceNormals {
    /// Smooth vertex normal for edge `edge_idx` of `face_idx`, if valid.
    #[inline]
    pub fn vertex_normal(&self, face_idx: usize, edge_idx: usize) -> Option<Vec3> {
        let base = *self.face_offsets.get(face_idx)? as usize;
        let idx = self.per_vert.get(base + edge_idx)?.normal as usize;
        self.vecs.get(idx).copied()
    }
}

/// Parse `FACENORMALS` (ericw-tools `light -wrnormals`).
pub fn parse_face_normals(data: &[u8], faces: &[Face]) -> Option<FaceNormals> {
    let mut r = Reader::new(data, "bspx");
    let num_vecs = r.u32().ok()? as usize;
    let mut total_verts = 0usize;
    let mut face_offsets = Vec::with_capacity(faces.len());
    for f in faces {
        face_offsets.push(u32::try_from(total_verts).ok()?);
        total_verts = total_verts.checked_add(f.num_edges as usize)?;
    }
    let expected = 4usize
        .checked_add(num_vecs.checked_mul(12)?)?
        .checked_add(total_verts.checked_mul(12)?)?;
    if data.len() != expected {
        return None;
    }
    let mut vecs = Vec::with_capacity(num_vecs);
    for _ in 0..num_vecs {
        let v = r.vec3().ok()?;
        if !finite3(v) {
            return None;
        }
        vecs.push(v);
    }
    let mut per_vert = Vec::with_capacity(total_verts);
    for _ in 0..total_verts {
        let normal = r.u32().ok()?;
        let tangent = r.u32().ok()?;
        let bitangent = r.u32().ok()?;
        if normal as usize >= num_vecs || tangent as usize >= num_vecs || bitangent as usize >= num_vecs {
            return None;
        }
        per_vert.push(FaceNormalIndices { normal, tangent, bitangent });
    }
    Some(FaceNormals { vecs, per_vert, face_offsets })
}
