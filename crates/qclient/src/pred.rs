//! Client-side movement prediction for NetQuake with `PEXT2_PREDINFO`.
//!
//! When a remote NetQuake server negotiates `PEXT2_PREDINFO`, every outgoing
//! `clc_move` carries a 16-bit sequence number and every incoming
//! `svcfte_updateentities` echoes the latest acknowledged sequence alongside
//! the viewentity's server velocity, `pmovetype` and `EFLAGS_ONGROUND`.
//! [`ClientState::predict_move`] replays the unacknowledged moves from the
//! journal (`movecmds`) through [`qworld::pmove::predict`] against the local
//! [`CollisionModel`].

use qnet::delta::{EFLAGS_ONGROUND, PEXT2_PREDINFO};
use qnet::protocol::{SIGNONS, STAT_HEALTH};
use qworld::Vec3;
use qworld::hull::CollisionModel;
use qworld::pmove::{self, MoveCmd, MoveVars, PhysEnt, PhysShape, PlayerState};

use crate::input::UserCmd;
use crate::state::ClientState;

pub const MOVECMDS_SIZE: usize = 64;
pub const MOVECMDS_MASK: u32 = (MOVECMDS_SIZE - 1) as u32;
const RD_MOVETYPE_LOCK: u8 = 15;

/// One journaled outgoing client move for `PEXT2_PREDINFO` replay.
#[derive(Clone, Copy, Debug, Default)]
pub struct PredMove {
    pub cmd: UserCmd,
    pub msec: u8,
}

impl ClientState {
    /// Predicts the local player's position and velocity from the latest
    /// `PEXT2_PREDINFO` server snapshot and unacknowledged moves in `movecmds`
    /// (plus an optional partial-frame `pending` command).
    ///
    /// Returns `true` when prediction updated the viewentity origin.
    pub fn predict_move(
        &mut self,
        world: &CollisionModel,
        nopred: bool,
        pending: Option<(UserCmd, u8)>,
    ) -> bool {
        if self.pext2 & PEXT2_PREDINFO == 0
            || self.demoplayback
            || self.paused
            || self.intermission != 0
            || self.signon < SIGNONS
        {
            self.predicted = false;
            return false;
        }
        if nopred {
            self.predicted = false;
            return false;
        }
        let Some(ve) = self.entities.get(self.viewentity) else {
            self.predicted = false;
            return false;
        };
        if !ve.delta_live {
            self.predicted = false;
            return false;
        }
        let pmt_raw = ve.netstate.pmovetype;
        let pmt = pmt_raw & 0x3f;
        // Do not predict locked or push movetypes.
        if pmt == RD_MOVETYPE_LOCK || pmt == 7 {
            self.predicted = false;
            return false;
        }
        let start_seq = self.acked_movemessages.wrapping_add(1).max(2);
        let unacked = if self.movemessages >= start_seq {
            self.movemessages.wrapping_sub(start_seq) & 0xffff
        } else {
            0
        };
        if unacked >= MOVECMDS_SIZE as u32 {
            self.predicted = false;
            return false;
        }

        let base_origin = ve.netstate.origin;
        let base_vel = ve.netstate.velocity.map(|v| v as f32 * (1.0 / 8.0));
        let base_onground = (ve.netstate.eflags & EFLAGS_ONGROUND != 0) || (pmt_raw & 0x80 != 0);
        // Bit 0x40 in pmovetype signals jump is still held (!FL_JUMPRELEASED).
        let oldbuttons: u8 = if pmt_raw & 0x40 != 0 {
            2
        } else if self.acked_movemessages >= 2 {
            self.movecmds[(self.acked_movemessages & MOVECMDS_MASK) as usize].cmd.buttons as u8 & 2
        } else {
            0
        };

        let ents = self.build_phys_ents(world);
        let mv = MoveVars { gravity: self.sv_gravity, ..MoveVars::default() };
        let dead = self.stats[STAT_HEALTH] <= 0;
        let noclip = pmt == 8;

        let mut ps = PlayerState {
            origin: Vec3::from_array(base_origin),
            velocity: Vec3::from_array(base_vel),
            onground: base_onground,
            waterjumptime: 0.0,
            oldbuttons,
        };

        for i in 0..unacked {
            let seq = start_seq.wrapping_add(i);
            let pm = self.movecmds[(seq & MOVECMDS_MASK) as usize];
            if pm.msec == 0 {
                continue;
            }
            let mc = to_move_cmd(&pm.cmd, pm.msec, self.viewangles);
            let r = pmove::predict(&ents, &mv, &ps, &mc, dead, noclip);
            ps = PlayerState {
                origin: r.origin,
                velocity: r.velocity,
                oldbuttons: r.oldbuttons,
                waterjumptime: r.waterjumptime,
                onground: r.onground,
            };
        }
        if let Some((cmd, msec)) = pending.filter(|(_, ms)| *ms > 0) {
            let mc = to_move_cmd(&cmd, msec, self.viewangles);
            let r = pmove::predict(&ents, &mv, &ps, &mc, dead, noclip);
            ps = PlayerState {
                origin: r.origin,
                velocity: r.velocity,
                oldbuttons: r.oldbuttons,
                waterjumptime: r.waterjumptime,
                onground: r.onground,
            };
        }

        let org = ps.origin.to_array();
        let vel = ps.velocity.to_array();
        self.simorg = org;
        self.simvel = vel;
        self.velocity = vel;
        self.onground = ps.onground;
        self.predicted = true;
        if let Some(ent) = self.entities.get_mut(self.viewentity) {
            ent.origin = org;
        }
        true
    }

