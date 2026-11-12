use super::*;

impl QwState {
    /// CL_ParseBaseline.
    pub(super) fn parse_baseline(m: &mut MsgReader<'_>, number: usize) -> QwEntity {
        let mut e = QwEntity { number, ..Default::default() };
        e.modelindex = m.byte().max(0) as usize;
        e.frame = m.byte();
        e.colormap = m.byte();
        e.skinnum = m.byte();
        for i in 0..3 {
            e.origin[i] = read_coord(m);
            e.angles[i] = read_angle(m);
        }
        e
    }

    /// CL_ParseTEnt, rewritten as NQ temp entities or particles.
    pub(super) fn parse_tent(&mut self, m: &mut MsgReader<'_>, b: &mut Nq) -> R {
        let ty = m.byte() as u8;
        let particles = |b: &mut Nq, org: [f32; 3], color: u8, count: i32| {
            b.byte(nq::SVC_PARTICLE);
            b.vec(org);
            for _ in 0..3 {
                b.byte(0);
            }
            b.byte(count.clamp(1, 254) as u8);
            b.byte(color);
        };
        match ty {
            TE_GUNSHOT => {
                let count = m.byte();
                let pos = coord3(m);
                particles(b, pos, 0, 20 * count);
            }
            TE_BLOOD => {
                let count = m.byte();
                let pos = coord3(m);
                particles(b, pos, 73, 20 * count);
            }
            TE_LIGHTNINGBLOOD => {
                let pos = coord3(m);
                particles(b, pos, 225, 50);
            }
            TE_LIGHTNING1 | TE_LIGHTNING2 | TE_LIGHTNING3 => {
                let ent = m.short();
                let (s, e) = (coord3(m), coord3(m));
                b.byte(nq::SVC_TEMP_ENTITY);
                b.byte(ty);
                b.0.short(ent);
                b.vec(s);
                b.vec(e);
            }
            TE_SPIKE | TE_SUPERSPIKE | TE_EXPLOSION | TE_TAREXPLOSION | TE_WIZSPIKE | TE_KNIGHTSPIKE | TE_LAVASPLASH | TE_TELEPORT => {
                let pos = coord3(m);
                b.byte(nq::SVC_TEMP_ENTITY);
                b.byte(ty);
                b.vec(pos);
            }
            _ => return err(format!("CL_ParseTEnt: bad type {ty}")),
        }
        Ok(())
    }

    /// CL_ParsePlayerinfo.
    pub(super) fn parse_playerinfo(&mut self, m: &mut MsgReader<'_>) -> R {
        let num = m.byte();
        if !(0..MAX_CLIENTS as i32).contains(&num) {
            return err("CL_ParsePlayerinfo: bad num");
        }
        let fi = Self::frame_index(self.parsecount);
        let senttime = self.frames[fi].senttime;
        let playerindex = self.playerindex;
        let st = &mut self.frames[fi].playerstate[num as usize];
        let flags = m.short() as u16 as u32;
        st.flags = flags;
        st.messagenum = self.parsecount;
        st.present = true;
        st.origin = coord3(m);
        st.frame = m.byte();
        st.state_time = if flags & PF_MSEC != 0 { senttime - m.byte() as f64 * 0.001 } else { senttime };
        if flags & PF_COMMAND != 0 {
            st.command = read_delta_usercmd(m, &UserCmd::default());
        }
        for i in 0..3 {
            st.velocity[i] = if flags & (PF_VELOCITY1 << i) != 0 { m.short() as f32 } else { 0.0 };
        }
        st.modelindex = if flags & PF_MODEL != 0 { m.byte().max(0) as usize } else { playerindex };
        st.skinnum = if flags & PF_SKINNUM != 0 { m.byte() } else { 0 };
        st.effects = if flags & PF_EFFECTS != 0 { m.byte() } else { 0 };
        st.weaponframe = if flags & PF_WEAPONFRAME != 0 { m.byte() } else { 0 };
        st.viewangles = st.command.angles;
        Ok(())
    }

