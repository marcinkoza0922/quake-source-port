//! The 2021 re-release's enhanced (MD5) player model: one skinned mesh
//! whose gun and axe are bound to their own `Gun` and `Axe` joints. Its
//! animation follows player.mdl frame for frame, so the MDL's frame names
//! classify it. Weapons baked for it come out as MD3s, from the enhanced
//! pickups when the game has them.

use qcore::math::Vec3;
use qformats::Mdl;
use qformats::md3::{Md3, Md3Frame, Md3Surface, Md3Vert, XYZ_SCALE};
use qformats::md5::Md5;
use crate::debris::math::apply;

use super::arms::{Arms, Part, Posed};
use super::rig::{GunFrame, Rig, Spec, bbox};

fn position(s: &Md3Surface, frame: usize, v: usize) -> Vec3 {
    Vec3::from_array(s.verts[frame * s.num_verts + v].xyz.map(|c| c as f32 * XYZ_SCALE))
}

fn decode_normal(n: [u8; 2]) -> Vec3 {
    let k = std::f32::consts::TAU / 255.0;
    let (lat, lng) = (n[0] as f32 * k, n[1] as f32 * k);
    Vec3::new(lng.cos() * lat.sin(), lng.sin() * lat.sin(), lat.cos())
}

/// MD3 spherical normal encoding (alias.vert decodes it).
fn encode_normal(n: Vec3) -> [u8; 2] {
    const K: f32 = 255.0 / std::f32::consts::TAU;
    let lat = n.z.clamp(-1.0, 1.0).acos() * K;
    let lng = n.y.atan2(n.x).rem_euclid(std::f32::consts::TAU) * K;
    [lat.round().clamp(0.0, 255.0) as u8, (lng.round() as u32 % 255) as u8]
}

/// `md3` without the triangles that are mostly on the `drop` joints.
fn without(md5: &Md5, drop: &[u16]) -> Md3 {
    let mut md3 = md5.md3.clone();
    for (s, joints) in md3.surfaces.iter_mut().zip(&md5.vert_joints) {
        s.triangles.retain(|t| t.iter().filter(|&&v| drop.contains(&joints[v as usize])).count() < 2);
    }
    md3
}

/// The rig plus the body without its gun and axe, and without just its
/// gun. `mdl` is the player.mdl the MD5 replaces (frame names).
pub fn rig(md5: &Md5, mdl: &Mdl) -> Option<(Rig, Md3, Md3)> {
    let md3 = &md5.md3;
    if md3.frames.len() != mdl.frames.len() {
        return None;
    }
    let joint = |n: &str| md5.joints.iter().position(|j| j.eq_ignore_ascii_case(n)).map(|j| j as u16);
    let (gun_joint, axe_joint) = (joint("Gun")?, joint("Axe")?);
    let gun: Vec<(usize, usize)> = md5
        .vert_joints
        .iter()
        .enumerate()
        .flat_map(|(s, joints)| joints.iter().enumerate().filter(|j| *j.1 == gun_joint).map(move |(v, _)| (s, v)))
        .collect();
    if gun.len() < 3 {
        return None;
    }
    let frames: Vec<GunFrame> = mdl
        .frames
        .iter()
        .enumerate()
        .map(|(f, mf)| {
            let name = mf.poses.first().map_or(String::new(), |p| p.name.clone());
            (Vec::new(), vec![(name, gun.iter().map(|&(s, v)| position(&md3.surfaces[s], f, v)).collect())])
        })
        .collect();
    Some((Rig::fit(frames)?, without(md5, &[gun_joint, axe_joint]), without(md5, &[gun_joint])))
}

/// The arms of a model [`rig`] accepts, by its joints.
pub fn arms(md5: &Md5) -> Option<Arms> {
    let part = |j: u16| match md5.joints.get(j as usize).map(String::as_str) {
        Some("UpperArm_L") => Part::Upper(0),
        Some("UpperArm_R") => Part::Upper(1),
        Some("LowerArm_L") => Part::Fore(0),
        Some("LowerArm_R") => Part::Fore(1),
        Some("Hand_L") => Part::Hand(0),
        Some("Hand_R") => Part::Hand(1),
        _ => Part::Body,
    };
    let parts: Vec<Part> = md5.vert_joints.iter().flatten().map(|&j| part(j)).collect();
    let mut base = 0;
    let mut tris = Vec::new();
    for s in &md5.md3.surfaces {
        tris.extend(s.triangles.iter().map(|t| t.map(|v| base + v as usize)));
        base += s.num_verts;
    }
    Arms::new(&parts, &positions(&md5.md3, 0), tris)
}

