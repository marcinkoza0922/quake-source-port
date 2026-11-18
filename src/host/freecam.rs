//! `freecam [0|1]`: a client-side camera that flies freely (through walls)
//! while the player stands still and the game keeps running. Player 1 only,
//! in a demo or a local game no remote player has joined. The local server
//! also sends the entities near the camera (`Client::extra_view`), so
//! monsters in rooms the player can't see still show up.
use super::*;
use qcore::math::{Vec3, angle_vectors};

#[derive(Default)]
pub(super) struct FreeCam {
    /// Origin and angles while flying.
    pub cam: Option<([f32; 3], [f32; 3])>,
    /// The map the camera was started on; a level change ends it.
    map: String,
    /// Player 1's last placed view, where the camera starts.
    pub last_view: ([f32; 3], [f32; 3]),
}

pub(super) fn register(c: &mut Commands<Host>) {
    // `freecam` toggles, `freecam 0|1` sets, `freecam x y z [pitch yaw]`
    // turns it on at that spot (scripted screenshots).
    c.add("freecam", |h, a| {
        let f = |i: usize| qcore::cvar::q_atof(a.argv(i));
        match a.argc() {
            1 => h.set_freecam(h.freecam.cam.is_none()),
            2 => h.set_freecam(f(1) != 0.0),
            4 | 6 => {
                h.set_freecam(true);
                if let Some((o, ang)) = &mut h.freecam.cam {
                    *o = [f(1), f(2), f(3)];
                    if a.argc() == 6 {
                        *ang = [f(4), f(5), 0.0];
                    }
                }
            }
            _ => h.print("usage: freecam [0|1] or freecam <x> <y> <z> [pitch yaw]\n"),
        }
    });
}

impl Host {
    fn freecam_allowed(&self) -> Result<(), &'static str> {
        if self.cl.signon != 4 || self.level.mapname.is_empty() {
            return Err("not in a game");
        }
        if !self.guests.is_empty() {
            return Err("not available in splitscreen");
        }
        if self.demo.is_some() {
            return Ok(());
        }
        if self.remote() || self.local.is_none() {
            return Err("only in a local game or a demo");
        }
        if self.server.core.clients.iter().any(|c| c.active && !c.local) {
            return Err("not while other players are connected");
        }
        Ok(())
    }

    pub(super) fn set_freecam(&mut self, on: bool) {
        if !on {
            if self.freecam.cam.take().is_some() {
                self.print("freecam OFF\n");
            }
            return;
        }
        if let Err(why) = self.freecam_allowed() {
            self.print(&format!("freecam: {why}\n"));
            return;
        }
        let (origin, mut angles) = self.freecam.last_view;
        angles[2] = 0.0;
        self.freecam.cam = Some((origin, angles));
        self.freecam.map = self.level.mapname.clone();
        self.print("freecam ON\n");
    }

    /// Flies the camera with this frame's input, then takes the input away
    /// from the player. Ends the camera when it is no longer allowed.
    pub(super) fn freecam_frame(&mut self, inp: &mut InputState, ft: f32) {
        if self.freecam.cam.is_some() && (self.freecam_allowed().is_err() || self.level.mapname != self.freecam.map) {
            self.set_freecam(false);
        }
        let extra = self.freecam.cam.map(|(o, _)| o);
        if let Some(c) = self.local.and_then(|i| self.server.core.clients.get_mut(i)) {
            c.extra_view = extra;
        }
        let sp = self.move_speeds();
        let Some((origin, angles)) = &mut self.freecam.cam else { return };
        fly(origin, angles, inp, &sp, ft);
        *inp = InputState::default();
    }
}

/// Noclip-style flight: look like CL_AdjustAngles, move along the view with
/// the cl_*speed speeds; jump/moveup rise and movedown sinks straight up/down.
fn fly(origin: &mut [f32; 3], angles: &mut [f32; 3], inp: &InputState, sp: &MoveSpeeds, ft: f32) {
    let turn = if inp.speed { ft * sp.anglespeedkey } else { ft };
    if !inp.strafe {
        angles[1] += turn * sp.yaw * (inp.left - inp.right);
    }
    angles[0] += turn * sp.pitch * (inp.lookdown - inp.lookup);
    angles[1] = qcore::math::angle_mod(angles[1] + inp.look_delta[0]);
    angles[0] = (angles[0] + inp.look_delta[1]).clamp(-89.0, 89.0);

    let mut side = sp.side * (inp.moveright - inp.moveleft);
    if inp.strafe {
        side += sp.side * (inp.right - inp.left);
    }
    let fwd_speed = sp.forward * inp.forward - sp.back * inp.back;
    let up = sp.up * (inp.moveup - inp.movedown + f32::from(u8::from(inp.jump)));
    let run = if inp.speed { sp.movespeedkey } else { 1.0 };
    let (fwd, right, _) = angle_vectors(Vec3::from_array(*angles));
    let step = (fwd * fwd_speed + right * side + Vec3::new(0.0, 0.0, up)) * run * ft;
    *origin = (Vec3::from_array(*origin) + step).to_array();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flies_along_the_view_and_turns() {
        let sp = MoveSpeeds::default();
        let (mut o, mut a) = ([0.0; 3], [0.0, 90.0, 0.0]);
        fly(&mut o, &mut a, &InputState { forward: 1.0, ..Default::default() }, &sp, 0.5);
        assert!(o[0].abs() < 0.01 && (o[1] - 100.0).abs() < 0.01, "{o:?}");
        fly(&mut o, &mut a, &InputState { jump: true, speed: true, ..Default::default() }, &sp, 0.5);
        assert!((o[2] - 200.0).abs() < 0.01, "{o:?}");
        fly(&mut o, &mut a, &InputState { look_delta: [10.0, 200.0], ..Default::default() }, &sp, 0.0);
        assert!(a[0] == 89.0 && (a[1] - 100.0).abs() < 0.01, "{a:?}");
    }
}
