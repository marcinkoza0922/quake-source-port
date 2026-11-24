//! Akimbo pistol models: the pistol (`assets/akimbo`, made from a Meshy mesh
//! by `tools/prep_akimbo.py` and painted by `tools/paint_akimbo_texture.py`)
//! with poses for the slide and the magazine made here, its mirror image for
//! the left hand, and a magazine box.
use qcore::anorms::AVERTEXNORMALS;
use qcore::math::Vec3;
use qformats::mdl::{Frame, Mdl, Pose, Skin, StVert, Triangle, TriVert};

/// Frames of the pistol: at rest, slide back; [`MAG_OUT`] is added while
/// the magazine is out.
pub const SLIDE: u32 = 1;
pub const MAG_OUT: u32 = 2;
/// How far the slide travels.
const TRAVEL: f32 = 2.0;
/// Where a shot leaves the pistol.
pub const MUZZLE: Vec3 = Vec3 { x: 11.0, y: 0.0, z: 4.5 };
/// The ejection port.
pub const PORT: Vec3 = Vec3 { x: -1.0, y: -1.3, z: 5.3 };
/// The grip's axis, from the magazine's base up, and its lower end.
pub const GRIP: Vec3 = Vec3 { x: 0.28, y: 0.0, z: 0.96 };
pub const WELL: Vec3 = Vec3 { x: -7.6, y: 0.0, z: -5.4 };

/// Which vertices belong to the slide and which to the magazine's base.
/// The mesh is made of separate parts, told apart by their size and place.
fn moving_parts(m: &Mdl) -> (Vec<bool>, Vec<bool>) {
    let verts = &m.frames[0].poses[0].verts;
    // connected vertices (welded by position)
    let mut root: Vec<usize> = (0..verts.len()).collect();
    fn find(root: &mut [usize], mut i: usize) -> usize {
        while root[i] != i {
            root[i] = root[root[i]];
            i = root[i];
        }
        i
    }
    let mut at = std::collections::HashMap::new();
    for (i, v) in verts.iter().enumerate() {
        let j = *at.entry(v.v).or_insert(i);
        let (a, b) = (find(&mut root, i), find(&mut root, j));
        root[a] = b;
    }
    for t in &m.triangles {
        let a = find(&mut root, t.verts[0] as usize);
        for &v in &t.verts[1..] {
            let b = find(&mut root, v as usize);
            root[b] = a;
        }
    }
    struct Part {
        lo: Vec3,
        hi: Vec3,
        tris: usize,
    }
    let mut parts = std::collections::HashMap::new();
    for t in &m.triangles {
        let k = find(&mut root, t.verts[0] as usize);
        let p = parts.entry(k).or_insert(Part { lo: Vec3::splat(99.0), hi: Vec3::splat(-99.0), tris: 0 });
        p.tris += 1;
        for &v in &t.verts {
            let q = Vec3::from(m.decode(verts[v as usize]));
            p.lo = Vec3::new(p.lo.x.min(q.x), p.lo.y.min(q.y), p.lo.z.min(q.z));
            p.hi = Vec3::new(p.hi.x.max(q.x), p.hi.y.max(q.y), p.hi.z.max(q.z));
        }
    }
    let mut pick = |f: &dyn Fn(&Part) -> bool| -> Vec<bool> {
        (0..verts.len()).map(|i| parts.get(&find(&mut root, i)).is_some_and(f)).collect()
    };
    // everything of the top but the frame and the barrel in the port; the
    // muzzle brake is as wide as the slide, so it travels with it
    let slide = pick(&|p| p.lo.z > 2.3 && p.hi.z > 5.0 && (p.hi.x - p.lo.x < 8.0 || p.tris > 500));
    let base = pick(&|p| p.hi.z < -5.0 && p.hi.y - p.lo.y > 2.5);
    (slide, base)
}

