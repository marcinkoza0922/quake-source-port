//! Gel sprayer (Portal 2 homage): fire sprays blue Repulsion gel, alt-fire
//! orange Propulsion gel. Gel flies as engine-side blobs (no edicts) under
//! gravity; a blob reaching a world surface leaves a round splat on it.
//! Splats are level state: they persist, are saved in the `.qsa` header and
//! only go away when sprayed over, washed off by the powerwasher
//! ([`Gel::wash`]), pushed out by [`MAX_SPLATS`], or the toy is switched off.
//!
//! What gel does to movers is in `gel_phys.rs`.
//!
//! Clients learn everything from `//qsp_gel` stufftext:
//! `b owner kind sx sy sz vx vy vz` blob launched (the client flies it itself),
//! `a id kind x y z nx ny nz r` splat added, `d id` removed, `c` clear,
//! `s kind x y z speed` a footstep on gel, `j kind x y z speed` a bounce or
//! speed boost kick.

use qcore::math::{Vec3, angle_vectors};
use qworld::{CONTENTS_SKY, Edicts, Ent, MoveType, V, World};

use crate::client::Client;

/// Splats per level (all players); the oldest go first.
pub const MAX_SPLATS: usize = 1024;
/// Blobs in flight at once.
const MAX_BLOBS: usize = 256;
/// Seconds between blobs while a trigger is held.
const RATE: f64 = 0.045;
/// Muzzle speed of a blob (units/s).
const SPEED: f32 = 950.0;
/// Splat radius range.
const RADIUS: (f32, f32) = (26.0, 40.0);

pub const BLUE: u8 = 0;
pub const ORANGE: u8 = 1;

/// A patch of gel on a world surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Splat {
    pub id: u32,
    pub kind: u8,
    pub origin: Vec3,
    pub normal: Vec3,
    pub radius: f32,
}

#[derive(Clone, Copy, Debug)]
struct Blob {
    owner: usize,
    kind: u8,
    origin: Vec3,
    vel: Vec3,
}

#[derive(Clone, Debug, Default)]
pub struct Gel {
    pub splats: Vec<Splat>,
    blobs: Vec<Blob>,
    next_id: u32,
    next_blob: Vec<f64>,
    synced: Vec<bool>,
    outbox: Vec<String>,
    seed: u32,
    /// Movers before physics: (entity, origin, velocity, on ground).
    pub(crate) snapshot: Vec<(Ent, Vec3, Vec3, bool)>,
    /// Next footstep sound per player.
    pub(crate) next_step: Vec<f64>,
}

