use super::*;

impl QwState {
    /// CL_SendCmd: store `cmd` as the command for the next outgoing
    /// sequence and build the clc_move (+ clc_delta) payload. The caller
    /// passes the payload to the netchan, whose outgoing_sequence must
    /// equal `self.outgoing_sequence` before the call.
    pub fn build_cmd(&mut self, cmd: UserCmd, lost: u8, realtime: f64) -> Vec<u8> {
        let seq = self.outgoing_sequence;
        let i = Self::frame_index(seq);
        self.frames[i].cmd = cmd;
        self.frames[i].senttime = realtime;
        self.frames[i].receivedtime = -1.0;
        let mut b = SizeBuf::new(128);
        b.byte(CLC_MOVE as i32);
        let ck = b.len();
        b.byte(0);
        b.byte(lost as i32);
        let c2 = self.frames[Self::frame_index(seq.wrapping_sub(2))].cmd;
        let c1 = self.frames[Self::frame_index(seq.wrapping_sub(1))].cmd;
        write_delta_usercmd(&mut b, &UserCmd::default(), &c2);
        write_delta_usercmd(&mut b, &c2, &c1);
        write_delta_usercmd(&mut b, &c1, &cmd);
        b.data[ck] = sequence_crc_byte(&b.data[ck + 1..], seq as i32);
        if seq.wrapping_sub(self.validsequence) >= UPDATE_BACKUP as u32 - 1 {
            self.validsequence = 0;
        }
        if self.validsequence != 0 && self.stage == Stage::Active {
            self.frames[i].delta_sequence = self.validsequence as i32;
            b.byte(CLC_DELTA as i32);
            b.byte((self.validsequence & 255) as i32);
        } else {
            self.frames[i].delta_sequence = -1;
        }
        self.outgoing_sequence = seq.wrapping_add(1);
        b.data
    }

    /// Physents for prediction: the world, inline brush models in the
    /// current frame, and (except `skip`) solid players.
    pub fn physents<'a>(&self, world: &'a qworld::CollisionModel, skip: Option<usize>) -> Vec<PhysEnt<'a>> {
        let mut v = vec![PhysEnt { origin: Vec3::ZERO, shape: PhysShape::Model(world, 0) }];
        let fi = Self::frame_index(self.parsecount);
        for e in &self.frames[fi].entities {
            let Some(name) = self.model_names.get(e.modelindex) else { continue };
            let Some(sub) = name.strip_prefix('*').and_then(|n| n.parse::<usize>().ok()) else { continue };
            if world.submodels.get(sub).is_some_and(|s| s.headnode[1] != 0) {
                v.push(PhysEnt { origin: Vec3::from_array(e.origin), shape: PhysShape::Model(world, sub) });
            }
        }
        for (j, st) in self.frames[fi].playerstate.iter().enumerate() {
            if Some(j) == skip || j == self.playernum || st.messagenum != self.parsecount || !st.present || st.modelindex == 0 {
                continue;
            }
            if st.flags & PF_DEAD != 0 {
                continue;
            }
            v.push(PhysEnt {
                origin: Vec3::from_array(st.origin),
                shape: PhysShape::Box { mins: pmove::PLAYER_MINS, maxs: pmove::PLAYER_MAXS },
            });
        }
        v
    }

    /// CL_PredictMove: run the commands the server hasn't acknowledged yet
    /// from the last received local player state; sets `simorg`/`simvel`.
    pub fn predict(&mut self, world: &qworld::CollisionModel, realtime: f64, nopred: bool) {
        self.predicted = false;
        if self.paused || self.intermission.is_some() || self.validsequence == 0 || self.stage != Stage::Active {
            return;
        }
        let ack = self.parsecount;
        if self.outgoing_sequence.wrapping_sub(ack) >= UPDATE_BACKUP as u32 - 1 {
            return;
        }
        let me = self.playernum;
        let base = self.frames[Self::frame_index(ack)].playerstate[me];
        if nopred {
            self.simorg = base.origin;
            self.simvel = base.velocity;
            self.predicted = true;
            return;
        }
        let ents = self.physents(world, None);
        let time = (realtime - self.latency).min(realtime);
        let dead = self.stats[nq::STAT_HEALTH] <= 0;
        let mut from = base;
        let mut fromtime = self.frames[Self::frame_index(ack)].senttime;
        let mut to = base;
        let mut totime = fromtime;
        let mut i = 1u32;
        while i < UPDATE_BACKUP as u32 - 1 && ack.wrapping_add(i) != self.outgoing_sequence {
            let f = &self.frames[Self::frame_index(ack.wrapping_add(i))];
            let c = f.cmd;
            let mc = MoveCmd {
                msec: c.msec,
                angles: c.angles,
                forwardmove: c.forwardmove as f32,
                sidemove: c.sidemove as f32,
                upmove: c.upmove as f32,
                buttons: c.buttons,
            };
            let ps = PlayerState {
                origin: Vec3::from_array(from.origin),
                velocity: Vec3::from_array(from.velocity),
                oldbuttons: from.oldbuttons,
                waterjumptime: from.waterjumptime,
                onground: from.onground,
            };
            let r = pmove::predict(&ents, &self.movevars, &ps, &mc, dead, self.spectator);
            to = QwPlayerState {
                origin: r.origin.to_array(),
                velocity: r.velocity.to_array(),
                oldbuttons: r.oldbuttons,
                waterjumptime: r.waterjumptime,
                onground: r.onground,
                ..from
            };
            totime = f.senttime;
            if totime >= time {
                break;
            }
            from = to;
            fromtime = totime;
            i += 1;
        }
        let f = if totime == fromtime { 0.0 } else { ((time - fromtime) / (totime - fromtime)).clamp(0.0, 1.0) as f32 };
        let teleported = (0..3).any(|k| (from.origin[k] - to.origin[k]).abs() > 128.0);
        for k in 0..3 {
            if teleported {
                self.simorg[k] = to.origin[k];
                self.simvel[k] = to.velocity[k];
            } else {
                self.simorg[k] = from.origin[k] + f * (to.origin[k] - from.origin[k]);
                self.simvel[k] = from.velocity[k] + f * (to.velocity[k] - from.velocity[k]);
            }
        }
        self.sim_onground = to.onground;
        self.predicted = true;
    }

    /// Ping of player slot `i` (svc_updateping).
    pub fn ping(&self, i: usize) -> i32 {
        self.players.get(i).map_or(0, |p| p.ping)
    }

    /// Prepare for `reconnect` (a new map): stop delta compression and
    /// start the signon over.
    pub fn reconnect(&mut self) {
        self.validsequence = 0;
        self.stage = Stage::Connected;
        self.nq_signon = 0;
        self.nq_ready = false;
    }
}
