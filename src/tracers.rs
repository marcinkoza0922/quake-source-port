//! Shotgun tracers: thin glowing streaks for shotgun pellets. Projectile
//! pellets (Add-ons, flagged `EF_QSP_PELLET`) are drawn as a streak at their
//! real position instead of their nail model. With `r_shotgun_tracers`,
//! hitscan shots get a streak too: each `TE_GUNSHOT` (or blood hit during a
//! recent muzzle flash) launches one that races from the inferred shooter
//! to the impact. Client-only and cosmetic.
use crate::blood::Geometry;
use qclient::{ClientState, EF_QSP_PELLET};
use qcore::math::{Vec3, angle_vectors};
use qrender::{FteBatch, FteBlend, FteVertex, PicId, Renderer, View};

/// Hitscan streak speed (units/s).
const SPEED: f32 = 7000.0;
/// Streak length (units).
const LEN: f32 = 56.0;
/// Hitscan streaks start this far out, so the shooter's eye stays clear.
const SKIP: f32 = 40.0;
/// Half-width of a streak.
const WIDTH: f32 = 0.6;
/// A gunshot this close to a projectile pellet of the last frame was that
/// pellet's impact, which already had its streak.
const PELLET_REACH: f32 = 200.0;
const COLOR: [u8; 3] = [255, 225, 170];
const MAX: usize = 256;

struct Shot {
    from: Vec3,
    to: Vec3,
    start: f64,
}

#[derive(Default)]
pub struct Tracers {
    shots: Vec<Shot>,
    pellets: Vec<Vec3>,
    pic: Option<PicId>,
}

impl Tracers {
    pub fn clear(&mut self) {
        self.shots.clear();
        self.pellets.clear();
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(&mut self, hitscan: bool, cl: &ClientState, debris: &crate::debris::Debris, view: &View, r: &mut Renderer, out: &mut Geometry) {
        let time = cl.time;
        self.shots.retain(|s| s.start <= time);
        let eye = Vec3::from(view.origin);
        if hitscan {
            let flash = debris.recent_flash();
            let gunshots = cl.weapon_impacts.iter().filter(|i| i.0 == 0).map(|i| i.1);
            let blood = cl.blood_hits.iter().copied().filter(|_| flash);
            for p in gunshots.chain(blood).map(Vec3::from) {
                if self.pellets.iter().any(|&q| (q - p).length() < PELLET_REACH) {
                    continue;
                }
                let from = debris.shot_origin(eye, p);
                if (p - from).length() > SKIP + 8.0 && self.shots.len() < MAX {
                    self.shots.push(Shot { from, to: p, start: time });
                }
            }
        }
        // Projectile pellets: a streak behind each, as long as it has flown.
        let mut segs = Vec::new();
        self.pellets.clear();
        let dt = (cl.mtime[0] - cl.mtime[1]) as f32;
        for (_, e) in cl.active_entities() {
            if e.effects & EF_QSP_PELLET == 0 {
                continue;
            }
            let head = Vec3::from(e.origin);
            self.pellets.push(head);
            let d = Vec3::from(e.msg_origins[0]) - Vec3::from(e.msg_origins[1]);
            if dt <= 0.001 || d.length() < 1.0 {
                continue;
            }
            segs.push((head - d.normalize() * LEN.min(d.length()), head, 1.0));
        }
        self.shots.retain(|s| {
            let along = (s.to - s.from).length();
            let dir = (s.to - s.from) / along;
            let head = SKIP + (time - s.start) as f32 * SPEED;
            let tail = head - LEN;
            if tail >= along {
                return false;
            }
            segs.push((s.from + dir * tail.max(SKIP), s.from + dir * head.min(along), 1.0));
            true
        });
        if segs.is_empty() {
            return;
        }
        let pic = match self.pic {
            Some(p) => p,
            None => {
                let pixels: Vec<u8> = (0..16 * 16)
                    .flat_map(|i| {
                        let d = (((i % 16) as f32 - 7.5).powi(2) + ((i / 16) as f32 - 7.5).powi(2)).sqrt() / 7.5;
                        [255, 255, 255, ((1.0 - d).max(0.0).powf(1.5) * 255.0) as u8]
                    })
                    .collect();
                let Ok(p) = r.register_pic_rgba_linear(16, 16, &pixels) else { return };
                *self.pic.insert(p)
            }
        };
        let (_, right, _) = angle_vectors(view.angles.into());
        let first = out.1.len() as u32;
        for (tail, head, a) in segs {
            streak(out, tail, head, eye, right, a);
        }
        let n = out.1.len() as u32 - first;
        out.2.push(FteBatch { pic, blend: FteBlend::AddA, lines: false, first_index: first, num_indices: n, soft: false, gel: false });
    }
}

/// A camera-facing quad from `tail` to `head`, faint at the tail.
fn streak(g: &mut Geometry, tail: Vec3, head: Vec3, eye: Vec3, right: Vec3, a: f32) {
    let along = (head - tail).normalize();
    let side = along.cross(eye - head).normalize();
    // Widen with distance so a far streak stays about a pixel and a half.
    let w = WIDTH.max((head - eye).length() * 0.0025);
    let side = if side.length_sq() > 0.5 { side * w } else { right * w };
    let (head, tail) = (head + along * w, tail - along * w);
    let base = g.0.len() as u32;
    let c = |k: f32| [COLOR[0], COLOR[1], COLOR[2], (a * k * 255.0) as u8];
    for (pos, uv, col) in [
        (tail - side, [0.0, 0.0], c(0.0)),
        (head - side, [0.5, 0.0], c(1.0)),
        (head + side, [0.5, 1.0], c(1.0)),
        (tail + side, [0.0, 1.0], c(0.0)),
    ] {
        g.0.push(FteVertex { pos: pos.to_array(), uv, color: col });
    }
    g.1.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
}
