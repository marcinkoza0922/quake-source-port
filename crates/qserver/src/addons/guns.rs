//! Shared parts of the add-ons that shoot bullets from magazines (akimbo
//! pistols, assault rifle): [`Magazine`] with reloading, [`bullet`] hitscan
//! through the progs' own `T_Damage`, and what clients are told.
//!
//! - `//qsp_gun owner toy part kind secs flesh sx sy sz ex ey ez nx ny nz`:
//!   one event of gun `part` ([`Kind`]).
//! - `//qsp_mag owner toy cap rounds left [rounds left]`: magazine state per
//!   part, `left` = seconds until its reload ends (0 = not reloading), sent
//!   on change. `//qsp_mag owner toy` forgets it.
//!
//! Other engines skip `//` lines, so demos still play.

use qcore::math::{Vec3, angle_vectors};
use qworld::{CONTENTS_SKY, Ent, MoveType, V};

use crate::client::Client;
use crate::ents::Ents;
use crate::server::{Core, Level};

const RANGE: f32 = 8192.0;

/// What a `//qsp_gun` line reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Trigger pulled on an empty magazine.
    Dry = 0,
    /// A shot: `start` muzzle, `end`, `normal` (zero: hit nothing solid),
    /// `flesh` when a damageable entity was hit.
    Fire = 1,
    /// A reload began; `secs` is how long it takes.
    Reload = 2,
    /// The reload was interrupted.
    Cancel = 3,
    /// The reload finished.
    Done = 4,
}

/// A gun's magazine. Times are level times (`vm.time`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Magazine {
    pub rounds: u32,
    pub cap: u32,
    /// When the running reload finishes.
    pub reload_end: Option<f64>,
}

impl Magazine {
    pub fn full(cap: u32) -> Self {
        Self { rounds: cap, cap, reload_end: None }
    }

    /// Takes a round for a shot; false when empty or reloading.
    pub fn take(&mut self) -> bool {
        if self.reload_end.is_some() || self.rounds == 0 {
            return false;
        }
        self.rounds -= 1;
        true
    }

    /// Starts a reload; `reserve` is the spare rounds (None = endless).
    /// False when full, already reloading or nothing to load.
    pub fn start(&mut self, now: f64, secs: f64, reserve: Option<u32>) -> bool {
        if self.reload_end.is_some() || self.rounds >= self.cap || reserve == Some(0) {
            return false;
        }
        self.reload_end = Some(now + secs);
        true
    }

    /// Finishes a reload that is due: loads up to `reserve` rounds and
    /// returns how many (the caller takes them from the reserve).
    pub fn finish(&mut self, now: f64, reserve: Option<u32>) -> Option<u32> {
        if self.reload_end.is_none_or(|t| now < t) {
            return None;
        }
        self.reload_end = None;
        let want = self.cap.saturating_sub(self.rounds);
        let n = reserve.map_or(want, |r| want.min(r));
        self.rounds += n;
        Some(n)
    }

    /// Interrupts a reload; true if one was running.
    pub fn cancel(&mut self) -> bool {
        self.reload_end.take().is_some()
    }
}

/// Where a bullet went.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub end: Vec3,
    /// Wall normal, zero when it hit nothing solid.
    pub normal: Vec3,
    /// The entity hit (not the world).
    pub hit: Option<Ent>,
    /// It was damageable.
    pub flesh: bool,
}

/// Where player `e`'s bullets start (the eye) and the view axes.
pub(crate) fn aim(l: &mut Level, c: &mut Core, e: Ent) -> (Vec3, [Vec3; 3]) {
    use qworld::Edicts;
    let ents = Ents { vm: &mut l.vm, c };
    let (f, r, u) = angle_vectors(ents.v(e, V::VAngle));
    (ents.v(e, V::Origin) + ents.v(e, V::ViewOfs), [f, r, u])
}

/// A direction inside a cone about `axes[0]`: `x`, `y` are the offsets in
/// units of `spread` (the tangent of the cone's half angle).
pub fn spread(axes: [Vec3; 3], spread: f32, x: f32, y: f32) -> Vec3 {
    (axes[0] + axes[1] * (x * spread) + axes[2] * (y * spread)).normalize()
}