/// The pistol of the right hand, muzzle at +X, or its mirror image.
pub fn pistol(palette: &[u8], left: bool) -> Mdl {
    let mut m = super::toy_model::parse(
        include_bytes!("../../assets/akimbo/model.mdl.zlib"),
        include_bytes!("../../assets/akimbo/texture.rgb.zlib"),
        palette,
    );
    let (slide, base) = moving_parts(&m);
    let rest = m.frames[0].clone();
    let (scale, origin) = (m.scale, m.scale_origin);
    let pack = |p: Vec3, normal: u8| {
        let p = p.to_array();
        TriVert { v: std::array::from_fn(|k| ((p[k] - origin[k]) / scale[k]).round().clamp(0.0, 255.0) as u8), normal }
    };
    for k in 1..4u32 {
        let mut f = rest.clone();
        f.poses[0].name = ["rest", "slide", "magout", "both"][k as usize].into();
        for (i, v) in f.poses[0].verts.iter_mut().enumerate() {
            let p = Vec3::from(m.decode(*v));
            if k & SLIDE != 0 && slide[i] {
                *v = pack(p - Vec3::new(TRAVEL, 0.0, 0.0), v.normal);
            }
            if k & MAG_OUT != 0 && base[i] {
                // the base plate is tucked into the grip, out of sight
                let q = p + GRIP * 2.8;
                *v = pack(Vec3::new(WELL.x + (q.x - WELL.x) * 0.6, q.y * 0.4, q.z), v.normal);
            }
        }
        m.frames.push(f);
    }
    if left {
        mirror(&mut m);
    }
    m
}

/// Mirrors a model left to right.
fn mirror(m: &mut Mdl) {
    let flip: Vec<u8> = AVERTEXNORMALS
        .iter()
        .map(|n| {
            let n = Vec3::new(n[0], -n[1], n[2]);
            (0..AVERTEXNORMALS.len()).max_by(|&a, &b| n.dot(Vec3::from(AVERTEXNORMALS[a])).total_cmp(&n.dot(Vec3::from(AVERTEXNORMALS[b])))).unwrap_or(0) as u8
        })
        .collect();
    m.scale_origin[1] = -(m.scale_origin[1] + m.scale[1] * 255.0);
    for f in &mut m.frames {
        for p in &mut f.poses {
            for v in &mut p.verts {
                v.v[1] = 255 - v.v[1];
                v.normal = flip[v.normal as usize];
            }
        }
    }
    for t in &mut m.triangles {
        t.verts.swap(1, 2);
    }
}

