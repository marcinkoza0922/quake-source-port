//! Assault rifle toy: full automatic from a magazine that reloads from the
//! player's nails ([`super::guns`]). Alt-fire aims down the sight: a tight
//! cone and slower steps. Sustained fire, moving and being in the air widen
//! the cone. Fire timing carries the remainder over frames, so the rate does
//! not depend on the tic rate.
//!
//! The view kick is the client's (`src/addons/rifle_ads.rs`): it moves the
//! real view angles, so shots always go where the view points.

use qcore::cvar::Cvars;
use qcore::math::Vec3;
use qworld::{Edicts, Ent, V};

use super::guns::{self, Kind, MagSync, Magazine};
use super::{BUTTON_TOY_ALT, BUTTON_TOY_FIRE, BUTTON_TOY_RELOAD, Toy};
use crate::ents::Ents;
use crate::server::{Core, Level};

/// Seconds between shots (750 rounds a minute).
pub const INTERVAL: f64 = 0.08;
/// Seconds to raise or lower the sight.
pub const AIM_TIME: f32 = 0.2;
/// Extra reload time for chambering a round after running dry.
pub const CHAMBER: f64 = 0.35;
/// Seconds between running dry and the automatic reload.
const AUTO_DELAY: f64 = 0.3;
/// Most shots in one (very long) frame.
const MAX_SHOTS: usize = 4;
/// Cone (tangent of the half angle) from the hip and aimed, what full bloom
/// adds, and what running flat out or being in the air adds.
const CONE: [f32; 2] = [0.026, 0.0012];
const BLOOM: [f32; 2] = [0.040, 0.0065];
const RUN: [f32; 2] = [0.018, 0.004];
const AIR: [f32; 2] = [0.030, 0.012];
/// Bloom per shot, and lost per second once the trigger rests.
const BLOOM_SHOT: f32 = 0.09;
const BLOOM_DECAY: f32 = 2.4;
/// Walking speed while aiming.
const AIM_SPEED: f32 = 0.6;

/// The settings a frame uses.
#[derive(Clone, Copy, Debug)]
struct Rules {
    on: bool,
    damage: f32,
    cap: u32,
    reload: f64,
    auto: bool,
}

impl Rules {
    fn read(cv: &Cvars) -> Self {
        Self {
            on: cv.value_of(Toy::Rifle.cvar()) != 0.0,
            damage: cv.value_of("addon_rifle_damage").clamp(1.0, 1000.0),
            cap: cv.value_of("addon_rifle_mag").clamp(1.0, 200.0) as u32,
            reload: f64::from(cv.value_of("addon_rifle_reload").clamp(0.2, 10.0)),
            auto: cv.value_of("addon_rifle_autoreload") != 0.0,
        }
    }
}

#[derive(Clone, Debug)]
struct Player {
    mag: Magazine,
    /// When the next shot may leave.
    next: f64,
    /// When the automatic reload starts.
    auto: Option<f64>,
    /// Sight raised, 0..1.
    aim: f32,
    bloom: f32,
    prev: u16,
    rng: u32,
}

impl Player {
    fn new(i: usize, cap: u32) -> Self {
        Self { mag: Magazine::full(cap), next: 0.0, auto: None, aim: 0.0, bloom: 0.0, prev: 0, rng: 0x9e37_79b9 ^ ((i as u32 + 1) * 0x0101_6b55) }
    }

    /// Uniform in -1..1.
    fn rand(&mut self) -> f32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        self.rng as i32 as f32 / 2_147_483_648.0
    }

    /// Shots due by `now` with the trigger held since `pressed`.
    fn due(&mut self, now: f64) -> bool {
        if now + 1e-6 < self.next {
            return false;
        }
        // catching up never goes back further than a frame's worth of shots
        self.next = self.next.max(now - INTERVAL * (MAX_SHOTS - 1) as f64) + INTERVAL;
        true
    }

    /// The current cone; `speed` is horizontal.
    fn cone(&self, speed: f32, air: bool) -> f32 {
        let k = self.aim;
        let mix = |c: [f32; 2]| c[0] + (c[1] - c[0]) * k;
        mix(CONE) + mix(BLOOM) * self.bloom + mix(RUN) * (speed / 320.0).clamp(0.0, 1.0) + if air { mix(AIR) } else { 0.0 }
    }
}

/// Per-level state.
#[derive(Clone, Debug, Default)]
pub struct Rifle {
    players: Vec<Player>,
    sync: MagSync,
}

