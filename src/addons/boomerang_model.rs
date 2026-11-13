//! The Meshy boomerang, converted offline to native alias geometry
//! (`tools/import_toy_model.py`), and the procedural sprite atlas for its
//! target markers, queue numbers and stun stars.
use qformats::mdl::{Mdl, Skin};

pub fn model(palette: &[u8]) -> Mdl {
    let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../assets/boomerang/boomerang.mdl.zlib"
    ))
    .expect("bundled boomerang geometry");
    let mut model = Mdl::parse(&bytes).expect("bundled boomerang MDL");
    let rgb = miniz_oxide::inflate::decompress_to_vec_zlib(include_bytes!(
        "../../assets/boomerang/texture.rgb.zlib"
    ))
    .expect("bundled boomerang texture");
    let mut cache = std::collections::HashMap::new();
    let pixels = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .map(|&p| *cache.entry(p).or_insert_with(|| nearest(palette, p)))
        .collect();
    model.skins = vec![Skin {
        intervals: vec![],
        images: vec![pixels],
    }];
    model
}

/// Nearest non-fullbright palette index.
fn nearest(palette: &[u8], rgb: [u8; 3]) -> u8 {
    palette
        .as_chunks::<3>()
        .0
        .iter()
        .take(224)
        .enumerate()
        .min_by_key(|(_, p)| {
            (0..3)
                .map(|k| (i32::from(p[k]) - i32::from(rgb[k])).pow(2))
                .sum::<i32>()
        })
        .map_or(0, |(i, _)| i as u8)
}

/// Atlas cell size and grid (4×4 cells of 16×16 texels).
pub const CELL: usize = 16;
pub const GRID: usize = 4;
/// Cells after the digits 0-9.
pub const RING: usize = 10;
pub const STAR: usize = 11;

/// 3×5 digit glyphs, one row per u8 (bit 2 = left column).
const DIGITS: [[u8; 5]; 10] = [
    [7, 5, 5, 5, 7],
    [2, 6, 2, 2, 7],
    [7, 1, 7, 4, 7],
    [7, 1, 7, 1, 7],
    [5, 5, 7, 1, 1],
    [7, 4, 7, 1, 7],
    [7, 4, 7, 5, 7],
    [7, 1, 1, 2, 2],
    [7, 5, 7, 5, 7],
    [7, 5, 7, 1, 7],
];

/// White-on-transparent RGBA atlas (tinted by vertex colour): digits drawn
/// 3× with a dark outline, a target ring and a five-pointed star.
pub fn atlas() -> Vec<u8> {
    let side = CELL * GRID;
    let mut px = vec![0u8; side * side * 4];
    let mut put = |cell: usize, x: usize, y: usize, v: u8, a: u8| {
        let (cx, cy) = (cell % GRID * CELL, cell / GRID * CELL);
        let i = ((cy + y) * side + cx + x) * 4;
        px[i..i + 4].copy_from_slice(&[v, v, v, a]);
    };
    for (d, glyph) in DIGITS.iter().enumerate() {
        let on = |x: i32, y: i32| {
            (0..3).contains(&x) && (0..5).contains(&y) && glyph[y as usize] >> (2 - x) & 1 != 0
        };
        for y in 0..CELL as i32 {
            for x in 0..CELL as i32 {
                // glyph 9×15 texels centred in the cell
                let (gx, gy) = ((x - 3).div_euclid(3), (y).div_euclid(3));
                let fill = x >= 3 && on(gx, gy) && y < 15;
                let edge = !fill
                    && (-1..=1).any(|dy| {
                        (-1..=1).any(|dx| {
                            let (xx, yy) = (x + dx, y + dy);
                            xx >= 3 && yy < 15 && on((xx - 3).div_euclid(3), yy.div_euclid(3))
                        })
                    });
                if fill {
                    put(d, x as usize, y as usize, 255, 255);
                } else if edge {
                    put(d, x as usize, y as usize, 0, 230);
                }
            }
        }
    }
    for y in 0..CELL {
        for x in 0..CELL {
            let (dx, dy) = (x as f32 - 7.5, y as f32 - 7.5);
            let r = (dx * dx + dy * dy).sqrt();
            // ring with four notches (crosshair ticks)
            let notch = dx.abs() < 1.0 || dy.abs() < 1.0;
            if (5.0..7.5).contains(&r) && !notch {
                put(RING, x, y, 255, 255);
            } else if (4.0..5.0).contains(&r) && !notch {
                put(RING, x, y, 0, 200);
            }
            // star: radius varies with angle, five points
            let a = dy.atan2(dx) + std::f32::consts::FRAC_PI_2;
            let lim = 3.2 + 4.0 * (0.5 + 0.5 * (a * 5.0).cos()).powi(3);
            if r < lim {
                put(STAR, x, y, 255, 255);
            } else if r < lim + 1.0 {
                put(STAR, x, y, 40, 220);
            }
        }
    }
    px
}

/// Texture coordinates of atlas cell `c`.
pub fn cell_uv(c: usize) -> ([f32; 2], [f32; 2]) {
    let s = 1.0 / GRID as f32;
    let (x, y) = ((c % GRID) as f32 * s, (c / GRID) as f32 * s);
    ([x, y], [x + s, y + s])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_boomerang_is_valid_and_flat() {
        let palette: Vec<u8> = (0..256).flat_map(|i| [i as u8; 3]).collect();
        let m = model(&palette);
        assert!(m.triangles.len() > 300 && m.triangles.len() <= 4000);
        assert_eq!(
            m.skins[0].images[0].len(),
            (m.skin_width * m.skin_height) as usize
        );
        let pose = &m.frames[0].poses[0];
        assert!(pose.verts.iter().all(|v| (v.normal as usize) < 162));
        let (lo, hi) = (m.decode(pose.bboxmin), m.decode(pose.bboxmax));
        // centred, 28 units long, thinnest along Z
        assert!((hi[0] - lo[0] - 28.0).abs() < 0.5);
        assert!(hi[2] - lo[2] < hi[1] - lo[1]);
        assert!(m.skins[0].images[0].iter().all(|&p| p < 224));
    }

    #[test]
    fn atlas_cells_have_ink() {
        let a = atlas();
        let side = CELL * GRID;
        for c in 0..=STAR {
            let (cx, cy) = (c % GRID * CELL, c / GRID * CELL);
            let lit = (0..CELL)
                .flat_map(|y| (0..CELL).map(move |x| (x, y)))
                .filter(|&(x, y)| a[((cy + y) * side + cx + x) * 4] == 255)
                .count();
            assert!(lit > 10, "cell {c}");
        }
    }
}
