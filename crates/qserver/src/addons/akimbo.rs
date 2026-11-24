//! Akimbo pistols: fire shoots the left pistol, alt-fire the right one
//! (`addon_akimbo_swap` exchanges them). Each has its own magazine and
//! timers; the reserve is endless. A press shoots at once, a held trigger
//! repeats more slowly. The shared reload action reloads both, the second
//! one a little later ([`STAGGER`]); a pistol that isn't being reloaded can
//! still shoot. Putting the toy away or dying interrupts reloads.

use qcore::cvar::Cvars;
use qworld::Ent;

use super::guns::{self, Kind, MagSync, Magazine};
use super::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, BUTTON_TOY_RELOAD, Toy};
use crate::server::{Core, Level};

/// Least seconds between two presses of one trigger.
pub const PRESS: f64 = 0.11;
/// Seconds between shots of a held trigger.
pub const HOLD: f64 = 0.22;
/// Seconds between dry clicks of a held trigger.
const DRY: f64 = 0.4;
/// How much longer the second of two reloads started together takes.
pub const STAGGER: f64 = 0.35;

/// The add-on's options, clamped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rules {
    pub damage: f32,
    /// Tangent of the spread cone's half angle.
    pub spread: f32,
    pub cap: u32,
    pub reload: f64,
    pub auto_reload: bool,
    pub swap: bool,
}

impl Rules {
    pub fn read(cv: &Cvars) -> Self {
        let v = |n| cv.value_of(n);
        Rules {
            damage: v("addon_akimbo_damage").clamp(1.0, 1000.0),
            // the option counts degrees
            spread: v("addon_akimbo_spread").clamp(0.0, 15.0).to_radians().tan(),
            cap: v("addon_akimbo_mag").clamp(1.0, 200.0) as u32,
            reload: f64::from(v("addon_akimbo_reload").clamp(0.2, 10.0)),
            auto_reload: v("addon_akimbo_autoreload") != 0.0,
            swap: v("addon_akimbo_swap") != 0.0,
        }
    }
}

/// One player's pistols: 0 left, 1 right.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pair {
    mags: [Magazine; 2],
    /// When a new press / a held trigger may shoot again.
    press: [f64; 2],
    hold: [f64; 2],
    prev: u16,
}

impl Pair {
    fn new(cap: u32) -> Self {
        Self { mags: [Magazine::full(cap); 2], press: [0.0; 2], hold: [0.0; 2], prev: 0 }
    }

    /// Follows a changed magazine size: full magazines stay full.
    fn resize(&mut self, cap: u32) {
        for m in &mut self.mags {
            if m.cap != cap {
                m.rounds = if m.rounds >= m.cap { cap } else { m.rounds.min(cap) };
                m.cap = cap;
            }
        }
    }

    /// Reloads pistol `p`; true when it started.
    fn reload(&mut self, p: usize, now: f64, secs: f64) -> bool {
        self.mags[p].start(now, secs, None)
    }
}

/// Per-level state.
#[derive(Clone, Debug, Default)]
pub struct Akimbo {
    pairs: Vec<Option<Pair>>,
    /// Rounds from a save, by client, until that client plays.
    loaded: Vec<(usize, [u32; 2])>,
    sync: MagSync,
    seed: u32,
}

impl Akimbo {
    /// `.qsa` header lines: `akimbo client left right`.
    pub fn save_text(&self) -> String {
        self.pairs
            .iter()
            .enumerate()
            .filter_map(|(i, p)| p.map(|p| format!("akimbo {i} {} {}\n", p.mags[0].rounds, p.mags[1].rounds)))
            .collect()
    }

    /// Reads the `akimbo` lines of a save header (others are skipped).
    pub fn load_text(&mut self, text: &str) {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("akimbo ") else { continue };
            let n: Vec<u32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
            if let [i, l, r] = n[..] {
                self.loaded.push((i as usize, [l, r]));
            }
        }
    }

    /// Add-ons went off.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// A number in -1..1.
    fn random(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (self.seed >> 8) as f32 / (1u32 << 23) as f32 - 1.0
    }

    /// A point in the unit disc, denser towards the middle.
    fn scatter(&mut self) -> (f32, f32) {
        let (r, a) = (self.random().abs(), self.random() * std::f32::consts::PI);
        (r * a.cos(), r * a.sin())
    }
}

