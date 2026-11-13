//! Split-screen guests: extra local players on the local server.
//!
//! Player 1 is the host's own `ClientState`; each guest (players 2..6) has
//! its own client state on its own loopback slot. Guests are joined and
//! dropped to match `cl_splitscreen` (total local players, 1..6).

use qclient::{ClientEvent, ClientState, UserCmd};
use qnet::protocol::CLC_STRINGCMD;
use qserver::{ClientMove, Server};

pub const MAX_LOCAL_PLAYERS: usize = 6;

pub struct Guest {
    /// Server client slot.
    pub slot: usize,
    pub cl: ClientState,
    /// Input for the next server tic (set by whoever drives this player).
    pub cmd: UserCmd,
}

impl Guest {
    /// Connect a guest on a free loopback slot. `n` is the player number (2..).
    pub fn connect(sv: &mut Server, n: usize) -> Option<Self> {
        let mut cl = ClientState::new();
        cl.name = format!("player{n}");
        // Distinct shirt/pants so local players tell each other apart.
        let c = ((n * 2) % 14) as i32;
        let slot = sv.connect_local_as(&cl.name, c * 16 + c)?;
        let mut g = Self { slot, cl, cmd: UserCmd::default() };
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
        // Prints, sounds and stufftext belong to player 1's host for now;
        // per-player HUD and messages come with the split views.
        for e in self.cl.take_events() {
            if let ClientEvent::Disconnect = e {
                self.cl = ClientState::new();
            }
        }
    }

    /// Send queued string commands and this tic's move to the server.
    pub fn send(&mut self, sv: &mut Server) {
        let out = self.cl.take_reliable_out();
        for s in string_cmds(&out) {
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

fn string_cmds(out: &[u8]) -> Vec<String> {
    let mut v = Vec::new();
    let mut p = 0;
    while p < out.len() && out[p] as i32 == CLC_STRINGCMD as i32 {
        let end = out[p + 1..].iter().position(|&b| b == 0).map_or(out.len(), |e| p + 1 + e);
        v.push(String::from_utf8_lossy(&out[p + 1..end]).into_owned());
        p = end + 1;
    }
    v
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

#[cfg(test)]
mod tests;
