//! Client presentation of the hookshot: chain and hook head (FTE quads
//! from the server-sent head positions), synthesised rattles and clanks,
//! and the imported launcher view model.
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, RenderEntity, Renderer, View};
use qsound::{Sfx, SoundEvent, SoundSystem};
use std::sync::Arc;

type Geometry = (Vec<FteVertex>, Vec<u32>, Vec<FteBatch>);

/// Chain link spacing and size (units).
const LINK: f32 = 6.0;
const LINK_LEN: f32 = 4.4;
const LINK_W: f32 = 2.6;
const HEAD: f32 = 7.0;

#[derive(Default)]
pub struct HookFx {
    /// Last known state per owner (for transition sounds) and fire time.
    prev: Vec<(u8, f64)>,
    rate: u32,
    cues: Vec<Arc<Sfx>>,
    pics: Option<(PicId, PicId)>,
}

impl HookFx {
    pub fn clear(&mut self) {
        self.prev.clear();
    }

    /// A hook changed state: play its cue at the head.
    pub fn receive(&mut self, h: qclient::HookView, time: f64, snd: Option<&SoundSystem>) {
        let o = h.owner as usize;
        if self.prev.len() <= o {
            self.prev.resize(o + 1, (0, -100.0));
        }
        let before = self.prev[o].0;
        self.prev[o] = (h.state, if h.state == 1 { time } else { self.prev[o].1 });
        let cue = match (before, h.state) {
            (_, 1) => 0,
            (_, 2) => 1,
            (1, 3) => 3,
            (_, 3) => 2,
            _ => return,
        };
        self.play(cue, h.head, snd);
        if before == 1 && h.state == 3 {
            self.play(2, h.head, snd);
        }
    }

    fn play(&mut self, cue: usize, origin: [f32; 3], snd: Option<&SoundSystem>) {
        let Some(snd) = snd else { return };
        if self.rate != snd.rate() || self.cues.is_empty() {
            self.rate = snd.rate();
            self.cues = (0..4).map(|k| Arc::new(qsound::hookshot::render(k, snd.rate()))).collect();
        }
        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx: self.cues[cue].clone(), origin, volume: 0.9, attenuation: 0.8 });
    }

    /// The launcher view model for player `owner`.
    pub fn weapon(&self, owner: usize, time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<RenderEntity> {
        let key = "@hookshot";
        if !lv.by_name.contains_key(key) {
            let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
            let model = r
                .load_alias(&super::hook_model::model(&palette))
                .ok()
                .map(|id| crate::assets::Model { id, kind: crate::assets::Kind::Alias, flags: 0 });
            lv.by_name.insert(key.into(), model);
        }
        let model = lv.by_name.get(key).copied().flatten()?;
        let shot = self.prev.get(owner).map_or(-100.0, |p| p.1);
        let recoil = (-((time - shot).max(0.0) as f32) * 14.0).exp();
        let bob = (time as f32 * 2.5).sin() * 0.15;
        // held low and centred; the Right/Left view-model setting adds the side offset
        let (f, _, u) = angle_vectors(view.angles.into());
        let origin = (Vec3::from(view.origin) + f * (22.0 - recoil * 4.0) - u * (13.0 + bob)).to_array();
        let angles = [-view.angles[0] - recoil * 6.0, view.angles[1], view.angles[2]];
        Some(RenderEntity { model: model.id, origin, angles, viewmodel: true, ..Default::default() })
    }

    /// Chain and head quads for every live hook. `me` is the local player
    /// (index 0-based), whose chain leaves the view model's muzzle.
    pub fn frame(&mut self, cl: &qclient::ClientState, me: usize, view: &View, r: &mut Renderer, geom: &mut Geometry) {
        if cl.hooks.is_empty() {
            return;
        }
        let (link, head) = match self.pics {
            Some(p) => p,
            None => {
                let (Ok(a), Ok(b)) = (r.register_pic_rgba_linear(16, 16, &link_pic()), r.register_pic_rgba_linear(16, 16, &head_pic())) else { return };
                self.pics = Some((a, b));
                (a, b)
            }
        };
        let (vf, vr, vu) = angle_vectors(view.angles.into());
        let eye = Vec3::from(view.origin);
        let first = geom.1.len() as u32;
        let mut heads = Vec::new();
        for h in &cl.hooks {
            let end = Vec3::from(h.head);
            let start = if h.owner as usize == me {
                eye + vf * 40.0 + vr * 4.0 - vu * 11.0
            } else {
                match cl.entities.get(h.owner as usize + 1) {
                    Some(e) => Vec3::from(e.origin) + Vec3::new(0.0, 0.0, 16.0),
                    None => continue,
                }
            };
            let d = end - start;
            let len = d.length();
            if len < 1.0 {
                continue;
            }
            let dir = d / len;
            // link quads lie along the chain and face the camera as well
            // as they can; every other link is turned a quarter
            let side = dir.cross(eye - start).normalize();
            let side = if side.length_sq() > 0.5 { side } else { vr };
            let n = ((len / LINK) as usize).min(800);
            for i in 0..n {
                let c = start + dir * (i as f32 + 0.5) * LINK;
                let w = if i % 2 == 0 { LINK_W } else { LINK_W * 0.55 };
                quad(geom, c, dir * LINK_LEN, side * w, [255; 4]);
            }
            heads.push((end, dir, side));
        }
        geom.2.push(FteBatch { pic: link, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: geom.1.len() as u32 - first, soft: false, gel: false });
        let first = geom.1.len() as u32;
        for (end, dir, side) in heads {
            quad(geom, end - dir * HEAD * 0.4, dir * HEAD, side * HEAD, [255; 4]);
        }
        geom.2.push(FteBatch { pic: head, blend: FteBlend::Blend, lines: false, first_index: first, num_indices: geom.1.len() as u32 - first, soft: false, gel: false });
    }
}