/// One server frame (after physics), also while the toy is off so it can
/// drop its state.
pub(crate) fn frame(st: &mut Akimbo, l: &mut Level, c: &mut Core) {
    let now = l.vm.time;
    if c.cvars.value_of(Toy::Akimbo.cvar()) == 0.0 {
        if !st.pairs.is_empty() {
            st.pairs.clear();
            st.sync.update(&mut c.clients, now, Toy::Akimbo, |_| None);
        }
        return;
    }
    let rules = Rules::read(&c.cvars);
    let f = c.sv.fields.clone();
    st.pairs.resize(c.clients.len(), None);
    let mut events = String::new();
    for i in 0..c.clients.len() {
        let e = i as Ent + 1;
        let cl = &c.clients[i];
        if !cl.spawned {
            // a spot waiting for its player to come back after a load
            st.pairs[i] = None;
            continue;
        }
        let buttons = cl.cmd.buttons;
        let mut pair = st.pairs[i].unwrap_or_else(|| {
            let mut p = Pair::new(rules.cap);
            if let Some(k) = st.loaded.iter().position(|s| s.0 == i) {
                let (_, rounds) = st.loaded.swap_remove(k);
                p.mags.iter_mut().zip(rounds).for_each(|(m, r)| m.rounds = r.min(m.cap));
            }
            p
        });
        pair.resize(rules.cap);
        let held = Toy::from_buttons(buttons) == Some(Toy::Akimbo) && l.vm.ent_f(e, f.health) > 0.0;
        if held {
            play(st, &mut pair, rules, buttons, i, l, c, &mut events);
        } else {
            for (p, m) in pair.mags.iter_mut().enumerate() {
                if m.cancel() {
                    events += &guns::line(i, Toy::Akimbo, p, Kind::Cancel, 0.0, Default::default(), None);
                }
            }
            pair.prev = 0;
        }
        st.pairs[i] = Some(pair);
    }
    guns::send_all(&mut c.clients, &events);
    let pairs = &st.pairs;
    st.sync.update(&mut c.clients, now, Toy::Akimbo, |i| pairs[i].map(|p| p.mags.to_vec()));
}

/// One frame of client `i` holding the pistols.
#[allow(clippy::too_many_arguments)]
fn play(st: &mut Akimbo, pair: &mut Pair, rules: Rules, buttons: u16, i: usize, l: &mut Level, c: &mut Core, events: &mut String) {
    let now = l.vm.time;
    let e = i as Ent + 1;
    let pressed = buttons & !pair.prev;
    pair.prev = buttons;
    let mut event = |p, kind, secs, start, shot| *events += &guns::line(i, Toy::Akimbo, p, kind, secs, start, shot);
    for (p, m) in pair.mags.iter_mut().enumerate() {
        if m.finish(now, None).is_some() {
            event(p, Kind::Done, 0.0, Default::default(), None);
        }
    }
    if pressed & BUTTON_TOY_RELOAD != 0 {
        let mut extra = 0.0;
        for p in 0..2 {
            let secs = rules.reload + extra;
            if pair.reload(p, now, secs) {
                event(p, Kind::Reload, secs, Default::default(), None);
                extra = STAGGER;
            }
        }
    }
    for p in 0..2 {
        let bit = if (p == 1) == rules.swap { BUTTON_TOY_FIRE } else { BUTTON_TOY_ALT };
        let ready = if pressed & bit != 0 { pair.press[p] } else { pair.hold[p] };
        if buttons & bit == 0 || now < ready || pair.mags[p].reload_end.is_some() {
            continue;
        }
        let (eye, axes) = guns::aim(l, c, e);
        let side = if p == 0 { -5.0 } else { 5.0 };
        let muzzle = eye + axes[0] * 16.0 + axes[1] * side - axes[2] * 5.0;
        if !pair.mags[p].take() {
            pair.press[p] = now + PRESS;
            pair.hold[p] = now + DRY;
            event(p, Kind::Dry, 0.0, muzzle, None);
            if rules.auto_reload && pair.reload(p, now, rules.reload) {
                event(p, Kind::Reload, rules.reload, Default::default(), None);
            }
            continue;
        }
        pair.press[p] = now + PRESS;
        pair.hold[p] = now + HOLD;
        let (x, y) = st.scatter();
        let dir = guns::spread(axes, rules.spread, x, y);
        guns::hostile(l, e);
        l.vm.set_ent_f(e, c.sv.fields.punchangle, -0.7);
        let shot = guns::bullet(l, c, e, eye, dir, rules.damage);
        event(p, Kind::Fire, 0.0, muzzle, Some(shot));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn magazines_follow_the_size_option() {
        let mut p = Pair::new(12);
        p.mags[1].rounds = 5;
        p.resize(20);
        assert_eq!((p.mags[0].rounds, p.mags[1].rounds, p.mags[1].cap), (20, 5, 20));
        p.resize(4);
        assert_eq!((p.mags[0].rounds, p.mags[1].rounds), (4, 4));
    }

    #[test]
    fn scatter_stays_in_the_disc_and_moves_about() {
        let mut a = Akimbo::default();
        let pts: Vec<_> = (0..200).map(|_| a.scatter()).collect();
        assert!(pts.iter().all(|p| p.0.hypot(p.1) <= 1.0));
        for (sx, sy) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)] {
            assert!(pts.iter().any(|p| p.0 * sx > 0.1 && p.1 * sy > 0.1));
        }
    }

    #[test]
    fn save_text_round_trips() {
        let mut a = Akimbo { pairs: vec![None, Some(Pair::new(12))], ..Default::default() };
        a.pairs[1].as_mut().unwrap().mags[0].rounds = 3;
        assert_eq!(a.save_text(), "akimbo 1 3 12\n");
        let mut b = Akimbo::default();
        b.load_text(&format!("sticky 1\n{}", a.save_text()));
        assert_eq!(b.loaded, vec![(1, [3, 12])]);
    }
}