impl Gel {
    fn ensure(&mut self, n: usize) {
        if self.synced.len() < n {
            self.synced.resize(n, false);
            self.next_blob.resize(n, 0.0);
            self.next_step.resize(n, 0.0);
        }
    }

    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / 16_777_216.0
    }

    pub(crate) fn event(&mut self, s: String) {
        self.outbox.push(s);
    }

    /// Adds a splat, replacing the ones it mostly covers.
    pub fn add(&mut self, kind: u8, origin: Vec3, normal: Vec3, radius: f32) {
        let mut k = 0;
        while k < self.splats.len() {
            let s = self.splats[k];
            let same_plane = s.normal.dot(normal) > 0.9 && (s.origin - origin).dot(normal).abs() < 4.0;
            let d = (s.origin - origin).length();
            // the same colour is replaced when nearly on top; the other
            // colour wherever the new splat covers its centre
            let limit = if s.kind == kind { radius * 0.45 } else { radius * 0.8 };
            if same_plane && d < limit {
                self.remove(k);
            } else {
                k += 1;
            }
        }
        while self.splats.len() >= MAX_SPLATS {
            self.remove(0);
        }
        self.next_id = self.next_id.wrapping_add(1);
        let s = Splat { id: self.next_id, kind, origin, normal, radius };
        self.outbox.push(add_line(&s));
        self.splats.push(s);
    }

    fn remove(&mut self, k: usize) -> Splat {
        let s = self.splats.remove(k);
        self.outbox.push(format!("//qsp_gel d {}\n", s.id));
        s
    }

    /// Washes off every splat reaching within `r` of `p`. Returns how many.
    pub fn wash(&mut self, p: Vec3, r: f32) -> usize {
        let mut n = 0;
        let mut k = 0;
        while k < self.splats.len() {
            let s = self.splats[k];
            if (s.origin - p).length() < r + s.radius * 0.6 {
                self.remove(k);
                n += 1;
            } else {
                k += 1;
            }
        }
        n
    }

    /// Removes all gel (toy switched off) and tells the clients.
    pub(crate) fn clear(&mut self, clients: &mut [Client]) {
        self.splats.clear();
        self.blobs.clear();
        self.outbox.push("//qsp_gel c\n".into());
        self.send(clients);
    }

    /// Header lines: `gel kind x y z nx ny nz r`.
    pub fn save_text(&self) -> String {
        let mut s = String::new();
        for g in &self.splats {
            let (o, n) = (g.origin, g.normal);
            s += &format!("gel {} {} {} {} {} {} {} {}\n", g.kind, o.x, o.y, o.z, n.x, n.y, n.z, g.radius);
        }
        s
    }

    /// Reads the `gel` lines of a save header (other lines are skipped).
    pub fn load_text(&mut self, text: &str) {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("gel ") else { continue };
            let n: Vec<f32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
            if let [k, x, y, z, nx, ny, nz, r] = n[..]
                && n.iter().all(|v| v.is_finite())
            {
                self.add((k as u8).min(1), Vec3::new(x, y, z), Vec3::new(nx, ny, nz), r.clamp(4.0, 128.0));
            }
        }
        self.outbox.clear();
    }

    /// Full state for newly spawned clients, then the news to everyone.
    pub(crate) fn send(&mut self, clients: &mut [Client]) {
        self.ensure(clients.len());
        for (i, cl) in clients.iter_mut().enumerate() {
            if !cl.spawned {
                self.synced[i] = false;
                continue;
            }
            if !self.synced[i] {
                self.synced[i] = true;
                write_stuff(cl, "//qsp_gel c\n");
                for s in &self.splats {
                    write_stuff(cl, &add_line(s));
                }
            } else {
                for s in &self.outbox {
                    write_stuff(cl, s);
                }
            }
        }
        self.outbox.clear();
    }

    /// One server frame: spray from held triggers, then fly the blobs.
    pub(crate) fn frame<D: Edicts + ?Sized>(&mut self, w: &World, ents: &D, clients: &[Client], now: f64, dt: f32, gravity: f32) {
        self.ensure(clients.len());
        for (i, cl) in clients.iter().enumerate() {
            let b = cl.cmd.buttons;
            if !cl.spawned || super::Toy::from_buttons(b) != Some(super::Toy::GelSprayer) {
                continue;
            }
            let e = i as Ent + 1;
            if ents.f(e, qworld::F::Health) <= 0.0 {
                continue;
            }
            let kind = if b & super::BUTTON_TOY_FIRE != 0 {
                BLUE
            } else if b & super::BUTTON_TOY_ALT != 0 {
                ORANGE
            } else {
                continue;
            };
            if now < self.next_blob[i] {
                continue;
            }
            self.next_blob[i] = now.max(self.next_blob[i]) + RATE;
            if self.blobs.len() >= MAX_BLOBS {
                self.blobs.remove(0);
            }
            let (fwd, right, up) = angle_vectors(ents.v(e, V::VAngle));
            let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
            let (sx, sy) = (self.random() - 0.5, self.random() - 0.5);
            let dir = (fwd + right * sx * 0.07 + up * sy * 0.07).normalize();
            let origin = eye + fwd * 12.0 - up * 6.0;
            let vel = dir * SPEED * (0.92 + self.random() * 0.16) + ents.v(e, V::Velocity) * 0.5;
            self.outbox.push(format!(
                "//qsp_gel b {i} {kind} {:.1} {:.1} {:.1} {:.1} {:.1} {:.1}\n",
                origin.x, origin.y, origin.z, vel.x, vel.y, vel.z
            ));
            self.blobs.push(Blob { owner: i, kind, origin, vel });
        }
        let mut k = 0;
        while k < self.blobs.len() {
            let mut b = self.blobs[k];
            b.vel.z -= gravity * dt;
            let end = b.origin + b.vel * dt;
            let tr = w.trace(ents, b.origin, Vec3::ZERO, Vec3::ZERO, end, MoveType::Missile, Some(b.owner as Ent + 1));
            if tr.allsolid || b.origin.z.abs() > 16384.0 {
                self.blobs.remove(k);
                continue;
            }
            let Some(hit) = tr.ent.filter(|_| tr.fraction < 1.0) else {
                b.origin = end;
                self.blobs[k] = b;
                k += 1;
                continue;
            };
            self.blobs.remove(k);
            let (pos, normal) = if hit == 0 {
                (tr.endpos, tr.plane_normal)
            } else {
                // a monster or mover: the gel drips to the floor below it
                let t = w.trace(ents, tr.endpos, Vec3::ZERO, Vec3::ZERO, tr.endpos - Vec3::new(0.0, 0.0, 512.0), MoveType::NoMonsters, None);
                if t.fraction >= 1.0 || t.ent != Some(0) {
                    continue;
                }
                (t.endpos, t.plane_normal)
            };
            if w.point_contents(pos - normal * 2.0) == CONTENTS_SKY {
                continue;
            }
            let r = RADIUS.0 + self.random() * (RADIUS.1 - RADIUS.0);
            self.add(b.kind, pos, normal, r);
        }
    }
}

fn add_line(s: &Splat) -> String {
    let (o, n) = (s.origin, s.normal);
    format!("//qsp_gel a {} {} {:.2} {:.2} {:.2} {:.4} {:.4} {:.4} {:.1}\n", s.id, s.kind, o.x, o.y, o.z, n.x, n.y, n.z, s.radius)
}

fn write_stuff(cl: &mut Client, s: &str) {
    cl.message.byte(crate::msg::SVC_STUFFTEXT);
    cl.message.string(s.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splats_replace_overlaps_and_round_trip_saves() {
        let up = Vec3::new(0.0, 0.0, 1.0);
        let mut g = Gel::default();
        g.add(BLUE, Vec3::ZERO, up, 30.0);
        g.add(BLUE, Vec3::new(100.0, 0.0, 0.0), up, 30.0);
        // orange over the first blue one replaces it
        g.add(ORANGE, Vec3::new(10.0, 0.0, 0.0), up, 30.0);
        assert_eq!(g.splats.len(), 2);
        assert_eq!(g.splats[1].kind, ORANGE);
        // a wall splat nearby is on another plane and stays
        g.add(BLUE, Vec3::new(10.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 30.0);
        assert_eq!(g.splats.len(), 3);
        let mut h = Gel::default();
        h.load_text(&format!("sticky 1\n{}", g.save_text()));
        assert_eq!(h.save_text(), g.save_text());
        assert_eq!(g.wash(Vec3::new(100.0, 0.0, 0.0), 20.0), 1);
        assert_eq!(g.splats.len(), 2);
    }
}
