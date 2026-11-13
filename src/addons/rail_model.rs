//! The railgun view model: a Meshy meshy-t2 mesh converted offline by
//! `tools/import_toy_model.py` (see `assets/railgun/README.md`). The MDL's
//! stored skin is a glow mask (1 = energy, drawn with fullbright colours).
use qformats::mdl::{Mdl, Skin};

pub fn model(palette: &[u8]) -> Mdl {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/railgun/railgun.mdl.zlib")).expect("bundled railgun geometry");
    let mut model = Mdl::parse(&bytes).expect("bundled railgun MDL");
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/railgun/texture.rgb.zlib")).expect("bundled railgun texture");
    let mask = &model.skins[0].images[0];
    assert_eq!(rgb.len(), mask.len() * 3);
    let mut cache = std::collections::HashMap::new();
    let pixels = mask
        .iter()
        .zip(rgb.as_chunks::<3>().0)
        .map(|(&k, p)| *cache.entry((*p, k)).or_insert_with(|| super::portal_skin::nearest(palette, if k == 1 { *p } else { super::portal_skin::shade(*p, BODY_GAIN) }, k == 1)))
        .collect();
    model.skins = vec![Skin { intervals: vec![], images: vec![pixels] }];
    // the mesh is a slim rod; thicken it so it reads at view-model size
    super::portal_skin::rescale(&mut model, [1.0, THICKEN, THICKEN]);
    model
}

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
        let m = model(&palette);
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
