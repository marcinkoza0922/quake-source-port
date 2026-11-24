//! Socket highlight of the cable add-on: while the cable is held and not
//! plugged in, every surface that takes the plug near the player gets a
//! faint pulsing outline, and the one under the crosshair within reach a
//! brighter outline and fill. Uses the client's own BSP (so demos show it
//! too) with the server's texture rules (`hook_surface`, `cable::Rules`).
use qcore::math::Vec3;
use crate::gfx::Renderer;
use qrender::{FteBatch, FteBlend, FteVertex, PicId};
use qserver::addons::cable::REACH;
use qserver::addons::hook_surface::{Material, Surfaces, inside};

use super::cable_rope_fx::Geometry;

/// Faces this far from the eye get the faint outline.
const NEAR: f32 = 320.0;
const EDGE: f32 = 0.6;

#[derive(Default)]
pub struct SocketGlow {
    /// The BSP the surfaces were built from (address and face count).
    key: (usize, usize),
    surfaces: Surfaces,
    pic: Option<PicId>,
}

/// A candidate face moved into world space.
struct Cand {
    verts: Vec<Vec3>,
    normal: Vec3,
}

impl SocketGlow {
    fn surfaces(&mut self, lv: &crate::assets::Level) -> Option<&Surfaces> {
        let bsp = lv.bsp.as_ref()?;
        let key = (bsp as *const _ as usize, bsp.faces.len());
        if self.key != key {
            self.key = key;
            self.surfaces = Surfaces::new(bsp);
        }
        Some(&self.surfaces)
    }

    /// Outline nearby sockets and fill the one the player would plug into.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, cl: &qclient::ClientState, lv: &crate::assets::Level, eye: Vec3, fwd: Vec3, metal: bool, time: f64, r: &mut Renderer, geom: &mut Geometry) {
        let Some(cm) = lv.collision_model() else { return };
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let Ok(p) = r.register_pic_rgba_linear(4, 4, &[255; 64]) else { return };
                self.pic = Some(p);
                p
            }
        };
        let Some(surf) = self.surfaces(lv) else { return };
        let ok = |electronic: bool, m: Material| electronic || (metal && m == Material::Metal);
        let mut cands = Vec::new();
        let mut add = |sub: usize, origin: Vec3| {
            for f in surf.faces(sub) {
                if !ok(f.electronic, f.material) || f.verts.len() < 3 {
                    continue;
                }
                let verts: Vec<Vec3> = f.verts.iter().map(|&v| v + origin).collect();
                let centre = verts.iter().fold(Vec3::ZERO, |a, &v| a + v) * (1.0 / verts.len() as f32);
                // facing the eye, and near it
                let plane = eye.dot(f.normal) - (f.dist + origin.dot(f.normal));
                if plane <= 0.5 || plane > NEAR {
                    continue;
                }
                let radius = verts.iter().map(|&v| (v - centre).length()).fold(0.0, f32::max);
                if (centre - eye).length() - radius > NEAR {
                    continue;
                }
                cands.push(Cand { verts, normal: f.normal });
            }
        };
        add(0, Vec3::ZERO);
        for (_, e) in cl.active_entities() {
            let Some(sub) = cl.model_name(e.model).and_then(|n| n.strip_prefix('*')).and_then(|n| n.parse::<usize>().ok()) else { continue };
            if sub != 0 && e.angles == [0.0; 3] {
                add(sub, Vec3::from(e.origin));
            }
        }
        if cands.is_empty() {
            return;
        }
        // the face under the crosshair, not behind world geometry
        let wall = cm.hull(0, 0).trace(eye, eye + fwd * REACH);
        let limit = wall.fraction * REACH + 2.0;
        let mut target: Option<(f32, usize)> = None;
        for (k, c) in cands.iter().enumerate() {
            let den = fwd.dot(c.normal);
            if den >= -1e-3 {
                continue;
            }
            let t = (c.verts[0] - eye).dot(c.normal) / den;
            if t <= 0.0 || t > limit.min(REACH) {
                continue;
            }
            if inside(&c.verts, c.normal, eye + fwd * t) && target.is_none_or(|(bt, _)| t < bt) {
                target = Some((t, k));
            }
        }
        let pulse = 0.5 + 0.5 * (time as f32 * 4.0).sin();
        let first = geom.1.len() as u32;
        for (k, c) in cands.iter().enumerate() {
            let hot = target.is_some_and(|(_, t)| t == k);
            let a = if hot { 150.0 + 100.0 * pulse } else { 30.0 + 40.0 * pulse };
            let color = [70, 200, 255, a as u8];
            let lift = c.normal * 0.4;
            let n = c.verts.len();
            let centre = c.verts.iter().fold(Vec3::ZERO, |a, &v| a + v) * (1.0 / n as f32);
            for i in 0..n {
                let (p, q) = (c.verts[i] + lift, c.verts[(i + 1) % n] + lift);
                let d = q - p;
                let side = c.normal.cross(d).normalize() * if hot { EDGE * 1.6 } else { EDGE };
                // inside the polygon, not over its neighbours
                let side = if side.dot(centre - p) < 0.0 { side * -1.0 } else { side };
                quad(geom, [p, q, q + side, p + side], color);
            }
            if hot {
                let fill = [70, 200, 255, (25.0 + 35.0 * pulse) as u8];
                let base = geom.0.len() as u32;
                for &v in &c.verts {
                    geom.0.push(FteVertex { pos: (v + lift).to_array(), uv: [0.5, 0.5], color: fill });
                }
                for i in 1..n as u32 - 1 {
                    geom.1.extend([base, base + i, base + i + 1]);
                }
            }
        }
        let count = geom.1.len() as u32 - first;
        if count > 0 {
            geom.2.push(FteBatch { pic, blend: FteBlend::AddA, lines: false, first_index: first, num_indices: count, soft: false, gel: false });
        }
    }
}

fn quad(g: &mut Geometry, p: [Vec3; 4], color: [u8; 4]) {
    let base = g.0.len() as u32;
    for (v, uv) in p.iter().zip([[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]) {
        g.0.push(FteVertex { pos: v.to_array(), uv, color });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}
