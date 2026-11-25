//! Split-screen input: which player a gamepad plays, joining with Start,
//! and guest moves built from each guest's own binds and sticks.

use super::*;
use qinput::GamepadButton;

impl Host {
    /// The pad player 1's rumble goes to, once other pads play guests.
    pub fn rumble_pad(&self) -> Option<u32> {
        self.p1_pad.filter(|_| self.guests.iter().any(|g| g.pad.is_some()))
    }

    /// A local multiplayer game has room for another local player.
    fn join_open(&self) -> bool {
        let local = self.guests.len() + 1;
        !self.dedicated
            && self.demo.is_none()
            && self.server.active()
            && self.local.is_some()
            && self.cl.signon == 4
            && self.server.core.maxclients() > local
            && local < crate::splitscreen::MAX_LOCAL_PLAYERS
    }

    /// Gamepad input: to the pad's guest, to player 1, or (Start on a free
    /// pad while a local multiplayer game has room) a new player joins.
    pub(super) fn pad_event(&mut self, id: u32, ie: InputEvent) {
        if let Some(k) = self.guests.iter().position(|g| g.pad == Some(id)) {
            self.guest_event(k, ie);
            return;
        }
        if self.p1_pad.is_none() || self.p1_pad == Some(id) {
            self.p1_pad = Some(id);
            if let Some(&family) = self.pad_families.get(&id) {
                self.pad_family = family;
            }
            self.handle_event(HostEvent::Input(ie));
            return;
        }
        let start = matches!(ie, InputEvent::GamepadButton { button: GamepadButton::Start, down: true });
        // A padless guest (from cl_splitscreen, or whose pad went away) is
        // taken over first; otherwise a new player joins.
        if start && let Some(k) = self.guests.iter().position(|g| g.pad.is_none()) {
            self.guests[k].pad = Some(id);
            self.con.print(&qcore::ctr!("{} joined on a gamepad\n", self.guests[k].cl.name));
            return;
        }
        if start && self.join_open() {
            let n = self.guests.len() + 2;
            self.server.core.cvars.set("cl_splitscreen", &n.to_string());
            self.sync_guests();
            if let Some(g) = self.guests.last_mut().filter(|g| g.pad.is_none()) {
                g.pad = Some(id);
            }
            return;
        }
        // Free pads keep driving player 1 outside local multiplayer.
        if !self.join_open() && self.guests.is_empty() {
            if let Some(&family) = self.pad_families.get(&id) {
                self.pad_family = family;
            }
            self.handle_event(HostEvent::Input(ie));
        }
    }

    /// Forget a disconnected pad: its guest stays in the game, padless,
    /// until another free pad presses Start.
    pub(super) fn pad_removed(&mut self, id: u32) {
        self.pad_families.remove(&id);
        for g in self.guests.iter_mut().filter(|g| g.pad == Some(id)) {
            g.pad = None;
            g.training.sequence.reset_sticks();
            g.input.buttons = Default::default();
        }
        let p1 = self.p1_pad == Some(id);
        if p1 {
            self.p1_pad = None;
            self.training.sequence.reset_sticks();
            self.pad_gestures.reset_all();
        }
        // player 1 keeps its sticks when another pad goes
        let left = self.pad_families.keys().any(|k| !self.guests.iter().any(|g| g.pad == Some(*k)));
        if p1 || !left {
            let _ = qinput::handle_event(self, InputEvent::GamepadConnected(false));
            if left {
                let _ = qinput::handle_event(self, InputEvent::GamepadConnected(true));
            }
        }
    }

    /// Run a guest's pad event against that guest's own input. Its binds
    /// execute at once so `+forward` and `impulse` land on the guest. Menu
    /// and console navigation stays shared: presses (with their releases) and
    /// sticks go to player 1's path while either is open, and the guest's own
    /// sticks and held buttons still follow the pad.
    fn guest_event(&mut self, k: usize, ie: InputEvent) {
        let shared = self.key_dest() != KeyDest::Game;
        match ie {
            InputEvent::GamepadButton { button, down: true } if shared => {
                self.menu_presses.push((k, button));
                return self.handle_event(HostEvent::Input(ie));
            }
            InputEvent::GamepadButton { button, down: false } if self.menu_presses.contains(&(k, button)) => {
                self.menu_presses.retain(|p| *p != (k, button));
                return self.handle_event(HostEvent::Input(ie));
            }
            InputEvent::GamepadAxis { axis, value } if shared => {
                self.menu_axes = true;
                self.handle_event(HostEvent::Input(ie));
                let trigger = matches!(axis, qinput::GamepadAxis::LeftTrigger | qinput::GamepadAxis::RightTrigger);
                // a trigger pulled in the menu isn't the guest's to fire with
                if trigger && value > self.cv("joy_deadzone_trigger") {
                    return;
                }
            }
            _ => {}
        }
        self.record_hud_input(Some(k), ie);
        self.input_target = Some(k);
        self.guests[k].input.dest = KeyDest::Game;
        let routed = qinput::handle_event(self, ie);
        while let Some(line) = self.guest_cbuf.next_line() {
            self.exec_line(&line);
        }
        self.input_target = None;
        if routed == Some(qinput::Routed::Escape) {
            self.menu.open(Screen::Main);
            self.sync_key_dest();
        }
    }

