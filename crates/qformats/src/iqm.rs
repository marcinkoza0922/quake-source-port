//! Inter-Quake Model (`.iqm`, version 2) skeletal model parser and pose evaluator.
//!
//! All offsets and counts are bounds-checked; malformed input returns [`Error`], never panics.

use crate::{Error, Reader, Result, Vec3};
mod pose;

const FMT: &str = "iqm";
pub const IQM_MAGIC: &[u8; 16] = b"INTERQUAKEMODEL\0";
pub const IQM_VERSION: u32 = 2;

pub const IQM_POSITION: u32 = 0;
pub const IQM_TEXCOORD: u32 = 1;
pub const IQM_NORMAL: u32 = 2;
pub const IQM_TANGENT: u32 = 3;
pub const IQM_BLENDINDEXES: u32 = 4;
pub const IQM_BLENDWEIGHTS: u32 = 5;
pub const IQM_COLOR: u32 = 6;

const IQM_BYTE: u32 = 0;
const IQM_UBYTE: u32 = 1;
const IQM_SHORT: u32 = 2;
const IQM_USHORT: u32 = 3;
const IQM_INT: u32 = 4;
const IQM_UINT: u32 = 5;
const IQM_HALF: u32 = 6;
const IQM_FLOAT: u32 = 7;
const IQM_DOUBLE: u32 = 8;

pub const IQM_LOOP: u32 = 1;

#[derive(Debug, Clone)]
pub struct IqmMesh {
    pub name: String,
    pub material: String,
    pub first_vertex: u32,
    pub num_vertexes: u32,
    pub first_triangle: u32,
    pub num_triangles: u32,
}

#[derive(Debug, Clone)]
pub struct IqmJoint {
    pub name: String,
    pub parent: i32,
    pub translate: Vec3,
    /// Quaternion `(x, y, z, w)`.
    pub rotate: [f32; 4],
    pub scale: Vec3,
}

#[derive(Debug, Clone)]
pub struct IqmPose {
    pub parent: i32,
    pub mask: u32,
    pub channel_offset: [f32; 10],
    pub channel_scale: [f32; 10],
}

#[derive(Debug, Clone)]
pub struct IqmAnim {
    pub name: String,
    pub first_frame: u32,
    pub num_frames: u32,
    pub framerate: f32,
    pub flags: u32,
}

