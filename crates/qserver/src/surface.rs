//! BSP surface queries for `DP_QC_GETSURFACE`, `DP_QC_GETSURFACETRIANGLE`,
//! and `DP_QC_GETSURFACEPOINTATTRIBUTE` (port of vkQuake `pr_ext.c`).

use qformats::bsp::{Bsp, Face};
use qworld::Vec3;

const NEARSURFACE_MAXDIST: f32 = 256.0;

#[derive(Debug, Clone)]
pub struct SurfaceFace {
    pub verts: Vec<[f32; 3]>,
    pub normal: [f32; 3],
    pub dist: f32,
    pub flipped: bool,
    pub texture: String,
    pub tex_vecs: [[f32; 4]; 2],
    pub tex_size: [f32; 2],
}

#[derive(Debug, Clone)]
pub struct SurfaceSubmodel {
    pub first_face: usize,
    pub num_faces: usize,
}

/// Server-side surface data extracted from a BSP for `getsurface*` builtins.
#[derive(Debug, Clone)]
pub struct SurfaceModel {
    pub faces: Vec<SurfaceFace>,
    pub submodels: Vec<SurfaceSubmodel>,
}

impl SurfaceModel {
    pub fn from_bsp(bsp: &Bsp) -> Self {
        let mut faces = Vec::with_capacity(bsp.faces.len());
        for f in &bsp.faces {
            faces.push(build_face(bsp, f));
        }
        let submodels = bsp
            .models
            .iter()
            .map(|m| SurfaceSubmodel {
                first_face: m.first_face.max(0) as usize,
                num_faces: m.num_faces.max(0) as usize,
            })
            .collect();
        Self { faces, submodels }
    }

    pub fn submodel_faces(&self, sub: usize) -> &[SurfaceFace] {
        let Some(sm) = self.submodels.get(sub) else { return &[] };
        let end = sm.first_face.saturating_add(sm.num_faces).min(self.faces.len());
        if sm.first_face >= end {
            &[]
        } else {
            &self.faces[sm.first_face..end]
        }
    }

    pub fn face(&self, sub: usize, surfidx: usize) -> Option<&SurfaceFace> {
        self.submodel_faces(sub).get(surfidx)
    }

    /// Outward normal (flipped when `SURF_PLANEBACK`).
    pub fn face_outward_normal(f: &SurfaceFace) -> [f32; 3] {
        if f.flipped {
            [-f.normal[0], -f.normal[1], -f.normal[2]]
        } else {
            f.normal
        }
    }

    /// `getsurface_clippointpoly` (vkQuake `pr_ext.c`): projects `point` onto `f`'s
    /// polygon and returns `(dist_squared, clipped_point)` if closer than `best_dist_sq`.
    pub fn clip_point_poly(f: &SurfaceFace, point: [f32; 3], best_dist_sq: f32) -> Option<(f32, [f32; 3])> {
        if f.verts.len() < 3 {
            return None;
        }
        let n = Vec3::from(f.normal);
        let p = Vec3::from(point);
        let plane_dist = p.dot(n) - f.dist;
        if plane_dist * plane_dist >= best_dist_sq {
            return None;
        }
        // Project onto plane: p - plane_dist * n (note vkQuake uses VectorMA(p, dist, n)
        // which only matters at plane_dist ≈ 0; true projection is p - plane_dist * n).
        let mut cpoint = p - n * plane_dist;
        let nv = f.verts.len();
        for i in 0..nv {
            let v2 = Vec3::from(f.verts[i]);
            let v1 = Vec3::from(f.verts[(i + 1) % nv]);
            let edgedir = v1 - v2;
            let mut edgenormal = edgedir.cross(n);
            if !f.flipped {
                edgenormal = -edgenormal;
            }
            let len = edgenormal.length();
            if len > 1e-6 {
                edgenormal *= 1.0 / len;
                let d = v1.dot(edgenormal) - cpoint.dot(edgenormal);
                if d < 0.0 {
                    cpoint += edgenormal * d;
                }
            }
        }
        let delta = cpoint - p;
        let d2 = delta.dot(delta);
        (d2 < best_dist_sq).then_some((d2, cpoint.to_array()))
    }