    /// CL_ParseProjectiles.
    pub(super) fn parse_nails(&mut self, m: &mut MsgReader<'_>) {
        let c = m.byte().max(0);
        for _ in 0..c {
            let mut b = [0i32; 6];
            for x in &mut b {
                *x = m.byte() & 255;
            }
            if self.nails.len() == MAX_PROJECTILES {
                continue;
            }
            let o = [
                (((b[0] + ((b[1] & 15) << 8)) << 1) - 4096) as f32,
                ((((b[1] >> 4) + (b[2] << 4)) << 1) - 4096) as f32,
                (((b[3] + ((b[4] & 15) << 8)) << 1) - 4096) as f32,
            ];
            let a = [(360 * (b[4] >> 4) / 16) as f32, (360 * b[5] / 256) as f32, 0.0];
            self.nails.push((o, a));
        }
    }

    /// CL_ParseDelta.
    pub(super) fn parse_delta(m: &mut MsgReader<'_>, from: &QwEntity, word: u32) -> QwEntity {
        let mut to = *from;
        to.number = (word & 511) as usize;
        let mut bits = word & !511;
        if bits & U_MOREBITS != 0 {
            bits |= m.byte() as u32 & 255;
        }
        to.flags = bits;
        if bits & U_MODEL != 0 {
            to.modelindex = m.byte().max(0) as usize;
        }
        if bits & U_FRAME != 0 {
            to.frame = m.byte();
        }
        if bits & U_COLORMAP != 0 {
            to.colormap = m.byte();
        }
        if bits & U_SKIN != 0 {
            to.skinnum = m.byte();
        }
        if bits & U_EFFECTS != 0 {
            to.effects = m.byte();
        }
        for (i, (ob, ab)) in [(U_ORIGIN1, U_ANGLE1), (U_ORIGIN2, U_ANGLE2), (U_ORIGIN3, U_ANGLE3)].into_iter().enumerate() {
            if bits & ob != 0 {
                to.origin[i] = read_coord(m);
            }
            if bits & ab != 0 {
                to.angles[i] = read_angle(m);
            }
        }
        to
    }

    /// CL_ParsePacketEntities. Frames are keyed by the acknowledged
    /// outgoing sequence (the server keys its copies by the sequence it
    /// received), and `from` names the frame the server deltas against.
    pub(super) fn parse_packet_entities(&mut self, m: &mut MsgReader<'_>, delta: bool, _inseq: u32) -> R {
        let newseq = self.parsecount;
        let newi = Self::frame_index(newseq);
        let mut old: Vec<QwEntity> = Vec::new();
        let mut full = true;
        let mut valid = true;
        if delta {
            let from = m.byte() as u32;
            full = false;
            let oldi = Self::frame_index(from);
            let f = &self.frames[oldi];
            match f.entities_seq {
                Some(s) if s & 255 == from && self.outgoing_sequence.wrapping_sub(s) < UPDATE_BACKUP as u32 - 1 => {
                    old = f.entities.clone();
                }
                _ => valid = false,
            }
        }
        let mut new: Vec<QwEntity> = Vec::with_capacity(old.len());
        let mut oi = 0;
        loop {
            let word = m.short() as u16 as u32;
            if m.badread {
                return err("msg_badread in packetentities");
            }
            if word == 0 {
                new.extend_from_slice(&old[oi.min(old.len())..]);
                break;
            }
            if !valid {
                // FlushEntityPacket: read and ignore.
                Self::parse_delta(m, &QwEntity::default(), word);
                continue;
            }
            let newnum = (word & 511) as usize;
            while oi < old.len() && old[oi].number < newnum {
                if full {
                    valid = false;
                    break;
                }
                new.push(old[oi]);
                oi += 1;
            }
            if !valid {
                Self::parse_delta(m, &QwEntity::default(), word);
                continue;
            }
            if oi < old.len() && old[oi].number == newnum {
                if word & U_REMOVE != 0 {
                    oi += 1;
                    continue;
                }
                let e = Self::parse_delta(m, &old[oi], word);
                new.push(e);
                oi += 1;
            } else {
                // New from the baseline.
                if word & U_REMOVE != 0 {
                    if full {
                        valid = false;
                    }
                    continue;
                }
                let base = self.baselines.get(newnum).copied().unwrap_or_default();
                let e = Self::parse_delta(m, &base, word);
                new.push(e);
            }
            if new.len() > MAX_PACKET_ENTITIES * 8 {
                return err("CL_ParsePacketEntities: too many entities");
            }
        }
        let f = &mut self.frames[newi];
        if valid {
            f.entities = new;
            f.entities_seq = Some(newseq);
            self.validsequence = newseq;
        } else {
            f.entities_seq = None;
            self.validsequence = 0;
        }
        Ok(())
    }
}
