//! Learns how each weapon throws, so the arc assist works for any mod
//! (qbj3's rebar cannon and flak, hipnotic's proximity gun, ...). The first
//! gravity-affected projectile (MOVETYPE_TOSS/BOUNCE) a weapon spawns gives
//! its launch offset and velocity in the player's view basis and its gravity
//! scale; watching it fly tells whether it sticks or explodes on contact and
//! how long its fuse is. Profiles are keyed by view model and kept per game.
//! Weapons whose QuakeC we know are hardcoded in [`known`] and need no shot.
use std::collections::HashMap;

use qcore::math::{Vec3, angle_vectors};
use qserver::Thrown;

/// How one weapon throws: offset and velocity in (forward, right, up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Profile {
    pub off: Vec3,
    /// World-z offset added to `off` (QC `self.view_ofs`).
    pub lift: f32,
    pub vel: Vec3,
    pub gravity: f32,
    /// Velocity clip overbounce: 1.5 MOVETYPE_BOUNCE, 1.0 TOSS (slides and
    /// stops on floors).
    pub backoff: f32,
    /// Stops at the first contact (sticks or explodes).
    pub stick: bool,
    /// Seconds until it explodes or is removed.
    pub fuse: f32,
    /// Share of the player's velocity the projectile inherits.
    pub inherit: f32,
    /// Collides as MOVE_MISSILE (monster boxes grown by 15).
    pub missile: bool,
    /// Random launch velocity half-range in (forward, right, up) ups, for
    /// the landing ellipse (id1: `crandom()*10` along right and up).
    pub spread: Vec3,
}

/// id1 W_FireGrenade: forward*600 + up*200 from the player origin, 2.5 s.
pub(crate) const ID_GRENADE: Profile =
    Profile { off: Vec3::ZERO, lift: 0.0, vel: Vec3::new(600.0, 0.0, 200.0), gravity: 1.0, backoff: 1.5, stick: false, fuse: 2.5, inherit: 0.0, missile: false, spread: Vec3::new(0.0, 10.0, 10.0) };

/// Hardcoded throws by view model, from the mods' QuakeC.
pub(crate) fn known(model: &str) -> Option<Profile> {
    Some(match model {
        // id1, hipnotic, rogue; qbj3 keeps id1's W_FireGrenade
        "progs/v_rock.mdl" | "progs/v_grenlauncher.mdl" => ID_GRENADE,
        // qbj3 rebar cannon: origin + view_ofs - up*8 + forward*15,
        // forward*1400 + up*160, gravity .75, impales on contact
        "progs/v_rebar.mdl" => Profile {
            off: Vec3::new(15.0, 0.0, -8.0), lift: 22.0, vel: Vec3::new(1400.0, 0.0, 160.0),
            gravity: 0.75, backoff: 1.5, stick: true, fuse: 10.0, inherit: 0.0, missile: false, spread: Vec3::ZERO,
        },
        // qbj3 flak shotgun (centre of the pellet spread): forward*12 - up*4
        // from the eye, forward*2000 + up*100, gravity .5, FlakBounce
        // reflects with 2/1.5, pellets live 1 s
        "progs/v_flakshotgun.mdl" => Profile {
            off: Vec3::new(12.0, 0.0, -4.0), lift: 22.0, vel: Vec3::new(2000.0, 0.0, 100.0),
            gravity: 0.5, backoff: 4.0 / 3.0, stick: false, fuse: 1.0, inherit: 0.0, missile: false, spread: Vec3::ZERO,
        },
        _ => return None,
    })
}

/// Add-on throws (engine-side, so never learned from edicts). `speed` is
/// the stickybomb's current charge.
pub(crate) fn toy(t: qserver::addons::Toy, speed: f32) -> Option<Profile> {
    use qserver::addons::{Toy, gel, sticky};
    Some(match t {
        // sticks to world, movers and monsters; detonated by alt-fire
        Toy::Sticky => Profile {
            off: Vec3::ZERO, lift: 0.0, vel: Vec3::new(speed, 0.0, sticky::LIFT),
            gravity: 1.0, backoff: 1.0, stick: true, fuse: 10.0, inherit: 0.0, missile: true, spread: Vec3::ZERO,
        },
        // eye + forward*12 - up*6, ±3.5% aim and ±8% speed spread, half the
        // player's velocity;
        // splats on the first thing hit
        Toy::GelSprayer => Profile {
            off: Vec3::new(12.0, 0.0, -6.0), lift: 22.0, vel: Vec3::new(gel::SPEED, 0.0, 0.0),
            gravity: 1.0, backoff: 1.0, stick: true, fuse: 10.0, inherit: 0.5, missile: true,
            spread: Vec3::new(0.08, 0.035, 0.035) * gel::SPEED,
        },
        _ => return None,
    })
}
const DEFAULT_FUSE: f32 = 4.0;
/// Slower spawns are drops (backpacks, casings), not shots.
const MIN_SPEED: f32 = 250.0;
const BLEND: f32 = 0.35;

struct Track {
    key: String,
    born: f64,
    pos: Vec3,
    vel: Vec3,
    gravity: f32,
    contacted: bool,
}

#[derive(Default)]
pub(crate) struct ArcLearn {
    game: String,
    profiles: HashMap<String, Profile>,
    tracks: HashMap<u32, Track>,
    last_time: f64,
}

impl ArcLearn {
    /// The profile for a weapon view model: hardcoded, else learned.
    pub(crate) fn profile(&self, model: &str) -> Option<Profile> {
        known(model).or_else(|| self.profiles.get(model).copied())
    }

