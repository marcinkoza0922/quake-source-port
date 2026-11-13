//! Imported RGB atlas remapped to the active mod's palette and owner colors.
use qformats::mdl::Skin;

pub(super) fn nearest(palette: &[u8], rgb: [u8; 3], glow: bool) -> u8 {
    palette
        .as_chunks::<3>()
        .0
        .iter()
        .take(if glow { 255 } else { 224 })
        .enumerate()
        .min_by_key(|(_, p)| {
            (0..3)
                .map(|k| (i32::from(p[k]) - i32::from(rgb[k])).pow(2))
                .sum::<i32>()
        })
        .map_or(0, |(i, _)| i as u8)
}

/// Scale an imported texel's brightness by `k` (clamped), so Meshy's
/// pastel atlases land near Quake's darker palette under alias lighting.
pub(super) fn shade(rgb: [u8; 3], k: f32) -> [u8; 3] {
    rgb.map(|c| (f32::from(c) * k).round().clamp(0.0, 255.0) as u8)
}

/// Resize an alias model about its local origin, per axis.
pub(super) fn rescale(m: &mut qformats::mdl::Mdl, k: [f32; 3]) {
    m.bounding_radius *= k.iter().copied().fold(0.0, f32::max);
    for (a, k) in k.iter().enumerate() {
        m.scale[a] *= k;
        m.scale_origin[a] *= k;
    }
    
}

pub fn skins(palette: &[u8], mask: &[u8]) -> Vec<Skin> {
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../assets/portal-gun/texture.rgb.zlib"
    ))
    .expect("bundled portal texture");
    assert_eq!(rgb.len(), mask.len() * 3);
    // Quantize metal once; cache the source's pixel-art colors across the atlas.
    let mut cache = std::collections::HashMap::new();
    let metal: Vec<u8> = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|p| {
            let color = [p[0], p[1], p[2]];
            *cache
                .entry(color)
                .or_insert_with(|| nearest(palette, color, false))
        })
        .collect();
    (0..8)
        .map(|index| {
            let mut pixels = metal.clone();
            let mut tint_cache = std::collections::HashMap::new();
            for (i, (&kind, p)) in mask.iter().zip(rgb.as_chunks::<3>().0.iter()).enumerate() {
                let slot = match kind {
                    1 | 4 => 0,
                    2 | 5 => 1,
                    3 => index % 2,
                    _ => continue,
                };
                let color = super::portal_draw::colour(index / 2, slot);
                let gain = *p.iter().max().unwrap();
                let glow = kind <= 3;
                pixels[i] = *tint_cache.entry((slot, gain, glow)).or_insert_with(|| {
                    nearest(
                        palette,
                        color.map(|c| (u16::from(c) * u16::from(gain) / 255) as u8),
                        glow,
                    )
                });
            }
            Skin {
                intervals: vec![],
                images: vec![pixels],
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_and_fire_mode_only_recolor_the_marked_texture_regions() {
        let palette: Vec<u8> = (0..256)
            .flat_map(|i| super::super::portal_draw::colour(i / 2, i % 2))
            .collect();
        let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
            "../../assets/portal-gun/projector.mdl.zlib"
        ))
        .unwrap();
        let mdl = qformats::mdl::Mdl::parse(&bytes).unwrap();
        let mask = &mdl.skins[0].images[0];
        let skins = skins(&palette, mask);
        assert!((0..=5).all(|k| mask.contains(&k)));
        for owner in 0..4 {
            let a = &skins[owner * 2].images[0];
            let b = &skins[owner * 2 + 1].images[0];
            assert!(
                mask.iter()
                    .enumerate()
                    .any(|(i, &k)| k == 3 && a[i] != b[i])
            );
            for (i, &k) in mask.iter().enumerate() {
                if k != 3 {
                    assert_eq!(a[i], b[i]);
                }
                if k == 0 {
                    assert_eq!(a[i], skins[0].images[0][i]);
                    assert!(a[i] < 224);
                }
            }
            if owner > 0 {
                assert_ne!(a, &skins[0].images[0]);
            }
        }
    }
}
