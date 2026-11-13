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
            self.handle_event(HostEvent::Input(ie));
            return;
        }
        let start = matches!(ie, InputEvent::GamepadButton { button: GamepadButton::Start, down: true });
        // A padless guest (from cl_splitscreen, or whose pad went away) is
        // taken over first; otherwise a new player joins.
        if start && let Some(k) = self.guests.iter().position(|g| g.pad.is_none()) {
            self.guests[k].pad = Some(id);
            self.con.print(&format!("{} joined on a gamepad\n", self.guests[k].cl.name));
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
            self.handle_event(HostEvent::Input(ie));
        }
    }

    /// Forget a disconnected pad: its guest stays in the game, padless,
    /// until another free pad presses Start.
    pub(super) fn pad_removed(&mut self, id: u32) {
        for g in self.guests.iter_mut().filter(|g| g.pad == Some(id)) {
            g.pad = None;
            g.input.buttons = Default::default();
        }
        if self.p1_pad == Some(id) {
            self.p1_pad = None;
        }
    }

    /// Run a guest's pad event against that guest's own input. Its binds
    /// execute at once so `+forward` and `impulse` land on the guest. Menu
    /// and console navigation stays shared: presses go to player 1's path
    /// while either is open (releases still reach the guest).
    fn guest_event(&mut self, k: usize, ie: InputEvent) {
        let pressed = matches!(ie, InputEvent::GamepadButton { down: true, .. });
        if self.key_dest() != KeyDest::Game && (pressed || matches!(ie, InputEvent::GamepadAxis { .. })) {
            self.handle_event(HostEvent::Input(ie));
            return;
        }
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

    /// Each frame: turn every guest's input into its next move.
    pub(super) fn guest_moves(&mut self, ft: f32) {
        let in_game = self.key_dest() == KeyDest::Game;
        let sp = self.move_speeds();
        let run = self.cv("cl_alwaysrun") != 0.0;
        let cvars = &self.server.core.cvars;
        for g in &mut self.guests {
            if g.cl.signon != 4 {
                continue;
            }
            let mut inp = g.input.frame(cvars, g.cl.viewangles, ft, None);
            if !in_game {
                inp = InputState::default();
            }
            if run {
                inp.speed = !inp.speed;
            }
            let buttons = g.cmd.buttons | inp.attack as u8 | (inp.jump as u8) << 1;
            let impulse = if inp.impulse != 0 { inp.impulse } else { g.cmd.impulse };
            g.cmd = g.cl.base_move(&InputState { attack: false, jump: false, impulse: 0, ..inp }, &sp, ft);
            g.cmd.buttons = buttons;
            g.cmd.impulse = impulse;
            // Intermission: any button moves on, as for player 1.
            if g.cl.intermission != 0 && g.input.buttons.iter().any(|b| b.is_down()) {
                g.cmd.buttons |= 1;
            }
        }
    }
}