    /// Feed one server snapshot. `weapon` is the current view model,
    /// `hits_world` reports whether a short segment hits the map.
    pub(crate) fn update(&mut self, game: &str, snap: &Thrown, weapon: Option<&str>, world_g: f32, mut hits_world: impl FnMut(Vec3, Vec3) -> bool) {
        if self.game != game {
            *self = Self { game: game.to_string(), ..Default::default() };
        }
        let dt = (snap.time - self.last_time) as f32;
        if dt == 0.0 {
            return; // several rendered frames per server tick
        }
        if dt < 0.0 || dt > 0.5 {
            self.last_time = snap.time;
            self.tracks.clear();
            return;
        }
        self.last_time = snap.time;
        // gone: stuck/exploded on contact, or the fuse ran out
        let live: Vec<u32> = snap.list.iter().map(|e| e.ent).collect();
        let gone: Vec<u32> = self.tracks.keys().copied().filter(|e| !live.contains(e)).collect();
        for e in gone {
            let t = self.tracks.remove(&e).unwrap();
            let age = (snap.time - t.born) as f32;
            let Some(p) = self.profiles.get_mut(&t.key) else { continue };
            if !t.contacted && hits_world(t.pos, t.pos + t.vel * (dt * 2.0)) {
                p.stick = true;
            } else if t.contacted || age >= 1.0 {
                p.fuse = age.clamp(0.2, 10.0);
            }
        }
        for e in &snap.list {
            if let Some(t) = self.tracks.get_mut(&e.ent) {
                let pred = t.vel - Vec3::new(0.0, 0.0, world_g * t.gravity * dt);
                if !t.contacted && (e.velocity - pred).length() > 40.0 {
                    t.contacted = true;
                }
                (t.pos, t.vel) = (e.origin, e.velocity);
                continue;
            }
            let Some(key) = weapon else { continue };
            if e.velocity.length() < MIN_SPEED { continue; }
            // undo the physics step it already took this frame
            let vel = e.velocity + Vec3::new(0.0, 0.0, world_g * e.gravity * dt);
            let d = e.origin - vel * dt - snap.origin;
            let (f, r, u) = angle_vectors(snap.v_angle);
            let basis = |v: Vec3| Vec3::new(v.dot(f), v.dot(r), v.dot(u));
            let sample = Profile { off: basis(d), lift: 0.0, vel: basis(vel), gravity: e.gravity, backoff: if e.bounce { 1.5 } else { 1.0 }, stick: false, fuse: DEFAULT_FUSE, inherit: 0.0, missile: false, spread: Vec3::ZERO };
            match self.profiles.get_mut(key) {
                // average the spread of repeated shots and pellets
                Some(p) => {
                    // widest recent deviation from the mean, capped so the
                    // player's own movement isn't taken for spread
                    let d = sample.vel - p.vel;
                    let dev = Vec3::new(d.x.abs(), d.y.abs(), d.z.abs()).min(Vec3::splat(p.vel.length() * 0.1));
                    p.spread = (p.spread * 0.95).max(dev);
                    p.off = p.off + (sample.off - p.off) * BLEND;
                    p.vel = p.vel + (sample.vel - p.vel) * BLEND;
                    (p.gravity, p.backoff) = (sample.gravity, sample.backoff);
                }
                None => {
                    self.profiles.insert(key.to_string(), sample);
                }
            }
            self.tracks.insert(e.ent, Track { key: key.to_string(), born: snap.time - dt as f64, pos: e.origin, vel: e.velocity, gravity: e.gravity, contacted: false });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qserver::ThrownEnt;

    fn snap(time: f64, list: Vec<ThrownEnt>) -> Thrown {
        Thrown { time, origin: Vec3::ZERO, v_angle: Vec3::ZERO, list }
    }
    fn ent(origin: Vec3, velocity: Vec3) -> ThrownEnt {
        ThrownEnt { ent: 9, origin, velocity, bounce: true, gravity: 0.5 }
    }

    #[test]
    fn learns_launch_and_sticking() {
        let mut l = ArcLearn::default();
        let dt = 0.05;
        l.update("m", &snap(1.0, vec![]), Some("progs/v_x.mdl"), 800.0, |_, _| false);
        // spawned at (0,0,16) going 1000 forward + 100 up, moved one step
        let v = Vec3::new(1000.0, 0.0, 100.0 - 400.0 * dt as f32);
        let o = Vec3::new(0.0, 0.0, 16.0) + Vec3::new(1000.0, 0.0, 100.0) * dt as f32;
        l.update("m", &snap(1.0 + dt, vec![ent(o, v)]), Some("progs/v_x.mdl"), 800.0, |_, _| false);
        let p = l.profiles["progs/v_x.mdl"];
        assert!((p.vel - Vec3::new(1000.0, 0.0, 100.0)).length() < 1.0, "{:?}", p.vel);
        assert!((p.off - Vec3::new(0.0, 0.0, 16.0)).length() < 1.0, "{:?}", p.off);
        assert_eq!(p.gravity, 0.5);
        assert!(!p.stick);
        // removed next to a wall without bouncing: sticks on contact
        l.update("m", &snap(1.0 + 2.0 * dt, vec![]), Some("progs/v_x.mdl"), 800.0, |_, _| true);
        assert!(l.profiles["progs/v_x.mdl"].stick);
    }

    #[test]
    fn known_weapons_need_no_shot() {
        assert_eq!(ArcLearn::default().profile("progs/v_rock.mdl"), Some(ID_GRENADE));
        assert!(ArcLearn::default().profile("progs/v_rebar.mdl").unwrap().stick);
        assert_eq!(ArcLearn::default().profile("progs/v_shot.mdl"), None);
    }
}
