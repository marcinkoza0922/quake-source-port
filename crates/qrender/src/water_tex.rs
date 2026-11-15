//! Procedural textures for enhanced water effects (spray puffs and tiling caustics).

/// RGBA of a soft round blob with a slightly lumpy edge (`n`×`n`).
pub fn spray_blob(n: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let u = (x as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let v = (y as f32 + 0.5) / n as f32 * 2.0 - 1.0;
            let a = v.atan2(u);
            let edge = 0.85 + 0.08 * (a * 5.0).sin() + 0.05 * (a * 9.0 + 1.3).sin();
            let d = (u * u + v * v).sqrt() / edge;
            let alpha = (1.0 - d * d).clamp(0.0, 1.0).powf(0.7);
            // brighter core so overlapping puffs read as foam
            let l = (200.0 + 55.0 * (1.0 - d).clamp(0.0, 1.0)) as u8;
            out.extend_from_slice(&[l, l, l, (alpha * 255.0) as u8]);
        }
    }
    out
}

fn worley_edge(px: f32, py: f32, cells: i32, seed: [f32; 2]) -> f32 {
    let (ix, iy) = (px.floor() as i32, py.floor() as i32);
    let (fx, fy) = (px - ix as f32, py - iy as f32);
    let (mut d1, mut d2) = (8.0f32, 8.0f32);
    for oy in -1..=1 {
        for ox in -1..=1 {
            let cx = (ix + ox).rem_euclid(cells) as f32 + seed[0];
            let cy = (iy + oy).rem_euclid(cells) as f32 + seed[1];
            let fract = |v: f32| v - v.floor();
            let jx = fract((cx * 127.1 + cy * 311.7).sin() * 43_758.547);
            let jy = fract((cx * 269.5 + cy * 183.3).sin() * 43_758.547);
            let qx = ox as f32 + 0.5 + 0.38 * (std::f32::consts::TAU * jx).sin() - fx;
            let qy = oy as f32 + 0.5 + 0.38 * (std::f32::consts::TAU * jy).cos() - fy;
            let d = qx * qx + qy * qy;
            if d < d1 {
                d2 = d1;
                d1 = d;
            } else if d < d2 {
                d2 = d;
            }
        }
    }
    let t = ((d2.sqrt() - d1.sqrt()) / 0.26).clamp(0.0, 1.0);
    (1.0 - t * t * (3.0 - 2.0 * t)).powi(4)
}

/// Seamless tiling `n`×`n` RGBA8 Worley caustic texture (R and G hold two
/// independent 4×4 cell edge layers for scrolling interference).
pub fn caustic_texture(n: u32) -> Vec<u8> {
    const CELLS: i32 = 4;
    let mut out = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        let py = (y as f32 + 0.5) / n as f32 * CELLS as f32;
        for x in 0..n {
            let px = (x as f32 + 0.5) / n as f32 * CELLS as f32;
            let r = (worley_edge(px, py, CELLS, [0.0, 0.0]) * 255.0).round() as u8;
            let g = (worley_edge(px, py, CELLS, [13.0, 29.0]) * 255.0).round() as u8;
            out.extend_from_slice(&[r, g, r, 255]);
        }
    }
    out
}
