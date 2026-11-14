//! CPU-side world preparation: surfaces, lightmap atlas packing, texture
//! animation tables, PVS/frustum marking and light sampling.

use qformats::bsp::{Bsp, TEX_SPECIAL, TextureKind};
mod build;
mod query;
mod cull;
mod light;
mod hash;
pub use hash::*;

pub const ATLAS: u32 = 2048;
pub const NO_LM: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Default, Debug)]
pub struct WorldVertex {
    pub pos: [f32; 3],
    pub st: [f32; 2],
    pub lm: [f32; 3],
    /// Underwater faces: height of the liquid surface above and liquid kind
    /// (1 water, 2 lava, 3 slime; +4 when the column is roofed). 0 kind = dry.
    pub liquid: [f32; 2],
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
    pub(super) transparent_liquids: bool,
    /// R_LightPoint cache (ironwail's per-entity lightcache_t): sample point
    /// floored to whole units -> trace result. Entities that don't move skip
    /// the BSP trace; the colour is re-read from the lightmap each call so
    /// lightstyles still animate.
    light_cache: std::sync::Mutex<std::collections::HashMap<[i32; 3], LightHit, PointHash>>,
    pub vertices: Vec<WorldVertex>,
    pub indices: Vec<u32>,
    pub surfaces: Vec<Surface>,
    pub hidden_surfaces: std::collections::HashSet<u32>,
    pub texture_names: Vec<String>,
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
