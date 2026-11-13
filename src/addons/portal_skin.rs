//! Original palette-indexed, pixel-painted industrial metal atlas.
use qformats::mdl::Skin;

pub const TILE: usize = 32;
pub const WIDTH: usize = TILE * 4;
pub const HEIGHT: usize = TILE * 2;

#[derive(Clone, Copy)]
pub enum Material {
    Iron,
    Dark,
    Edge,
    Copper,
    Core,
    Vent,
    Grip,
    Panel,
}

impl Material {
    pub fn tile(self) -> (i32, i32) {
        let i = self as i32;
        ((i % 4) * TILE as i32, (i / 4) * TILE as i32)
    }
}

fn nearest(palette: &[u8], rgb: [i32; 3], glow: bool) -> u8 {
    palette
        .as_chunks::<3>()
        .0
        .iter()
        .take(if glow { 255 } else { 224 })
        .enumerate()
        .min_by_key(|(_, p)| {
            (0..3)
                .map(|k| (i32::from(p[k]) - rgb[k]).pow(2))
                .sum::<i32>()
        })
        .map_or(0, |(i, _)| i as u8)
}

fn pixel(material: usize, x: usize, y: usize, core: [u8; 3]) -> [i32; 3] {
    // Integer hash gives fixed grain, pitting and wear without runtime RNG.
    let hash = (x as u32)
        .wrapping_mul(374761393)
        .wrapping_add((y as u32).wrapping_mul(668265263))
        .wrapping_add((material as u32).wrapping_mul(2246822519));
    let hash = (hash ^ (hash >> 13)).wrapping_mul(1274126177);
    let grain = ((hash >> 16) % 13) as i32 - 6;
    let edge = x.min(31 - x).min(y.min(31 - y));
    let mut rgb: [i32; 3] = match material {
        0 => [65, 64, 59],  // oily brown steel
        1 => [33, 31, 27],  // soot-black barrel and recesses
        2 => [99, 98, 87],  // worn steel bevels
        3 => [111, 69, 36], // tarnished copper
        4 => core.map(i32::from),
        5 => [57, 51, 40], // milled cooling slots
        6 => [54, 38, 24], // ribbed grip
        _ => [88, 71, 49], // bolted bronze armor plates
    };
    let mut shade = grain;
    if material == Material::Core as usize {
        let d = (x as i32 - 15).abs();
        let gain = if d < 4 { 1.0 } else { 0.45 };
        return rgb.map(|v| (v as f32 * gain) as i32 + grain.max(0));
    }
    shade += match edge {
        0 => -29,
        1 => {
            if x + y < 32 {
                27
            } else {
                -17
            }
        }
        2 => 9,
        _ => 0,
    };
    if material == Material::Vent as usize {
        shade += match y % 6 {
            0 => 21,
            1..=3 => -34,
            _ => -5,
        };
    } else if material == Material::Grip as usize {
        shade += if y.is_multiple_of(4) {
            13
        } else if y % 4 == 1 {
            -18
        } else {
            0
        };
    } else {
        // Broken scratches run along the barrel; patchy oxidation stays subtle.
        if (y == 9 || y == 23 || y == 25) && hash % 7 < 4 {
            shade += 19;
        }
        if ((x / 4 + y / 3 * 7 + material * 3) % 13) < 3 {
            rgb[0] += 7;
            rgb[1] -= 7;
            rgb[2] -= 10;
        }
        // Four slotted steel fasteners, inset from the panel seams.
        let dx = (x as i32 - if x < 16 { 5 } else { 26 }).abs();
        let dy = (y as i32 - if y < 16 { 5 } else { 26 }).abs();
        if material != Material::Dark as usize && dx <= 1 && dy <= 1 {
            return if dy == 0 {
                [32, 29, 24]
            } else if y < 16 {
                [151, 139, 111]
            } else {
                [93, 85, 68]
            };
        }
    }
    rgb.map(|v| (v + shade).clamp(0, 223))
}

pub fn skins(palette: &[u8]) -> Vec<Skin> {
    // Only the emitter tile changes with player/portal slot; metal stays unlit.
    let mut metal = vec![0; WIDTH * HEIGHT];
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let material = (y / TILE) * 4 + x / TILE;
            metal[y * WIDTH + x] =
                nearest(palette, pixel(material, x % TILE, y % TILE, [0; 3]), false);
        }
    }
    (0..8)
        .map(|index| {
            let color = super::portal_draw::colour(index / 2, index % 2);
            let mut pixels = metal.clone();
            let (ox, oy) = Material::Core.tile();
            for y in 0..TILE {
                for x in 0..TILE {
                    pixels[(oy as usize + y) * WIDTH + ox as usize + x] =
                        nearest(palette, pixel(Material::Core as usize, x, y, color), true);
                }
            }
            Skin {
                intervals: vec![],
                images: vec![pixels],
            }
        })
        .collect()
}
