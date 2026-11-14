//! Local bullet time uses real seconds for its resource and scales server
//! and client clocks together, including physical projectiles.
use super::*;

#[derive(Default)]
pub(super) struct BulletTime {
    pub active: bool,
    remaining: Option<f64>,
    latched: bool,
    exhausted: bool,
    map: String,
    time: f64,
    visible: bool,
    fraction: f32,
}

impl BulletTime {
    fn update(&mut self, dt: f64, (held, pressed): (bool, bool), toggle: bool,
        meter: bool, duration: f64, regen: f64) {
        let remaining = self.remaining.get_or_insert(duration);
        *remaining = remaining.min(duration);
        if pressed { self.latched = !self.latched; }
        if !held { self.exhausted = false; }
        let wanted = if toggle { self.latched } else { held || pressed };
        self.active = wanted && !self.exhausted && (!meter || *remaining > 0.0);
        if self.active && meter {
            *remaining = (*remaining - dt).max(0.0);
            if *remaining == 0.0 {
                self.active = false;
                self.latched = false;
                self.exhausted = held;
            }
        } else {
            *remaining = (*remaining + dt * regen).min(duration);
        }
        self.fraction = (*remaining / duration) as f32;
    }

    pub(super) fn draw(&self, c: &mut Canvas, meter: bool) {
        if !self.visible || !meter { return; }
        let x = c.vw() / 2.0 - 60.0;
        c.fill(x, 22.0, 120.0, 6.0, [0.0, 0.0, 0.0, 0.7]);
        c.fill(x + 1.0, 23.0, 118.0 * self.fraction, 4.0,
            if self.active { [0.4, 0.8, 1.0, 0.9] } else { [0.8, 0.7, 0.4, 0.8] });
        c.text_center(c.vw() / 2.0, 10.0, "BULLET TIME", 8.0, [0.9, 0.9, 0.9, 1.0]);
    }
}

impl Host {
    pub(super) fn bullet_time_frame(&mut self, dt: f64, playing: bool) {
        let (mut held, mut pressed) = self.input.bullet_time_button();
        for g in &mut self.guests {
            let (h, p) = g.input.bullet_time_button();
            held |= h && g.cl.stats[STAT_HEALTH] > 0;
            pressed |= p && g.cl.stats[STAT_HEALTH] > 0;
        }
        let allowed = self.local.is_some() && self.server.active() && self.server.core.addons_on
            && self.cv("addon_bullet_time") != 0.0 && self.demo.is_none()
            && qserver::addons::allowed(true, self.cv("deathmatch") != 0.0,
                qserver::addons::all_local(&self.server.core.clients));
        let reset = self.bullet_time.map != self.level.mapname || self.cl.time < self.bullet_time.time;
        if !allowed || reset { self.bullet_time = Default::default(); }
        self.bullet_time.map.clone_from(&self.level.mapname);
        self.bullet_time.time = self.cl.time;
        self.bullet_time.visible = allowed && playing && self.input.dest == KeyDest::Game
            && !self.server.core.sv.paused && self.cl.intermission == 0;
        if !self.bullet_time.visible {
            self.bullet_time.active = false;
            self.bullet_time.latched = false;
            return;
        }
        self.bullet_time.update(dt, (held, pressed),
            self.cv("addon_bullet_time_toggle") != 0.0,
            self.cv("addon_bullet_time_meter") != 0.0,
            self.cv("addon_bullet_time_duration").clamp(1.0, 60.0) as f64,
            self.cv("addon_bullet_time_regen").clamp(0.1, 10.0) as f64);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn depletion_requires_release_and_regenerates_in_real_seconds() {
        let mut s = BulletTime::default();
        s.update(1.0, (true, true), false, true, 2.0, 0.5);
        assert!(s.active);
        s.update(1.0, (true, false), false, true, 2.0, 0.5);
        assert!(!s.active);
        s.update(1.0, (true, false), false, true, 2.0, 0.5);
        assert!(!s.active);
        assert_eq!(s.remaining, Some(0.5));
        s.update(1.0, (false, false), false, true, 2.0, 0.5);
        s.update(0.1, (true, true), false, true, 2.0, 0.5);
        assert!(s.active);
    }
    #[test]
    fn toggle_and_unlimited_use() {
        let mut s = BulletTime::default();
        s.update(20.0, (false, true), true, false, 2.0, 1.0);
        assert!(s.active);
        s.update(20.0, (false, false), true, false, 2.0, 1.0);
        assert!(s.active);
        s.update(0.1, (true, true), true, false, 2.0, 1.0);
        assert!(!s.active);
    }
    #[test]
    fn menu_suspends_bullet_time_without_spending_charge() {
        let base = qcore::fs::Fs::default_basedir();
        let mut fs = qcore::fs::Fs::new(base);
        if !fs.add_game_dir("id1") { return; }
        let Some(pal) = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok()) else { return; };
        let Ok(r) = Renderer::headless(320, 240, &pal) else { return; };
        let mut h = Host::new(fs, r, None, Vec::new());
        h.headless = true;
        for cmd in ["sv_autosave 0", "addons_enable 1", "map e1m1"] { h.exec_line(cmd); }
        for _ in 0..30 { h.frame(1.0 / 72.0, Vec::new()); }
        h.menu.open(Screen::None);
        h.con.open = false;
        h.sync_key_dest();
        h.exec_line("+bullettime 42");
        h.bullet_time_frame(0.5, true);
        assert!(h.bullet_time.active);
        let charge = h.bullet_time.remaining;
        h.menu.open(Screen::Main);
        h.sync_key_dest();
        h.bullet_time_frame(2.0, true);
        assert!(!h.bullet_time.active);
        assert!(!h.bullet_time.visible);
        assert_eq!(h.bullet_time.remaining, charge);
        h.exec_line("-bullettime 42");
        h.menu.open(Screen::None);
        h.sync_key_dest();
        h.bullet_time_frame(0.25, true);
        assert!(!h.bullet_time.active);
        assert!(h.bullet_time.remaining > charge);
    }

}
