//! BSP29 / BSP2 / 2PSB map parsing. All variants normalize into the same structs.

use crate::{Error, Reader, Result, Vec3, cstr};

const FMT: &str = "bsp";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BspVersion {
    Bsp29,
    /// "2PSB": BSP29 with 32-bit indices, 16-bit bounds.
    Bsp2Rmq,
    /// "BSP2": 32-bit indices, float bounds.
    Bsp2,
    /// Quake64 re-release: BSP29 layout, `miptex64_t` texture headers (extra
    /// `shift`), 16-bit RGB lightmap samples.
    Quake64,
}

/// `('Q' << 24) | ('6' << 16) | ('4' << 8) | ' '`
pub const BSPVERSION_QUAKE64: i32 = 0x5136_3420;

pub const LUMP_ENTITIES: usize = 0;
pub const LUMP_PLANES: usize = 1;
pub const LUMP_TEXTURES: usize = 2;
pub const LUMP_VERTEXES: usize = 3;
pub const LUMP_VISIBILITY: usize = 4;
pub const LUMP_NODES: usize = 5;
pub const LUMP_TEXINFO: usize = 6;
pub const LUMP_FACES: usize = 7;
pub const LUMP_LIGHTING: usize = 8;
pub const LUMP_CLIPNODES: usize = 9;
pub const LUMP_LEAFS: usize = 10;
pub const LUMP_MARKSURFACES: usize = 11;
pub const LUMP_EDGES: usize = 12;
pub const LUMP_SURFEDGES: usize = 13;
pub const LUMP_MODELS: usize = 14;
pub const HEADER_LUMPS: usize = 15;
pub const MAX_MAP_HULLS: usize = 4;

pub const CONTENTS_EMPTY: i32 = -1;
pub const CONTENTS_SOLID: i32 = -2;
pub const CONTENTS_WATER: i32 = -3;
pub const CONTENTS_SLIME: i32 = -4;
pub const CONTENTS_LAVA: i32 = -5;
pub const CONTENTS_SKY: i32 = -6;

pub const TEX_SPECIAL: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub normal: Vec3,
    pub dist: f32,
    pub kind: i32,
}

/// Node child: `>= 0` node index, `< 0` leaf index encoded as `-(leaf + 1)`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Node {
    pub plane: i32,
    pub children: [i32; 2],
    pub mins: Vec3,
    pub maxs: Vec3,
    pub first_face: u32,
    pub num_faces: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Leaf {
    pub contents: i32,
    /// Offset into visdata, -1 = none.
    pub visofs: i32,
    pub mins: Vec3,
    pub maxs: Vec3,
    pub first_marksurface: u32,
    pub num_marksurfaces: u32,
    pub ambient_level: [u8; 4],
}