/// A magazine standing on its base plate (+Z into the grip, +X forwards).
pub fn magazine(palette: &[u8]) -> Mdl {
    const SKIN: usize = 16;
    let boxes = [
        (Vec3::new(-1.5, -0.75, 0.3), Vec3::new(1.5, 0.75, 6.0)),
        (Vec3::new(-1.9, -1.2, -0.4), Vec3::new(1.9, 1.2, 0.35)),
    ];
    let (lo, hi) = (Vec3::new(-2.0, -1.3, -0.5), Vec3::new(2.0, 1.3, 6.1));
    let scale = (hi - lo) * (1.0 / 255.0);
    let (mut verts, mut st, mut tris) = (Vec::new(), Vec::new(), Vec::new());
    for (b, (a, z)) in boxes.into_iter().enumerate() {
        for axis in 0..3 {
            for sign in [-1.0f32, 1.0] {
                let normal = (0..AVERTEXNORMALS.len()).max_by(|&i, &j| (AVERTEXNORMALS[i][axis] * sign).total_cmp(&(AVERTEXNORMALS[j][axis] * sign))).unwrap_or(0) as u8;
                let first = verts.len() as i32;
                let (u, v) = ((axis + 1) % 3, (axis + 2) % 3);
                for (iu, iv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                    let mut p = [0.0f32; 3];
                    let (a, z) = (a.to_array(), z.to_array());
                    p[axis] = if sign < 0.0 { a[axis] } else { z[axis] };
                    p[u] = if iu == 0 { a[u] } else { z[u] };
                    p[v] = if iv == 0 { a[v] } else { z[v] };
                    let d = Vec3::from(p) - lo;
                    verts.push(TriVert { v: [d.x / scale.x, d.y / scale.y, d.z / scale.z].map(|c| c.round().clamp(0.0, 255.0) as u8), normal });
                    // body on the left half of the skin, base on the right
                    let across = if axis == 2 { iu } else if u == 2 { iv } else { iu };
                    let up = if axis == 2 { iv * 15 } else { ((p[2] + 0.5) / 6.6 * 15.0) as i32 };
                    st.push(StVert { onseam: 0, s: (b * 8) as i32 + across * 7, t: 15 - up });
                }
                // clockwise seen from outside, the native alias winding
                let q = if sign > 0.0 { [[0, 2, 1], [0, 3, 2]] } else { [[0, 1, 2], [0, 2, 3]] };
                tris.extend(q.map(|t| Triangle { faces_front: 1, verts: t.map(|k| first + k) }));
            }
        }
    }
    let shade = |c: [u8; 3]| super::toy_model::nearest(palette, c);
    let (steel, dark, light, brass) = (shade([47, 47, 47]), shade([23, 23, 23]), shade([75, 75, 75]), shade([115, 87, 31]));
    let image = (0..SKIN * SKIN)
        .map(|i| {
            let (x, y) = (i % SKIN, i / SKIN);
            match (x < 8, x % 8, y) {
                (true, _, 0..=1) => brass,
                (true, 0 | 7, _) => light,
                (true, 3 | 4, y) if y % 3 == 0 => dark,
                (true, ..) => steel,
                (false, _, y) if y % 2 == 0 => steel,
                _ => dark,
            }
        })
        .collect();
    let (bmin, bmax) = (TriVert { v: [0; 3], normal: 0 }, TriVert { v: [255; 3], normal: 0 });
    Mdl {
        scale: scale.to_array(),
        scale_origin: lo.to_array(),
        bounding_radius: 7.0,
        eye_position: [0.0; 3],
        skin_width: SKIN as u32,
        skin_height: SKIN as u32,
        num_verts: verts.len(),
        synctype: 0,
        flags: 0,
        size: 4.0,
        skins: vec![Skin { intervals: vec![], images: vec![image] }],
        stverts: st,
        triangles: tris,
        frames: vec![Frame { bboxmin: bmin, bboxmax: bmax, intervals: vec![], poses: vec![Pose { name: "mag".into(), bboxmin: bmin, bboxmax: bmax, verts }] }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Vec<u8> {
        (0..256).flat_map(|i| [i as u8; 3]).collect()
    }

    #[test]
    fn pistol_is_a_valid_low_poly_model_whose_slide_and_magazine_move() {
        let m = pistol(&palette(), false);
        assert!(m.triangles.len() > 2000 && m.triangles.len() <= 4000);
        assert_eq!(m.skins[0].images[0].len(), (m.skin_width * m.skin_height) as usize);
        assert_eq!(m.frames.len(), 4);
        let pos = |f: usize, i: usize| Vec3::from(m.decode(m.frames[f].poses[0].verts[i]));
        let moved = |f: usize| (0..m.num_verts).filter(|&i| pos(f, i) != pos(0, i)).collect::<Vec<_>>();
        let slide = moved(SLIDE as usize);
        assert!(slide.len() > m.num_verts / 5 && slide.len() < m.num_verts / 2, "{} of {}", slide.len(), m.num_verts);
        for &i in &slide {
            let d = pos(0, i) - pos(1, i);
            assert!((d.x - TRAVEL).abs() < 0.1 && d.y == 0.0 && d.z == 0.0, "{d:?}");
            assert!(pos(0, i).z > 2.0);
        }
        // the muzzle brake goes with it, all of it
        assert!((0..m.num_verts).filter(|&i| pos(0, i).x > 8.3).all(|i| slide.contains(&i)));
        let base = moved(MAG_OUT as usize);
        assert!(!base.is_empty() && base.iter().all(|&i| pos(0, i).z < -5.0));
        assert_eq!(moved(3).len(), slide.len() + base.len());
    }

    #[test]
    fn the_left_pistol_is_the_mirror_image() {
        let (r, l) = (pistol(&palette(), false), pistol(&palette(), true));
        for f in 0..4 {
            for i in (0..r.num_verts).step_by(37) {
                let a = r.decode(r.frames[f].poses[0].verts[i]);
                let b = l.decode(l.frames[f].poses[0].verts[i]);
                assert!((a[0] - b[0]).abs() + (a[1] + b[1]).abs() + (a[2] - b[2]).abs() < 0.01);
            }
        }
        assert_eq!(r.triangles[5].verts[1], l.triangles[5].verts[2]);
    }

    #[test]
    fn magazine_faces_point_outwards() {
        let m = magazine(&palette());
        assert_eq!((m.triangles.len(), m.num_verts), (24, 48));
        let verts = &m.frames[0].poses[0].verts;
        for t in &m.triangles {
            let [a, b, c] = t.verts.map(|v| Vec3::from(m.decode(verts[v as usize])));
            let n = (c - a).cross(b - a);
            let stored = Vec3::from(AVERTEXNORMALS[verts[t.verts[0] as usize].normal as usize]);
            assert!(n.dot(stored) > 0.0);
        }
    }
}
