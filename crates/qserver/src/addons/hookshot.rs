//! Hookshot (homage to Zelda's): fire sends a hook head out on a chain;
//! a hookable surface (by material, `hook_surface`) or enemy latches it and
//! the player is reeled in, then let go with some momentum. A miss, an
//! unhookable surface or the range limit retracts it. Fire or alt-fire
//! while latched lets go early.
//!
//! Safety: the pull only sets the player's velocity, so the ordinary player
//! physics does all movement and collision (no teleporting into
//! geometry). It stops when the player arrives, stalls against something
//! for a moment, loses sight of the hook, the hooked enemy dies, or after
//! a timeout.
//!
//! Clients get `//qsp_hook owner state x y z` stufftext (state 0 idle,
//! 1 flying, 2 latched, 3 retracting; xyz = hook head) every frame the
//! hook is out; other engines ignore it.

use qcore::math::{Vec3, angle_vectors};
use qworld::{Edicts, Ent, F, FL_MONSTER, FL_ONGROUND, MoveType, SOLID_BSP, V, World};

use super::hook_surface::{Material, Surfaces};
use crate::client::Client;

/// Hook head speed out and back (units/s).
const SPEED_OUT: f32 = 1600.0;
const SPEED_BACK: f32 = 2200.0;
/// Reel-in speed of the player.
pub const PULL: f32 = 750.0;
/// Fraction of the pull speed kept on release, plus a hop up.
const KEEP: f32 = 0.45;
const HOP: f32 = 180.0;
/// Longest pull (s) and how long the player may stall before it gives up.
const MAX_PULL: f32 = 3.0;
const MAX_STALL: f32 = 0.2;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum State {
    #[default]
    Idle,
    /// Head at `pos` travelling along `dir`, `dist` so far.
    Flying { pos: Vec3, dir: Vec3, dist: f32 },
    /// Hooked at `point`, or to entity `ent` at `point` relative to its origin.
    Latched { point: Vec3, ent: Option<Ent>, time: f32, stall: f32, last: f32 },
    Retracting { pos: Vec3 },
}

impl State {
    fn code(&self) -> u8 {
        match self {
            State::Idle => 0,
            State::Flying { .. } => 1,
            State::Latched { .. } => 2,
            State::Retracting { .. } => 3,
        }
    }
}

/// What the hook may latch onto (from the `addon_hook_*` cvars).
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub range: f32,
    pub wood: bool,
    pub metal: bool,
    pub stone: bool,
    pub enemies: bool,
}

impl Rules {
    pub fn read(cv: &qcore::cvar::Cvars) -> Self {
        let on = |n: &str| cv.value_of(n) != 0.0;
        Rules {
            range: cv.value_of("addon_hook_range").clamp(128.0, 8192.0),
            wood: on("addon_hook_wood"),
            metal: on("addon_hook_metal"),
            stone: on("addon_hook_stone"),
            enemies: on("addon_hook_enemies"),
        }
    }
    pub fn material(&self, m: Material) -> bool {
        match m {
            Material::Wood => self.wood,
            Material::Metal => self.metal,
            Material::Stone => self.stone,
            Material::None => false,
        }
    }
}

/// Every player's hook; lives on `Level`.
#[derive(Default)]
pub struct Hooks {
    pub surfaces: Surfaces,
    pub state: Vec<State>,
    prev_buttons: Vec<u8>,
    sent: Vec<u8>,
    last_time: f64,
}

impl Hooks {
    pub fn new(surfaces: Surfaces) -> Self {
        Hooks { surfaces, ..Default::default() }
    }
    fn ensure(&mut self, n: usize) {
        if self.state.len() < n {
            self.state.resize(n, State::Idle);
            self.prev_buttons.resize(n, 0);
            self.sent.resize(n, 0);
        }
    }
    /// Drop every hook (toy off, add-ons shut down, level change).
    pub fn reset(&mut self) {
        self.state.iter_mut().for_each(|s| *s = State::Idle);
    }
}

