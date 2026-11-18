//! MD5 skeletal models as shipped by the 2021 Quake re-release
//! (`progs/*.md5mesh` + `progs/*.md5anim`), port of vkQuake's
//! Mod_LoadMD5MeshModel. Rather than skinning on the GPU, every animation
//! frame is skinned once here and baked into an [`Md3`] with one pose per
//! frame, so the regular MD3 render path (lerping, lighting, skins) draws it.
//! Entity frame N shows md5anim frame N, as in vkQuake.

use crate::md3::{Md3, Md3Frame, Md3Surface, Md3Vert, XYZ_SCALE};
use crate::{Error, Result};
use qcore::math::Vec3;

const FMT: &str = "md5";

/// Row-major 3x4 affine transform.
type Mat = [f32; 12];

fn err(msg: impl Into<String>) -> Error {
    Error::new(FMT, msg)
}

/// COM_Parse-style tokens: quoted strings, `{}()` singly, `//` comments.
struct Tokens<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> Tokens<'a> {
    fn new(text: &'a [u8]) -> Self {
        Tokens { s: text, i: 0 }
    }
    fn next(&mut self) -> Option<&'a str> {
        let s = self.s;
        loop {
            while self.i < s.len() && s[self.i].is_ascii_whitespace() {
                self.i += 1;
            }
            if s[self.i..].starts_with(b"//") {
                while self.i < s.len() && s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            break;
        }
        if self.i >= s.len() {
            return None;
        }
        let start = self.i;
        let tok = if s[start] == b'"' {
            self.i += 1;
            while self.i < s.len() && s[self.i] != b'"' {
                self.i += 1;
            }
            let t = &s[start + 1..self.i];
            self.i = (self.i + 1).min(s.len());
            t
        } else if b"{}()".contains(&s[start]) {
            self.i += 1;
            &s[start..self.i]
        } else {
            while self.i < s.len() && !s[self.i].is_ascii_whitespace() && !b"{}()\"".contains(&s[self.i]) {
                self.i += 1;
            }
            &s[start..self.i]
        };
        std::str::from_utf8(tok).ok()
    }
    fn peek(&mut self) -> Option<&'a str> {
        let i = self.i;
        let t = self.next();
        self.i = i;
        t
    }
    fn expect(&mut self, want: &str) -> Result<()> {
        match self.next() {
            Some(t) if t == want => Ok(()),
            t => Err(err(format!("expected \"{want}\", found \"{}\"", t.unwrap_or("EOF")))),
        }
    }
    fn word(&mut self) -> Result<&'a str> {
        self.next().ok_or_else(|| err("unexpected end of file"))
    }
    fn float(&mut self) -> Result<f32> {
        let t = self.word()?;
        t.parse().map_err(|_| err(format!("bad number \"{t}\"")))
    }
    fn int(&mut self) -> Result<i64> {
        let t = self.word()?;
        t.parse().map_err(|_| err(format!("bad integer \"{t}\"")))
    }
    fn count(&mut self, max: usize) -> Result<usize> {
        let n = self.int()?;
        if n < 0 || n as usize > max {
            return Err(err(format!("count {n} out of range")));
        }
        Ok(n as usize)
    }
    fn vec3(&mut self) -> Result<[f32; 3]> {
        self.expect("(")?;
        let v = [self.float()?, self.float()?, self.float()?];
        self.expect(")")?;
        Ok(v)
    }
    /// Header: `MD5Version 10` and an optional `commandline "..."`.
    fn header(&mut self) -> Result<()> {
        self.expect("MD5Version")?;
        self.expect("10")?;
        if self.peek() == Some("commandline") {
            self.next();
            self.next();
        }
        Ok(())
    }
}

/// Quaternion with the implied negative w the re-release exporter uses.
fn quat(q: [f32; 3]) -> [f32; 4] {
    let w = (1.0 - (q[0] * q[0] + q[1] * q[1] + q[2] * q[2])).max(0.0);
    [q[0], q[1], q[2], -w.sqrt()]
}

/// GenMatrixPosQuat4Scale with unit scale.
fn pos_quat(p: [f32; 3], q: [f32; 4]) -> Mat {
    let (x2, y2, z2) = (q[0] * 2.0, q[1] * 2.0, q[2] * 2.0);
    let (xx, xy, xz) = (q[0] * x2, q[0] * y2, q[0] * z2);
    let (yy, yz, zz) = (q[1] * y2, q[1] * z2, q[2] * z2);
    let (xw, yw, zw) = (q[3] * x2, q[3] * y2, q[3] * z2);
    [
        1.0 - (yy + zz), xy - zw, xz + yw, p[0],
        xy + zw, 1.0 - (xx + zz), yz - xw, p[1],
        xz - yw, yz + xw, 1.0 - (xx + yy), p[2],
    ]
}

