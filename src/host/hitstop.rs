//! Add-on hitstop: a short freeze of the local single-player game when the
//! player damages an enemy. The server confirms hits with `//qsp_hitstop`
//! (kill, damage, class, target health); everything else, including the
//! timer, is client side and measured in real seconds.
use super::*;

/// What the player chose; clamped where read in [`Settings::read`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Settings {
    /// 0 all, 1 melee, 2 ranged, 3 explosives.
    pub weapons: u8,
    /// 0 all enemies, 1 only enemies whose health was at least `tough`.
    pub tough_only: bool,
    pub tough: i32,
    pub duration: f64,
    /// Extra seconds per 10 damage.
    pub per_damage: f64,
    pub kill_bonus: f64,
    pub cooldown: f64,
    pub max: f64,
    pub min_damage: i32,
}

/// One server-confirmed hit.
#[derive(Clone, Copy, Debug)]
pub(super) struct Hit {
    pub kill: bool,
    pub damage: i32,
    pub class: u8,
    pub health: i32,
}

#[derive(Default)]
pub(super) struct HitStop {
    until: f64,
    next: f64,
    map: String,
}

impl HitStop {
    pub fn active(&self, now: f64) -> bool { now < self.until }

    /// Starts or extends a freeze; returns whether the hit counted.
    pub fn trigger(&mut self, now: f64, s: &Settings, h: Hit) -> bool {
        let class_ok = match s.weapons { 1 => h.class == 1, 2 => h.class == 0, 3 => h.class == 2, _ => true };
        if !class_ok || h.damage < s.min_damage.max(1) || (s.tough_only && h.health < s.tough) {
            return false;
        }
        // Pellets and rapid fire count once; a kill always lands.
        if now < self.next && !h.kill { return false; }
        let mut d = s.duration + s.per_damage * h.damage as f64 / 10.0;
        if h.kill { d *= s.kill_bonus; }
        let d = d.min(s.max);
        if d <= 0.0 { return false; }
        self.until = self.until.max(now + d);
        self.next = now + s.cooldown;
        true
    }
}

impl Settings {
    fn read(h: &Host) -> Self {
        let ms = |n: &str, hi: f32| h.cv(n).clamp(0.0, hi) as f64 / 1000.0;
        Self {
            weapons: h.cv("addon_hitstop_weapons").clamp(0.0, 3.0) as u8,
            tough_only: h.cv("addon_hitstop_targets") != 0.0,
            tough: h.cv("addon_hitstop_tough").clamp(1.0, 1000.0) as i32,
            duration: ms("addon_hitstop_duration", 250.0),
            per_damage: ms("addon_hitstop_scale_damage", 50.0),
            kill_bonus: h.cv("addon_hitstop_kill_bonus").clamp(1.0, 4.0) as f64,
            cooldown: ms("addon_hitstop_cooldown", 1000.0),
            max: ms("addon_hitstop_max", 1000.0),
            min_damage: h.cv("addon_hitstop_min_damage").clamp(0.0, 100.0) as i32,
        }
    }
}

impl Host {
    /// Hitstop only runs in a local one-player game started with add-ons.
    fn hitstop_allowed(&self) -> bool {
        self.local.is_some() && self.guests.is_empty() && self.demo.is_none()
            && self.net_client.is_none() && self.server.core.maxclients() == 1
            && self.server.active() && self.server.core.addons_on
            && self.cv("addon_hitstop") != 0.0 && self.cv("deathmatch") == 0.0
    }

    /// Whether the server should be held this frame.
    pub(super) fn hitstop_hold(&mut self) -> bool {
        if self.hitstop.map != self.level.mapname {
            self.hitstop = HitStop { map: self.level.mapname.clone(), ..Default::default() };
        }
        self.hitstop.active(self.realtime) && self.hitstop_allowed()
            && !self.server.core.sv.paused && self.cl.intermission == 0
    }

    pub(super) fn receive_hitstop(&mut self, hit: Hit) {
        if !self.hitstop_allowed() || self.cl.stats[STAT_HEALTH] <= 0 { return; }
        let s = Settings::read(self);
        if self.hitstop.trigger(self.realtime, &s, hit) {
            let shake = self.cv("addon_hitstop_shake").clamp(0.0, 1.0);
            if shake > 0.0 && self.cv("a11y_reducemotion") == 0.0 {
                let r = (self.realtime * 977.0).sin() as f32;
                self.cl.punchangles[1] = [-2.0 * shake, 3.0 * shake * r, 1.5 * shake * -r];
                self.cl.punchangles[0] = [0.0; 3];
                self.cl.punchtime = self.cl.time;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s() -> Settings {
        Settings { weapons: 0, tough_only: false, tough: 100, duration: 0.06, per_damage: 0.0,
            kill_bonus: 2.0, cooldown: 0.15, max: 0.25, min_damage: 0 }
    }
    fn hit(kill: bool, damage: i32, class: u8, health: i32) -> Hit { Hit { kill, damage, class, health } }

    #[test]
    fn freeze_lasts_duration_and_cooldown_ignores_pellets() {
        let mut h = HitStop::default();
        assert!(h.trigger(1.0, &s(), hit(false, 4, 0, 30)));
        assert!(h.active(1.05) && !h.active(1.07));
        assert!(!h.trigger(1.0, &s(), hit(false, 4, 0, 30)));
        assert!(!h.trigger(1.1, &s(), hit(false, 4, 0, 30)));
        assert!(h.trigger(1.16, &s(), hit(false, 4, 0, 30)));
    }
    #[test]
    fn kills_bonus_and_cap_apply_and_bypass_cooldown() {
        let mut h = HitStop::default();
        h.trigger(1.0, &s(), hit(false, 4, 0, 30));
        assert!(h.trigger(1.01, &s(), hit(true, 4, 0, 30)));
        assert!(h.active(1.12) && !h.active(1.14));
        let mut big = s();
        big.per_damage = 0.1;
        let mut h = HitStop::default();
        h.trigger(0.0, &big, hit(false, 100, 0, 30));
        assert!(h.active(0.24) && !h.active(0.26));
    }
    #[test]
    fn weapon_target_and_damage_filters() {
        let mut cfg = s();
        cfg.weapons = 1;
        let mut h = HitStop::default();
        assert!(!h.trigger(1.0, &cfg, hit(false, 20, 0, 30)));
        assert!(!h.trigger(1.0, &cfg, hit(false, 20, 2, 30)));
        assert!(h.trigger(1.0, &cfg, hit(false, 20, 1, 30)));
        cfg.weapons = 3;
        assert!(h.trigger(2.0, &cfg, hit(false, 20, 2, 30)));
        cfg.weapons = 0;
        cfg.tough_only = true;
        assert!(!h.trigger(3.0, &cfg, hit(false, 20, 0, 99)));
        assert!(h.trigger(3.0, &cfg, hit(false, 20, 0, 100)));
        cfg.tough_only = false;
        cfg.min_damage = 10;
        assert!(!h.trigger(4.0, &cfg, hit(false, 9, 0, 30)));
        assert!(h.trigger(4.0, &cfg, hit(false, 10, 0, 30)));
    }
}
