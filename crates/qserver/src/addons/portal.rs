//! Portal gun: each player owns two linked portals (fire = slot 0,
//! alt-fire = slot 1) on static, flat, dry world surfaces. Placement lives
//! here; crossing is in `portal_move`. Clients learn about portals from
//! `//qsp_portal` stufftext lines, which other engines skip as comments, so
//! demos stay playable elsewhere.

use qcore::math::{Vec3, angle_vectors};
use qworld::{CONTENTS_EMPTY, CONTENTS_SOLID, Edicts, Ent, MoveType, V, World};

use crate::client::Client;

/// Half width and half height of a portal (fits the 32×32×56 player hull).
pub const HALF_W: f32 = 28.0;
pub const HALF_H: f32 = 48.0;
/// Longest shot.
const RANGE: f32 = 8192.0;
const FL_MONSTER: i32 = 32;
/// How far past a monster's footprint a floor shot still counts as
/// "under its feet".
const MONSTER_SLACK: f32 = 16.0;

/// A placed portal: centre, outward normal and in-plane up / right axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Portal {
    pub origin: Vec3,
    pub normal: Vec3,
    pub up: Vec3,
    pub right: Vec3,
}

impl Portal {
    pub fn new(origin: Vec3, normal: Vec3, up: Vec3) -> Self {
        Portal { origin, normal, up, right: up.cross(normal) }
    }
    /// Point at in-plane offsets (`r` along right, `u` along up).
    pub fn at(&self, r: f32, u: f32) -> Vec3 {
        self.origin + self.right * r + self.up * u
    }
    /// The stufftext line clients parse (see `qclient` `PortalView`).
    fn line(owner: usize, slot: usize, p: Option<&Portal>) -> String {
        match p {
            None => format!("//qsp_portal {owner} {slot}\n"),
            Some(p) => {
                let v = |v: Vec3| format!("{} {} {}", v.x, v.y, v.z);
                format!("//qsp_portal {owner} {slot} {} {} {}\n", v(p.origin), v(p.normal), v(p.up))
            }
        }
    }
}

/// Every player's portals plus per-client bookkeeping; lives on `Level`.
#[derive(Clone, Debug, Default)]
pub struct Portals {
    /// Indexed by client slot.
    pub pairs: Vec<[Option<Portal>; 2]>,
    /// Usercmd buttons last frame (edge detection).
    prev_buttons: Vec<u8>,
    /// Client has been sent the full portal state since it spawned.
    synced: Vec<bool>,
    /// Pending stufftext for every client.
    outbox: Vec<String>,
    /// (entity, origin, velocity) before this frame's physics.
    pub(crate) snapshot: Vec<(Ent, Vec3, Vec3)>,
}

impl Portals {
    fn ensure(&mut self, n: usize) {
        if self.pairs.len() != n {
            self.pairs.resize(n, [None, None]);
            self.prev_buttons.resize(n, 0);
            self.synced.resize(n, false);
        }
    }

    /// Some player has both portals down (teleporting is possible).
    pub fn any_linked(&self) -> bool {
        self.pairs.iter().any(|p| p[0].is_some() && p[1].is_some())
    }

    pub fn set(&mut self, owner: usize, slot: usize, p: Option<Portal>) {
        self.ensure(self.pairs.len().max(owner + 1));
        self.pairs[owner][slot] = p;
        self.outbox.push(Portal::line(owner, slot, p.as_ref()));
    }

    /// Fire edges from each client's usercmd buttons; returns
    /// (client, slot) presses with the portal gun held.
    pub(crate) fn presses(&mut self, clients: &[Client]) -> Vec<(usize, usize)> {
        self.ensure(clients.len());
        let mut out = Vec::new();
        for (i, cl) in clients.iter().enumerate() {
            let b = if cl.spawned && super::Toy::from_buttons(cl.cmd.buttons) == Some(super::Toy::PortalGun) {
                cl.cmd.buttons
            } else {
                0
            };
            let new = b & !self.prev_buttons[i];
            self.prev_buttons[i] = b;
            if new & super::BUTTON_TOY_FIRE != 0 {
                out.push((i, 0));
            }
            if new & super::BUTTON_TOY_ALT != 0 {
                out.push((i, 1));
            }
            if !cl.active && self.pairs[i] != [None, None] {
                self.set(i, 0, None);
                self.set(i, 1, None);
            }
        }
        out
    }

