//! Opaque portal ovals (one textured quad each) in the owner's colour pair.
//! Nothing is built or uploaded until a portal exists.

use qcore::math::Vec3;
use qrender::{FteBatch, FteBlend, FteVertex, PicId, Renderer};
use qserver::addons::portal::{HALF_H, HALF_W};

use crate::blood::Geometry;

/// (fire, alt-fire) colours per player, cycling after the fourth.
const COLOURS: [[[u8; 3]; 2]; 4] = [
    [[40, 130, 255], [255, 140, 20]],
    [[60, 230, 90], [180, 70, 255]],
    [[40, 230, 230], [240, 50, 50]],
    [[250, 230, 40], [240, 60, 200]],
];
/// Lift off the wall to avoid z-fighting.
const OFFSET: f32 = 0.6;
const TEX: u32 = 64;

pub fn colour(owner: usize, slot: usize) -> [u8; 3] {
    COLOURS[owner % COLOURS.len()][slot & 1]
}

#[derive(Default)]
pub struct PortalDraw {
    pic: Option<PicId>,
}

impl PortalDraw {
    pub fn frame(&mut self, portals: &[qclient::PortalView], r: &mut Renderer, out: &mut Geometry) {
        if portals.is_empty() {
            return;
        }
        if self.pic.is_none() {
            self.pic = r.register_pic_rgba_linear(TEX, TEX, &texture()).ok();
        }
        let Some(pic) = self.pic else { return };
        let first = out.1.len() as u32;
        for p in portals {
            let (n, up) = (Vec3::from(p.normal), Vec3::from(p.up));
            let right = up.cross(n);
            let c = Vec3::from(p.origin) + n * OFFSET;
            let [cr, cg, cb] = colour(p.owner as usize, p.slot as usize);
            let base = out.0.len() as u32;
            for (u, v) in [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)] {
                let pos = c + right * ((u * 2.0 - 1.0) * HALF_W) + up * ((1.0 - v * 2.0) * HALF_H);
                out.0.push(FteVertex { pos: pos.to_array(), uv: [u, v], color: [cr, cg, cb, 255] });
            }
            out.1.extend([0, 1, 2, 0, 2, 3].map(|i| base + i));
        }
        out.2.push(FteBatch {
            pic,
            blend: FteBlend::Blend,
            lines: false,
            soft: false,
            first_index: first,
            num_indices: out.1.len() as u32 - first,
        });
    }
}

/// White rim and dark swirl-less centre inside an ellipse; tinted by the
/// vertex colour.
fn texture() -> Vec<u8> {
    let mut px = Vec::with_capacity((TEX * TEX * 4) as usize);
    for y in 0..TEX {
        for x in 0..TEX {
            let fx = (x as f32 + 0.5) / TEX as f32 * 2.0 - 1.0;
            let fy = (y as f32 + 0.5) / TEX as f32 * 2.0 - 1.0;
            let d = (fx * fx + fy * fy).sqrt();
            let alpha = ((1.0 - d) * 24.0).clamp(0.0, 1.0);
            let rim = (1.0 - ((d - 0.86) / 0.1).abs()).clamp(0.0, 1.0);
            let inner = 0.25 + 0.2 * d;
            let l = (inner + rim * 0.9).min(1.0);
            let lv = (l * 255.0) as u8;
            px.extend([lv, lv, lv, (alpha * 255.0) as u8]);
        }
    }
    px
}
