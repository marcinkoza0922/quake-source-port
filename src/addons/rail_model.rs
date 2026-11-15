//! The railgun view model: a Meshy meshy-t2.5 mesh converted offline by
//! `tools/import_toy_model.py` (see `assets/railgun/README.md`). The MDL's
//! stored skin is a glow mask (1 = blue coil energy). Quake's palette has no
//! electric-blue fullbrights, so the energy texels go to the renderer as a
//! true-colour RGBA glow layer (`Renderer::load_alias_glow`) instead.
use qformats::mdl::{Mdl, Skin};

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
        .map(|(&k, p)| {
            *cache.entry((*p, k)).or_insert_with(|| {
                let tinted = super::portal_skin::shade(*p, if k == 1 { 1.0 } else { BODY_GAIN });
                super::portal_skin::nearest(palette, tinted, false)
            })
        })
        .collect();
    let glow = mask
        .iter()
        .zip(rgb.as_chunks::<3>().0)
        .flat_map(|(&k, p)| if k == 1 { [p[0], p[1], p[2], 255] } else { [0; 4] })
        .collect();
    model.skins = vec![Skin { intervals: vec![], images: vec![pixels] }];
    (model, glow)
}

/// Body gain multiplier applied before palette quantisation (compensates for
/// Quake's alias overbright lighting so dark iron stays gritty).
const BODY_GAIN: f32 = 0.78;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_railgun_is_a_valid_low_poly_view_model() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        let (m, glow) = model(&palette);
        assert_eq!(glow.len(), m.skins[0].images[0].len() * 4);
        // the coil energy is blue
        let lit: Vec<_> = glow.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).collect();
        assert!(!lit.is_empty());
        let b = lit.iter().filter(|p| p[2] > p[0] && p[2] > p[1]).count();
        assert!(b * 2 > lit.len(), "{b} of {}", lit.len());
        assert!(m.triangles.len() > 500 && m.triangles.len() <= 4_000);
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
