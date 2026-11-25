//! QuakeWorld `.qwd` recording and `.mvd` multi-POV helpers on [`QwState`].

use super::*;

impl QwState {
    /// Active non-spectator players `(slot 0..31, name)` for MVD POV selection.
    pub fn active_players(&self) -> Vec<(usize, String)> {
        (0..MAX_CLIENTS)
            .filter_map(|i| {
                let p = &self.players[i];
                let active = !p.spectator && (!p.name.is_empty() || self.mvd_players[i].present);
                if active {
                    let name = if p.name.is_empty() { format!("Player {}", i + 1) } else { p.name.clone() };
                    Some((i, name))
                } else {
                    None
                }
            })
            .collect()
    }

    /// View angles of the currently tracked POV player (or intermission camera).
    pub fn pov_angles(&self) -> [f32; 3] {
        if let Some((_, ang)) = self.intermission {
            return ang;
        }
        if self.mvd {
            return self.mvd_players[self.playernum].viewangles;
        }
        let fi = Self::frame_index(self.parsecount);
        self.frames[fi].playerstate[self.playernum].viewangles
    }

    /// In MVD mode, if the current `playernum` is not an active player and
    /// wasn't explicitly locked, picks the first active non-spectator slot.
    pub(super) fn auto_select_mvd_pov(&mut self, body: &mut Nq) {
        if !self.mvd || self.pov_locked {
            return;
        }
        let cur_ok = !self.players[self.playernum].spectator
            && (!self.players[self.playernum].name.is_empty() || self.mvd_players[self.playernum].present);
        if cur_ok {
            return;
        }
        if let Some((slot, _)) = self.active_players().first() {
            let slot = *slot;
            if slot != self.playernum {
                self.playernum = slot;
                self.stats = self.player_stats[slot];
                if self.nq_ready {
                    body.byte(nq::SVC_SETVIEW);
                    body.0.short(slot as i32 + 1);
                }
            }
        }
    }

    /// Encodes the initial `.qwd` gamestate header (`CL_Record_f` in `cl_demo.c:468-655`)
    /// so recording can begin mid-connection on an active QuakeWorld server.
    pub fn encode_qwd_header(&self, realtime: f32, incoming_sequence: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(8192);
        let mut seq = 1u32;

        // 1. serverdata, cdtrack, fullserverinfo
        let mut buf = SizeBuf::new(MAX_MSGLEN);
        buf.byte(SVC_SERVERDATA as i32);
        buf.long(PROTOCOL_VERSION);
        buf.long(self.servercount);
        buf.string(&qcore::text::wire(&self.gamedir));
        let pn = (self.playernum as u8 & 127) | ((self.spectator as u8) << 7);
        buf.byte(pn as i32);
        buf.string(&qcore::text::wire(&self.levelname));
        for f in [
            self.movevars.gravity,
            self.movevars.stopspeed,
            self.movevars.maxspeed,
            self.movevars.spectatormaxspeed,
            self.movevars.accelerate,
            self.movevars.airaccelerate,
            self.movevars.wateraccelerate,
            self.movevars.friction,
            self.movevars.waterfriction,
            self.movevars.entgravity,
        ] {
            buf.float(f);
        }
        buf.byte(SVC_CDTRACK as i32);
        buf.byte(self.cdtrack as i32);
        buf.byte(SVC_STUFFTEXT as i32);
        let fsi = format!("fullserverinfo \"{}\"\n", self.serverinfo);
        buf.string(&qcore::text::wire(&fsi));
        write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);

        // 2. soundlist chunks
        write_qwd_precache_list(&mut out, realtime, &mut seq, SVC_SOUNDLIST, &self.sound_names);
        // 3. modellist chunks
        write_qwd_precache_list(&mut out, realtime, &mut seq, SVC_MODELLIST, &self.model_names);