#[derive(Debug, Clone, Copy)]
pub struct IqmBounds {
    pub bbmin: Vec3,
    pub bbmax: Vec3,
    pub xyradius: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct Iqm {
    pub flags: u32,
    pub meshes: Vec<IqmMesh>,
    pub positions: Vec<Vec3>,
    pub texcoords: Vec<[f32; 2]>,
    pub normals: Vec<Vec3>,
    pub blend_indexes: Vec<[u8; 4]>,
    pub blend_weights: Vec<[u8; 4]>,
    pub triangles: Vec<[u32; 3]>,
    pub joints: Vec<IqmJoint>,
    pub poses: Vec<IqmPose>,
    pub anims: Vec<IqmAnim>,
    pub num_frames: u32,
    pub num_framechannels: u32,
    pub frames: Vec<u16>,
    pub bounds: Vec<IqmBounds>,
}

fn text_at(table: &[u8], ofs: u32) -> String {
    let Some(s) = table.get(ofs as usize..) else {
        return String::new();
    };
    crate::cstr(s)
}

fn format_size(fmt: u32) -> Option<usize> {
    match fmt {
        IQM_BYTE | IQM_UBYTE => Some(1),
        IQM_SHORT | IQM_USHORT | IQM_HALF => Some(2),
        IQM_INT | IQM_UINT | IQM_FLOAT => Some(4),
        IQM_DOUBLE => Some(8),
        _ => None,
    }
}

fn half_to_f32(h: u16) -> f32 {
    let sign = ((h >> 15) as u32) << 31;
    let exp = ((h >> 10) & 0x1f) as u32;
    let frac = (h & 0x3ff) as u32;
    if exp == 0 {
        if frac == 0 {
            return f32::from_bits(sign);
        }
        let mut e = -14i32;
        let mut f = frac;
        while (f & 0x400) == 0 {
            f <<= 1;
            e -= 1;
        }
        f &= 0x3ff;
        return f32::from_bits(sign | (((e + 127) as u32) << 23) | (f << 13));
    }
    if exp == 31 {
        return f32::from_bits(sign | (0xff << 23) | (frac << 13));
    }
    f32::from_bits(sign | ((exp + 112) << 23) | (frac << 13))
}

fn read_f32_elem(r: &mut Reader<'_>, fmt: u32) -> Result<f32> {
    Ok(match fmt {
        IQM_BYTE => r.u8()? as i8 as f32,
        IQM_UBYTE => r.u8()? as f32,
        IQM_SHORT => r.i16()? as f32,
        IQM_USHORT => r.u16()? as f32,
        IQM_INT => r.i32()? as f32,
        IQM_UINT => r.u32()? as f32,
        IQM_HALF => half_to_f32(r.u16()?),
        IQM_FLOAT => r.f32()?,
        IQM_DOUBLE => f64::from_le_bytes(r.arr::<8>()?) as f32,
        _ => return Err(r.err(format!("unknown vertex array format {fmt}"))),
    })
}

fn read_u8_norm(r: &mut Reader<'_>, fmt: u32) -> Result<u8> {
    Ok(match fmt {
        IQM_UBYTE => r.u8()?,
        IQM_BYTE => (r.u8()? as i8).max(0) as u8,
        IQM_USHORT => (r.u16()? >> 8) as u8,
        IQM_SHORT => (r.i16()?.max(0) >> 7).min(255) as u8,
        IQM_FLOAT => (r.f32()?.clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        IQM_HALF => (half_to_f32(r.u16()?).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        IQM_DOUBLE => (f64::from_le_bytes(r.arr::<8>()?).clamp(0.0, 1.0) * 255.0 + 0.5) as u8,
        IQM_UINT => r.u32()?.min(255) as u8,
        IQM_INT => r.i32()?.clamp(0, 255) as u8,
        _ => return Err(r.err(format!("unknown vertex array format {fmt}"))),
    })
}

fn read_u8_index(r: &mut Reader<'_>, fmt: u32) -> Result<u8> {
    Ok(match fmt {
        IQM_UBYTE => r.u8()?,
        IQM_BYTE => r.u8()?,
        IQM_USHORT => r.u16()?.min(255) as u8,
        IQM_SHORT => r.i16()?.clamp(0, 255) as u8,
        IQM_UINT => r.u32()?.min(255) as u8,
        IQM_INT => r.i32()?.clamp(0, 255) as u8,
        IQM_FLOAT => r.f32()?.clamp(0.0, 255.0) as u8,
        _ => return Err(r.err(format!("unknown blend index format {fmt}"))),
    })
}

impl Iqm {
    pub fn parse(data: &[u8]) -> Result<Iqm> {
        let mut r = Reader::new(data, FMT);
        if &r.arr::<16>()? != IQM_MAGIC {
            return Err(r.err("bad magic"));
        }
        let version = r.u32()?;
        if version != IQM_VERSION {
            return Err(r.err(format!("unsupported version {version}")));
        }
        let _filesize = r.u32()?;
        let flags = r.u32()?;
        let num_text = r.u32()? as usize;
        let ofs_text = r.u32()? as usize;
        let num_meshes = r.u32()? as usize;
        let ofs_meshes = r.u32()? as usize;
        let num_vertexarrays = r.u32()? as usize;
        let num_vertexes = r.u32()? as usize;
        let ofs_vertexarrays = r.u32()? as usize;
        let num_triangles = r.u32()? as usize;
        let ofs_triangles = r.u32()? as usize;
        let _ofs_adjacency = r.u32()?;
        let num_joints = r.u32()? as usize;
        let ofs_joints = r.u32()? as usize;
        let num_poses = r.u32()? as usize;
        let ofs_poses = r.u32()? as usize;
        let num_anims = r.u32()? as usize;
        let ofs_anims = r.u32()? as usize;
        let num_frames = r.u32()?;
        let num_framechannels = r.u32()?;
        let ofs_frames = r.u32()? as usize;
        let ofs_bounds = r.u32()? as usize;
        let _num_comment = r.u32()?;
        let _ofs_comment = r.u32()?;
        let _num_extensions = r.u32()?;
        let _ofs_extensions = r.u32()?;

        if num_vertexes > 1_000_000 || num_triangles > 2_000_000 || num_joints > 4096 || num_poses > 4096 {
            return Err(Error::new(FMT, "excessive element count"));
        }

        let slice = |ofs: usize, len: usize, what: &str| -> Result<&[u8]> {
            if len == 0 {
                return Ok(&[]);
            }
            let end = ofs
                .checked_add(len)
                .filter(|&e| e <= data.len())
                .ok_or_else(|| Error::new(FMT, format!("{what} ({ofs}+{len}) out of bounds")))?;
            Ok(&data[ofs..end])
        };

        let text = slice(ofs_text, num_text, "text")?;

        // Meshes (24 bytes each)
        let mesh_bytes = slice(ofs_meshes, num_meshes.checked_mul(24).ok_or_else(|| Error::new(FMT, "mesh size overflow"))?, "meshes")?;
        let mut mr = Reader::new(mesh_bytes, FMT);
        let mut meshes = Vec::with_capacity(num_meshes);
        for _ in 0..num_meshes {
            let name = text_at(text, mr.u32()?);
            let material = text_at(text, mr.u32()?);
            let first_vertex = mr.u32()?;
            let nv = mr.u32()?;
            let first_triangle = mr.u32()?;
            let nt = mr.u32()?;
            if (first_vertex as usize).saturating_add(nv as usize) > num_vertexes {
                return Err(Error::new(FMT, "mesh vertex range out of bounds"));
            }
            if (first_triangle as usize).saturating_add(nt as usize) > num_triangles {
                return Err(Error::new(FMT, "mesh triangle range out of bounds"));
            }
            meshes.push(IqmMesh {
                name,
                material,
                first_vertex,
                num_vertexes: nv,
                first_triangle,
                num_triangles: nt,
            });
        }

        // Vertex arrays (20 bytes each)
        let va_bytes = slice(
            ofs_vertexarrays,
            num_vertexarrays.checked_mul(20).ok_or_else(|| Error::new(FMT, "vertexarray overflow"))?,
            "vertexarrays",
        )?;
        let mut vr = Reader::new(va_bytes, FMT);
        let mut positions = Vec::new();
        let mut texcoords = Vec::new();
        let mut normals = Vec::new();
        let mut blend_indexes = Vec::new();
        let mut blend_weights = Vec::new();

        for _ in 0..num_vertexarrays {
            let kind = vr.u32()?;
            let _vflags = vr.u32()?;
            let format = vr.u32()?;
            let size = vr.u32()? as usize;
            let offset = vr.u32()? as usize;
            let Some(esize) = format_size(format) else {
                continue;
            };
            if size == 0 || size > 16 {
                return Err(Error::new(FMT, format!("invalid vertex array size {size}")));
            }
            let total = num_vertexes
                .checked_mul(size)
                .and_then(|n| n.checked_mul(esize))
                .ok_or_else(|| Error::new(FMT, "vertex array byte length overflow"))?;
            let raw = slice(offset, total, "vertex data")?;
            let mut dr = Reader::new(raw, FMT);
            match kind {
                IQM_POSITION if size >= 3 && positions.is_empty() => {
                    positions.reserve(num_vertexes);
                    for _ in 0..num_vertexes {
                        let x = read_f32_elem(&mut dr, format)?;
                        let y = read_f32_elem(&mut dr, format)?;
                        let z = read_f32_elem(&mut dr, format)?;
                        for _ in 3..size {
                            let _ = read_f32_elem(&mut dr, format)?;
                        }
                        positions.push([x, y, z]);
                    }
                }
                IQM_TEXCOORD if size >= 2 && texcoords.is_empty() => {
                    texcoords.reserve(num_vertexes);
                    for _ in 0..num_vertexes {
                        let u = read_f32_elem(&mut dr, format)?;
                        let v = read_f32_elem(&mut dr, format)?;
                        for _ in 2..size {
                            let _ = read_f32_elem(&mut dr, format)?;
                        }
                        texcoords.push([u, v]);
                    }
                }
                IQM_NORMAL if size >= 3 && normals.is_empty() => {
                    normals.reserve(num_vertexes);
                    for _ in 0..num_vertexes {
                        let x = read_f32_elem(&mut dr, format)?;
                        let y = read_f32_elem(&mut dr, format)?;
                        let z = read_f32_elem(&mut dr, format)?;
                        for _ in 3..size {
                            let _ = read_f32_elem(&mut dr, format)?;
                        }
                        normals.push([x, y, z]);
                    }
                }
                IQM_BLENDINDEXES if size >= 4 && blend_indexes.is_empty() => {
                    blend_indexes.reserve(num_vertexes);
                    for _ in 0..num_vertexes {
                        let idx = [
                            read_u8_index(&mut dr, format)?,
                            read_u8_index(&mut dr, format)?,
                            read_u8_index(&mut dr, format)?,
                            read_u8_index(&mut dr, format)?,
                        ];
                        for _ in 4..size {
                            let _ = read_u8_index(&mut dr, format)?;
                        }
                        blend_indexes.push(idx);
                    }
                }
                IQM_BLENDWEIGHTS if size >= 4 && blend_weights.is_empty() => {
                    blend_weights.reserve(num_vertexes);
                    for _ in 0..num_vertexes {
                        let wt = [
                            read_u8_norm(&mut dr, format)?,
                            read_u8_norm(&mut dr, format)?,
                            read_u8_norm(&mut dr, format)?,
                            read_u8_norm(&mut dr, format)?,
                        ];
                        for _ in 4..size {
                            let _ = read_u8_norm(&mut dr, format)?;
                        }
                        blend_weights.push(wt);
                    }
                }
                _ => {}
            }
        }

        if positions.len() != num_vertexes {
            positions.resize(num_vertexes, [0.0; 3]);
        }
        if texcoords.len() != num_vertexes {
            texcoords.resize(num_vertexes, [0.0; 2]);
        }
        if normals.len() != num_vertexes {
            normals.resize(num_vertexes, [0.0, 0.0, 1.0]);
        }
        if blend_indexes.len() != num_vertexes {
            blend_indexes.resize(num_vertexes, [0; 4]);
        }
        if blend_weights.len() != num_vertexes {
            blend_weights.resize(num_vertexes, [255, 0, 0, 0]);
        }

        // Triangles (12 bytes each)
        let tri_bytes = slice(
            ofs_triangles,
            num_triangles.checked_mul(12).ok_or_else(|| Error::new(FMT, "triangles overflow"))?,
            "triangles",
        )?;
        let mut tr = Reader::new(tri_bytes, FMT);
        let mut triangles = Vec::with_capacity(num_triangles);
        for _ in 0..num_triangles {
            let tri = [tr.u32()?, tr.u32()?, tr.u32()?];
            if tri.iter().any(|&v| v as usize >= num_vertexes) {
                return Err(Error::new(FMT, "triangle vertex index out of bounds"));
            }
            triangles.push(tri);
        }

        // Joints (48 bytes each)
        let joint_bytes = slice(
            ofs_joints,
            num_joints.checked_mul(48).ok_or_else(|| Error::new(FMT, "joints overflow"))?,
            "joints",
        )?;
        let mut jr = Reader::new(joint_bytes, FMT);
        let mut joints = Vec::with_capacity(num_joints);
        for _ in 0..num_joints {
            let name = text_at(text, jr.u32()?);
            let parent = jr.i32()?;
            let translate = jr.vec3()?;
            let rotate = [jr.f32()?, jr.f32()?, jr.f32()?, jr.f32()?];
            let scale = jr.vec3()?;
            joints.push(IqmJoint { name, parent, translate, rotate, scale });
        }

        // Poses (88 bytes each)
        let pose_bytes = slice(
            ofs_poses,
            num_poses.checked_mul(88).ok_or_else(|| Error::new(FMT, "poses overflow"))?,
            "poses",
        )?;
        let mut pr = Reader::new(pose_bytes, FMT);
        let mut poses = Vec::with_capacity(num_poses);
        for _ in 0..num_poses {
            let parent = pr.i32()?;
            let mask = pr.u32()?;
            let mut channel_offset = [0.0f32; 10];
            for c in &mut channel_offset {
                *c = pr.f32()?;
            }
            let mut channel_scale = [0.0f32; 10];
            for c in &mut channel_scale {
                *c = pr.f32()?;
            }
            poses.push(IqmPose { parent, mask, channel_offset, channel_scale });
        }

        // Anims (20 bytes each)
        let anim_bytes = slice(
            ofs_anims,
            num_anims.checked_mul(20).ok_or_else(|| Error::new(FMT, "anims overflow"))?,
            "anims",
        )?;
        let mut ar = Reader::new(anim_bytes, FMT);
        let mut anims = Vec::with_capacity(num_anims);
        for _ in 0..num_anims {
            anims.push(IqmAnim {
                name: text_at(text, ar.u32()?),
                first_frame: ar.u32()?,
                num_frames: ar.u32()?,
                framerate: ar.f32()?,
                flags: ar.u32()?,
            });
        }

        // Frame data (num_frames * num_framechannels * 2 bytes)
        let frame_vals = (num_frames as usize)
            .checked_mul(num_framechannels as usize)
            .ok_or_else(|| Error::new(FMT, "frames overflow"))?;
        let frame_bytes = slice(
            ofs_frames,
            frame_vals.checked_mul(2).ok_or_else(|| Error::new(FMT, "frame bytes overflow"))?,
            "frames",
        )?;
        let frames: Vec<u16> = frame_bytes.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c)).collect();

        // Bounds (optional, 32 bytes per frame)
        let mut bounds = Vec::new();
        if ofs_bounds != 0 && num_frames > 0 {
            let b_len = (num_frames as usize).checked_mul(32).ok_or_else(|| Error::new(FMT, "bounds overflow"))?;
            if let Ok(b_bytes) = slice(ofs_bounds, b_len, "bounds") {
                let mut br = Reader::new(b_bytes, FMT);
                bounds.reserve(num_frames as usize);
                for _ in 0..num_frames {
                    bounds.push(IqmBounds { bbmin: br.vec3()?, bbmax: br.vec3()?, xyradius: br.f32()?, radius: br.f32()? });
                }
            }
        }

        Ok(Iqm {
            flags, meshes, positions, texcoords, normals, blend_indexes, blend_weights,
            triangles, joints, poses, anims, num_frames, num_framechannels, frames, bounds,
        })
    }
}
