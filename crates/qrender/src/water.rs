//! Optional liquid effects, CPU side: watches entities cross liquid surfaces
//! (and wade through them), keeps the ripple events the water and world
//! shaders read from the frame UBO, and simulates splash droplets that are
//! drawn as classic palette particles.

use crate::Particle;
use crate::world::World;
use std::collections::HashMap;

/// Ripple slots in the frame UBO.
pub const MAX_RIPPLES: usize = 32;
/// Seconds a ripple ring lives (must match `water_grad` in common.glsl).
pub const RIPPLE_LIFE: f32 = 2.5;
const MAX_DROPS: usize = 2048;
const GRAVITY: f32 = 800.0;
/// Key used for the player (view entity) track.
const PLAYER_KEY: u32 = u32::MAX;

#[derive(Clone, Copy, Debug)]
pub struct Ripple {
    /// x, y and the liquid surface height.
    pub pos: [f32; 3],
    pub t0: f32,
    pub amp: f32,
}

#[derive(Clone, Copy)]
struct Track {
    origin: [f32; 3],
    kind: u32,
    last_wake: f64,
    seen: u64,
}

struct Drop {
    pos: [f32; 3],
    vel: [f32; 3],
    color: u8,
    surface: f32,
    die: f64,
}

#[derive(Default)]
pub struct WaterFx {
    tracks: HashMap<u32, Track>,
    pub ripples: Vec<Ripple>,
    drops: Vec<Drop>,
    last_time: f64,
    frame: u64,
    rng: u32,
    /// Palette shade rows (start index, 16 entries ramp) per liquid kind 1..3.
    shades: [[u8; 4]; 3],
}

/// Index of the palette colour closest to `rgb`.
fn nearest(palette: &crate::convert::Palette, rgb: [i32; 3]) -> u8 {
    let mut best = (i32::MAX, 0u8);
    // skip the fullbright rows (224+) so droplets are lit like the old particles
    for (i, c) in palette.iter().enumerate().take(224) {
        let d = (0..3).map(|k| (c[k] as i32 - rgb[k]).pow(2)).sum::<i32>();
        if d < best.0 {
            best = (d, i as u8);
        }
    }
    best.1
}

impl WaterFx {
    pub fn set_palette(&mut self, palette: &crate::convert::Palette) {
        let pick = |cols: [[i32; 3]; 4]| cols.map(|c| nearest(palette, c));
        self.shades = [
            // water: pale blue-grey foam
            pick([[150, 160, 170], [115, 125, 140], [90, 105, 120], [180, 185, 190]]),
            // lava: glowing orange
            pick([[230, 120, 30], [200, 80, 20], [250, 170, 60], [170, 50, 10]]),
            // slime: murky green
            pick([[90, 150, 40], [70, 120, 30], [120, 170, 60], [50, 90, 20]]),
        ];
    }

    pub fn reset(&mut self) {
        self.tracks.clear();
        self.ripples.clear();
        self.drops.clear();
        self.last_time = 0.0;
    }

