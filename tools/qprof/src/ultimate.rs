//! The ultimate stress test: Arcane Dimensions' ad_sepulcher with 3 extra
//! copies of every monster (`sv_horde 3`), all of them re-woken every 4 s
//! to hunt the players (`prof_wake`), every enhancement and add-on on, and
//! each player doing everything at once: all 8 stock weapons, all 7 toys
//! with primary and alternate fire (both portals, both gels, sticky
//! detonation, ...), dives in every direction, jumping, strafing,
//! back-pedalling, a constant spin, plus goggles cycling and bullet time.
//! `ultimate-coop` does the same with four local split-screen players
//! (coop 1), each driven through `splitbtn`/`splittoy`/`splitcmd`.

use crate::scenarios::{ADDON_ON, Group, Scenario, pulse};

/// Seconds per weapon/toy slot: primary fire, then alternate fire.
const SLOT: f32 = 1.25;

/// Console commands acting as player 1 or split-screen guest `n` (2..=4).
struct Actor(usize);

impl Actor {
    fn btn(&self, b: &str, down: bool) -> String {
        let s = if down { '+' } else { '-' };
        if self.0 == 1 { format!("{s}{b}") } else { format!("splitbtn {} {s}{b}", self.0) }
    }
    fn toy(&self, k: usize) -> String {
        if self.0 == 1 { format!("toy {k}") } else { format!("splittoy {} {k}", self.0) }
    }
    fn impulse(&self, k: u8) -> String {
        if self.0 == 1 { format!("impulse {k}") } else { format!("splitbtn {} impulse {k}", self.0) }
    }
    fn cmd(&self, c: &str) -> String {
        if self.0 == 1 { c.to_string() } else { format!("splitcmd {} {c}", self.0) }
    }
    /// Every weapon and full ammo through `give` (mods such as AD refuse
    /// `impulse 9` in co-op).
    fn arm(&self, t: f32) -> Vec<(f32, String)> {
        let mut v: Vec<(f32, String)> = (2..=8).map(|w| (t, self.cmd(&format!("give {w}")))).collect();
        v.extend(["give s 100", "give n 200", "give r 100", "give c 200"].map(|c| (t, self.cmd(c))));
        v
    }
    fn pulse(&self, secs: f32, period: f32, hold: f32, offset: f32, b: &str) -> Vec<(f32, String)> {
        pulse(secs, period, hold, &self.btn(b, true), &self.btn(b, false))
            .into_iter().map(|(t, c)| (t + offset, c)).filter(|(t, _)| *t < secs).collect()
    }

    /// Everything one player does for `secs` seconds.
    fn everything(&self, secs: f32) -> Vec<(f32, String)> {
        let off = (self.0 - 1) as f32 * 0.37;
        // Player 1 already gets god mode and the spin from the common
        // script (a second `god` would toggle it off again).
        let mut v = if self.0 == 1 { vec![] } else { vec![(0.0, self.cmd("god")), (0.0, self.btn("right", true))] };
        // Weapons, then ammo top-ups: nonstop fire drains rockets and cells.
        for t in (0..).map(|k| k as f32 * 5.0).take_while(|t| *t < secs) {
            v.extend(self.arm(t));
        }
        // Movement: strafe left/right, forward/back, jump, and dive.
        v.extend(self.pulse(secs, 4.0, 2.0, off, "moveleft"));
        v.extend(self.pulse(secs, 4.0, 2.0, off + 2.0, "moveright"));
        v.extend(self.pulse(secs, 6.0, 1.5, off + 0.5, "forward"));
        v.extend(self.pulse(secs, 6.0, 1.5, off + 3.5, "back"));
        v.extend(self.pulse(secs, 1.3, 0.2, off, "jump"));
        v.extend(self.pulse(secs, 0.9, 0.1, off + 0.3, "dodge"));
        // Weapon cycle: stock weapons 1..8 held, then each toy's primary and
        // alternate fire. Each player starts at a different point.
        let mut t = 0.1;
        let mut k = (self.0 - 1) * 4;
        while t < secs {
            match k % 15 {
                w @ 0..8 => {
                    v.push((t, self.toy(99)));
                    v.push((t + 0.02, self.impulse(w as u8 + 1)));
                    v.push((t + 0.1, self.btn("attack", true)));
                    v.push((t + SLOT - 0.1, self.btn("attack", false)));
                }
                toy => {
                    v.push((t, self.toy(toy - 8)));
                    v.push((t + 0.05, self.btn("attack", true)));
                    v.push((t + 0.45, self.btn("attack", false)));
                    v.push((t + 0.6, self.btn("attack2", true)));
                    v.push((t + 1.0, self.btn("attack2", false)));
                }
            }
            t += SLOT;
            k += 1;
        }
        v
    }
}

/// Shared by both ultimate variants: wake the horde, cycle goggles
/// (player 1's; guests share the post effect), pulse bullet time.
fn world(secs: f32) -> Vec<(f32, String)> {
    let mut v = pulse(secs, 4.0, 0.1, "prof_wake", "echo");
    v.extend(pulse(secs, 7.0, 4.0, "+bullettime", "-bullettime"));
    let modes = ["goggles nv", "goggles thermal", "goggles off"];
    v.extend((0..).map(|k| (k as f32 * 3.0, modes[k % 3].to_string())).take_while(|(t, _)| *t < secs));
    v
}

pub fn scenarios(secs: f32, every_enhancement: Vec<(&'static str, &'static str)>) -> Vec<Scenario> {
    let mut cvars = every_enhancement;
    cvars.extend_from_slice(ADDON_ON);
    cvars.push(("sv_horde", "3"));
    let mut solo = world(secs);
    solo.extend(Actor(1).everything(secs));
    let mut coop_cvars = cvars.clone();
    coop_cvars.extend([("coop", "1"), ("maxplayers", "4"), ("cl_splitscreen", "4")]);
    let mut coop = world(secs);
    for p in 1..=4 {
        coop.extend(Actor(p).everything(secs));
    }
    vec![
        Scenario {
            name: "ultimate", group: Group::Ultimate,
            what: "ad_sepulcher, x4 monsters hunting the player; every enhancement and add-on; every weapon, toy (both fires), dive, jump, goggles, bullet time",
            cvars, map: Some(("ad", "ad_sepulcher")), script: solo, own_fire: true,
        },
        Scenario {
            name: "ultimate-coop", group: Group::Ultimate,
            what: "the ultimate test with 4 local split-screen co-op players all doing everything",
            cvars: coop_cvars, map: Some(("ad", "ad_sepulcher")), script: coop, own_fire: true,
        },
    ]
}
