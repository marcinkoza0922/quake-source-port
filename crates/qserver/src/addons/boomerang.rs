//! Boomerang: hold fire and sweep the crosshair over monsters to queue
//! them, release to throw. The boomerang is a virtual projectile (no
//! edict, so no QC sees it): it curves out, homes in on each queued
//! monster in turn, stuns everything it touches and flies back. One
//! boomerang per player; it can't be thrown again until it is caught.
//!
//! Stunned monsters keep their `nextthink` pushed to the stun's end (so
//! their AI resumes by itself) and lose horizontal speed. Clients learn
//! about marks, flight and stuns from `//qsp_boom` stufftext comments.

use qcore::math::{Vec3, angle_vectors};
use qworld::{Edicts, Ent, F, MoveType, V, World};

use crate::client::Client;

const FL_MONSTER: i32 = 32;
const FL_FLY: i32 = 1;
const FL_SWIM: i32 = 2;
/// Most monsters one throw can queue.
pub const MAX_MARKS: usize = 32;
/// Longest marking reach and straight-throw distance.
const MARK_RANGE: f32 = 2048.0;
const THROW_RANGE: f32 = 640.0;
/// Contact slack around a monster's box and the catch radius.
const HIT_PAD: f32 = 6.0;
const CATCH: f32 = 40.0;
/// Give up on a queued target after this long, the whole outbound flight
/// after `FLIGHT_MAX`, and fly through walls once returning that long.
const TARGET_MAX: f32 = 3.0;
const FLIGHT_MAX: f32 = 15.0;
const RETURN_NOCLIP: f32 = 1.5;
const RETURN_MAX: f32 = 8.0;

/// One boomerang in the air.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub pos: Vec3,
    pub dir: Vec3,
    /// Monsters still to visit, in order.
    pub queue: Vec<Ent>,
    /// Straight throw: the point it flies to before turning back.
    pub aim: Option<Vec3>,
    pub returning: bool,
    /// Monsters already stunned by this throw.
    hit: Vec<Ent>,
    age: f32,
    leg_age: f32,
}

#[derive(Clone, Debug, Default)]
struct Player {
    prev: u8,
    marks: Vec<Ent>,
    flight: Option<Flight>,
}

/// Every player's boomerang plus the stunned monsters; lives on `Level`.
#[derive(Clone, Debug, Default)]
pub struct Boomerangs {
    players: Vec<Player>,
    /// (monster, stun end time).
    pub stuns: Vec<(Ent, f64)>,
    outbox: Vec<String>,
}

/// Settings read from the `addon_boomerang_*` cvars.
#[derive(Clone, Copy, Debug)]
pub struct Tuning {
    pub stun: f32,
    pub speed: f32,
}

fn centre<D: Edicts + ?Sized>(ents: &D, e: Ent) -> Vec3 {
    (ents.v(e, V::AbsMin) + ents.v(e, V::AbsMax)) * 0.5
}

fn alive_monster<D: Edicts + ?Sized>(ents: &D, e: Ent) -> bool {
    e > 0
        && e < ents.num_edicts()
        && !ents.is_free(e)
        && ents.flags(e) & FL_MONSTER != 0
        && ents.f(e, F::Health) > 0.0
}

/// Segment `a`→`b` touches the box `lo`..`hi` (slab test).
pub fn segment_hits_box(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let d = b - a;
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (s, dd, l, h) in [
        (a.x, d.x, lo.x, hi.x),
        (a.y, d.y, lo.y, hi.y),
        (a.z, d.z, lo.z, hi.z),
    ] {
        if dd.abs() < 1e-6 {
            if s < l || s > h {
                return false;
            }
            continue;
        }
        let (mut u, mut v) = ((l - s) / dd, (h - s) / dd);
        if u > v {
            std::mem::swap(&mut u, &mut v);
        }
        t0 = t0.max(u);
        t1 = t1.min(v);
        if t0 > t1 {
            return false;
        }
    }
    true
}