    fn rand(&mut self) -> f32 {
        // xorshift32
        let mut x = self.rng.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    fn add_ripple(&mut self, pos: [f32; 3], time: f64, amp: f32) {
        if self.ripples.len() >= MAX_RIPPLES {
            // replace the oldest
            if let Some(i) = (0..self.ripples.len()).min_by(|&a, &b| self.ripples[a].t0.total_cmp(&self.ripples[b].t0)) {
                self.ripples.remove(i);
            }
        }
        self.ripples.push(Ripple { pos, t0: time as f32, amp });
    }

    fn splash(&mut self, at: [f32; 3], kind: u32, strength: f32, time: f64) {
        let n = (6.0 + 26.0 * strength) as usize;
        let shades = self.shades[(kind as usize).clamp(1, 3) - 1];
        for _ in 0..n {
            if self.drops.len() >= MAX_DROPS {
                break;
            }
            let a = self.rand() * std::f32::consts::TAU;
            let r = 4.0 + 10.0 * self.rand();
            let hs = (25.0 + 70.0 * self.rand()) * strength.max(0.4);
            let vz = (110.0 + 170.0 * self.rand()) * strength.clamp(0.4, 1.4);
            let color = shades[(self.rand() * 4.0) as usize & 3];
            let life = 0.8 + 0.6 * self.rand() as f64;
            self.drops.push(Drop {
                pos: [at[0] + a.cos() * r, at[1] + a.sin() * r, at[2] + 1.0],
                vel: [a.cos() * hs, a.sin() * hs, vz],
                color,
                surface: at[2],
                die: time + life,
            });
        }
    }

    /// A bullet, nail or explosion impact at `hit`: splashes where the line
    /// from the view to it crosses a liquid surface (the shooter is unknown;
    /// the view is right for the player's own shots), and explosions just
    /// under a surface throw up a big splash above them.
    fn shot(&mut self, world: &World, eye: [f32; 3], hit: [f32; 3], time: f64, ripples: bool, splashes: bool) {
        let wet = |p: [f32; 3]| world.liquid_surface(p).map_or(0, |(_, k)| k & 3);
        let (we, wh) = (wet(eye), wet(hit));
        let emit = |fx: &mut Self, at: [f32; 3], k: u32, strength: f32| {
            if ripples {
                fx.add_ripple(at, time, 1.2 * strength);
            }
            if splashes {
                fx.splash(at, k, strength, time);
            }
        };
        if we != wh {
            // bisect the segment for the crossing
            let (mut a, mut b) = (eye, hit);
            for _ in 0..12 {
                let m = [0.5 * (a[0] + b[0]), 0.5 * (a[1] + b[1]), 0.5 * (a[2] + b[2])];
                if wet(m) == we { a = m } else { b = m }
            }
            let (dry, wetp) = if we == 0 { (a, b) } else { (b, a) };
            // only horizontal surfaces: the crossing must be vertical-ish
            if let Some((z, k)) = world.liquid_surface(wetp)
                && k & 4 == 0
                && (z - dry[2]).abs() < 4.0
            {
                emit(self, [wetp[0], wetp[1], z], k & 3, 0.35);
            }
        } else if wh != 0
            && let Some((z, k)) = world.liquid_surface(hit)
            && k & 4 == 0
            && z - hit[2] < 2.0
        {
            // impact right at the surface (e.g. shot along it)
            emit(self, [hit[0], hit[1], z], k & 3, 0.35);
        }
    }

    /// Advances tracks, ripples and droplets; returns droplet particles.
    /// `movers` are (key, origin) of alias entities; `player` is the view
    /// entity origin.
    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        world: &World,
        time: f64,
        movers: impl Iterator<Item = (u32, [f32; 3])>,
        player: Option<[f32; 3]>,
        eye: Option<[f32; 3]>,
        shots: &[[f32; 3]],
        ripples: bool,
        splashes: bool,
    ) -> Vec<Particle> {
        let dt = time - self.last_time;
        if !(0.0..=1.0).contains(&dt) {
            // level change, demo seek or a long pause: start over
            self.tracks.clear();
            self.ripples.clear();
            self.drops.clear();
        }
        self.last_time = time;
        self.frame += 1;
        self.rng ^= (time * 1000.0) as u32;
        let dt = dt.clamp(0.0, 0.1) as f32;
        if !ripples && !splashes {
            self.reset();
            return Vec::new();
        }
        // the player's origin is 24 units above the feet; splash at the shins
        let player = player.map(|p| (PLAYER_KEY, [p[0], p[1], p[2] - 16.0]));
        for (key, o) in movers.chain(player) {
            let kind = world.liquid_surface(o).map_or(0, |(_, k)| k & 3);
            let prev = self.tracks.insert(key, Track { origin: o, kind, last_wake: 0.0, seen: self.frame });
            let Some(prev) = prev else { continue };
            let d = [o[0] - prev.origin[0], o[1] - prev.origin[1], o[2] - prev.origin[2]];
            if d.iter().map(|c| c * c).sum::<f32>() > 256.0 * 256.0 {
                continue; // teleported
            }
            let tr = self.tracks.get_mut(&key).unwrap();
            tr.last_wake = prev.last_wake;
            if dt <= 0.0 {
                continue;
            }
            if prev.kind != kind {
                // crossed a surface: find it from the submerged end
                let (wet, k) = if kind != 0 { (o, kind) } else { (prev.origin, prev.kind) };
                let Some((z, _)) = world.liquid_surface(wet) else { continue };
                let vz = (d[2] / dt).abs();
                let speed = (d.iter().map(|c| c * c).sum::<f32>()).sqrt() / dt;
                let strength = (vz.max(speed * 0.5) / 350.0).clamp(0.25, 1.5) * if kind != 0 { 1.0 } else { 0.5 };
                let at = [o[0], o[1], z];
                if ripples {
                    self.add_ripple(at, time, 1.6 * strength);
                }
                if splashes {
                    self.splash(at, k, strength, time);
                }
            } else if kind != 0 && ripples {
                // wading near the surface leaves a wake
                let hs = (d[0] * d[0] + d[1] * d[1]).sqrt() / dt;
                if hs > 40.0
                    && time - prev.last_wake > 0.3
                    && let Some((z, k)) = world.liquid_surface(o)
                    && k & 4 == 0
                    && z - o[2] < 36.0
                {
                    self.tracks.get_mut(&key).unwrap().last_wake = time;
                    self.add_ripple([o[0], o[1], z], time, (hs / 320.0).clamp(0.2, 0.6));
                }
            }
        }
        if dt > 0.0
            && let Some(eye) = eye
        {
            for &hit in shots {
                self.shot(world, eye, hit, time, ripples, splashes);
            }
        }
        let frame = self.frame;
        self.tracks.retain(|_, t| t.seen == frame);
        self.ripples.retain(|r| time as f32 - r.t0 < RIPPLE_LIFE && time as f32 >= r.t0);

        // droplets: fall back into the liquid with a tiny ring
        let mut landed = Vec::new();
        self.drops.retain_mut(|p| {
            p.vel[2] -= GRAVITY * dt;
            for k in 0..3 {
                p.pos[k] += p.vel[k] * dt;
            }
            if p.pos[2] < p.surface && p.vel[2] < 0.0 {
                landed.push([p.pos[0], p.pos[1], p.surface]);
                return false;
            }
            time < p.die
        });
        if ripples {
            for at in landed.into_iter().take(2) {
                if self.ripples.len() < MAX_RIPPLES {
                    self.add_ripple(at, time, 0.25);
                }
            }
        }
        self.drops.iter().map(|p| Particle { origin: p.pos, color: p.color, alpha: 1.0 }).collect()
    }
}