    /// Queue state for newly spawned clients and flush the outbox into
    /// every spawned client's reliable message.
    pub(crate) fn send(&mut self, clients: &mut [Client]) {
        self.ensure(clients.len());
        for (i, cl) in clients.iter_mut().enumerate() {
            if !cl.spawned {
                self.synced[i] = false;
                continue;
            }
            if !self.synced[i] {
                self.synced[i] = true;
                for (owner, pair) in self.pairs.iter().enumerate() {
                    for (slot, p) in pair.iter().enumerate() {
                        if let Some(p) = p {
                            write_stuff(cl, &Portal::line(owner, slot, Some(p)));
                        }
                    }
                }
            } else {
                for s in &self.outbox {
                    write_stuff(cl, s);
                }
            }
        }
        self.outbox.clear();
    }

    /// Sidecar save text: one `owner slot ox oy oz nx ny nz ux uy uz` line
    /// per portal (None when there are none).
    pub fn save_text(&self) -> Option<String> {
        let mut s = String::new();
        for (owner, pair) in self.pairs.iter().enumerate() {
            for (slot, p) in pair.iter().enumerate() {
                if let Some(p) = p {
                    s += &Portal::line(owner, slot, Some(p))["//qsp_portal ".len()..];
                }
            }
        }
        (!s.is_empty()).then_some(s)
    }

    pub fn load_text(&mut self, text: &str) {
        for line in text.lines() {
            let n: Vec<f32> = line.split_whitespace().filter_map(|w| w.parse().ok()).collect();
            if let [o, s, ox, oy, oz, nx, ny, nz, ux, uy, uz] = n[..]
                && (s as usize) < 2
            {
                let p = Portal::new(Vec3::new(ox, oy, oz), Vec3::new(nx, ny, nz), Vec3::new(ux, uy, uz));
                self.set(o as usize, s as usize, Some(p));
            }
        }
    }
}

fn write_stuff(cl: &mut Client, s: &str) {
    cl.message.byte(crate::msg::SVC_STUFFTEXT);
    cl.message.string(s.as_bytes());
}

/// Where a shot from player `e` would put a portal, or None when the spot
/// is not static, flat, dry world surface big enough (after nudging the
/// portal away from edges). `others` are the portals it may not overlap.
pub(crate) fn place<D: Edicts + ?Sized>(w: &World, ents: &D, e: Ent, others: &[Portal]) -> Option<Portal> {
    let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
    let (fwd, _, _) = angle_vectors(ents.v(e, V::VAngle));
    let tr = w.trace(ents, eye, Vec3::ZERO, Vec3::ZERO, eye + fwd * RANGE, MoveType::NoMonsters, Some(e));
    if tr.fraction >= 1.0 || tr.ent != Some(0) || tr.startsolid {
        return None;
    }
    let n = tr.plane_normal;
    // solid behind (not sky, water surface or a window into it)
    if w.point_contents(tr.endpos - n * 2.0) != CONTENTS_SOLID {
        return None;
    }
    let up = if n.z.abs() > 0.7 {
        // floor / ceiling: the portal's top faces away from the shooter
        let f = fwd - n * fwd.dot(n);
        if f.length_sq() < 1e-4 { Vec3::new(1.0, 0.0, 0.0) } else { f.normalize() }
    } else {
        let z = Vec3::new(0.0, 0.0, 1.0);
        (z - n * z.dot(n)).normalize()
    };
    let mut p = Portal::new(tr.endpos, n, up);
    if n.z > 0.99
        && let Some(o) = under_monster(w, ents, e, eye, fwd, tr.endpos)
    {
        p.origin = o;
    }
    let plane = tr.endpos.dot(n);
    for _ in 0..8 {
        let mut push = Vec3::ZERO;
        let mut bad = false;
        for (r, u) in samples() {
            if !sample_ok(w, ents, &p, plane, r, u) {
                bad = true;
                push -= p.right * sgn(r) + p.up * sgn(u);
            }
        }
        if !bad {
            return (!overlaps(&p, others)).then_some(p);
        }
        if push.length_sq() < 1e-4 {
            return None;
        }
        p.origin += push.normalize() * 8.0;
    }
    None
}

