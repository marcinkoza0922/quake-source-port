//! Seeded irregular pools and satellite droplets, packed into a 4×4 atlas.
pub const TILE: usize = 64;
pub const GRID: usize = 4;
pub const WIDTH: usize = TILE * GRID;

pub struct Rng(u32);
impl Rng {
    pub fn new(seed: u32) -> Self {
        Self(seed)
    }
    pub fn unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.0 >> 8) as f32 / 16777216.0
    }
}

pub fn uv(uv: [f32; 2], variant: u8) -> [f32; 2] {
    let tile = [variant as usize % GRID, variant as usize / GRID];
    std::array::from_fn(|k| {
        (tile[k] as f32 * TILE as f32 + 0.5 + uv[k] * (TILE - 1) as f32) / WIDTH as f32
    })
}

pub fn texture() -> Vec<u8> {
    let mut rgba = vec![0; WIDTH * WIDTH * 4];
    for tile in 0..GRID * GRID {
        let mut rng = Rng::new(0x93ab_52f1u32.wrapping_add(tile as u32 * 7919));
        // Overlapping off-centre pools have no repeated radial lobes.
        let mut blobs = vec![(0.0, 0.0, 0.30 + rng.unit() * 0.14, 0.30 + rng.unit() * 0.14)];
        for _ in 0..5 + (rng.unit() * 5.0) as usize {
            let a = rng.unit() * std::f32::consts::TAU;
            let r = 0.20 + rng.unit() * 0.26;
            let s = 0.10 + rng.unit() * 0.17;
            blobs.push((a.cos() * r, a.sin() * r, s, s * (0.55 + rng.unit() * 0.7)));
        }
        for _ in 0..8 + (rng.unit() * 15.0) as usize {
            let a = rng.unit() * std::f32::consts::TAU;
            let r = 0.48 + rng.unit() * 0.35;
            let s = 0.014 + rng.unit() * 0.052;
            blobs.push((a.cos() * r, a.sin() * r, s, s * (0.65 + rng.unit() * 0.6)));
        }
        for y in 0..TILE {
            for x in 0..TILE {
                let (u, v) = ((x as f32 - 31.5) / 31.5, (y as f32 - 31.5) / 31.5);
                let grain = rng.unit();
                let mut alpha = 0.0f32;
                for &(cx, cy, rx, ry) in &blobs {
                    let d = (((u - cx) / rx).powi(2) + ((v - cy) / ry).powi(2)).sqrt();
                    alpha = alpha.max(
                        ((1.0 - d) * rx.min(ry) * 40.0 + (grain - 0.5) * 0.22).clamp(0.0, 1.0),
                    );
                }
                // Keep the atlas cells isolated, including bilinear filtering.
                if x == 0 || y == 0 || x == TILE - 1 || y == TILE - 1 {
                    alpha = 0.0;
                }
                let shade = (175.0 + grain * 80.0) as u8;
                let i = ((tile / GRID * TILE + y) * WIDTH + tile % GRID * TILE + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[shade, shade, shade, (alpha * 255.0) as u8]);
            }
        }
    }
    rgba
}
