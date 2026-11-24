//! Classic MDL player models. id's player.mdl (also rogue's and
//! Alkaline's) keeps its weapons as separate mesh pieces: a 38-vertex gun
//! (held in gun frames, slung on the back in axe frames) and a 16-vertex axe
//! (squashed out of sight inside the body in gun frames). Weapons baked for
//! any rig come out as MDLs here.

use qcore::math::Vec3;
use qformats::mdl::{Frame, Mdl, Pose, Triangle, TriVert};

use super::arms::{Arms, Part, Posed};
use super::rig::{GunFrame, Rig, Spec, bbox, components};
use crate::debris::math::{anorm, apply, normal_index};

/// Component vertex counts of id's player.mdl, largest first: body, gun,
/// axe and a tiny always-collapsed piece. Other player models (Arcane
/// Dimensions', hipnotic's Mjolnir model) keep their own weapons.
const SIGNATURE: [usize; 4] = [149, 38, 16, 9];

fn tris(m: &Mdl) -> impl Iterator<Item = [usize; 3]> + '_ {
    m.triangles.iter().map(|t| t.verts.map(|v| v as usize))
}

fn without(m: &Mdl, drop: &[&[usize]]) -> Mdl {
    let mut gone = vec![false; m.num_verts];
    for v in drop.iter().flat_map(|d| d.iter()) {
        gone[*v] = true;
    }
    Mdl { triangles: m.triangles.iter().filter(|t| !gone[t.verts[0] as usize]).copied().collect(), ..m.clone() }
}

/// The rig plus the body without its gun and axe, and without just its
/// gun (the axe is held).
pub fn rig(m: &Mdl) -> Option<(Rig, Mdl, Mdl)> {
    let comps = components(m.num_verts, tris(m));
    if comps.iter().map(Vec::len).ne(SIGNATURE) {
        return None;
    }
    let (gun, axe) = (&comps[1], &comps[2]);
    let frames: Vec<GunFrame> = m
        .frames
        .iter()
        .map(|f| {
            let poses = f.poses.iter().map(|p| (p.name.clone(), gun.iter().map(|&i| Vec3::from_array(m.decode(p.verts[i]))).collect())).collect();
            (f.intervals.clone(), poses)
        })
        .collect();
    Some((Rig::fit(frames)?, without(m, &[gun, axe]), without(m, &[gun])))
}

/// The arms of the [`SIGNATURE`] mesh (found by clustering its vertices by
/// how they move): upper arm and hand, left then right. The wrist ring is the
/// hand's; there are no vertices between it and the elbow.
const UPPER: [[usize; 12]; 2] =
    [[58, 59, 63, 64, 95, 96, 97, 159, 160, 161, 165, 166], [29, 30, 35, 36, 69, 70, 94, 112, 115, 125, 126, 170]];
const HAND: [[usize; 12]; 2] =
    [[139, 140, 141, 149, 150, 167, 168, 169, 182, 183, 192, 193], [113, 114, 132, 133, 203, 205, 206, 207, 208, 209, 210, 211]];

/// The arms of a model [`rig`] accepts.
pub fn arms(m: &Mdl) -> Option<Arms> {
    let mut parts = vec![Part::Body; m.num_verts];
    for s in 0..2 {
        UPPER[s].iter().for_each(|&v| parts[v] = Part::Upper(s));
        HAND[s].iter().for_each(|&v| parts[v] = Part::Hand(s));
    }
    let f = m.frames.iter().position(|f| f.poses.first().is_some_and(|p| p.name.eq_ignore_ascii_case("axstnd1")))?;
    Arms::new(&parts, &positions(m, f), tris(m))
}

/// Removes the stock gun's muzzle flash (the tiny piece) from a body of `m`.
pub fn drop_flash(body: &mut Mdl, m: &Mdl) {
    if let Some(flash) = components(m.num_verts, tris(m)).last() {
        body.triangles.retain(|t| !flash.contains(&(t.verts[0] as usize)));
    }
}

/// Frame `f`'s vertex positions.
pub fn positions(m: &Mdl, f: usize) -> Vec<Vec3> {
    m.frames[f].poses[0].verts.iter().map(|&v| Vec3::from_array(m.decode(v))).collect()
}