/// Muzzle of player `e`: where the chain starts.
pub fn muzzle<D: Edicts + ?Sized>(ents: &D, e: Ent) -> Vec3 {
    let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
    let (f, r, u) = angle_vectors(ents.v(e, V::VAngle));
    eye + f * 16.0 + r * 4.0 - u * 6.0
}

/// One server frame of every player's hook.
pub(crate) fn frame<D: Edicts + ?Sized>(h: &mut Hooks, w: &World, ents: &mut D, clients: &mut [Client], rules: Rules, enabled: bool, time: f64) {
    h.ensure(clients.len());
    let dt = ((time - h.last_time) as f32).clamp(0.0, 0.1);
    h.last_time = time;
    for (i, cl) in clients.iter().enumerate() {
        let e = i as Ent + 1;
        let held = enabled && cl.active && cl.spawned && super::Toy::from_buttons(cl.cmd.buttons) == Some(super::Toy::Hookshot);
        let b = if held { cl.cmd.buttons } else { 0 };
        let pressed = b & !h.prev_buttons[i];
        h.prev_buttons[i] = b;
        if !cl.active || !cl.spawned || ents.f(e, F::Health) <= 0.0 {
            h.state[i] = State::Idle;
            continue;
        }
        let fire = pressed & super::BUTTON_TOY_FIRE != 0;
        let any = pressed & (super::BUTTON_TOY_FIRE | super::BUTTON_TOY_ALT) != 0;
        let st = &mut h.state[i];
        match *st {
            State::Idle if fire => {
                let (dir, _, _) = angle_vectors(ents.v(e, V::VAngle));
                *st = State::Flying { pos: muzzle(ents, e), dir, dist: 0.0 };
                fly(st, &h.surfaces, w, ents, e, rules, dt);
            }
            State::Idle => {}
            State::Flying { .. } => fly(st, &h.surfaces, w, ents, e, rules, dt),
            State::Latched { .. } if any || !held => release(st, ents, e, false),
            State::Latched { .. } => pull(st, w, ents, e, dt),
            State::Retracting { pos } => {
                let to = muzzle(ents, e) - pos;
                let step = SPEED_BACK * dt;
                *st = if to.length() <= step.max(24.0) { State::Idle } else { State::Retracting { pos: pos + to.normalize() * step } };
            }
        }
    }
    let mut lines = String::new();
    for (i, s) in h.state.iter().enumerate() {
        let code = s.code();
        if code == 0 && h.sent[i] == 0 {
            continue;
        }
        h.sent[i] = code;
        let p = head(s, ents);
        lines += &format!("//qsp_hook {i} {code} {} {} {}\n", p.x, p.y, p.z);
    }
    if !lines.is_empty() {
        for cl in clients.iter_mut().filter(|c| c.spawned) {
            cl.message.byte(crate::msg::SVC_STUFFTEXT);
            cl.message.string(lines.as_bytes());
        }
    }
}

/// Where the hook head is now.
pub fn head<D: Edicts + ?Sized>(s: &State, ents: &D) -> Vec3 {
    match *s {
        State::Idle => Vec3::ZERO,
        State::Flying { pos, .. } | State::Retracting { pos } => pos,
        State::Latched { point, ent: Some(m), .. } => ents.v(m, V::Origin) + point,
        State::Latched { point, .. } => point,
    }
}

