//! Quake 3 alias model (.md3, "IDP3" version 15) and `.skin` manifest parser.

use std::collections::HashSet;

use crate::{Reader, Result, Vec3, check_count};

const FMT: &str = "md3";

/// Fixed coordinate scale of MD3 16-bit vertex positions (`1.0 / 64.0`).
pub const XYZ_SCALE: f32 = 1.0 / 64.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Md3Frame {
    pub bounds: [Vec3; 2],
    pub local_origin: Vec3,
    pub radius: f32,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Md3Vert {
    pub xyz: [i16; 3],
    /// Spherical normal `(lat, lng)` in `[0, 255]` (`* 2*PI / 255`).
    pub normal: [u8; 2],
}

#[derive(Debug, Clone, PartialEq)]
pub struct Md3Surface {
    pub name: String,
    pub flags: i32,
    pub shaders: Vec<String>,
    pub st: Vec<[f32; 2]>,
    pub triangles: Vec<[u32; 3]>,
    /// Pose-major vertex array: `num_frames * num_verts`.
    pub verts: Vec<Md3Vert>,
    pub num_verts: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Md3 {
    pub name: String,
    pub flags: i32,
    pub frames: Vec<Md3Frame>,
    pub surfaces: Vec<Md3Surface>,
}

fn read_offset(r: &mut Reader, what: &str) -> Result<usize> {
    let ofs = r.i32()?;
    if ofs < 0 {
        return Err(r.err(format!("negative {what} offset {ofs}")));
    }
    let u = ofs as usize;
    if u > r.data.len() {
        return Err(r.err(format!("{what} offset {u} past end {}", r.data.len())));
    }
    Ok(u)
}

impl Md3 {
    pub fn parse(data: &[u8]) -> Result<Md3> {
        let mut r = Reader::new(data, FMT);
        if &r.arr::<4>()? != b"IDP3" {
            return Err(r.err("bad magic"));
        }
        let version = r.i32()?;
        if version != 15 {
            return Err(r.err(format!("unsupported version {version}")));
        }
        let name = r.name::<64>()?;
        let flags = r.i32()?;
        let num_frames = r.count("frame", 56)?;
        let _num_tags = r.i32()?;
        let num_surfaces = r.count("surface", 108)?;
        let _num_skins = r.i32()?;
        if num_frames == 0 {
            return Err(r.err("model has no frames"));
        }
        if num_surfaces == 0 {
            return Err(r.err("model has no surfaces"));
        }
        let ofs_frames = read_offset(&mut r, "frames")?;
        let _ofs_tags = r.i32()?;
        let ofs_surfaces = read_offset(&mut r, "surfaces")?;
        let _ofs_end = r.i32()?;

        r.seek(ofs_frames)?;
        let mut frames = Vec::with_capacity(num_frames);
        for _ in 0..num_frames {
            frames.push(Md3Frame {
                bounds: [r.vec3()?, r.vec3()?],
                local_origin: r.vec3()?,
                radius: r.f32()?,
                name: r.name::<16>()?,
            });
        }

        let mut surfaces = Vec::with_capacity(num_surfaces);
        let mut sofs = ofs_surfaces;
        for _ in 0..num_surfaces {
            let sdata = data
                .get(sofs..)
                .ok_or_else(|| r.err(format!("surface offset {sofs} out of bounds")))?;
            let mut sr = Reader::new(sdata, FMT);
            let _ident = sr.arr::<4>()?;
            let sname = sr.name::<64>()?;
            let sflags = sr.i32()?;
            let snum_frames = sr.count("surface frame", 0)?;
            if snum_frames < num_frames {
                return Err(sr.err(format!(
                    "surface frame count {snum_frames} < header {num_frames}"
                )));
            }
            let num_shaders = sr.count("shader", 68)?;
            let num_verts = sr.count("vertex", 8)?;
            let num_tris = sr.count("triangle", 12)?;
            let ofs_tris = read_offset(&mut sr, "surface triangles")?;
            let ofs_shaders = read_offset(&mut sr, "surface shaders")?;
            let ofs_st = read_offset(&mut sr, "surface st")?;
            let ofs_xyz = read_offset(&mut sr, "surface xyz")?;
            let ofs_end = sr.i32()?;
            if ofs_end <= 0 {
                return Err(sr.err(format!("invalid surface ofs_end {ofs_end}")));
            }

            sr.seek(ofs_shaders)?;
            let mut shaders = Vec::with_capacity(num_shaders);
            for _ in 0..num_shaders {
                shaders.push(sr.name::<64>()?);
                let _idx = sr.i32()?;
            }

            sr.seek(ofs_tris)?;
            let mut triangles = Vec::with_capacity(num_tris);
            for _ in 0..num_tris {
                let tri = [sr.i32()?, sr.i32()?, sr.i32()?];
                if tri.iter().any(|&v| v < 0 || v as usize >= num_verts) {
                    return Err(sr.err("triangle vertex index out of range"));
                }
                triangles.push([tri[0] as u32, tri[1] as u32, tri[2] as u32]);
            }

            sr.seek(ofs_st)?;
            let mut st = Vec::with_capacity(num_verts);
            for _ in 0..num_verts {
                st.push([sr.f32()?, sr.f32()?]);
            }

            let total_verts = num_frames
                .checked_mul(num_verts)
                .ok_or_else(|| sr.err("vertex count overflow"))?;
            let _ = check_count(&sr, i32::try_from(total_verts).unwrap_or(i32::MAX), "surface verts", 8)?;
            sr.seek(ofs_xyz)?;
            let mut verts = Vec::with_capacity(total_verts);
            for _ in 0..total_verts {
                let xyz = [sr.i16()?, sr.i16()?, sr.i16()?];
                let normal = [sr.u8()?, sr.u8()?];
                verts.push(Md3Vert { xyz, normal });
            }

            surfaces.push(Md3Surface {
                name: sname,
                flags: sflags,
                shaders,
                st,
                triangles,
                verts,
                num_verts,
            });
            sofs = sofs
                .checked_add(ofs_end as usize)
                .ok_or_else(|| r.err("surface offset overflow"))?;
        }

        Ok(Md3 {
            name,
            flags,
            frames,
            surfaces,
        })
    }
}

/// Parses and validates an MD3 `.skin` file (`Mod_ValidateSkinFile` from ironwail).
/// Rejects manifests whose unique surface set does not match `surface_names`.
pub fn parse_skin(text: &str, surface_names: &[&str]) -> Option<Vec<(String, String)>> {
    let mut lines = text.lines().peekable();
    // Skip optional header line that has no comma.
    if let Some(&first) = lines.peek()
        && !first.contains(',')
    {
        lines.next();
    }

    let expected: HashSet<&str> = surface_names.iter().copied().collect();
    let mut seen: HashSet<String> = HashSet::new();
    let mut entries = Vec::new();
    for raw in lines {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let Some((mesh, path)) = line.split_once(',') else {
            continue;
        };
        let mesh = mesh.trim();
        let path = path.trim();
        if !expected.contains(mesh) {
            return None;
        }
        seen.insert(mesh.to_string());
        entries.push((mesh.to_string(), path.to_string()));
    }
    (seen.len() == expected.len()).then_some(entries)
}