/// Replaces each frame by frame `src[i]`, then by the reposed vertices.
pub fn set_frames(m: &mut Mdl, src: &[usize], posed: Option<&[Posed]>) {
    let old = m.frames.clone();
    let (scale, origin) = (m.scale, m.scale_origin);
    for (i, f) in m.frames.iter_mut().enumerate() {
        let from = &old[src[i]].poses[0];
        for pose in &mut f.poses {
            pose.verts = from.verts.clone();
            if let Some(p) = posed.map(|p| &p[i]) {
                for (k, v) in pose.verts.iter_mut().enumerate() {
                    let q = p.pos[k].to_array();
                    *v = TriVert {
                        v: [0, 1, 2].map(|c| ((q[c] - origin[c]) / scale[c]).round().clamp(0.0, 255.0) as u8),
                        normal: normal_index(apply(&p.rot[k], anorm(v.normal))),
                    };
                }
            }
            (pose.bboxmin, pose.bboxmax) = bounds(&pose.verts);
        }
        (f.bboxmin, f.bboxmax) = (f.poses[0].bboxmin, f.poses[0].bboxmax);
    }
}

fn bounds(v: &[TriVert]) -> (TriVert, TriVert) {
    let lo = [0, 1, 2].map(|i| v.iter().map(|t| t.v[i]).min().unwrap_or(0));
    let hi = [0, 1, 2].map(|i| v.iter().map(|t| t.v[i]).max().unwrap_or(0));
    (TriVert { v: lo, normal: 0 }, TriVert { v: hi, normal: 0 })
}

/// `src`'s first pose fitted into the hand per `spec`, as an MDL with the
/// rig's frames.
pub fn bake(rig: &Rig, src: &Mdl, spec: &Spec) -> Option<Mdl> {
    let pose = src.frames.first()?.poses.first()?;
    let pos: Vec<Vec3> = pose.verts.iter().map(|&v| Vec3::from_array(src.decode(v))).collect();
    let (keep, placed) = rig.place(&pos, tris(src), spec)?;
    let normals: Vec<Vec3> = pose.verts.iter().map(|v| anorm(v.normal)).collect();
    let posed = rig.posed(&placed, &normals);
    let (lo, hi) = bbox(posed.iter().flatten().flatten().map(|p| p.0));
    let scale = ((hi - lo) / 255.0).max(Vec3::splat(1e-3));
    let encode = |p: Vec3, n: Vec3| TriVert {
        v: [0, 1, 2].map(|i| ((p[i] - lo[i]) / scale[i]).round().clamp(0.0, 255.0) as u8),
        normal: normal_index(n),
    };
    let frames = rig
        .frames
        .iter()
        .zip(&posed)
        .map(|(f, fposes)| {
            let poses: Vec<Pose> = f
                .poses
                .iter()
                .zip(fposes)
                .map(|(p, verts)| {
                    let verts: Vec<TriVert> = verts.iter().map(|&(p, n)| encode(p, n)).collect();
                    let (bmin, bmax) = bounds(&verts);
                    Pose { name: p.name.clone(), bboxmin: bmin, bboxmax: bmax, verts }
                })
                .collect();
            let (bmin, bmax) = bounds(&poses.iter().flat_map(|p| [p.bboxmin, p.bboxmax]).collect::<Vec<_>>());
            Frame { bboxmin: bmin, bboxmax: bmax, intervals: f.intervals.clone(), poses }
        })
        .collect();
    Some(Mdl {
        scale: scale.to_array(),
        scale_origin: lo.to_array(),
        bounding_radius: lo.length().max(hi.length()),
        eye_position: [0.0; 3],
        skin_width: src.skin_width,
        skin_height: src.skin_height,
        num_verts: src.num_verts,
        synctype: 0,
        // not the pickup's rotation or trail
        flags: src.flags & qformats::mdl::MF_HOLEY,
        size: src.size,
        skins: src.skins.clone(),
        stverts: src.stverts.clone(),
        triangles: src.triangles.iter().filter(|t| keep[t.verts[0] as usize]).copied().collect::<Vec<Triangle>>(),
        frames,
    })
}