    /// `PF_getsurfacenearpoint`: index of the closest face within 256^2 units, or `None`.
    pub fn near_point(&self, sub: usize, point: [f32; 3]) -> Option<usize> {
        let mut best_dist = NEARSURFACE_MAXDIST;
        let mut best = None;
        for (i, f) in self.submodel_faces(sub).iter().enumerate() {
            if let Some((d2, _)) = Self::clip_point_poly(f, point, best_dist) {
                best_dist = d2;
                best = Some(i);
            }
        }
        best
    }

    /// `PF_getsurfaceclippedpoint`: clips `point` onto face `surfidx`.
    pub fn clipped_point(&self, sub: usize, surfidx: usize, point: [f32; 3]) -> [f32; 3] {
        let Some(f) = self.face(sub, surfidx) else { return point };
        Self::clip_point_poly(f, point, f32::MAX).map_or(point, |(_, cp)| cp)
    }

    /// `PF_getsurfacepointattribute` (DP_QC_GETSURFACEPOINTATTRIBUTE).
    pub fn point_attribute(&self, sub: usize, surfidx: usize, pt_idx: usize, attr: u32) -> [f32; 3] {
        let Some(f) = self.face(sub, surfidx) else { return [0.0; 3] };
        let Some(&pos) = f.verts.get(pt_idx) else { return [0.0; 3] };
        match attr {
            0 => pos, // SPA_POSITION
            1 | 2 => {
                // SPA_S_AXIS / SPA_T_AXIS perpendicular to face normal
                let n = Vec3::from(f.normal);
                let axis = Vec3::new(f.tex_vecs[(attr - 1) as usize][0], f.tex_vecs[(attr - 1) as usize][1], f.tex_vecs[(attr - 1) as usize][2]);
                let sc = -n.dot(axis);
                let v = axis + n * sc;
                let len = v.length();
                if len > 1e-6 { (v * (1.0 / len)).to_array() } else { [0.0; 3] }
            }
            3 => Self::face_outward_normal(f), // SPA_R_AXIS
            4 => {
                // SPA_TEXCOORDS0
                let s = (pos[0] * f.tex_vecs[0][0] + pos[1] * f.tex_vecs[0][1] + pos[2] * f.tex_vecs[0][2] + f.tex_vecs[0][3]) / f.tex_size[0].max(1.0);
                let t = (pos[0] * f.tex_vecs[1][0] + pos[1] * f.tex_vecs[1][1] + pos[2] * f.tex_vecs[1][2] + f.tex_vecs[1][3]) / f.tex_size[1].max(1.0);
                [s, t, 0.0]
            }
            6 => [1.0, 1.0, 1.0], // SPA_LIGHTMAP0_COLOR
            _ => [0.0; 3],
        }
    }
}

fn build_face(bsp: &Bsp, f: &Face) -> SurfaceFace {
    let mut verts = Vec::with_capacity(f.num_edges as usize);
    for e in 0..f.num_edges {
        let Some(&se) = bsp.surfedges.get((f.first_edge + e) as usize) else { break };
        let edge = bsp.edges.get(se.unsigned_abs() as usize).copied().unwrap_or([0, 0]);
        let vi = if se >= 0 { edge[0] } else { edge[1] };
        verts.push(bsp.vertexes.get(vi as usize).copied().unwrap_or([0.0; 3]));
    }
    let (normal, dist) = bsp
        .planes
        .get(f.plane as usize)
        .map_or(([0.0, 0.0, 1.0], 0.0), |p| (p.normal, p.dist));
    let (texture, tex_vecs, tex_size) = bsp
        .texinfo
        .get(f.texinfo as usize)
        .map(|ti| {
            let mt = bsp.textures.get(ti.miptex.max(0) as usize).and_then(|t| t.as_ref());
            let name = mt.map_or_else(String::new, |t| t.name.clone());
            let sz = mt.map_or([16.0, 16.0], |t| [t.width.max(1) as f32, t.height.max(1) as f32]);
            (name, ti.vecs, sz)
        })
        .unwrap_or_else(|| (String::new(), [[0.0; 4]; 2], [16.0, 16.0]));
    SurfaceFace {
        verts,
        normal,
        dist,
        flipped: f.side != 0,
        texture,
        tex_vecs,
        tex_size,
    }
}