/// Removes the stock gun's muzzle flash from a body of `md5`.
pub fn drop_flash(md5: &Md5, body: &mut Md3) {
    let flame = |j: u16| md5.joints.get(j as usize).is_some_and(|n| n.ends_with("_flame"));
    for (s, joints) in body.surfaces.iter_mut().zip(&md5.vert_joints) {
        s.triangles.retain(|t| !t.iter().any(|&v| flame(joints[v as usize])));
    }
}

/// Frame `f`'s vertex positions, surface by surface.
pub fn positions(m: &Md3, f: usize) -> Vec<Vec3> {
    m.surfaces.iter().flat_map(|s| (0..s.num_verts).map(move |v| position(s, f, v))).collect()
}

/// Replaces each frame by frame `src[i]`, then by the reposed vertices.
pub fn set_frames(m: &mut Md3, src: &[usize], posed: Option<&[Posed]>) {
    let mut base = 0;
    for s in &mut m.surfaces {
        let n = s.num_verts;
        let old = s.verts.clone();
        for (i, &from) in src.iter().enumerate() {
            for v in 0..n {
                let mut vert = old[from * n + v];
                if let Some(p) = posed.map(|p| &p[i]) {
                    vert = Md3Vert {
                        xyz: p.pos[base + v].to_array().map(|c| (c / XYZ_SCALE).round().clamp(-32768.0, 32767.0) as i16),
                        normal: encode_normal(apply(&p.rot[base + v], decode_normal(vert.normal)).normalize()),
                    };
                }
                s.verts[i * n + v] = vert;
            }
        }
        base += n;
    }
    for (i, &from) in src.iter().enumerate() {
        let (lo, hi) = bbox(positions(m, i));
        let f = &mut m.frames[i];
        (f.bounds, f.radius) = ([lo.to_array(), hi.to_array()], lo.length().max(hi.length()));
        debug_assert!(from < src.len());
    }
}

/// `src`'s first frame fitted into the hand per `spec`, as an MD3 with the
/// rig's frames (first pose of each).
pub fn bake(rig: &Rig, src: &Md3, spec: &Spec) -> Option<Md3> {
    if src.frames.is_empty() {
        return None;
    }
    let mut first = Vec::new();
    let (mut pos, mut normals, mut tris) = (Vec::new(), Vec::new(), Vec::new());
    for s in &src.surfaces {
        first.push(pos.len());
        let base = pos.len();
        for v in 0..s.num_verts {
            pos.push(position(s, 0, v));
            normals.push(decode_normal(s.verts[v].normal));
        }
        tris.extend(s.triangles.iter().map(|t| t.map(|v| base + v as usize)));
    }
    let (keep, placed) = rig.place(&pos, tris, spec)?;
    let posed = rig.posed(&placed, &normals);
    let surfaces = src
        .surfaces
        .iter()
        .zip(&first)
        .map(|(s, &base)| {
            let verts = posed
                .iter()
                .flat_map(|f| {
                    f[0][base..base + s.num_verts].iter().map(|&(p, n)| Md3Vert {
                        xyz: p.to_array().map(|c| (c / XYZ_SCALE).round().clamp(-32768.0, 32767.0) as i16),
                        normal: encode_normal(n.normalize()),
                    })
                })
                .collect();
            Md3Surface {
                triangles: s.triangles.iter().filter(|t| keep[base + t[0] as usize]).copied().collect(),
                verts,
                ..s.clone()
            }
        })
        .collect();
    let frames = rig
        .frames
        .iter()
        .zip(&posed)
        .map(|(f, fposes)| {
            let (lo, hi) = bbox(fposes[0].iter().map(|p| p.0));
            Md3Frame {
                bounds: [lo.to_array(), hi.to_array()],
                local_origin: [0.0; 3],
                radius: lo.length().max(hi.length()),
                name: f.poses[0].name.clone(),
            }
        })
        .collect();
    Some(Md3 { name: src.name.clone(), flags: 0, frames, surfaces })
}