fn fly<D: Edicts + ?Sized>(st: &mut State, surf: &Surfaces, w: &World, ents: &mut D, e: Ent, rules: Rules, dt: f32) {
    let State::Flying { pos, dir, dist } = *st else { return };
    let step = (SPEED_OUT * dt).min(rules.range - dist).max(0.0);
    let end = pos + dir * step;
    let tr = w.trace(&*ents, pos, Vec3::ZERO, Vec3::ZERO, end, MoveType::Normal, Some(e));
    if tr.startsolid {
        *st = State::Retracting { pos };
        return;
    }
    let Some(m) = tr.ent.filter(|_| tr.fraction < 1.0) else {
        let dist = dist + step;
        *st = if dist >= rules.range - 0.5 { State::Retracting { pos: end } } else { State::Flying { pos: end, dir, dist } };
        return;
    };
    let hit = tr.endpos;
    let latch = if m != 0 && ents.flags(m) & FL_MONSTER != 0 {
        rules.enemies && ents.f(m, F::Health) > 0.0
    } else if m == 0 || ents.f(m, F::Solid) as i32 == SOLID_BSP {
        let sub = if m == 0 { 0 } else { w.model(ents.f(m, F::ModelIndex) as usize).map_or(usize::MAX, |&(_, s)| s) };
        let local = hit - if m == 0 { Vec3::ZERO } else { ents.v(m, V::Origin) };
        rules.material(surf.at(sub, local, tr.plane_normal))
    } else {
        false
    };
    *st = if !latch {
        State::Retracting { pos: hit }
    } else if m == 0 {
        State::Latched { point: hit, ent: None, time: 0.0, stall: 0.0, last: f32::MAX }
    } else {
        // follow a mover or monster: offset from its origin
        State::Latched { point: hit - ents.v(m, V::Origin), ent: Some(m), time: 0.0, stall: 0.0, last: f32::MAX }
    };
}

fn pull<D: Edicts + ?Sized>(st: &mut State, w: &World, ents: &mut D, e: Ent, dt: f32) {
    let State::Latched { ent, time, stall, last, .. } = *st else { return };
    if ent.is_some_and(|m| ents.is_free(m) || (ents.flags(m) & FL_MONSTER != 0 && ents.f(m, F::Health) <= 0.0)) {
        return release(st, ents, e, false);
    }
    let target = head(st, ents);
    let centre = ents.v(e, V::Origin);
    let to = target - centre;
    let dist = to.length();
    // reach: a monster's own size, or a body length off a wall
    let reach = match ent {
        Some(m) if ents.flags(m) & FL_MONSTER != 0 => 40.0 + (ents.v(m, V::Maxs) - ents.v(m, V::Mins)).length() * 0.35,
        _ => 36.0,
    };
    if dist <= reach {
        return release(st, ents, e, true);
    }
    // obstruction: line from the eye to the hook blocked by something else
    let eye = centre + ents.v(e, V::ViewOfs);
    let tr = w.trace(&*ents, eye, Vec3::ZERO, Vec3::ZERO, target, MoveType::NoMonsters, Some(e));
    let blocked = tr.fraction < 1.0 && (tr.endpos - target).length() > 24.0 && tr.ent != ent;
    let stall = if last - dist < PULL * dt * 0.2 { stall + dt } else { 0.0 };
    let time = time + dt;
    if blocked || stall > MAX_STALL || time > MAX_PULL {
        return release(st, ents, e, true);
    }
    let dir = to / dist;
    ents.set_v(e, V::Velocity, dir * PULL);
    let fl = ents.flags(e);
    ents.set_flags(e, fl & !FL_ONGROUND);
    if let State::Latched { time: t, stall: s, last: l, .. } = st {
        (*t, *s, *l) = (time, stall, dist);
    }
}

/// Let go: keep part of the pull speed (and hop, to clear a ledge) when
/// `momentum`, then reel the chain back in.
fn release<D: Edicts + ?Sized>(st: &mut State, ents: &mut D, e: Ent, momentum: bool) {
    let pos = head(st, ents);
    if momentum {
        let v = ents.v(e, V::Velocity) * KEEP;
        ents.set_v(e, V::Velocity, Vec3::new(v.x, v.y, v.z.max(0.0) + HOP));
    } else {
        let v = ents.v(e, V::Velocity);
        ents.set_v(e, V::Velocity, v * KEEP);
    }
    *st = State::Retracting { pos };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_gate_materials() {
        let r = Rules { range: 1000.0, wood: true, metal: false, stone: true, enemies: false };
        assert!(r.material(Material::Wood));
        assert!(!r.material(Material::Metal));
        assert!(r.material(Material::Stone));
        assert!(!r.material(Material::None));
    }
}
