//! The hookshot launcher: a Meshy model converted offline by
//! `tools/import_toy_model.py` (assets/hookshot), skin quantised to the
//! active mod's palette (no fullbrights).
use qformats::mdl::{Mdl, Skin};

pub fn model(palette: &[u8]) -> Mdl {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/hookshot/model.mdl.zlib")).expect("bundled hookshot geometry");
    let mut model = Mdl::parse(&bytes).expect("bundled hookshot MDL");
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!("../../assets/hookshot/texture.rgb.zlib")).expect("bundled hookshot texture");
    let mut cache = std::collections::HashMap::new();
    let pixels = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&p| *cache.entry(p).or_insert_with(|| nearest(palette, p)))
        .collect();
    model.skins = vec![Skin { intervals: Vec::new(), images: vec![pixels] }];
    model
}

/// Nearest non-fullbright palette index (black when the palette is missing).
fn nearest(palette: &[u8], rgb: [u8; 3]) -> u8 {
    palette
        .as_chunks::<3>()
        .0
        .iter()
        .take(224)
        .enumerate()
        .min_by_key(|(_, p)| (0..3).map(|k| (i32::from(p[k]) - i32::from(rgb[k])).pow(2)).sum::<i32>())
        .map_or(0, |(i, _)| i as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_launcher_is_low_poly_with_one_skin() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        let m = model(&palette);
        assert!(m.triangles.len() > 500 && m.triangles.len() <= 2_000);
        assert_eq!(m.skins.len(), 1);
        assert_eq!(m.skins[0].images[0].len(), (m.skin_width * m.skin_height) as usize);
        assert!(m.skins[0].images[0].iter().all(|&p| p < 224));
        assert_eq!(m.num_verts, m.stverts.len());
    }
}