/// a * b (apply b first).
fn concat(a: &Mat, b: &Mat) -> Mat {
    let mut o = [0.0; 12];
    for r in 0..3 {
        for c in 0..4 {
            o[r * 4 + c] = a[r * 4] * b[c] + a[r * 4 + 1] * b[4 + c] + a[r * 4 + 2] * b[8 + c];
        }
        o[r * 4 + 3] += a[r * 4 + 3];
    }
    o
}

/// Inverse of a rigid transform.
fn invert(m: &Mat) -> Mat {
    let r = [m[0], m[4], m[8], m[1], m[5], m[9], m[2], m[6], m[10]];
    let t = [m[3], m[7], m[11]];
    let mut o = [0.0; 12];
    for i in 0..3 {
        o[i * 4] = r[i * 3];
        o[i * 4 + 1] = r[i * 3 + 1];
        o[i * 4 + 2] = r[i * 3 + 2];
        o[i * 4 + 3] = -(r[i * 3] * t[0] + r[i * 3 + 1] * t[1] + r[i * 3 + 2] * t[2]);
    }
    o
}

/// `m` applied to `p`, with the translation scaled by `w` (weights store
/// their position premultiplied by the bias).
fn transform(m: &Mat, p: [f32; 3], w: f32) -> [f32; 3] {
    std::array::from_fn(|r| m[r * 4] * p[0] + m[r * 4 + 1] * p[1] + m[r * 4 + 2] * p[2] + m[r * 4 + 3] * w)
}

struct Joint {
    name: String,
    parent: i64,
    /// Absolute bind pose.
    bind: Mat,
}

struct Mesh {
    shader: String,
    /// (st, first weight, weight count)
    verts: Vec<([f32; 2], usize, usize)>,
    tris: Vec<[u32; 3]>,
    /// (joint, bias, bias-scaled joint-local position)
    weights: Vec<(usize, f32, [f32; 3])>,
}

/// A parsed MD5 model: per-mesh shader names plus the baked [`Md3`].
pub struct Md5 {
    /// The `shader` of each surface, in surface order (the re-release
    /// names skins `<dir>/<shader>_SS_FF`).
    pub shaders: Vec<String>,
    pub md3: Md3,
}

fn parse_mesh(text: &[u8]) -> Result<(Vec<Joint>, Vec<Mesh>)> {
    let mut t = Tokens::new(text);
    t.header()?;
    t.expect("numJoints")?;
    let num_joints = t.count(4096)?;
    t.expect("numMeshes")?;
    let num_meshes = t.count(256)?;
    t.expect("joints")?;
    t.expect("{")?;
    let mut joints = Vec::with_capacity(num_joints);
    for j in 0..num_joints {
        let name = t.word()?.to_string();
        let parent = t.int()?;
        // Parents may come later in the list (MG3's ogre_rocket parents
        // its root to an armature joint at the end); vkQuake only checks
        // the range here and resolves the order against the animation.
        if parent < -1 || parent >= num_joints as i64 || parent == j as i64 {
            return Err(err(format!("joint \"{name}\" has bad parent {parent}")));
        }
        let p = t.vec3()?;
        let q = quat(t.vec3()?);
        joints.push(Joint { name, parent, bind: pos_quat(p, q) });
    }
    t.expect("}")?;
    let mut meshes = Vec::with_capacity(num_meshes);
    for _ in 0..num_meshes {
        t.expect("mesh")?;
        t.expect("{")?;
        t.expect("shader")?;
        let shader = t.word()?.to_string();
        t.expect("numverts")?;
        let mut verts = vec![([0.0; 2], 0, 0); t.count(1 << 20)?];
        while t.peek() == Some("vert") {
            t.next();
            let i = t.count(verts.len().saturating_sub(1))?;
            t.expect("(")?;
            let st = [t.float()?, t.float()?];
            t.expect(")")?;
            verts[i] = (st, t.count(1 << 24)?, t.count(64)?);
        }
        t.expect("numtris")?;
        let mut tris = vec![[0; 3]; t.count(1 << 21)?];
        while t.peek() == Some("tri") {
            t.next();
            let i = t.count(tris.len().saturating_sub(1))?;
            let last = verts.len().saturating_sub(1);
            tris[i] = [t.count(last)? as u32, t.count(last)? as u32, t.count(last)? as u32];
        }
        t.expect("numweights")?;
        let mut weights = vec![(0, 0.0, [0.0; 3]); t.count(1 << 24)?];
        while t.peek() == Some("weight") {
            t.next();
            let i = t.count(weights.len().saturating_sub(1))?;
            let joint = t.count(num_joints.saturating_sub(1))?;
            let bias = t.float()?;
            let p = t.vec3()?;
            weights[i] = (joint, bias, p.map(|x| x * bias));
        }
        t.expect("}")?;
        if verts.iter().any(|v| v.1 + v.2 > weights.len()) {
            return Err(err("vertex weight out of range"));
        }
        meshes.push(Mesh { shader, verts, tris, weights });
    }
    Ok((joints, meshes))
}