/// Fires one bullet of player `e` from `eye` along `dir`: through linked
/// portals, hurting the first entity hit via QC `T_Damage`, with the stock
/// blood or gunshot effect where it lands.
pub(crate) fn bullet(l: &mut Level, c: &mut Core, e: Ent, eye: Vec3, dir: Vec3, damage: f32) -> Shot {
    let (tr, dir) = trace(l, c, e, eye, dir);
    let hit = tr.ent.filter(|&t| t != 0 && tr.fraction < 1.0);
    let f = c.sv.fields.clone();
    let flesh = hit.is_some_and(|t| l.vm.ent_f(t, f.takedamage) != 0.0);
    let shot = Shot { end: tr.endpos, normal: if tr.fraction < 1.0 { tr.plane_normal } else { Vec3::ZERO }, hit, flesh };
    if tr.fraction >= 1.0 || l.world.point_contents(tr.endpos - tr.plane_normal * 2.0) == CONTENTS_SKY {
        return Shot { normal: Vec3::ZERO, ..shot };
    }
    let at = tr.endpos - dir * 4.0;
    let pf = c.sv.protocolflags;
    let m = &mut c.sv.datagram;
    if m.len() <= crate::msg::MAX_DATAGRAM - 18 {
        if flesh {
            m.byte(crate::msg::SVC_PARTICLE);
            for x in at.to_array() {
                m.coord(x, pf);
            }
            for x in (dir * 16.0).to_array() {
                m.char(x as i32);
            }
            m.byte((damage * 2.0).min(255.0) as i32);
            m.byte(73);
        } else {
            m.byte(SVC_TEMP_ENTITY);
            m.byte(TE_GUNSHOT);
            for x in at.to_array() {
                m.coord(x, pf);
            }
        }
    }
    if let Some(t) = hit.filter(|_| flesh) {
        super::damage::damage(&mut l.vm, c, &mut l.world, t, e, e, damage);
    }
    shot
}

const SVC_TEMP_ENTITY: i32 = 23;
const TE_GUNSHOT: i32 = 2;

/// The bullet's final trace segment and its direction there.
fn trace(l: &mut Level, c: &mut Core, e: Ent, eye: Vec3, dir: Vec3) -> (qworld::Trace, Vec3) {
    let ents = Ents { vm: &mut l.vm, c };
    let (mut from, mut to, mut pass) = (eye, eye + dir * RANGE, Some(e));
    let mut hops = 0;
    loop {
        let tr = l.world.trace(&ents, from, Vec3::ZERO, Vec3::ZERO, to, MoveType::Normal, pass);
        let portal = (!ents.c.linked_portals.is_empty() && hops < 4)
            .then(|| super::portal_move::intersect_segment(&ents.c.linked_portals, from, to, &tr, 0.5))
            .flatten();
        let Some(h) = portal else { return (tr, (to - from).normalize()) };
        (from, to, pass) = (h.exit_pt, h.exit_pt + h.rem, None);
        hops += 1;
    }
}

/// Sets `show_hostile` as stock weapons do, so monsters hear the shot.
pub(crate) fn hostile(l: &mut Level, e: Ent) {
    if let Some(k) = l.vm.field_ofs("show_hostile") {
        let t = l.vm.time as f32 + 1.0;
        l.vm.set_ent_f(e, k, t);
    }
}

/// A `//qsp_gun` line (see the module comment).
pub fn line(owner: usize, toy: super::Toy, part: usize, kind: Kind, secs: f64, start: Vec3, shot: Option<Shot>) -> String {
    let v = |v: Vec3| format!(" {:.1} {:.1} {:.1}", v.x, v.y, v.z);
    let (end, normal, flesh) = shot.map_or((start, Vec3::ZERO, false), |s| (s.end, s.normal, s.flesh));
    format!(
        "//qsp_gun {owner} {} {part} {} {secs:.2} {}{}{} {:.3} {:.3} {:.3}\n",
        toy.id(), kind as u8, u8::from(flesh), v(start), v(end), normal.x, normal.y, normal.z
    )
}