    fn build_phys_ents<'a>(&self, world: &'a CollisionModel) -> Vec<PhysEnt<'a>> {
        let mut ents = Vec::with_capacity(16);
        ents.push(PhysEnt { origin: Vec3::ZERO, shape: PhysShape::Model(world, 0) });
        let cur_time = self.mtime[0];
        let max_cl = self.maxclients.min(self.entities.len().saturating_sub(1));
        for (num, e) in self.entities.iter().enumerate().skip(1) {
            if num == self.viewentity || e.model == 0 || e.msgtime != cur_time {
                continue;
            }
            if num <= max_cl {
                ents.push(PhysEnt {
                    origin: Vec3::from_array(e.origin),
                    shape: PhysShape::Box { mins: pmove::PLAYER_MINS, maxs: pmove::PLAYER_MAXS },
                });
                continue;
            }
            let Some(name) = self.model_precache.get(e.model) else {
                continue;
            };
            if let Some(sub) = name.strip_prefix('*').and_then(|s| s.parse::<usize>().ok())
                && world.submodels.get(sub).is_some_and(|s| s.headnode[1] != 0)
            {
                ents.push(PhysEnt {
                    origin: Vec3::from_array(e.origin),
                    shape: PhysShape::Model(world, sub),
                });
            }
        }
        ents
    }
}

fn to_move_cmd(cmd: &UserCmd, msec: u8, fallback_angles: [f32; 3]) -> MoveCmd {
    let angles = if cmd.viewangles == [0.0; 3] { fallback_angles } else { cmd.viewangles };
    MoveCmd {
        msec,
        angles,
        forwardmove: cmd.forwardmove,
        sidemove: cmd.sidemove,
        upmove: cmd.upmove,
        buttons: cmd.buttons as u8,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnet::delta::{PEXT2_REPLACEMENTDELTAS, PROTOCOL_FTE_PEXT2};
    use qworld::hull::{CNode, HPlane, SubModel};
    use qworld::{CONTENTS_EMPTY, CONTENTS_SOLID};

    fn flat_floor_world() -> CollisionModel {
        let plane = HPlane { normal: Vec3::new(0.0, 0.0, 1.0), dist: -200.0, kind: 2 };
        let node = CNode { plane: 0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] };
        CollisionModel {
            planes: vec![plane],
            hull0: vec![node],
            clipnodes: vec![node],
            node_children: Vec::new(),
            leaf_contents: Vec::new(),
            submodels: vec![SubModel {
                mins: Vec3::new(-4096.0, -4096.0, -4096.0),
                maxs: Vec3::new(4096.0, 4096.0, 4096.0),
                origin: Vec3::ZERO,
                headnode: [0; 4],
            }],
        }
    }

    fn setup_pred_client() -> ClientState {
        let mut cl = ClientState::new();
        cl.signon = SIGNONS;
        cl.pext2 = PEXT2_REPLACEMENTDELTAS | PEXT2_PREDINFO;
        cl.viewentity = 1;
        cl.maxclients = 4;
        cl.stats[STAT_HEALTH] = 100;
        cl.entities.resize_with(2, Default::default);
        cl.entities[1].delta_live = true;
        cl.entities[1].netstate.origin = [0.0, 0.0, -175.0];
        cl.entities[1].netstate.velocity = [0, 0, 0];
        cl.entities[1].netstate.pmovetype = 3 | 0x80; // MOVETYPE_WALK | onground
        cl.entities[1].netstate.eflags = EFLAGS_ONGROUND;
        cl.entities[1].origin = cl.entities[1].netstate.origin;
        let _ = PROTOCOL_FTE_PEXT2;
        cl
    }

    #[test]
    fn predict_move_advances_unacked_moves_and_converges_on_ack() {
        let world = flat_floor_world();
        let mut cl = setup_pred_client();
        let cmd = UserCmd { forwardmove: 320.0, ..Default::default() };

        // First two build_move calls are suppressed by CL_SendMove; next 4 are sent (seq 2..=5).
        assert!(cl.build_move_dt(&cmd, 1.0 / 72.0).is_none());
        assert!(cl.build_move_dt(&cmd, 1.0 / 72.0).is_none());
        for _ in 0..4 {
            assert!(cl.build_move_dt(&cmd, 1.0 / 72.0).is_some());
        }
        assert_eq!(cl.movemessages, 6);
        assert_eq!(cl.acked_movemessages, 0);

        assert!(cl.predict_move(&world, false, None));
        assert!(cl.predicted);
        let pred_x = cl.entities[1].origin[0];
        assert!(pred_x > 1.0, "predicted origin should move forward, got {pred_x}");

        // Once the server acknowledges move seq 5 at the predicted spot, replay is 0 steps.
        cl.acked_movemessages = 5;
        cl.entities[1].netstate.origin = cl.simorg;
        cl.entities[1].netstate.velocity = cl.simvel.map(|v| (v * 8.0) as i16);
        assert!(cl.predict_move(&world, false, None));
        assert!((cl.entities[1].origin[0] - pred_x).abs() < 1e-4);
    }

    #[test]
    fn nopred_or_missing_pext2_skips_prediction() {
        let world = flat_floor_world();
        let mut cl = setup_pred_client();
        let cmd = UserCmd { forwardmove: 320.0, ..Default::default() };
        for _ in 0..3 {
            cl.build_move_dt(&cmd, 1.0 / 72.0);
        }

        // cl_nopred = 1 disables prediction.
        assert!(!cl.predict_move(&world, true, None));
        assert!(!cl.predicted);
        assert_eq!(cl.entities[1].origin[0], 0.0);

        // Without PEXT2_PREDINFO, prediction is inactive.
        cl.pext2 = PEXT2_REPLACEMENTDELTAS;
        assert!(!cl.predict_move(&world, false, None));
        assert!(!cl.predicted);
    }
}
