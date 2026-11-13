//! Stickybomb models: the launcher (Meshy import, `assets/stickybomb`) and
//! the bomb itself, built here as a tiny Quake-style alias model (a
//! 40-triangle studded ball with a light band; skin 1 lights the band).
use qcore::anorms::AVERTEXNORMALS;
use qcore::math::Vec3;
use qformats::mdl::{Frame, Mdl, Pose, Skin, StVert, Triangle, TriVert};

/// Nearest palette index: normal colours only, or fullbrights (224..255).
fn nearest(palette: &[u8], rgb: [u8; 3], glow: bool) -> u8 {
    let range = if glow { 224..255 } else { 0..224 };
    range
        .min_by_key(|&i| (0..3).map(|k| (i32::from(*palette.get(i * 3 + k).unwrap_or(&0)) - i32::from(rgb[k])).pow(2)).sum::<i32>())
        .unwrap_or(0) as u8
}

/// The launcher view model with its texture quantized to `palette`.
pub fn launcher(palette: &[u8]) -> Mdl {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/stickybomb/model.mdl.zlib"))
        .expect("bundled stickybomb launcher");
    let mut m = Mdl::parse(&bytes).expect("bundled stickybomb MDL");
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/stickybomb/texture.rgb.zlib"))
        .expect("bundled stickybomb texture");
    let mut cache = std::collections::HashMap::new();
    let px = rgb.as_chunks::<3>().0.iter().map(|p| *cache.entry(*p).or_insert_with(|| nearest(palette, *p, false))).collect();
    m.skins = vec![Skin { intervals: vec![], images: vec![px] }];
    m
}

const SKIN: usize = 16;
const SEG: usize = 8;
const RINGS: usize = 4;
pub const RADIUS: f32 = 4.0;

fn pack(p: Vec3, lo: Vec3, scale: Vec3) -> TriVert {
    let n = p.normalize();
    let normal = (0..AVERTEXNORMALS.len()).max_by(|&a, &b| {
        n.dot(Vec3::from(AVERTEXNORMALS[a])).total_cmp(&n.dot(Vec3::from(AVERTEXNORMALS[b])))
    });
    let d = p - lo;
    let v = Vec3::new(d.x / scale.x, d.y / scale.y, d.z / scale.z);
    TriVert { v: [v.x, v.y, v.z].map(|c| c.round().clamp(0.0, 255.0) as u8), normal: normal.unwrap_or(0) as u8 }
}

/// The bomb: a low-poly ball; skin 0 dim, skin 1 with the band lit.
pub fn bomb(palette: &[u8]) -> Mdl {
    // vertices: poles + RINGS-1 latitude rings, studs bulge on the middle ring
    let mut pts = vec![Vec3::new(0.0, 0.0, RADIUS)];
    let mut st = vec![StVert { onseam: 0, s: 8, t: 0 }];
    for r in 1..RINGS {
        let lat = std::f32::consts::PI * r as f32 / RINGS as f32;
        for s in 0..SEG {
            let lon = std::f32::consts::TAU * s as f32 / SEG as f32;
            let rad = if r == RINGS / 2 && s % 2 == 0 { RADIUS * 1.3 } else { RADIUS };
            pts.push(Vec3::new(lat.sin() * lon.cos(), lat.sin() * lon.sin(), lat.cos()) * rad);
            st.push(StVert { onseam: 0, s: (s * 2) as i32, t: (r * SKIN / RINGS) as i32 });
        }
    }
    pts.push(Vec3::new(0.0, 0.0, -RADIUS));
    st.push(StVert { onseam: 0, s: 8, t: SKIN as i32 - 1 });
    let ring = |r: usize, s: usize| (1 + (r - 1) * SEG + s % SEG) as i32;
    let bottom = pts.len() as i32 - 1;
    let mut tris = Vec::new();
    for s in 0..SEG {
        tris.push([0, ring(1, s + 1), ring(1, s)]);
        for r in 1..RINGS - 1 {
            tris.push([ring(r, s), ring(r, s + 1), ring(r + 1, s)]);
            tris.push([ring(r, s + 1), ring(r + 1, s + 1), ring(r + 1, s)]);
        }
        tris.push([bottom, ring(RINGS - 1, s), ring(RINGS - 1, s + 1)]);
    }
    let lo = Vec3::splat(-RADIUS * 1.3);
    let scale = Vec3::splat(RADIUS * 2.6 / 255.0);
    let verts: Vec<TriVert> = pts.iter().map(|&p| pack(p, lo, scale)).collect();
    let skin = |lit: bool| {
        let body = nearest(palette, [70, 22, 16], false);
        let dark = nearest(palette, [30, 26, 22], false);
        let band = if lit { nearest(palette, [255, 50, 20], true) } else { nearest(palette, [120, 30, 20], false) };
        let img = (0..SKIN * SKIN)
            .map(|i| {
                let (x, y) = (i % SKIN, i / SKIN);
                if (7..=8).contains(&y) { if x % 4 < 2 { band } else { dark } } else if x % 4 == 0 { dark } else { body }
            })
            .collect();
        Skin { intervals: vec![], images: vec![img] }
    };
    let (bmin, bmax) = (TriVert { v: [0; 3], normal: 0 }, TriVert { v: [255; 3], normal: 0 });
    Mdl {
        scale: scale.to_array(),
        scale_origin: lo.to_array(),
        bounding_radius: RADIUS * 1.3,
        eye_position: [0.0; 3],
        skin_width: SKIN as u32,
        skin_height: SKIN as u32,
        num_verts: verts.len(),
        synctype: 0,
        flags: 0,
        size: RADIUS,
        skins: vec![skin(false), skin(true)],
        stverts: st,
        // listed clockwise seen from outside, the native alias winding
        triangles: tris.into_iter().map(|verts| Triangle { faces_front: 1, verts }).collect(),
        frames: vec![Frame { bboxmin: bmin, bboxmax: bmax, intervals: vec![], poses: vec![Pose { name: "bomb".into(), bboxmin: bmin, bboxmax: bmax, verts }] }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn palette() -> Vec<u8> {
        (0..256).flat_map(|i| [i as u8, (i / 2) as u8, (255 - i) as u8]).collect()
    }

    #[test]
    fn bomb_is_a_closed_valid_alias_model() {
        let m = bomb(&palette());
        assert_eq!(m.triangles.len(), SEG * 2 * (RINGS - 1));
        assert_eq!(m.stverts.len(), m.num_verts);
        assert!(m.triangles.iter().all(|t| t.verts.iter().all(|&v| (v as usize) < m.num_verts)));
        assert_ne!(m.skins[0].images[0], m.skins[1].images[0]);
        assert!(m.skins[1].images[0].iter().any(|&p| p >= 224));
        assert!(m.skins[0].images[0].iter().all(|&p| p < 224));
    }

    #[test]
    fn launcher_asset_parses_with_a_quantized_skin() {
        let m = launcher(&palette());
        assert!(m.triangles.len() > 300 && m.triangles.len() <= 4000);
        assert_eq!(m.skins[0].images[0].len(), (m.skin_width * m.skin_height) as usize);
        assert!(m.skins[0].images[0].iter().all(|&p| p < 224));
    }
}