/// Turn unit `dir` towards unit `want` by at most `rate` radians.
pub fn steer(dir: Vec3, want: Vec3, rate: f32) -> Vec3 {
    let cos = dir.dot(want).clamp(-1.0, 1.0);
    let angle = cos.acos();
    if angle <= rate || angle < 1e-4 {
        return want;
    }
    let mut side = want - dir * cos;
    if side.length_sq() < 1e-8 {
        side = dir.cross(Vec3::new(0.0, 0.0, 1.0));
    }
    let side = side.normalize();
    (dir * rate.cos() + side * rate.sin()).normalize()
}

/// The monster under player `e`'s crosshair (nearest along the aim, in
/// sight), if any.
fn under_crosshair<D: Edicts + ?Sized>(w: &World, ents: &D, e: Ent) -> Option<Ent> {
    let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
    let (fwd, _, _) = angle_vectors(ents.v(e, V::VAngle));
    let mut best: Option<(f32, Ent)> = None;
    for m in 1..ents.num_edicts() {
        if m == e || !alive_monster(ents, m) {
            continue;
        }
        let c = centre(ents, m);
        let t = (c - eye).dot(fwd);
        if t <= 0.0 || t > MARK_RANGE {
            continue;
        }
        let size = ents.v(m, V::AbsMax) - ents.v(m, V::AbsMin);
        let radius = size.x.max(size.z) * 0.5 + 4.0 + t * 0.02;
        // closest to the aim line relative to its size, not nearest first,
        // so a target just behind another can still be picked
        let score = (c - (eye + fwd * t)).length() / radius;
        if score > 1.0 || best.is_some_and(|(bs, _)| bs <= score) {
            continue;
        }
        let tr = w.trace(
            ents,
            eye,
            Vec3::ZERO,
            Vec3::ZERO,
            c,
            MoveType::NoMonsters,
            Some(e),
        );
        if tr.fraction >= 1.0 || tr.ent == Some(m) {
            best = Some((score, m));
        }
    }
    best.map(|(_, m)| m)
}

impl Boomerangs {
    fn ensure(&mut self, n: usize) {
        if self.players.len() < n {
            self.players.resize(n, Player::default());
        }
    }

    /// The boomerang of client `i`, if thrown.
    pub fn flight(&self, i: usize) -> Option<&Flight> {
        self.players.get(i)?.flight.as_ref()
    }
    /// Client `i`'s queued monsters.
    pub fn marks(&self, i: usize) -> &[Ent] {
        self.players.get(i).map_or(&[], |p| &p.marks)
    }

    fn send_marks<D: Edicts + ?Sized>(&mut self, ents: &D, i: usize) {
        let mut s = format!("//qsp_boom m {i}");
        for &m in &self.players[i].marks {
            s += &format!(" {m} {}", ents.v(m, V::Maxs).z);
        }
        self.outbox.push(s + "\n");
    }

    /// Stun monster `m` until `until` (extends a running stun).
    pub fn stun<D: Edicts + ?Sized>(&mut self, ents: &mut D, m: Ent, now: f64, secs: f32) {
        let until = now + secs as f64;
        match self.stuns.iter_mut().find(|s| s.0 == m) {
            Some(s) => s.1 = s.1.max(until),
            None => self.stuns.push((m, until)),
        }
        hold(ents, m, until);
        self.outbox.push(format!(
            "//qsp_boom s {m} {secs} {}\n",
            ents.v(m, V::Maxs).z
        ));
    }