    /// centerview of the player whose bind runs.
    pub(super) fn centre_view(&mut self) {
        match self.input_target.and_then(|k| self.guests.get_mut(k)) {
            Some(g) => g.cl.viewangles[0] = 0.0,
            None => self.cl.viewangles[0] = 0.0,
        }
    }

    /// A guest's bind with a command about its player: true when it was run.
    pub(super) fn guest_player_command(&mut self, name: &str, line: &str) -> bool {
        const PLAYER: &[&str] = &["kill", "god", "notarget", "fly", "noclip", "give", "setpos", "say", "say_team", "tell"];
        let Some(slot) = self.input_target.and_then(|k| self.guests.get(k)).map(|g| g.slot) else { return false };
        if !PLAYER.contains(&name) {
            return false;
        }
        self.server.client_command(slot, &qcore::lang::quake_encode(line));
        self.drain_server_log();
        true
    }

    /// Play guests' sounds that started out of player 1's hearing range
    /// (player 1's client got the nearer ones already; the mixer pans and
    /// attenuates for the nearest player).
    pub(super) fn guest_sounds(&mut self) {
        let p1 = self.cl.entities.get(self.cl.viewentity).map(|e| e.origin);
        for k in 0..self.guests.len() {
            for e in std::mem::take(&mut self.guests[k].sounds) {
                let (origin, start) = match &e {
                    ClientEvent::Sound { origin, attenuation, .. } => (*origin, *attenuation > 0.0),
                    ClientEvent::TempSound { origin, .. } => (*origin, true),
                    _ => continue,
                };
                let far = p1.is_none_or(|p| {
                    let d = [origin[0] - p[0], origin[1] - p[1], origin[2] - p[2]];
                    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > 1000.0
                });
                if !start || !far {
                    continue;
                }
                let Some(snd) = &mut self.snd else { continue };
                match e {
                    ClientEvent::Sound { ent, channel, sound, origin, volume, attenuation } => {
                        if let Some(Some(sfx)) = self.level.sounds.get(sound) {
                            snd.event(SoundEvent::Start { entity: ent, channel, sfx: sfx.clone(), origin, volume, attenuation });
                        }
                    }
                    ClientEvent::TempSound { name, origin } => {
                        if let Some(sfx) = snd.precache(name) {
                            snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin, volume: 1.0, attenuation: 1.0 });
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    /// Translated text, hold time and wrap columns for a center print.
    pub(super) fn center_args(&self, s: &str) -> (String, f64, usize) {
        let t = self.cv("scr_centertime") as f64;
        let s = self.server.core.loc.translate(s);
        let cols = if self.cv("scr_usekfont") != 0.0 {
            let (w, h) = self.r.size();
            crate::console::center_wrap_cols(Canvas::new(w, h).vw())
        } else {
            0
        };
        (s, t, cols)
    }

    /// Guests' center prints go to their own message, drawn in their cell.
    pub(super) fn guest_center_prints(&mut self) {
        for k in 0..self.guests.len() {
            self.guests[k].center.realtime = self.realtime;
            for s in std::mem::take(&mut self.guests[k].center_prints) {
                let (s, t, cols) = self.center_args(&s);
                self.guests[k].center.center_print(&s, t, cols);
            }
        }
    }

    /// Each frame: turn every guest's input into its next move.
    pub(super) fn guest_moves(&mut self, ft: f32) {
        let in_game = self.key_dest() == KeyDest::Game;
        let sp = self.move_speeds();
        let run = self.cv("cl_alwaysrun") != 0.0;
        let demo = self.demo.is_some();
        let allowed = aim_assist_allowed(self.cv("deathmatch") != 0.0, self.cv("sv_aimassist") != 0.0);
        let cm = self.server.level.as_ref().map(|l| l.world.worldmodel.clone());
        let targets: Vec<Vec<AimTarget>> = self
            .guests
            .iter()
            .map(|g| if g.cl.signon == 4 && g.cl.intermission == 0 { self.aim_targets(&g.cl) } else { Vec::new() })
            .collect();
        let toys = self.allowed_toys();
        let autoaim_allowed = self.local_autoaim_allowed() && !demo && in_game;
        let (zoom, sight) = (self.zoom_fov(), self.cv("addon_rifle_zoom"));
        let toggle_aim = self.cv("addon_rifle_aim") != 0.0;
        let autoaim_fov: Vec<_> = self.guests.iter().map(|g| self.autoaim_fov(g.ads.fov(zoom, sight))).collect();
        let autoaim_players = self.cv("deathmatch") != 0.0;
        let autofire_allowed = !autoaim_players && !self.server.core.sv.paused;
        let cvars = &self.server.core.cvars;
        let entries = crate::wheel::inventory(cvars, &toys);
        for ((g, targets), autoaim_fov) in self.guests.iter_mut().zip(&targets).zip(autoaim_fov) {
            if g.cl.signon != 4 {
                continue;
            }
            let eye = g.cl.entities.get(g.cl.viewentity).map_or([0.0; 3], |e| [e.origin[0], e.origin[1], e.origin[2] + g.cl.viewheight]);
            let vis = cm.as_deref().map(|cm| WorldVis::new(cm, &g.cl));
            let ctx = vis.as_ref().map(|v| AimContext { eye, player_velocity: g.cl.velocity, targets, vis: v, allowed });
            g.input.autoaim_allowed = autoaim_allowed && g.cl.intermission == 0 && g.cl.stats[qnet::protocol::STAT_HEALTH] > 0;
            g.input.autoaim_fov = autoaim_fov;
            g.input.look_scale = g.ads.look_scale(sight);
            g.input.autoaim_players = autoaim_players;
            g.input.dest = if in_game { KeyDest::Game } else { KeyDest::Menu };
            let autofire = autofire_allowed && g.pad.is_some() && g.input.autofire_enabled(cvars);
            let mut inp = g.input.frame(cvars, g.cl.viewangles, ft, ctx.as_ref());
            if !in_game {
                inp = InputState::default();
            }
            if run {
                inp.speed = !inp.speed;
            }
            let mut fresh = inp.impulse;
            let alive = g.cl.stats[qnet::protocol::STAT_HEALTH] > 0;
            let allowed = in_game && alive && g.cl.intermission == 0 && !demo;
            let wheel_input = crate::wheel::WheelInput::Button { held: g.input.wheel_held(), aim: g.input.wheel_aim };
            let choice = g.wheel.update(ft, wheel_input, allowed, &g.cl, &entries, g.toy.held);
            g.input.wheel_open = g.wheel.open;
            g.toy.update_slots(cvars, &toys, (inp.weapon_slot, &mut fresh), &g.cl);
            if let Some(weapon) = choice {
                g.toy.select(weapon, &g.cl, &mut fresh);
            }
            g.ads.update(ft, crate::addons::rifle_ads::Frame {
                held: g.toy.held == Some(qserver::addons::Toy::Rifle),
                active: allowed,
                alt: inp.attack2,
                toggle: toggle_aim,
                reloading: crate::addons::rifle_ads::reloading(&g.cl),
            });
            g.ads.apply(ft, &mut g.cl.viewangles, inp.look_delta[1]);
            let alt = g.ads.alt(g.toy.held, inp.attack2);
            let dodge = g.input.dodge_pressed();
            let buttons = g.cmd.buttons | g.toy.buttons(inp.attack, alt, inp.reload, inp.jump)
                | if dodge && allowed { qserver::addons::BUTTON_DODGE } else { 0 };
            let impulse = if fresh != 0 { fresh } else { g.cmd.impulse };
            g.cmd = g.cl.base_move(&InputState { attack: false, jump: false, impulse: 0, ..inp }, &sp, ft);
            g.auto_attack = autofire && buttons & 1 == 0 && impulse == 0 && g.toy.held.is_none()
                && aim_vis::autofire_target(ctx.as_ref(), &g.cl);
            g.cmd.buttons = buttons;
            g.cmd.impulse = impulse;
            // Intermission: any button moves on, as for player 1.
            if g.cl.intermission != 0 && g.input.buttons.iter().any(|b| b.is_down()) {
                g.cmd.buttons |= 1;
            }
        }
    }
}
