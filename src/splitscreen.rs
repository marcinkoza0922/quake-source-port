//! Split-screen guests: extra local players on the local server.
//!
//! Player 1 is the host's own `ClientState`; each guest (players 2..6) has
//! its own client state on its own loopback slot. Guests are joined and
//! dropped to match `cl_splitscreen` (total local players, 1..6).

use qclient::{ClientEvent, ClientState, UserCmd};
use qserver::{ClientMove, Server};

pub const MAX_LOCAL_PLAYERS: usize = 6;

pub struct Guest {
    /// Server client slot.
    pub slot: usize,
    pub cl: ClientState,
    /// Input for the next server tic (set by whoever drives this player).
    pub cmd: UserCmd,
    /// View state (bob, kicks, damage flashes, entity lerps).
    pub view: crate::view::ViewState,
    /// Damage and bonus-flash events for `view`, applied by the host.
    pub view_events: Vec<ClientEvent>,
    /// This player's binds and button state (driven by `pad`).
    pub input: qinput::Input,
    /// SDL id of the gamepad that plays this guest; `None` until one joins.
    pub pad: Option<u32>,
    pub hud_fade: crate::sbar::HudFade,
    pub(crate) feedback: crate::host::feedback::Feedback,
    pub(crate) pending_feedback: Option<bool>,
    /// Sound events this frame; the host plays those player 1's client
    /// can't have heard.
    pub sounds: Vec<ClientEvent>,
    /// Center prints this frame, shown by the host in this guest's cell.
    pub center_prints: Vec<String>,
    /// This guest's center message (only its centerprint state is used).
    pub center: crate::console::Console,
    /// This guest's weapon wheel (opened with its own `+weaponwheel`).
    pub wheel: crate::wheel::Wheel,
}

impl Guest {
    /// Connect a guest on a free loopback slot. `n` is the player number (2..).
    pub fn connect(sv: &mut Server, n: usize) -> Option<Self> {
        let mut cl = ClientState::new();
        cl.name = format!("player{n}");
        // Distinct shirt/pants so local players tell each other apart.
        let c = ((n * 2) % 14) as i32;
        let slot = sv.connect_local_as(&cl.name, c * 16 + c)?;
        let input = qinput::Input::new(&mut sv.core.cvars);
        let mut g = Self {
            slot,
            cl,
            cmd: UserCmd::default(),
            view: Default::default(),
            view_events: Vec::new(),
            input,
            pad: None,
            hud_fade: Default::default(),
            feedback: Default::default(),
            pending_feedback: None,
            sounds: Vec::new(),
            center_prints: Vec::new(),
            center: crate::console::Console::new(),
            wheel: Default::default(),
        };
        g.feed(sv);
        Some(g)
    }

    /// The slot is still this guest's connection.
    pub fn active(&self, sv: &Server) -> bool {
        sv.core.clients.get(self.slot).is_some_and(|c| c.active && c.local)
    }

    /// Parse the slot's server messages into the guest's client state.
    pub fn feed(&mut self, sv: &mut Server) {
        let mut msgs = sv.take_reliable(self.slot);
        msgs.extend(sv.take_datagrams(self.slot));
        for m in msgs {
            self.cl.net_sequence = None;
            if self.cl.parse_server_message(&m).is_err() {
                self.cl = ClientState::new();
                return;
            }
        }
        // Prints and stufftext belong to player 1's host.
        for e in self.cl.take_events() {
            match e {
                ClientEvent::Disconnect => {
                    self.cl = ClientState::new();
                    self.feedback = Default::default();
                    self.pending_feedback = None;
                }
                ClientEvent::NewMap => {
                    self.feedback = Default::default();
                    self.pending_feedback = None;
                }
                ClientEvent::HitFeedback { kill } => {
                    self.pending_feedback = Some(self.pending_feedback.unwrap_or(false) || kill);
                }
                ClientEvent::Damage { .. } | ClientEvent::BonusFlash => self.view_events.push(e),
                ClientEvent::Sound { .. } | ClientEvent::TempSound { .. } => self.sounds.push(e),
                ClientEvent::CenterPrint(s) => self.center_prints.push(s),
                _ => {}
            }
        }
    }

    /// Send queued string commands and this tic's move to the server.
    pub fn send(&mut self, sv: &mut Server) {
        let out = self.cl.take_reliable_out();
        for s in crate::host::string_cmds(&out) {
            let low = s.trim_start().to_ascii_lowercase();
            // connect_local runs the signon itself.
            if !["prespawn", "spawn", "begin"].iter().any(|p| low.starts_with(p)) {
                sv.client_command(self.slot, s.trim_end());
            }
        }
        for a in self.cl.ack_frames.drain(..) {
            if a == -1 {
                sv.ack_frame(self.slot, -1);
            }
        }
        let cmd = self.cmd;
        if self.cl.build_move(&cmd).is_some() {
            sv.set_move(self.slot, ClientMove {
                angles: cmd.viewangles,
                forward: cmd.forwardmove,
                side: cmd.sidemove,
                up: cmd.upmove,
                buttons: cmd.buttons,
                impulse: cmd.impulse,
            });
            self.cmd.buttons = 0;
            self.cmd.impulse = 0;
        }
    }
}

/// Join or drop guests so there are `want - 1` of them (player 1 is the
/// host's own client). Reconnects guests whose slot went away (a new
/// server or `maxplayers`). Returns a warning when slots run out.
pub fn sync(guests: &mut Vec<Guest>, sv: &mut Server, want: usize) -> Option<String> {
    guests.retain(|g| g.active(sv));
    let want = want.clamp(1, MAX_LOCAL_PLAYERS) - 1;
    while guests.len() > want {
        let g = guests.pop().unwrap();
        sv.drop_client(g.slot, false);
    }
    while guests.len() < want {
        let Some(g) = Guest::connect(sv, guests.len() + 2) else {
            return Some(format!(
                "splitscreen: maxplayers {} leaves room for {} local players\n",
                sv.core.maxclients(),
                guests.len() + 1
            ));
        };
        guests.push(g);
    }
    None
}

/// Output rectangles (x, y, w, h) for `n` local players on a `w`×`h`
/// screen: 2 split top/bottom (or side by side), 3–4 a 2×2 grid, 5–6 a 3×2
/// grid; filled left to right, top to bottom.
pub fn layout(n: usize, w: u32, h: u32, side_by_side: bool) -> Vec<[u32; 4]> {
    let (cols, rows) = match n {
        0 | 1 => (1, 1),
        2 if side_by_side => (2, 1),
        2 => (1, 2),
        3 | 4 => (2, 2),
        _ => (3, 2),
    };
    // Edges from rounded fractions so cells tile the screen exactly.
    let edge = |i: u32, n: u32, len: u32| (len as u64 * i as u64 / n as u64) as u32;
    (0..n.min(MAX_LOCAL_PLAYERS) as u32)
        .map(|k| {
            let (c, r) = (k % cols, k / cols);
            let (x0, x1) = (edge(c, cols, w), edge(c + 1, cols, w));
            let (y0, y1) = (edge(r, rows, h), edge(r + 1, rows, h));
            [x0, y0, x1 - x0, y1 - y0]
        })
        .collect()
}

#[cfg(test)]
mod tests;