    /// One server frame: marking, throws, flights, stuns.
    #[allow(clippy::too_many_arguments)]
    pub fn frame<D: Edicts + ?Sized>(
        &mut self,
        w: &World,
        ents: &mut D,
        clients: &[Client],
        time: f64,
        dt: f32,
        tune: Tuning,
        on: bool,
    ) {
        self.ensure(clients.len());
        for (i, cl) in clients.iter().enumerate() {
            let e = i as Ent + 1;
            let held = on
                && cl.spawned
                && super::Toy::from_buttons(cl.cmd.buttons) == Some(super::Toy::Boomerang);
            let b = if held { cl.cmd.buttons } else { 0 };
            let p = &mut self.players[i];
            let prev = std::mem::replace(&mut p.prev, b);
            if !cl.active {
                if p.flight.take().is_some() {
                    self.outbox.push(format!("//qsp_boom c {i}\n"));
                }
                p.marks.clear();
                continue;
            }
            let fire = b & super::BUTTON_TOY_FIRE != 0;
            let before = p.marks.len();
            let mut changed = false;
            p.marks.retain(|&m| alive_monster(ents, m));
            changed |= p.marks.len() != before;
            if b & super::BUTTON_TOY_ALT != 0 && !p.marks.is_empty() {
                p.marks.clear();
                changed = true;
            }
            if fire
                && p.flight.is_none()
                && p.marks.len() < MAX_MARKS
                && let Some(m) = under_crosshair(w, ents, e)
                && !p.marks.contains(&m)
            {
                p.marks.push(m);
                changed = true;
            }
            let released = prev & super::BUTTON_TOY_FIRE != 0 && !fire;
            // putting the toy away mid-aim cancels the marks
            if !held && !p.marks.is_empty() && !released {
                p.marks.clear();
                changed = true;
            }
            if released && p.flight.is_none() {
                let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
                let (fwd, right, up) = angle_vectors(ents.v(e, V::VAngle));
                let start = eye + fwd * 16.0 + right * 8.0 - up * 6.0;
                let queue = std::mem::take(&mut p.marks);
                let aim = queue.is_empty().then(|| {
                    w.trace(
                        ents,
                        eye,
                        Vec3::ZERO,
                        Vec3::ZERO,
                        eye + fwd * THROW_RANGE,
                        MoveType::NoMonsters,
                        Some(e),
                    )
                    .endpos
                        - fwd * 16.0
                });
                p.flight = Some(Flight {
                    pos: start,
                    // starts off to the right, then curves in
                    dir: (fwd + right * 0.55).normalize(),
                    queue,
                    aim,
                    returning: false,
                    hit: Vec::new(),
                    age: 0.0,
                    leg_age: 0.0,
                });
                self.outbox.push(format!(
                    "//qsp_boom t {i} {} {} {}\n",
                    start.x, start.y, start.z
                ));
                changed = true;
            }
            if changed {
                self.send_marks(ents, i);
            }
        }
        for i in 0..self.players.len().min(clients.len()) {
            self.fly(w, ents, i, time, dt, tune);
        }
        // keep stunned monsters frozen
        self.stuns
            .retain(|&(m, until)| alive_monster(ents, m) && time < until);
        for &(m, until) in &self.stuns {
            hold(ents, m, until);
        }
    }

    fn fly<D: Edicts + ?Sized>(
        &mut self,
        w: &World,
        ents: &mut D,
        i: usize,
        time: f64,
        dt: f32,
        tune: Tuning,
    ) {
        let Some(mut f) = self.players[i].flight.take() else {
            return;
        };
        let owner = i as Ent + 1;
        let home = ents.v(owner, V::Origin) + ents.v(owner, V::ViewOfs) * 0.7;
        f.age += dt;
        f.leg_age += dt;
        // drop targets that died, vanished or took too long to reach
        while let Some(&t) = f.queue.first() {
            if alive_monster(ents, t) && f.leg_age < TARGET_MAX {
                break;
            }
            f.queue.remove(0);
            f.leg_age = 0.0;
        }
        if !f.returning && f.queue.is_empty() && (f.aim.is_none() || f.age > FLIGHT_MAX) {
            f.returning = true;
            f.leg_age = 0.0;
        }
        let goal = if f.returning {
            home
        } else if let Some(&t) = f.queue.first() {
            centre(ents, t)
        } else {
            f.aim.unwrap_or(home)
        };
        let want = (goal - f.pos).normalize();
        if want.length_sq() > 0.5 {
            // turning tightens with age so it never orbits a target
            f.dir = steer(f.dir, want, (4.0 + f.age * 6.0) * dt);
        }
        let step = f.dir * tune.speed * dt;
        let mut end = f.pos + step;
        let noclip = f.returning && f.leg_age > RETURN_NOCLIP;
        if !noclip {
            let tr = w.trace(
                ents,
                f.pos,
                Vec3::ZERO,
                Vec3::ZERO,
                end,
                MoveType::NoMonsters,
                None,
            );
            if tr.fraction < 1.0 && !tr.allsolid {
                let n = tr.plane_normal;
                end = tr.endpos + n;
                let slide = f.dir - n * f.dir.dot(n);
                f.dir = if slide.length_sq() > 0.01 {
                    slide.normalize()
                } else {
                    f.dir - n * 2.0 * f.dir.dot(n)
                };
            }
        }
        // stun every monster on the way
        for m in 1..ents.num_edicts() {
            if m == owner || f.hit.contains(&m) || !alive_monster(ents, m) {
                continue;
            }
            let pad = Vec3::splat(HIT_PAD);
            if segment_hits_box(
                f.pos,
                end,
                ents.v(m, V::AbsMin) - pad,
                ents.v(m, V::AbsMax) + pad,
            ) {
                f.hit.push(m);
                self.stun(ents, m, time, tune.stun);
            }
        }
        f.pos = end;
        if let Some(&t) = f.queue.first()
            && f.hit.contains(&t)
        {
            f.queue.remove(0);
            f.leg_age = 0.0;
        }
        if !f.returning && f.queue.is_empty() && f.aim.is_some_and(|a| (a - f.pos).length() < 32.0)
        {
            f.returning = true;
            f.leg_age = 0.0;
        }
        if f.returning && ((home - f.pos).length() < CATCH || f.leg_age > RETURN_MAX) {
            self.outbox.push(format!("//qsp_boom c {i}\n"));
            return;
        }
        let p = f.pos;
        self.outbox
            .push(format!("//qsp_boom f {i} {} {} {}\n", p.x, p.y, p.z));
        self.players[i].flight = Some(f);
    }