/// MD3 spherical normal encoding (inverse of alias.vert's decode).
fn encode_normal(n: [f32; 3]) -> [u8; 2] {
    const K: f32 = 255.0 / std::f32::consts::TAU;
    let lat = n[2].clamp(-1.0, 1.0).acos() * K;
    let lng = n[1].atan2(n[0]).rem_euclid(std::f32::consts::TAU) * K;
    [lat.round().clamp(0.0, 255.0) as u8, (lng.round() as u32 % 255) as u8]
}

impl Md5 {
    /// Parse `mesh` (.md5mesh text) with its optional `anim` (.md5anim).
    pub fn parse(mesh: &[u8], anim: Option<&[u8]>) -> Result<Md5> {
        let (joints, meshes) = parse_mesh(mesh)?;
        if meshes.is_empty() {
            return Err(err("model has no meshes"));
        }
        let poses = anim::parse_anim(anim, &joints)?;
        let mut bounds = vec![[Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)]; poses.len()];
        let mut surfaces = Vec::with_capacity(meshes.len());
        for m in &meshes {
            let n = m.verts.len();
            let mut verts = Vec::with_capacity(poses.len() * n);
            for (f, pose) in poses.iter().enumerate() {
                let pos: Vec<[f32; 3]> = m
                    .verts
                    .iter()
                    .map(|&(_, first, count)| {
                        m.weights[first..first + count].iter().fold([0.0; 3], |acc, &(j, bias, p)| {
                            let v = transform(&pose[j], p, bias);
                            [acc[0] + v[0], acc[1] + v[1], acc[2] + v[2]]
                        })
                    })
                    .collect();
                // Smooth normals from this frame's triangles (clockwise
                // front faces, as in Quake).
                let mut normals = vec![Vec3::ZERO; n];
                for tri in &m.tris {
                    let [a, b, c] = tri.map(|i| Vec3::from(pos[i as usize]));
                    let face = (c - a).cross(b - a);
                    for &i in tri {
                        normals[i as usize] += face;
                    }
                }
                for (p, nrm) in pos.iter().zip(&normals) {
                    let nrm = if nrm.length() > 0.0 { nrm.normalize() } else { Vec3::new(0.0, 0.0, 1.0) };
                    let q = p.map(|x| (x / XYZ_SCALE).round().clamp(-32768.0, 32767.0) as i16);
                    verts.push(Md3Vert { xyz: q, normal: encode_normal(nrm.to_array()) });
                    let v = Vec3::from(*p);
                    bounds[f][0] = bounds[f][0].min(v);
                    bounds[f][1] = bounds[f][1].max(v);
                }
            }
            surfaces.push(Md3Surface {
                name: m.shader.clone(),
                flags: 0,
                shaders: Vec::new(),
                st: m.verts.iter().map(|v| v.0).collect(),
                triangles: m.tris.clone(),
                verts,
                num_verts: n,
            });
        }
        let frames = bounds
            .iter()
            .enumerate()
            .map(|(i, b)| Md3Frame {
                bounds: [b[0].to_array(), b[1].to_array()],
                local_origin: [0.0; 3],
                radius: b[0].length().max(b[1].length()),
                name: format!("frame{i}"),
            })
            .collect();
        Ok(Md5 {
            shaders: meshes.iter().map(|m| m.shader.clone()).collect(),
            md3: Md3 { name: String::new(), flags: 0, frames, surfaces },
        })
    }
}

mod anim;
#[cfg(test)]
mod tests;