/// Quad centred at `c` with half-axes `a` (texture u) and `b` (texture v).
fn quad(g: &mut Geometry, c: Vec3, a: Vec3, b: Vec3, color: [u8; 4]) {
    let base = g.0.len() as u32;
    for (x, y, uv) in [(-1.0, -1.0, [0.0, 0.0]), (1.0, -1.0, [1.0, 0.0]), (1.0, 1.0, [1.0, 1.0]), (-1.0, 1.0, [0.0, 1.0])] {
        g.0.push(FteVertex { pos: (c + a * x + b * y).to_array(), uv, color });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}

/// Dark iron oval ring: one chain link (u along the chain).
fn link_pic() -> Vec<u8> {
    pic(|x, y| {
        let d = ((x / 1.0).powi(2) + (y / 0.62).powi(2)).sqrt();
        ((0.55..0.98).contains(&d)).then(|| {
            let shade = 1.0 - (d - 0.76).abs() * 3.0 - y * 0.25;
            [(92.0 * shade) as u8, (78.0 * shade) as u8, (62.0 * shade) as u8]
        })
    })
}

/// Hook head: a spike forward (+u) with two barbs swept back.
fn head_pic() -> Vec<u8> {
    pic(|x, y| {
        let spike = y.abs() < 0.22 * (1.0 - x).max(0.0) + 0.06 && x > -0.9;
        let barb = |s: f32| {
            let (bx, by) = (x - 0.1, y * s - 0.15);
            by > 0.0 && by < 0.75 && (bx + by * 0.9).abs() < 0.16
        };
        (spike || barb(1.0) || barb(-1.0)).then(|| {
            let shade = 0.7 + 0.3 * (1.0 - y.abs());
            [(120.0 * shade) as u8, (112.0 * shade) as u8, (100.0 * shade) as u8]
        })
    })
}

fn pic(f: impl Fn(f32, f32) -> Option<[u8; 3]>) -> Vec<u8> {
    let mut out = Vec::with_capacity(16 * 16 * 4);
    for j in 0..16 {
        for i in 0..16 {
            let (x, y) = ((i as f32 + 0.5) / 8.0 - 1.0, (j as f32 + 0.5) / 8.0 - 1.0);
            match f(x, y) {
                Some([r, g, b]) => out.extend([r, g, b, 255]),
                None => out.extend([0, 0, 0, 0]),
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pics_have_shape() {
        for p in [link_pic(), head_pic()] {
            let opaque = p.chunks(4).filter(|c| c[3] == 255).count();
            assert!(opaque > 20 && opaque < 200, "{opaque}");
        }
    }
}