/// Reliable text to every spawned client.
pub(crate) fn send_all(clients: &mut [Client], s: &str) {
    if s.is_empty() {
        return;
    }
    for cl in clients.iter_mut().filter(|c| c.spawned) {
        cl.message.byte(crate::msg::SVC_STUFFTEXT);
        cl.message.string(s.as_bytes());
    }
}

/// Sends each player's magazines to the clients when they change (or when
/// a client has just spawned).
#[derive(Clone, Debug, Default)]
pub struct MagSync {
    sent: Vec<Option<Vec<Magazine>>>,
    spawned: Vec<bool>,
}

impl MagSync {
    /// Once a frame: `mags(i)` are client `i`'s magazines, None to send none.
    pub(crate) fn update(&mut self, clients: &mut [Client], now: f64, toy: super::Toy, mags: impl Fn(usize) -> Option<Vec<Magazine>>) {
        self.sent.resize(clients.len(), None);
        let spawned: Vec<bool> = clients.iter().map(|c| c.spawned).collect();
        if spawned.iter().enumerate().any(|(i, &s)| s && !self.spawned.get(i).copied().unwrap_or(false)) {
            self.sent.fill(None);
        }
        self.spawned = spawned;
        for (i, sent) in self.sent.iter_mut().enumerate() {
            let m = mags(i);
            if m != *sent {
                send_all(clients, &mag_line(i, toy, now, m.as_deref().unwrap_or(&[])));
                *sent = m;
            }
        }
    }
}

/// A `//qsp_mag` line; no magazines forget the owner's.
pub fn mag_line(owner: usize, toy: super::Toy, now: f64, mags: &[Magazine]) -> String {
    let Some(first) = mags.first() else { return format!("//qsp_mag {owner} {}\n", toy.id()) };
    let mut s = format!("//qsp_mag {owner} {} {}", toy.id(), first.cap);
    for m in mags {
        s += &format!(" {} {:.2}", m.rounds, m.reload_end.map_or(0.0, |t| (t - now).max(0.0)));
    }
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::addons::Toy;

    #[test]
    fn magazines_load_only_what_the_reserve_holds() {
        let mut m = Magazine::full(3);
        assert!(!m.start(0.0, 1.0, None), "full");
        assert!(m.take() && m.take() && m.take() && !m.take());
        assert!(!m.start(0.0, 1.0, Some(0)), "no reserve");
        assert!(m.start(0.0, 1.0, Some(2)));
        assert!(!m.start(0.5, 1.0, Some(2)), "already reloading");
        assert_eq!(m.finish(0.9, Some(2)), None);
        assert_eq!(m.finish(1.0, Some(2)), Some(2));
        assert_eq!((m.rounds, m.reload_end), (2, None));
        assert!(m.start(2.0, 1.0, None) && !m.take(), "no shots while reloading");
        assert!(m.cancel() && !m.cancel() && m.take());
    }

    #[test]
    fn lines_name_the_gun_and_its_state() {
        let shot = Shot { end: Vec3::new(4.0, 5.0, 6.0), normal: Vec3::new(0.0, 0.0, 1.0), hit: Some(5), flesh: true };
        assert_eq!(
            line(1, Toy::Akimbo, 1, Kind::Fire, 0.0, Vec3::new(1.0, 2.0, 3.0), Some(shot)),
            "//qsp_gun 1 9 1 1 0.00 1 1.0 2.0 3.0 4.0 5.0 6.0 0.000 0.000 1.000\n"
        );
        let m = [Magazine::full(12), Magazine { rounds: 3, cap: 12, reload_end: Some(11.5) }];
        assert_eq!(mag_line(0, Toy::Akimbo, 10.0, &m), "//qsp_mag 0 9 12 12 0.00 3 1.50\n");
        assert_eq!(mag_line(0, Toy::Akimbo, 10.0, &[]), "//qsp_mag 0 9\n");
    }
}
