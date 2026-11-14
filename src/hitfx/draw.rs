//! Camera-facing geometry for hit particles and their small texture atlas.
use super::{Kind, Part};
use crate::blood::Geometry;
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, View};

pub(super) const W: u32 = 96;
pub(super) const H: u32 = 32;

/// Atlas cells: soft glow, hard-edged fragment, billowy smoke puff.
const GLOW: f32 = 0.0;
const CHIP: f32 = 1.0;
const PUFF: f32 = 2.0;

pub(super) fn texture() -> Vec<u8> {
    let mut data = vec![255u8; (W * H * 4) as usize];
    for y in 0..32 {
        for x in 0..32 {
            let dx = (x as f32 - 15.5) / 15.5;
            let dy = (y as f32 - 15.5) / 15.5;
            let r = (dx * dx + dy * dy).sqrt();
            let a = |cell: u32, v: f32| {
                let i = ((y * W + x + cell * 32) * 4 + 3) as usize;
                (i, (v.clamp(0.0, 1.0) * 255.0) as u8)
            };
            let glow = (1.0 - r).max(0.0).powf(1.8);
            // Irregular, mostly solid shard with a 1px soft edge.
            let edge = 0.8 + 0.15 * ((x * 7 + y * 13) % 5) as f32 / 5.0;
            let chip = (edge - dx.abs().max(dy.abs())) * 10.0;
            let noise = ((x * 37 + y * 61 + x * y * 11) % 23) as f32 / 23.0;
            let puff = ((1.0 - r) * 1.6 - noise * 0.5).clamp(0.0, 1.0).powf(1.3);
            for (cell, v) in [(0, glow), (1, chip), (2, puff)] {
                let (i, v) = a(cell, v);
                data[i] = v;
            }
        }
    }
    data
}

fn uv(cell: f32, u: f32, v: f32) -> [f32; 2] {
    [(cell + u) / 3.0, v]
}

fn quad(g: &mut Geometry, corners: [Vec3; 4], cell: f32, color: [u8; 3], alpha: f32) {
    let base = g.0.len() as u32;
    let a = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    for (p, (u, v)) in corners.into_iter().zip([(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]) {
        g.0.push(FteVertex { pos: p.to_array(), uv: uv(cell, u, v), color: [color[0], color[1], color[2], a] });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn batch(g: &mut Geometry, pic: PicId, first: u32, blend: FteBlend, soft: bool) {
    let n = g.1.len() as u32 - first;
    if n > 0 {
        g.2.push(FteBatch { pic, blend, lines: false, soft, gel: false, first_index: first, num_indices: n });
    }
}

pub(super) fn draw(parts: &[Part], view: &View, pic: PicId, g: &mut Geometry) {
    let (_, right, up) = angle_vectors(view.angles.into());
    let eye = Vec3::from(view.origin);
    // Smoke first (soft, alpha), then solid fragments, then additive light.
    let first = g.1.len() as u32;
    for p in parts.iter().filter(|p| p.kind == Kind::Smoke) {
        let t = p.age / p.life;
        let alpha = p.alpha * (p.age * 10.0).min(1.0) * (1.0 - t).powf(1.5);
        let (s, c) = p.angle.sin_cos();
        let a = (right * c + up * s) * p.size;
        let b = (up * c - right * s) * p.size;
        quad(g, [p.pos - a - b, p.pos + a - b, p.pos + a + b, p.pos - a + b], PUFF, p.color, alpha);
    }
    batch(g, pic, first, FteBlend::Blend, true);
    let first = g.1.len() as u32;
    for p in parts.iter().filter(|p| p.kind == Kind::Chip) {
        let alpha = ((p.life - p.age) / 0.4).min(1.0);
        let (s, c) = p.angle.sin_cos();
        let a = (right * c + up * s) * (p.size * p.stretch);
        let b = (up * c - right * s) * p.size;
        quad(g, [p.pos - a - b, p.pos + a - b, p.pos + a + b, p.pos - a + b], CHIP, p.color, alpha);
    }
    batch(g, pic, first, FteBlend::Blend, false);
    let first = g.1.len() as u32;
    for p in parts.iter().filter(|p| matches!(p.kind, Kind::Spark | Kind::Flash)) {
        let fade = 1.0 - p.age / p.life;
        if p.kind == Kind::Flash {
            let s = p.size * (0.6 + 0.4 * fade);
            quad(g, [p.pos - right * s - up * s, p.pos + right * s - up * s, p.pos + right * s + up * s,
                p.pos - right * s + up * s], GLOW, p.color, fade);
            continue;
        }
        // Streak from the head back along the velocity, as a motion-blurred ember.
        let tail = p.pos - p.vel * 0.03;
        let len = (p.pos - tail).length();
        let along = if len > 0.1 { (p.pos - tail) / len } else { up };
        let side = along.cross(eye - p.pos).normalize();
        let side = if side.length_sq() > 0.5 { side * p.size } else { right * p.size };
        let head = p.pos + along * p.size;
        let tail = tail - along * p.size;
        // Hot white-yellow cooling to orange.
        let col = [p.color[0], (p.color[1] as f32 * (0.6 + 0.4 * fade)) as u8, (p.color[2] as f32 * fade) as u8];
        quad(g, [tail - side, head - side, head + side, tail + side], GLOW, col, fade.sqrt());
    }
    batch(g, pic, first, FteBlend::AddA, false);
}