impl Rifle {
    /// `.qsa` header lines: `rifle <client> <rounds>`.
    pub fn save_text(&self) -> String {
        self.players.iter().enumerate().map(|(i, p)| format!("rifle {i} {}\n", p.mag.rounds)).collect()
    }

    /// Reads this toy's lines of a save header (others are skipped).
    pub fn load_text(&mut self, text: &str) {
        for l in text.lines() {
            let Some(rest) = l.strip_prefix("rifle ") else { continue };
            let n: Vec<u32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
            if let [i, rounds] = n[..]
                && i < 16
            {
                self.ensure(i as usize + 1, rounds.max(1));
                self.players[i as usize].mag.rounds = rounds;
            }
        }
    }

    /// Add-ons went off.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    /// Client `i` has the sight up (mostly).
    pub fn aiming(&self, i: usize) -> bool {
        self.players.get(i).is_some_and(|p| p.aim > 0.5)
    }

    fn ensure(&mut self, n: usize, cap: u32) {
        for i in self.players.len()..n {
            self.players.push(Player::new(i, cap));
        }
    }
}

/// Before a player's move: aiming slows the walk.
pub(crate) fn tune(st: &Rifle, i: usize, p: &mut qworld::phys::PhysParams, cmd: &mut qworld::user::UserCmd) {
    if st.aiming(i) {
        p.maxspeed *= AIM_SPEED;
        cmd.forwardmove *= AIM_SPEED;
        cmd.sidemove *= AIM_SPEED;
    }
}

/// One server frame (after physics), also while the toy is off so it can
/// drop its state.
pub(crate) fn frame(st: &mut Rifle, l: &mut Level, c: &mut Core) {
    let rules = Rules::read(&c.cvars);
    if !rules.on && st.players.is_empty() {
        return;
    }
    let n = c.clients.len();
    if !rules.on {
        st.sync.update(&mut c.clients, l.vm.time, Toy::Rifle, |_| None);
        st.reset();
        return;
    }
    st.ensure(n, rules.cap);
    for (i, p) in st.players.iter_mut().enumerate().take(n) {
        p.mag.cap = rules.cap;
        p.mag.rounds = p.mag.rounds.min(rules.cap);
        player(p, i, rules, l, c);
    }
    let players = &st.players;
    let spawned: Vec<bool> = c.clients.iter().map(|c| c.spawned).collect();
    st.sync.update(&mut c.clients, l.vm.time, Toy::Rifle, |i| spawned[i].then(|| vec![players[i].mag]));
}