        // 4. static entities + baselines + "cmd spawn\n"
        buf.clear();
        for s in &self.static_entities {
            if buf.data.len() >= MAX_MSGLEN / 2 {
                write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);
                buf.clear();
            }
            buf.byte(SVC_SPAWNSTATIC as i32);
            write_baseline_fields(&mut buf, s);
        }
        for (i, b) in self.baselines.iter().enumerate() {
            if b.modelindex == 0 {
                continue;
            }
            if buf.data.len() >= MAX_MSGLEN / 2 {
                write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);
                buf.clear();
            }
            buf.byte(SVC_SPAWNBASELINE as i32);
            buf.short(i as i32);
            write_baseline_fields(&mut buf, b);
        }
        buf.byte(SVC_STUFFTEXT as i32);
        buf.string(b"cmd spawn\n");
        write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);

        // 5. player info (frags, ping, pl, entertime, userinfo)
        buf.clear();
        for (i, p) in self.players.iter().enumerate().take(MAX_CLIENTS) {
            if buf.data.len() >= MAX_MSGLEN / 2 {
                write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);
                buf.clear();
            }
            buf.byte(SVC_UPDATEFRAGS as i32);
            buf.byte(i as i32);
            buf.short(p.frags);
            buf.byte(SVC_UPDATEPING as i32);
            buf.byte(i as i32);
            buf.short(p.ping);
            buf.byte(SVC_UPDATEPL as i32);
            buf.byte(i as i32);
            buf.byte(p.pl);
            buf.byte(SVC_UPDATEENTERTIME as i32);
            buf.byte(i as i32);
            buf.float(0.0);
            buf.byte(SVC_UPDATEUSERINFO as i32);
            buf.byte(i as i32);
            buf.long(p.userid);
            buf.string(&qcore::text::wire(&p.userinfo));
        }
        if !buf.is_empty() {
            write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);
        }

        // 6. lightstyles
        buf.clear();
        for (i, ls) in self.lightstyles.iter().enumerate().take(nq::MAX_LIGHTSTYLES) {
            if buf.data.len() >= MAX_MSGLEN / 2 {
                write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);
                buf.clear();
            }
            buf.byte(SVC_LIGHTSTYLE as i32);
            buf.byte(i as i32);
            buf.string(&qcore::text::wire(ls));
        }
        if !buf.is_empty() {
            write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);
        }

        // 7. stats + "skins\n"
        buf.clear();
        for (i, &v) in self.stats.iter().enumerate().take(32) {
            buf.byte(SVC_UPDATESTATLONG as i32);
            buf.byte(i as i32);
            buf.long(v);
        }
        buf.byte(SVC_STUFFTEXT as i32);
        buf.string(b"skins\n");
        write_qwd_record_msg(&mut out, realtime, &mut seq, &buf.data);

        // 8. dem_set (outgoing_sequence, incoming_sequence)
        out.extend(realtime.to_le_bytes());
        out.push(DEM_SET);
        out.extend(self.outgoing_sequence.to_le_bytes());
        out.extend(incoming_sequence.to_le_bytes());
        out
    }
}

fn write_baseline_fields(buf: &mut SizeBuf, b: &QwEntity) {
    buf.byte(b.modelindex as i32);
    buf.byte(b.frame);
    buf.byte(b.colormap);
    buf.byte(b.skinnum);
    for j in 0..3 {
        write_coord(buf, b.origin[j]);
        write_angle(buf, b.angles[j]);
    }
}

fn write_qwd_precache_list(out: &mut Vec<u8>, time: f32, seq: &mut u32, svc: u8, names: &[String]) {
    let mut buf = SizeBuf::new(MAX_MSGLEN);
    buf.byte(svc as i32);
    buf.byte(0);
    let mut n = 0usize;
    for s in names.iter().skip(1) {
        if s.is_empty() {
            break;
        }
        buf.string(&qcore::text::wire(s));
        n += 1;
        if buf.data.len() > MAX_MSGLEN / 2 {
            buf.byte(0);
            buf.byte(n as i32);
            write_qwd_record_msg(out, time, seq, &buf.data);
            buf.clear();
            buf.byte(svc as i32);
            buf.byte(n as i32);
        }
    }
    if !buf.is_empty() {
        buf.byte(0);
        buf.byte(0);
        write_qwd_record_msg(out, time, seq, &buf.data);
    }
}

fn write_qwd_record_msg(out: &mut Vec<u8>, time: f32, seq: &mut u32, payload: &[u8]) {
    out.extend(time.to_le_bytes());
    out.push(DEM_READ);
    out.extend(((payload.len() + 8) as i32).to_le_bytes());
    out.extend((*seq).to_le_bytes());
    out.extend((*seq).to_le_bytes());
    out.extend_from_slice(payload);
    *seq += 1;
}

/// Encodes one incoming QW packet (`dem_read`) for `.qwd` recording (`CL_WriteDemoMessage`).
pub fn encode_qwd_read(time: f32, packet: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(9 + packet.len());
    out.extend(time.to_le_bytes());
    out.push(DEM_READ);
    out.extend((packet.len() as i32).to_le_bytes());
    out.extend_from_slice(packet);
    out
}

/// Encodes one outgoing QW command (`dem_cmd`) for `.qwd` recording (`CL_WriteDemoCmd`).
pub fn encode_qwd_cmd(time: f32, cmd: &UserCmd, viewangles: [f32; 3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(5 + 24 + 12);
    out.extend(time.to_le_bytes());
    out.push(DEM_CMD);
    // C usercmd_t layout (24 bytes): u8 msec, 3 bytes pad, [f32; 3] angles, [i16; 3] move, u8 buttons, u8 impulse
    out.push(cmd.msec);
    out.extend([0u8; 3]);
    for a in cmd.angles {
        out.extend(a.to_le_bytes());
    }
    out.extend(cmd.forwardmove.to_le_bytes());
    out.extend(cmd.sidemove.to_le_bytes());
    out.extend(cmd.upmove.to_le_bytes());
    out.push(cmd.buttons);
    out.push(cmd.impulse);
    for a in viewangles {
        out.extend(a.to_le_bytes());
    }
    out
}

/// Encodes the `.qwd` disconnect trailer (`CL_Stop_f` in `cl_demo.c:383-390`).
pub fn encode_qwd_end(time: f32) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(15);
    pkt.extend((-1i32).to_le_bytes());
    pkt.push(SVC_DISCONNECT);
    pkt.extend_from_slice(b"EndOfDemo\0");
    encode_qwd_read(time, &pkt)
}