/// A floor shot at (or just past) a monster's feet: the spot under the
/// monster standing on that floor, so it drops in. Otherwise the shot would
/// miss its feet (monsters don't block the shot) and land on the floor
/// behind it, leaving the monster standing beside the hole.
fn under_monster<D: Edicts + ?Sized>(w: &World, ents: &D, e: Ent, eye: Vec3, fwd: Vec3, hit: Vec3) -> Option<Vec3> {
    let direct = w.trace(ents, eye, Vec3::ZERO, Vec3::ZERO, hit, MoveType::Normal, Some(e)).ent.filter(|&m| m != 0);
    let mut best: Option<(f32, Vec3)> = None;
    for m in 1..ents.num_edicts() {
        if ents.is_free(m) || ents.flags(m) & FL_MONSTER == 0 || m == e {
            continue;
        }
        let o = ents.v(m, V::Origin);
        let (mins, maxs) = (ents.v(m, V::Mins), ents.v(m, V::Maxs));
        if (o.z + mins.z - hit.z).abs() > 4.0 {
            continue; // not standing on this floor
        }
        let spot = Vec3::new(o.x, o.y, hit.z);
        let reach = (maxs.x - mins.x).max(maxs.y - mins.y) * 0.5 + MONSTER_SLACK;
        let dist = (spot - hit).length();
        if direct != Some(m) && dist > reach {
            continue;
        }
        // only monsters in front of the shooter
        if (spot - eye).dot(fwd) <= 0.0 {
            continue;
        }
        if best.is_none_or(|(d, _)| dist < d) {
            best = Some((dist, spot));
        }
    }
    best.map(|(_, s)| s)
}

fn sgn(x: f32) -> f32 {
    if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 }
}

/// Rim and centre sample offsets.
fn samples() -> impl Iterator<Item = (f32, f32)> {
    let (w, h) = (HALF_W, HALF_H);
    [(0.0, 0.0), (w, 0.0), (-w, 0.0), (0.0, h), (0.0, -h), (w, h), (-w, h), (w, -h), (-w, -h)].into_iter()
}

/// The surface under `(r, u)` is on the portal plane and nothing (wall,
/// ledge, liquid) sits in front of it.
fn sample_ok<D: Edicts + ?Sized>(w: &World, ents: &D, p: &Portal, plane: f32, r: f32, u: f32) -> bool {
    let s = p.at(r, u);
    let n = p.normal;
    if w.point_contents(s + n * 2.0) != CONTENTS_EMPTY {
        return false;
    }
    let tr = w.trace(ents, s + n * 4.0, Vec3::ZERO, Vec3::ZERO, s - n * 4.0, MoveType::NoMonsters, None);
    if tr.fraction >= 1.0 || tr.ent != Some(0) || tr.plane_normal.dot(n) < 0.999 || (tr.endpos.dot(n) - plane).abs() > 0.5 {
        return false;
    }
    // nothing jutting out in front (ledges, pillars)
    let front = w.trace(ents, s + n * 32.0, Vec3::ZERO, Vec3::ZERO, s + n * 1.0, MoveType::NoMonsters, None);
    front.fraction >= 1.0 && !front.startsolid
}

fn overlaps(p: &Portal, others: &[Portal]) -> bool {
    others.iter().any(|o| {
        let d = o.origin - p.origin;
        o.normal.dot(p.normal) > 0.99 && d.dot(p.normal).abs() < 2.0 && d.dot(p.right).abs() < 2.0 * HALF_W && d.dot(p.up).abs() < 2.0 * HALF_H
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_text_round_trips() {
        let mut a = Portals::default();
        a.set(1, 1, Some(Portal::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0))));
        let mut b = Portals::default();
        b.load_text(&a.save_text().unwrap());
        assert_eq!(b.pairs[1], a.pairs[1]);
        assert_eq!(Portals::default().save_text(), None);
    }
}
