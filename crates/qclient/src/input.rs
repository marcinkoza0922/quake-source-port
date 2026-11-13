//! Usercmd building and clc_move encoding (cl_input.c).

use qnet::SizeBuf;
use qnet::protocol::*;

use crate::state::ClientState;

/// usercmd_t.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct UserCmd {
    pub viewangles: [f32; 3],
    pub forwardmove: f32,
    pub sidemove: f32,
    pub upmove: f32,
    /// bit 0 attack, bit 1 jump.
    pub buttons: u8,
    pub impulse: u8,
}

/// Key/axis state for one frame. Key fields are CL_KeyState-style
/// fractions (0 = up, 1 = held all frame).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct InputState {
    pub forward: f32,
    pub back: f32,
    pub moveleft: f32,
    pub moveright: f32,
    pub moveup: f32,
    pub movedown: f32,
    pub left: f32,
    pub right: f32,
    pub lookup: f32,
    pub lookdown: f32,
    pub speed: bool,
    pub strafe: bool,
    pub attack: bool,
    pub jump: bool,
    /// Add-on secondary fire (`+attack2`), held.
    pub attack2: bool,
    /// `+toy` was pressed since the last frame (toggles the toy slot).
    pub toy: bool,
    pub impulse: u8,
    /// Mouse/gamepad look delta in degrees, applied before clamping.
    pub look_delta: [f32; 2],
}

/// cl_*speed cvars.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MoveSpeeds {
    pub forward: f32,
    pub back: f32,
    pub side: f32,
    pub up: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub movespeedkey: f32,
    pub anglespeedkey: f32,
    pub maxpitch: f32,
    pub minpitch: f32,
}

impl Default for MoveSpeeds {
    fn default() -> Self {
        // ironwail defaults (cl_alwaysrun-style forward speed 200).
        Self {
            forward: 200.0,
            back: 200.0,
            side: 350.0,
            up: 200.0,
            yaw: 140.0,
            pitch: 150.0,
            movespeedkey: 2.0,
            anglespeedkey: 1.5,
            maxpitch: 90.0,
            minpitch: -90.0,
        }
    }
}

impl ClientState {
    /// CL_AdjustAngles + CL_BaseMove: update `viewangles` and return the command.
    pub fn base_move(&mut self, inp: &InputState, sp: &MoveSpeeds, frametime: f32) -> UserCmd {
        let speed = if inp.speed { frametime * sp.anglespeedkey } else { frametime };
        let va = &mut self.viewangles;
        if !inp.strafe {
            va[1] -= speed * sp.yaw * inp.right;
            va[1] += speed * sp.yaw * inp.left;
            va[1] = qcore::math::angle_mod(va[1]);
        }
        va[0] -= speed * sp.pitch * inp.lookup;
        va[0] += speed * sp.pitch * inp.lookdown;
        va[1] += inp.look_delta[0];
        va[0] += inp.look_delta[1];
        va[0] = va[0].clamp(sp.minpitch, sp.maxpitch);
        va[2] = va[2].clamp(-50.0, 50.0);

        let mut cmd = UserCmd { viewangles: *va, ..Default::default() };
        if inp.strafe {
            cmd.sidemove += sp.side * (inp.right - inp.left);
        }
        cmd.sidemove += sp.side * (inp.moveright - inp.moveleft);
        cmd.upmove += sp.up * (inp.moveup - inp.movedown);
        cmd.forwardmove += sp.forward * inp.forward - sp.back * inp.back;
        if inp.speed {
            cmd.forwardmove *= sp.movespeedkey;
            cmd.sidemove *= sp.movespeedkey;
            cmd.upmove *= sp.movespeedkey;
        }
        cmd.buttons = inp.attack as u8 | (inp.jump as u8) << 1;
        cmd.impulse = inp.impulse;
        cmd
    }

    /// Encode a clc_move (CL_SendMove). Uses `cmd.viewangles`.
    pub fn encode_move(&self, cmd: &UserCmd) -> Vec<u8> {
        let mut b = SizeBuf::new(128);
        b.byte(CLC_MOVE as i32);
        b.float(self.mtime[0] as f32);
        for i in 0..3 {
            if self.protocol == PROTOCOL_NETQUAKE {
                b.angle(cmd.viewangles[i], self.protocolflags);
            } else {
                b.angle16(cmd.viewangles[i], self.protocolflags);
            }
        }
        b.short(cmd.forwardmove as i32);
        b.short(cmd.sidemove as i32);
        b.short(cmd.upmove as i32);
        b.byte(cmd.buttons as i32);
        b.byte(cmd.impulse as i32);
        b.data
    }

    /// CL_SendMove: the unreliable clc_move to send, or `None` for the first
    /// two moves of a level (they may carry stale input) or before signon.
    pub fn build_move(&mut self, cmd: &UserCmd) -> Option<Vec<u8>> {
        if self.signon < SIGNONS {
            return None;
        }
        let mut m = self.encode_move(cmd);
        self.movemessages += 1;
        if self.movemessages <= 2 {
            return None;
        }
        // Replacement-delta acks ride along (CL_SendMove).
        for a in self.ack_frames.drain(..) {
            m.push(crate::delta::CLCDP_ACKFRAME);
            m.extend_from_slice(&a.to_le_bytes());
        }
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnet::MsgReader;

    #[test]
    fn move_encoding() {
        let mut cl = ClientState::new();
        cl.protocol = PROTOCOL_FITZQUAKE;
        cl.signon = SIGNONS;
        let inp = InputState { forward: 1.0, moveright: 1.0, left: 1.0, attack: true, impulse: 7, ..Default::default() };
        let cmd = cl.base_move(&inp, &MoveSpeeds::default(), 0.1);
        assert_eq!(cmd.forwardmove, 200.0);
        assert_eq!(cmd.sidemove, 350.0);
        assert!((cmd.viewangles[1] - 14.0).abs() < 0.01);
        assert!(cl.build_move(&cmd).is_none() && cl.build_move(&cmd).is_none());
        let m = cl.build_move(&cmd).unwrap();
        let mut r = MsgReader::new(&m);
        assert_eq!(r.byte(), CLC_MOVE as i32);
        r.float();
        r.angle16(0);
        assert!((r.angle16(0) - 14.0).abs() < 0.01);
        r.angle16(0);
        assert_eq!((r.short(), r.short(), r.short(), r.byte(), r.byte()), (200, 350, 0, 1, 7));
        assert!(r.at_end());
    }
}