    /// Flush the outbox into every spawned client's reliable message.
    pub(crate) fn send(&mut self, clients: &mut [Client]) {
        for cl in clients.iter_mut().filter(|c| c.spawned) {
            for s in &self.outbox {
                cl.message.byte(crate::msg::SVC_STUFFTEXT);
                cl.message.string(s.as_bytes());
            }
        }
        self.outbox.clear();
    }

    /// Drop every flight, mark and stun (add-ons off or map end).
    pub(crate) fn clear(&mut self, clients: &mut [Client]) {
        for i in 0..self.players.len() {
            if self.players[i].flight.is_some() {
                self.outbox.push(format!("//qsp_boom c {i}\n"));
            }
            if !self.players[i].marks.is_empty() {
                self.outbox.push(format!("//qsp_boom m {i}\n"));
            }
        }
        self.send(clients);
        *self = Default::default();
    }

    pub fn is_empty(&self) -> bool {
        self.stuns.is_empty()
            && self
                .players
                .iter()
                .all(|p| p.flight.is_none() && p.marks.is_empty())
    }
}

/// Freeze monster `m` until `until`: no thinking, no walking or flying.
fn hold<D: Edicts + ?Sized>(ents: &mut D, m: Ent, until: f64) {
    let nt = ents.f(m, F::NextThink);
    // nextthink 0 means no think is scheduled: leave it alone
    if nt > 0.0 && (nt as f64) < until {
        ents.set_f(m, F::NextThink, until as f32);
    }
    let v = ents.v(m, V::Velocity);
    let fly = ents.flags(m) & (FL_FLY | FL_SWIM) != 0;
    ents.set_v(
        m,
        V::Velocity,
        Vec3::new(0.0, 0.0, if fly { 0.0 } else { v.z.min(0.0) }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_box() {
        let (lo, hi) = (Vec3::splat(-1.0), Vec3::splat(1.0));
        assert!(segment_hits_box(
            Vec3::new(-5.0, 0.0, 0.0),
            Vec3::new(5.0, 0.0, 0.0),
            lo,
            hi
        ));
        assert!(!segment_hits_box(
            Vec3::new(-5.0, 3.0, 0.0),
            Vec3::new(5.0, 3.0, 0.0),
            lo,
            hi
        ));
        assert!(!segment_hits_box(
            Vec3::new(-5.0, 0.0, 0.0),
            Vec3::new(-2.0, 0.0, 0.0),
            lo,
            hi
        ));
        assert!(segment_hits_box(Vec3::ZERO, Vec3::ZERO, lo, hi));
    }

    #[test]
    fn steer_turns_at_most_rate() {
        let a = Vec3::new(1.0, 0.0, 0.0);
        let b = Vec3::new(0.0, 1.0, 0.0);
        let s = steer(a, b, 0.1);
        assert!((s.dot(a) - 0.1f32.cos()).abs() < 1e-4);
        assert_eq!(steer(a, b, 2.0), b);
    }
}
