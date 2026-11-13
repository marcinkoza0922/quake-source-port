//! The railgun view model: a Meshy meshy-t2 mesh converted offline by
//! `tools/import_toy_model.py` (see `assets/railgun/README.md`). The MDL's
//! stored skin is a glow mask (1 = energy). Quake's palette has no green
//! fullbrights, so the energy texels go to the renderer as a true-colour
//! RGBA glow layer (`Renderer::load_alias_glow`) instead.
use qformats::mdl::{Mdl, Skin, StVert, Triangle, TriVert};

/// The model and its skin's RGBA glow layer.
pub fn model(palette: &[u8]) -> (Mdl, Vec<u8>) {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/railgun/railgun.mdl.zlib")).expect("bundled railgun geometry");
    let mut model = Mdl::parse(&bytes).expect("bundled railgun MDL");
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/railgun/texture.rgb.zlib")).expect("bundled railgun texture");
    let mask = &model.skins[0].images[0];
    assert_eq!(rgb.len(), mask.len() * 3);
    let mut cache = std::collections::HashMap::new();
    let pixels = mask
        .iter()
        .zip(rgb.as_chunks::<3>().0)
        .map(|(&k, p)| *cache.entry((*p, k)).or_insert_with(|| super::portal_skin::nearest(palette, super::portal_skin::shade(*p, BODY_GAIN), false)))
        .collect();
    let glow = mask
        .iter()
        .zip(rgb.as_chunks::<3>().0)
        .flat_map(|(&k, p)| if k == 1 { [p[0], p[1], p[2], 255] } else { [0; 4] })
        .collect();
    // the baked atlas kept only a few green texels of the concept's coil;
    // add glowing coil rings and side strips mapped onto one of them
    if let Some(i) = core_texel(mask, model.skin_width as usize) {
        let w = model.skin_width as usize;
        add_coils(&mut model, ((i % w) as i32, (i / w) as i32));
    }
    model.skins = vec![Skin { intervals: vec![], images: vec![pixels] }];
    // the mesh is a slim rod; thicken it so it reads at view-model size
    super::portal_skin::rescale(&mut model, [1.0, THICKEN, THICKEN]);
    (model, glow)
}

/// A glow texel whose 3×3 neighbourhood is all glow, so filtering stays green.
fn core_texel(mask: &[u8], w: usize) -> Option<usize> {
    let h = mask.len() / w.max(1);
    (w + 1..mask.len().saturating_sub(w + 1)).find(|&i| {
        let (x, y) = (i % w, i / w);
        x > 0 && x + 1 < w && y > 0 && y + 1 < h && (0..9).all(|k| mask[(y + k / 3 - 1) * w + x + k % 3 - 1] == 1)
    })
}

/// Appends emissive geometry (model units, before thickening): the mesh's
/// coil loops are remapped to glow and a strip runs down each side.
fn add_coils(m: &mut Mdl, st: (i32, i32)) {
    use qcore::math::Vec3;
    // the mesh's own coil loops (above the open section near the muzzle)
    let decode = |v: TriVert| Vec3::from(m.decode(v));
    let coil: Vec<usize> = (0..m.triangles.len())
        .filter(|&t| {
            let p = m.triangles[t].verts.map(|v| decode(m.frames[0].poses[0].verts[v as usize]));
            let small = (0..3).all(|k| (p[k] - p[(k + 1) % 3]).length() < COIL_EDGE);
            small && p.iter().all(|q| (COIL_X.0..COIL_X.1).contains(&q.x) && q.z > COIL_Z)
        })
        .collect();
    for t in coil {
        let verts = m.triangles[t].verts;
        let pose = &mut m.frames[0].poses[0];
        let fresh = verts.map(|v| {
            pose.verts.push(pose.verts[v as usize]);
            m.stverts.push(StVert { onseam: 0, s: st.0, t: st.1 });
            pose.verts.len() as i32 - 1
        });
        m.triangles[t].verts = fresh;
        m.num_verts = pose.verts.len();
    }
    let mut quads: Vec<[Vec3; 4]> = Vec::new();
    for y in [-2.05f32, 2.05] {
        let p = |x: f32, z: f32| Vec3::new(x, y, z);
        let q = [p(0.0, 1.0), p(12.0, 1.0), p(12.0, 1.6), p(0.0, 1.6)];
        quads.push(if y < 0.0 { q } else { [q[3], q[2], q[1], q[0]] });
    }
    let base = m.num_verts as i32;
    let pose = &mut m.frames[0].poses[0];
    for q in &quads {
        let n = (q[1] - q[0]).cross(q[3] - q[0]).normalize();
        let normal = (0..qcore::anorms::AVERTEXNORMALS.len())
            .max_by(|&a, &b| n.dot(Vec3::from(qcore::anorms::AVERTEXNORMALS[a])).total_cmp(&n.dot(Vec3::from(qcore::anorms::AVERTEXNORMALS[b]))))
            .unwrap_or(0) as u8;
        for p in q {
            let v: [u8; 3] = std::array::from_fn(|k| ((p.to_array()[k] - m.scale_origin[k]) / m.scale[k]).round().clamp(0.0, 255.0) as u8);
            pose.verts.push(TriVert { v, normal });
            m.stverts.push(StVert { onseam: 0, s: st.0, t: st.1 });
        }
    }
    for k in 0..quads.len() as i32 {
        let v = base + k * 4;
        // clockwise seen from outside (quads above are counter-clockwise)
        m.triangles.push(Triangle { faces_front: 1, verts: [v, v + 2, v + 1] });
        m.triangles.push(Triangle { faces_front: 1, verts: [v, v + 3, v + 2] });
    }
    m.num_verts = pose.verts.len();
}

/// Coil loops: triangles wholly in this x range above this z.
const COIL_X: (f32, f32) = (2.5, 22.5);
const COIL_Z: f32 = 0.8;
/// Longer edges belong to the barrel, not a loop.
const COIL_EDGE: f32 = 3.0;

/// Body texels are near-black in the source; lift them toward Quake metal.
const BODY_GAIN: f32 = 1.5;
/// Cross-section scale of the held gun.
const THICKEN: f32 = 1.9;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_railgun_is_a_valid_low_poly_view_model() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        let (m, glow) = model(&palette);
        assert_eq!(glow.len(), m.skins[0].images[0].len() * 4);
        // the energy strip is green
        let lit: Vec<_> = glow.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).collect();
        assert!(!lit.is_empty());
        let g = lit.iter().filter(|p| p[1] > p[0] && p[1] > p[2]).count();
        assert!(g * 2 > lit.len(), "{g} of {}", lit.len());
        assert!(m.triangles.len() > 500 && m.triangles.len() <= 2_000);
        assert_eq!(m.skins.len(), 1);
        assert_eq!(m.skins[0].images[0].len(), (m.skin_width * m.skin_height) as usize);
        let pose = &m.frames[0].poses[0];
        assert_eq!(pose.verts.len(), m.num_verts);
        assert!(m.triangles.iter().all(|t| t.verts.iter().all(|&v| v >= 0 && (v as usize) < m.num_verts)));
        // muzzle forward along +X, 44 units long, 14 behind the origin
        let lo = m.decode(pose.bboxmin);
        let hi = m.decode(pose.bboxmax);
        assert!((lo[0] + 14.0).abs() < 0.1 && (hi[0] - 30.0).abs() < 0.1);
    }
}