fn player(p: &mut Player, i: usize, rules: Rules, l: &mut Level, c: &mut Core) {
    let e = i as Ent + 1;
    let (now, dt) = (l.vm.time, c.sv.phys.frametime as f32);
    let f = c.sv.fields.clone();
    let cl = &c.clients[i];
    let live = cl.spawned && l.vm.ent_f(e, f.health) > 0.0;
    let held = live && Toy::from_buttons(cl.cmd.buttons) == Some(Toy::Rifle);
    let b = if held { cl.cmd.buttons } else { 0 };
    let pressed = b & !p.prev;
    p.prev = b;
    let mut out = String::new();
    let say = |part: Kind, secs: f64, at: Vec3, shot| guns::line(i, Toy::Rifle, 0, part, secs, at, shot);
    if !held {
        if p.mag.cancel() {
            guns::send_all(&mut c.clients, &say(Kind::Cancel, 0.0, Vec3::ZERO, None));
        }
        (p.aim, p.auto) = (0.0, None);
        p.bloom = (p.bloom - BLOOM_DECAY * dt).max(0.0);
        return;
    }
    let (eye, axes) = guns::aim(l, c, e);
    let muzzle = eye + axes[0] * 30.0 - axes[2] * 6.0;
    let nails = l.vm.ent_f(e, f.ammo_nails).max(0.0) as u32;

    // reloading
    if let Some(n) = p.mag.finish(now, Some(nails)) {
        take_nails(l, c, e, n);
        p.next = p.next.max(now);
        out += &say(Kind::Done, 0.0, muzzle, None);
    }
    let auto = p.auto.is_some_and(|t| now >= t);
    if pressed & BUTTON_TOY_RELOAD != 0 || auto {
        p.auto = None;
        let secs = rules.reload + if p.mag.rounds == 0 { CHAMBER } else { 0.0 };
        if p.mag.start(now, secs, Some(nails)) {
            out += &say(Kind::Reload, secs, muzzle, None);
        }
    }

    // the sight goes down for a reload
    let want = b & BUTTON_TOY_ALT != 0 && p.mag.reload_end.is_none();
    let step = dt / AIM_TIME;
    p.aim = (p.aim + if want { step } else { -step }).clamp(0.0, 1.0);

    // firing
    let trigger = b & BUTTON_TOY_FIRE != 0 && p.mag.reload_end.is_none();
    if pressed & BUTTON_TOY_FIRE != 0 {
        p.next = p.next.max(now);
    }
    let mut fired = false;
    for _ in 0..MAX_SHOTS {
        if !trigger || p.mag.rounds == 0 || !p.due(now) {
            break;
        }
        p.mag.take();
        fired = true;
        let (speed, air) = {
            let ents = Ents { vm: &mut l.vm, c: &mut *c };
            let v = ents.v(e, V::Velocity);
            ((v.x * v.x + v.y * v.y).sqrt(), ents.flags(e) & qworld::FL_ONGROUND == 0)
        };
        let cone = p.cone(speed, air);
        let (a, r) = (p.rand() * std::f32::consts::PI, p.rand().abs().sqrt());
        let dir = guns::spread(axes, cone, a.cos() * r, a.sin() * r);
        let shot = guns::bullet(l, c, e, eye, dir, rules.damage);
        p.bloom = (p.bloom + BLOOM_SHOT).min(1.0);
        out += &say(Kind::Fire, 0.0, muzzle, Some(shot));
    }
    if fired {
        guns::hostile(l, e);
        let fx = l.vm.ent_f(e, f.effects) as i32 | crate::msg::EF_MUZZLEFLASH;
        l.vm.set_ent_f(e, f.effects, fx as f32);
    } else {
        if !trigger {
            p.bloom = (p.bloom - BLOOM_DECAY * dt).max(0.0);
        }
        if pressed & BUTTON_TOY_FIRE != 0 && p.mag.rounds == 0 && p.mag.reload_end.is_none() {
            out += &say(Kind::Dry, 0.0, muzzle, None);
        }
    }
    // the trigger held on an empty magazine starts no burst after the reload
    if p.mag.rounds == 0 && p.mag.reload_end.is_none() && p.auto.is_none() && rules.auto && nails > 0 {
        p.auto = Some(now + AUTO_DELAY);
    }
    guns::send_all(&mut c.clients, &out);
}

/// Takes `n` nails for the magazine; the HUD's `currentammo` follows while
/// it shows nails.
fn take_nails(l: &mut Level, c: &Core, e: Ent, n: u32) {
    let f = &c.sv.fields;
    let have = l.vm.ent_f(e, f.ammo_nails);
    let left = (have - n as f32).max(0.0);
    l.vm.set_ent_f(e, f.ammo_nails, left);
    // IT_NAILGUN or IT_SUPER_NAILGUN in hand
    let nailgun = matches!(l.vm.ent_f(e, f.weapon) as i32, 4 | 8);
    if nailgun && l.vm.ent_f(e, f.currentammo) == have {
        l.vm.set_ent_f(e, f.currentammo, left);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fire_rate_does_not_depend_on_the_frame_rate() {
        for fps in [10.0, 30.0, 72.0, 250.0] {
            let mut p = Player::new(0, 30);
            p.next = 1.0;
            let mut shots = 0;
            for k in 0..(4.0 * fps) as usize {
                let now = 1.0 + k as f64 / fps;
                for _ in 0..MAX_SHOTS {
                    shots += usize::from(p.due(now));
                }
            }
            assert!((49..=51).contains(&shots), "{fps}: {shots} shots in 4 s");
        }
    }

    #[test]
    fn aiming_tightens_and_bloom_moving_and_air_widen_the_cone() {
        let mut p = Player::new(0, 30);
        let hip = p.cone(0.0, false);
        p.bloom = 1.0;
        assert!(p.cone(0.0, false) > hip && p.cone(320.0, true) > p.cone(320.0, false) && p.cone(320.0, false) > p.cone(0.0, false));
        p.aim = 1.0;
        assert!(p.cone(0.0, false) < hip);
        p.bloom = 0.0;
        assert!(p.cone(0.0, false) < hip / 15.0);
    }

    #[test]
    fn save_text_round_trips() {
        let mut a = Rifle::default();
        a.ensure(2, 30);
        a.players[1].mag.rounds = 7;
        let mut b = Rifle::default();
        b.load_text(&format!("sticky x\n{}", a.save_text()));
        assert_eq!((b.players[0].mag.rounds, b.players[1].mag.rounds), (30, 7));
        assert_eq!(b.save_text(), a.save_text());
    }
}
