//! Alias model (.mdl, "IDPO" version 6).

use crate::{Reader, Result, Vec3, check_count};

const FMT: &str = "mdl";

pub const EF_ROCKET: i32 = 1;
pub const EF_GRENADE: i32 = 2;
pub const EF_GIB: i32 = 4;
pub const EF_ROTATE: i32 = 8;
pub const EF_TRACER: i32 = 16;
pub const EF_ZOMGIB: i32 = 32;
pub const EF_TRACER2: i32 = 64;
pub const EF_TRACER3: i32 = 128;
/// Index 255 transparent (Quakespasm/ironwail extension).
pub const MF_HOLEY: i32 = 1 << 14;

#[derive(Debug, Clone, PartialEq)]
pub struct Skin {
    /// Group intervals (empty for a single skin).
    pub intervals: Vec<f32>,
    /// One or more images, each `width * height` palette indices.
    pub images: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StVert {
    pub onseam: i32,
    pub s: i32,
    pub t: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Triangle {
    pub faces_front: i32,
    pub verts: [i32; 3],
}

/// Compressed vertex: position bytes + normal index into anorms table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriVert {
    pub v: [u8; 3],
    pub normal: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Pose {
    pub name: String,
    pub bboxmin: TriVert,
    pub bboxmax: TriVert,
    pub verts: Vec<TriVert>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub bboxmin: TriVert,
    pub bboxmax: TriVert,
    /// Group intervals (empty for a single frame).
    pub intervals: Vec<f32>,
    pub poses: Vec<Pose>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mdl {
    pub scale: Vec3,
    pub scale_origin: Vec3,
    pub bounding_radius: f32,
    pub eye_position: Vec3,
    pub skin_width: u32,
    pub skin_height: u32,
    pub num_verts: usize,
    pub synctype: i32,
    pub flags: i32,
    pub size: f32,
    pub skins: Vec<Skin>,
    pub stverts: Vec<StVert>,
    pub triangles: Vec<Triangle>,
    pub frames: Vec<Frame>,
}

fn trivert(r: &mut Reader) -> Result<TriVert> {
    let b = r.arr::<4>()?;
    Ok(TriVert {
        v: [b[0], b[1], b[2]],
        normal: b[3],
    })
}

fn intervals(r: &mut Reader, n: usize) -> Result<Vec<f32>> {
    (0..n).map(|_| r.f32()).collect()
}

fn pose(r: &mut Reader, nv: usize) -> Result<Pose> {
    let bboxmin = trivert(r)?;
    let bboxmax = trivert(r)?;
    let name = r.name::<16>()?;
    let verts = (0..nv).map(|_| trivert(r)).collect::<Result<_>>()?;
    Ok(Pose {
        name,
        bboxmin,
        bboxmax,
        verts,
    })
}

impl Mdl {
    pub fn parse(data: &[u8]) -> Result<Mdl> {
        let mut r = Reader::new(data, FMT);
        if &r.arr::<4>()? != b"IDPO" {
            return Err(r.err("bad magic"));
        }
        let version = r.i32()?;
        if version != 6 {
            return Err(r.err(format!("unsupported version {version}")));
        }
        let scale = r.vec3()?;
        let scale_origin = r.vec3()?;
        let bounding_radius = r.f32()?;
        let eye_position = r.vec3()?;
        let num_skins = r.count("skin", 4)?;
        let sw = r.i32()?;
        let sh = r.i32()?;
        if sw <= 0 || sh <= 0 || sw > 4096 || sh > 4096 {
            return Err(r.err(format!("bad skin size {sw}x{sh}")));
        }
        let skin_size = (sw * sh) as usize;
        let num_verts = r.count("vertex", 12)?;
        let num_tris = r.count("triangle", 16)?;
        let num_frames = r.count("frame", 24)?;
        let synctype = r.i32()?;
        let flags = r.i32()?;
        let size = r.f32()?;

        let mut skins = Vec::with_capacity(num_skins);
        for _ in 0..num_skins {
            let kind = r.i32()?;
            if kind == 0 {
                skins.push(Skin {
                    intervals: Vec::new(),
                    images: vec![r.bytes(skin_size)?.to_vec()],
                });
            } else {
                let n = r.i32()?;
                let n = check_count(&r, n, "skin group", skin_size + 4)?;
                let intervals = intervals(&mut r, n)?;
                let images = (0..n)
                    .map(|_| Ok(r.bytes(skin_size)?.to_vec()))
                    .collect::<Result<_>>()?;
                skins.push(Skin { intervals, images });
            }
        }
        let stverts = (0..num_verts)
            .map(|_| {
                Ok(StVert {
                    onseam: r.i32()?,
                    s: r.i32()?,
                    t: r.i32()?,
                })
            })
            .collect::<Result<_>>()?;
        let triangles = (0..num_tris)
            .map(|_| {
                let faces_front = r.i32()?;
                let verts = [r.i32()?, r.i32()?, r.i32()?];
                if verts.iter().any(|&v| v < 0 || v as usize >= num_verts) {
                    return Err(r.err("triangle vertex index out of range"));
                }
                Ok(Triangle { faces_front, verts })
            })
            .collect::<Result<_>>()?;
        let mut frames = Vec::with_capacity(num_frames);
        for _ in 0..num_frames {
            let kind = r.i32()?;
            if kind == 0 {
                let p = pose(&mut r, num_verts)?;
                frames.push(Frame {
                    bboxmin: p.bboxmin,
                    bboxmax: p.bboxmax,
                    intervals: Vec::new(),
                    poses: vec![p],
                });
            } else {
                let n = r.i32()?;
                let n = check_count(&r, n, "frame group", 4 * num_verts + 28)?;
                let bboxmin = trivert(&mut r)?;
                let bboxmax = trivert(&mut r)?;
                let intervals = intervals(&mut r, n)?;
                let poses = (0..n)
                    .map(|_| pose(&mut r, num_verts))
                    .collect::<Result<_>>()?;
                frames.push(Frame {
                    bboxmin,
                    bboxmax,
                    intervals,
                    poses,
                });
            }
        }
        Ok(Mdl {
            scale,
            scale_origin,
            bounding_radius,
            eye_position,
            skin_width: sw as u32,
            skin_height: sh as u32,
            num_verts,
            synctype,
            flags,
            size,
            skins,
            stverts,
            triangles,
            frames,
        })
    }

    /// Decode a compressed vertex position to model space.
    pub fn decode(&self, v: TriVert) -> Vec3 {
        std::array::from_fn(|i| v.v[i] as f32 * self.scale[i] + self.scale_origin[i])
    }
}
