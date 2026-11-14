//! Imported toy view models (`tools/import_toy_model.py` output) with the
//! skin quantised to the active mod's palette (no fullbrights), uploaded
//! once per level under a `@key`.
use qformats::mdl::{Mdl, Skin};
use qrender::{ModelId, Renderer};

pub fn parse(mdl_zlib: &[u8], rgb_zlib: &[u8], palette: &[u8]) -> Mdl {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(mdl_zlib).expect("bundled toy geometry");
    let mut model = Mdl::parse(&bytes).expect("bundled toy MDL");
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(rgb_zlib).expect("bundled toy texture");
    let mut cache = std::collections::HashMap::new();
    let pixels = rgb.as_chunks::<3>().0.iter().map(|&p| *cache.entry(p).or_insert_with(|| nearest(palette, p))).collect();
    model.skins = vec![Skin { intervals: Vec::new(), images: vec![pixels] }];
    model
}

/// True-colour glow layer for saturated blue and orange texels (the gel
/// canisters: the Quake palette has no bright blue, so they would quantise
/// to purple).
pub fn gel_glow(rgb_zlib: &[u8]) -> Vec<u8> {
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(rgb_zlib).expect("bundled toy texture");
    rgb.as_chunks::<3>()
        .0
        .iter()
        .flat_map(|&[r, g, b]| {
            let (r, g, b) = (i32::from(r), i32::from(g), i32::from(b));
            let blue = b > 90 && b > r + 50 && b > g + 10;
            let orange = r > 110 && r > b + 70 && r > g + 25 && g > b;
            if blue || orange {
                let k = |v: i32| (v * 2).min(255) as u8;
                [k(r), k(g), k(b), 255]
            } else {
                [0; 4]
            }
        })
        .collect()
}

/// The model under `key`, built and uploaded on first use this level
/// (`glow`: add [`gel_glow`]).
pub fn get(key: &str, mdl_zlib: &[u8], rgb_zlib: &[u8], glow: bool, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<ModelId> {
    if !lv.by_name.contains_key(key) {
        let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
        let mdl = parse(mdl_zlib, rgb_zlib, &palette);
        let loaded = if glow { r.load_alias_glow(&mdl, &[gel_glow(rgb_zlib)]) } else { r.load_alias(&mdl) };
        let m = loaded.ok().map(|id| crate::assets::Model { id, kind: crate::assets::Kind::Alias, flags: 0 });
        lv.by_name.insert(key.into(), m);
    }
    lv.by_name.get(key).copied().flatten().map(|m| m.id)
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
    #[test]
    fn gel_and_washer_models_load() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        for m in [
            super::parse(include_bytes!("../../assets/gel/model.mdl.zlib"), include_bytes!("../../assets/gel/texture.rgb.zlib"), &palette),
            super::parse(include_bytes!("../../assets/washer/model.mdl.zlib"), include_bytes!("../../assets/washer/texture.rgb.zlib"), &palette),
        ] {
            assert!(m.triangles.len() > 1000 && m.triangles.len() <= 4000);
            assert_eq!(m.skins[0].images[0].len(), (m.skin_width * m.skin_height) as usize);
        }
        let glow = super::gel_glow(include_bytes!("../../assets/gel/texture.rgb.zlib"));
        let lit = glow.as_chunks::<4>().0.iter().filter(|p| p[3] > 0).count();
        assert!(lit > 500, "canister texels: {lit}");
    }
}