/// Clipnode child: `>= 0` clipnode index, `< 0` contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClipNode {
    pub plane: i32,
    pub children: [i32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexInfo {
    pub vecs: [[f32; 4]; 2],
    pub miptex: i32,
    pub flags: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Face {
    pub plane: u32,
    pub side: u32,
    pub first_edge: u32,
    pub num_edges: u32,
    pub texinfo: u32,
    pub styles: [u8; 4],
    /// Byte offset into lighting, -1 = none.
    pub lightofs: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Model {
    pub mins: Vec3,
    pub maxs: Vec3,
    pub origin: Vec3,
    pub headnode: [i32; MAX_MAP_HULLS],
    pub visleafs: i32,
    pub first_face: i32,
    pub num_faces: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureKind {
    Normal,
    /// Name starts with "sky".
    Sky,
    /// Name starts with '*' (water/slime/lava/tele).
    Liquid,
    /// Name starts with '{' (alpha-tested, index 255 transparent).
    Fence,
}

impl TextureKind {
    pub fn from_name(name: &str) -> Self {
        let b = name.as_bytes();
        if b.len() >= 3 && b[..3].eq_ignore_ascii_case(b"sky") {
            TextureKind::Sky
        } else if b.first() == Some(&b'*') {
            TextureKind::Liquid
        } else if b.first() == Some(&b'{') {
            TextureKind::Fence
        } else {
            TextureKind::Normal
        }
    }
}

/// Embedded mip texture. `mips` may be empty if the texture is external (offsets 0).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MipTex {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub kind: TextureKind,
    /// Up to 4 mip levels of 8-bit palette indices (level i is (w>>i)*(h>>i)).
    pub mips: Vec<Vec<u8>>,
    /// Quake64 texture shift: texture coordinates are divided by `2 * shift`
    /// when > 0. Always 0 elsewhere.
    pub shift: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Bsp {
    pub version: Option<BspVersion>,
    pub entities: String,
    pub planes: Vec<Plane>,
    /// One entry per miptex slot; `None` for missing (-1 offset) textures.
    pub textures: Vec<Option<MipTex>>,
    pub vertexes: Vec<Vec3>,
    pub visdata: Vec<u8>,
    pub nodes: Vec<Node>,
    pub texinfo: Vec<TexInfo>,
    pub faces: Vec<Face>,
    /// Mono samples, one byte each; `Face::lightofs` indexes this.
    pub lighting: Vec<u8>,
    /// Embedded RGB samples (3 bytes per mono sample), like a `.lit` file.
    /// Only Quake64 maps carry these.
    pub rgb_lighting: Option<Vec<u8>>,
    pub clipnodes: Vec<ClipNode>,
    pub leafs: Vec<Leaf>,
    pub marksurfaces: Vec<u32>,
    pub edges: Vec<[u32; 2]>,
    pub surfedges: Vec<i32>,
    pub models: Vec<Model>,
}

fn lump_records<'a, T>(
    data: &'a [u8],
    name: &str,
    size: usize,
    mut f: impl FnMut(&mut Reader<'a>) -> Result<T>,
) -> Result<Vec<T>> {
    if !data.len().is_multiple_of(size) {
        return Err(Error::new(
            FMT,
            format!("{name} lump size {} not multiple of {size}", data.len()),
        ));
    }
    let mut r = Reader::new(data, FMT);
    let n = data.len() / size;
    let mut v = Vec::with_capacity(n);
    for _ in 0..n {
        v.push(f(&mut r)?);
    }
    Ok(v)
}

impl Bsp {
    pub fn parse(data: &[u8]) -> Result<Bsp> {
        let mut r = Reader::new(data, FMT);
        let ident = r.arr::<4>()?;
        let version = match &ident {
            b"BSP2" => BspVersion::Bsp2,
            b"2PSB" => BspVersion::Bsp2Rmq,
            _ => match i32::from_le_bytes(ident) {
                29 => BspVersion::Bsp29,
                BSPVERSION_QUAKE64 => BspVersion::Quake64,
                v => return Err(r.err(format!("unsupported version {v}"))),
            },
        };
        let mut lumps = [&data[..0]; HEADER_LUMPS];
        for (i, l) in lumps.iter_mut().enumerate() {
            let ofs = r.i32()?;
            let len = r.i32()?;
            if ofs < 0 || len < 0 {
                return Err(r.err(format!("lump {i} has negative offset/length")));
            }
            let (ofs, len) = (ofs as usize, len as usize);
            *l = data
                .get(ofs..ofs.saturating_add(len))
                .ok_or_else(|| r.err(format!("lump {i} ({ofs}+{len}) out of bounds")))?;
        }
        let q64 = version == BspVersion::Quake64;
        let wide = !matches!(version, BspVersion::Bsp29 | BspVersion::Quake64);
        let fbounds = version == BspVersion::Bsp2;

        let planes = lump_records(lumps[LUMP_PLANES], "planes", 20, |r| {
            Ok(Plane {
                normal: r.vec3()?,
                dist: r.f32()?,
                kind: r.i32()?,
            })
        })?;
        let vertexes = lump_records(lumps[LUMP_VERTEXES], "vertexes", 12, |r| r.vec3())?;
        let texinfo = lump_records(lumps[LUMP_TEXINFO], "texinfo", 40, |r| {
            let mut vecs = [[0.0; 4]; 2];
            for v in vecs.iter_mut().flatten() {
                *v = r.f32()?;
            }
            Ok(TexInfo {
                vecs,
                miptex: r.i32()?,
                flags: r.i32()?,
            })
        })?;
        let surfedges = lump_records(lumps[LUMP_SURFEDGES], "surfedges", 4, |r| r.i32())?;
        let models = lump_records(lumps[LUMP_MODELS], "models", 64, |r| {
            let mins = r.vec3()?;
            let maxs = r.vec3()?;
            let origin = r.vec3()?;
            let mut headnode = [0; MAX_MAP_HULLS];
            for h in &mut headnode {
                *h = r.i32()?;
            }
            Ok(Model {
                mins,
                maxs,
                origin,
                headnode,
                visleafs: r.i32()?,
                first_face: r.i32()?,
                num_faces: r.i32()?,
            })
        })?;

        let (node_size, leaf_size, face_size, clip_size, mark_size, edge_size) = match version {
            BspVersion::Bsp29 | BspVersion::Quake64 => (24, 28, 20, 8, 2, 4),
            BspVersion::Bsp2Rmq => (32, 32, 28, 12, 4, 8),
            BspVersion::Bsp2 => (44, 44, 28, 12, 4, 8),
        };
        let num_nodes = lumps[LUMP_NODES].len() / node_size;
        let num_clip = lumps[LUMP_CLIPNODES].len() / clip_size;

        let nodes = lump_records(lumps[LUMP_NODES], "nodes", node_size, |r| {
            let plane = r.i32()?;
            let children = if wide {
                [r.i32()?, r.i32()?]
            } else {
                let fix = |c: u16| {
                    if (c as usize) < num_nodes {
                        c as i32
                    } else {
                        c as i32 - 65536
                    }
                };
                [fix(r.u16()?), fix(r.u16()?)]
            };
            let (mins, maxs) = if fbounds {
                (r.vec3()?, r.vec3()?)
            } else {
                (r.vec3_i16()?, r.vec3_i16()?)
            };
            let (first_face, num_faces) = if wide {
                (r.u32()?, r.u32()?)
            } else {
                (r.u16()? as u32, r.u16()? as u32)
            };
            Ok(Node {
                plane,
                children,
                mins,
                maxs,
                first_face,
                num_faces,
            })
        })?;
        let leafs = lump_records(lumps[LUMP_LEAFS], "leafs", leaf_size, |r| {
            let contents = r.i32()?;
            let visofs = r.i32()?;
            let (mins, maxs) = if fbounds {
                (r.vec3()?, r.vec3()?)
            } else {
                (r.vec3_i16()?, r.vec3_i16()?)
            };
            let (first_marksurface, num_marksurfaces) = if wide {
                (r.u32()?, r.u32()?)
            } else {
                (r.u16()? as u32, r.u16()? as u32)
            };
            Ok(Leaf {
                contents,
                visofs,
                mins,
                maxs,
                first_marksurface,
                num_marksurfaces,
                ambient_level: r.arr()?,
            })
        })?;
        let faces = lump_records(lumps[LUMP_FACES], "faces", face_size, |r| {
            let (plane, side, first_edge, num_edges, texinfo) = if wide {
                (r.u32()?, r.u32()?, r.u32()?, r.u32()?, r.u32()?)
            } else {
                let p = r.u16()? as u32;
                let s = r.u16()? as u32;
                let fe = r.u32()?;
                let ne = r.u16()? as u32;
                (p, s, fe, ne, r.u16()? as u32)
            };
            Ok(Face {
                plane,
                side,
                first_edge,
                num_edges,
                texinfo,
                styles: r.arr()?,
                // Q64 samples are 16 bits; offsets index the converted data.
                lightofs: match r.i32()? {
                    o if q64 && o > 0 => o / 2,
                    o => o,
                },
            })
        })?;
        let clipnodes = lump_records(lumps[LUMP_CLIPNODES], "clipnodes", clip_size, |r| {
            let plane = r.i32()?;
            let children = if wide {
                [r.i32()?, r.i32()?]
            } else {
                let fix = |c: u16| {
                    if (c as usize) < num_clip {
                        c as i32
                    } else {
                        c as i32 - 65536
                    }
                };
                [fix(r.u16()?), fix(r.u16()?)]
            };
            Ok(ClipNode { plane, children })
        })?;
        let marksurfaces =
            lump_records(lumps[LUMP_MARKSURFACES], "marksurfaces", mark_size, |r| {
                if wide { r.u32() } else { Ok(r.u16()? as u32) }
            })?;
        let edges = lump_records(lumps[LUMP_EDGES], "edges", edge_size, |r| {
            if wide {
                Ok([r.u32()?, r.u32()?])
            } else {
                Ok([r.u16()? as u32, r.u16()? as u32])
            }
        })?;

        let (lighting, rgb_lighting) = if q64 {
            let (mono, rgb) = q64_lighting(lumps[LUMP_LIGHTING]);
            (mono, Some(rgb))
        } else {
            (lumps[LUMP_LIGHTING].to_vec(), None)
        };
        Ok(Bsp {
            version: Some(version),
            entities: cstr(lumps[LUMP_ENTITIES]),
            planes,
            textures: parse_textures(lumps[LUMP_TEXTURES], q64)?,
            vertexes,
            visdata: lumps[LUMP_VISIBILITY].to_vec(),
            nodes,
            texinfo,
            faces,
            lighting,
            rgb_lighting,
            clipnodes,
            leafs,
            marksurfaces,
            edges,
            surfedges,
            models,
        })
    }

    /// Decompress the PVS row for a leaf (`visofs` from [`Leaf`]); see [`decompress_vis`].
    pub fn leaf_pvs(&self, leaf: usize) -> Vec<u8> {
        let numleafs = self
            .models
            .first()
            .map_or(0, |m| m.visleafs.max(0) as usize);
        let visofs = self.leafs.get(leaf).map_or(-1, |l| l.visofs);
        decompress_vis(&self.visdata, visofs, numleafs)
    }
}

/// Quake64 lightmaps: 16-bit samples packed RRRRRGGG GGBBBBBB (first byte
/// high bits), expanded like ironwail. Returns (mono, rgb).
fn q64_lighting(lump: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let n = lump.len() / 2;
    let mut mono = Vec::with_capacity(n);
    let mut rgb = Vec::with_capacity(n * 3);
    for &[b0, b1] in lump.as_chunks::<2>().0 {
        let c = [b0 & 0xf8, ((b0 & 0x07) << 5) + ((b1 & 0xc0) >> 5), (b1 & 0x3f) << 2];
        rgb.extend_from_slice(&c);
        mono.push(((c[0] as u32 + c[1] as u32 + c[2] as u32) / 3) as u8);
    }
    (mono, rgb)
}

fn parse_textures(lump: &[u8], q64: bool) -> Result<Vec<Option<MipTex>>> {
    if lump.is_empty() {
        return Ok(Vec::new());
    }
    let mut r = Reader::new(lump, FMT);
    let n = r.count("miptex", 4)?;
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let ofs = r.i32()?;
        if ofs < 0 {
            out.push(None);
            continue;
        }
        let mut t = Reader::new(lump, FMT);
        t.seek(ofs as usize)
            .map_err(|e| Error::new(FMT, format!("miptex {i}: {}", e.msg)))?;
        let name = t.name::<16>()?;
        let width = t.u32()?;
        let height = t.u32()?;
        let shift = if q64 { t.u32()? } else { 0 };
        let offsets = [t.u32()?, t.u32()?, t.u32()?, t.u32()?];
        if width > 16384 || height > 16384 {
            return Err(Error::new(
                FMT,
                format!("miptex {name} has bad size {width}x{height}"),
            ));
        }
        let mut mips = Vec::new();
        if offsets[0] != 0 {
            // Texel data past the end of the lump appears in the wild
            // (e.g. jam2_tronyn, qbj_grue). Like ironwail, clamp mip 0 to the
            // lump (zero-filling the rest) and derive broken smaller mips by
            // point sampling the previous level.
            for (lvl, &o) in offsets.iter().enumerate() {
                let (w, h) = ((width >> lvl) as usize, (height >> lvl) as usize);
                let sz = w * h;
                let start = ofs as usize + o as usize;
                // Quake64 stores only mip 0 (every offset points at it).
                let own = lvl == 0 || !q64;
                if let Some(px) = lump.get(start..start + sz).filter(|_| own) {
                    mips.push(px.to_vec());
                } else if let Some(prev) = mips.last() {
                    let pw = (width >> (lvl - 1)) as usize;
                    mips.push((0..sz).map(|i| prev[(i / w) * 2 * pw + (i % w) * 2]).collect());
                } else {
                    let mut px = lump.get(start.min(lump.len())..).unwrap_or(&[]).to_vec();
                    px.resize(sz, 0);
                    mips.push(px);
                }
            }
        }
        let kind = TextureKind::from_name(&name);
        out.push(Some(MipTex {
            name,
            width,
            height,
            kind,
            mips,
            shift,
        }));
    }
    Ok(out)
}

/// Decompress a run-length encoded PVS row. Returns `(numleafs + 7) / 8` bytes;
/// if `visofs < 0` or out of range, all leafs are visible (0xff).
pub fn decompress_vis(visdata: &[u8], visofs: i32, numleafs: usize) -> Vec<u8> {
    let row = numleafs.div_ceil(8);
    if visofs < 0 || visofs as usize >= visdata.len() {
        return vec![0xff; row];
    }
    let mut out = Vec::with_capacity(row);
    let mut i = visofs as usize;
    while out.len() < row {
        let Some(&b) = visdata.get(i) else { break };
        i += 1;
        if b != 0 {
            out.push(b);
            continue;
        }
        let Some(&c) = visdata.get(i) else { break };
        i += 1;
        let c = (c as usize).min(row - out.len());
        out.resize(out.len() + c, 0);
    }
    out.resize(row, 0);
    out
}

/// Parse a `.lit` colored lightmap file ("QLIT", version 1). Returns RGB triplets
/// matching the BSP lighting lump (3 bytes per mono sample).
pub fn parse_lit(data: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(data, "lit");
    if &r.arr::<4>()? != b"QLIT" {
        return Err(r.err("bad magic"));
    }
    let v = r.i32()?;
    if v != 1 {
        return Err(r.err(format!("unsupported version {v}")));
    }
    Ok(data[8..].to_vec())
}
