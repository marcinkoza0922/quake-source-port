use super::*;

impl QwState {
    /// Parse one QW server message (the netchan payload). `ack` is the
    /// netchan's incoming_acknowledged, `inseq` its incoming_sequence.
    pub fn parse(&mut self, data: &[u8], ack: u32, inseq: u32, realtime: f64) -> R<Parsed> {
        let mut out = Parsed::default();
        let mut body = Nq::new();
        // CL_ParseClientdata: the frame this packet answers.
        self.parsecount = ack;
        let fi = Self::frame_index(ack);
        self.frames[fi].receivedtime = realtime;
        let latency = realtime - self.frames[fi].senttime;
        if (0.0..=1.0).contains(&latency) {
            self.latency = if latency < self.latency { latency } else { self.latency + 0.001 };
        }
        self.nails.clear();
        let mut got_entities = false;
        let mut m = MsgReader::new(data);
        loop {
            if m.badread {
                return err("CL_ParseServerMessage: Bad server message");
            }
            if m.at_end() {
                break;
            }
            let cmd = m.byte() as u8;
            match cmd {
                SVC_NOP => {}
                SVC_DISCONNECT => {
                    out.actions.push(QwAction::Disconnect("Server disconnected\n".into()));
                    return Ok(out);
                }
                SVC_PRINT => {
                    let level = m.byte();
                    let s = m.string_lossy();
                    body.byte(nq::SVC_PRINT);
                    // \x01 marks chat (talk sound, coloured) as in NQ.
                    let s = if level == PRINT_CHAT && !s.starts_with('\x01') { format!("\x01{s}") } else { s };
                    body.string(&s);
                }
                SVC_CENTERPRINT => {
                    let s = m.string_lossy();
                    body.byte(nq::SVC_CENTERPRINT);
                    body.string(&s);
                }
                SVC_STUFFTEXT => {
                    let s = m.string_lossy();
                    self.stuff_text(&s, &mut body, &mut out);
                }
                SVC_DAMAGE => {
                    let armor = m.byte();
                    let blood = m.byte();
                    let from = coord3(&mut m);
                    body.byte(nq::SVC_DAMAGE);
                    body.byte(armor as u8);
                    body.byte(blood as u8);
                    body.vec(from);
                }
                SVC_SERVERDATA => self.parse_serverdata(&mut m, &mut out)?,
                SVC_SETANGLE => {
                    let a = [read_angle(&mut m), read_angle(&mut m), read_angle(&mut m)];
                    body.byte(nq::SVC_SETANGLE);
                    body.angles(a);
                }
                SVC_LIGHTSTYLE => {
                    let i = m.byte();
                    let s = m.string_lossy();
                    body.byte(nq::SVC_LIGHTSTYLE);
                    body.byte(i as u8);
                    body.string(&s);
                }
                SVC_SOUND => {
                    let channel = m.short();
                    let volume = if channel & SND_VOLUME != 0 { m.byte() } else { nq::DEFAULT_SOUND_PACKET_VOLUME };
                    let atten = if channel & SND_ATTENUATION != 0 { m.byte() } else { 64 };
                    let sound = m.byte();
                    let pos = coord3(&mut m);
                    let ent = (channel >> 3) & 1023;
                    body.byte(nq::SVC_SOUND);
                    body.byte((nq::SND_VOLUME | nq::SND_ATTENUATION) as u8);
                    body.byte(volume as u8);
                    body.byte(atten as u8);
                    body.0.short(ent << 3 | (channel & 7));
                    body.byte(sound as u8);
                    body.vec(pos);
                }
                SVC_STOPSOUND => {
                    let i = m.short();
                    body.byte(nq::SVC_STOPSOUND);
                    body.0.short(i);
                }
                SVC_UPDATEFRAGS => {
                    let i = self.slot(m.byte())?;
                    self.players[i].frags = m.short();
                    self.emit_frags(&mut body, i);
                }
                SVC_UPDATEPING => {
                    let i = self.slot(m.byte())?;
                    self.players[i].ping = m.short();
                }
                SVC_UPDATEPL => {
                    let i = self.slot(m.byte())?;
                    self.players[i].pl = m.byte();
                }
                SVC_UPDATEENTERTIME => {
                    self.slot(m.byte())?;
                    m.float();
                }
                SVC_SPAWNBASELINE => {
                    let i = m.short();
                    let b = Self::parse_baseline(&mut m, i.max(0) as usize);
                    if let Some(s) = self.baselines.get_mut(i.max(0) as usize) {
                        *s = b;
                    }
                }
                SVC_SPAWNSTATIC => {
                    let b = Self::parse_baseline(&mut m, 0);
                    body.byte(nq::SVC_SPAWNSTATIC);
                    body.byte(b.modelindex as u8);
                    body.byte(b.frame as u8);
                    body.byte(b.colormap as u8);
                    body.byte(b.skinnum as u8);
                    for i in 0..3 {
                        body.0.coord(b.origin[i], NQ_FLAGS);
                        body.0.angle(b.angles[i], NQ_FLAGS);
                    }
                }
                SVC_TEMP_ENTITY => self.parse_tent(&mut m, &mut body)?,
                SVC_KILLEDMONSTER => body.byte(nq::SVC_KILLEDMONSTER),
                SVC_FOUNDSECRET => body.byte(nq::SVC_FOUNDSECRET),
                SVC_UPDATESTAT | SVC_UPDATESTATLONG => {
                    let i = m.byte();
                    let v = if cmd == SVC_UPDATESTAT { m.byte() } else { m.long() };
                    if !(0..nq::MAX_CL_STATS as i32).contains(&i) {
                        return err(format!("CL_SetStat: {i} is invalid"));
                    }
                    self.stats[i as usize] = v;
                    // Stats 0..=10 and items ride in svc_clientdata.
                    if i as usize > nq::STAT_ACTIVEWEAPON && i as usize != nq::STAT_ITEMS {
                        body.byte(nq::SVC_UPDATESTAT);
                        body.byte(i as u8);
                        body.0.long(v);
                    }
                }
                SVC_SPAWNSTATICSOUND => {
                    let org = coord3(&mut m);
                    let (s, v, a) = (m.byte(), m.byte(), m.byte());
                    body.byte(nq::SVC_SPAWNSTATICSOUND);
                    body.vec(org);
                    body.byte(s as u8);
                    body.byte(v as u8);
                    body.byte(a as u8);
                }
                SVC_CDTRACK => {
                    self.cdtrack = m.byte() as u8;
                    if self.nq_ready {
                        self.emit_cdtrack(&mut body);
                    }
                }
                SVC_INTERMISSION => {
                    let org = coord3(&mut m);
                    let ang = [read_angle(&mut m), read_angle(&mut m), read_angle(&mut m)];
                    self.intermission = Some((org, ang));
                    body.byte(nq::SVC_SETANGLE);
                    body.angles(ang);
                    body.byte(nq::SVC_INTERMISSION);
                }
                SVC_FINALE => {
                    let s = m.string_lossy();
                    body.byte(nq::SVC_FINALE);
                    body.string(&s);
                }
                SVC_SELLSCREEN => body.byte(nq::SVC_SELLSCREEN),
                SVC_SMALLKICK => self.punch = -2.0,
                SVC_BIGKICK => self.punch = -4.0,
                SVC_MUZZLEFLASH => {
                    let i = m.short();
                    if (1..=MAX_CLIENTS as i32).contains(&i) {
                        self.muzzle.push(i as usize - 1);
                    }
                }
                SVC_UPDATEUSERINFO => {
                    let i = self.slot(m.byte())?;
                    self.players[i].userid = m.long();
                    self.players[i].userinfo = m.string_lossy();
                    self.process_userinfo(i, &mut body);
                }
                SVC_SETINFO => {
                    let i = self.slot(m.byte())?;
                    let k = m.string_lossy();
                    let v = m.string_lossy();
                    info_set(&mut self.players[i].userinfo, &k, &v);
                    self.process_userinfo(i, &mut body);
                }
                SVC_SERVERINFO => {
                    let k = m.string_lossy();
                    let v = m.string_lossy();
                    info_set(&mut self.serverinfo, &k, &v);
                }
                SVC_DOWNLOAD => {
                    let size = m.short();
                    let percent = m.byte() as u8;
                    let mut data = Vec::new();
                    for _ in 0..size.max(0) {
                        data.push(m.byte() as u8);
                    }
                    out.actions.push(QwAction::Download { size, percent, data });
                }
                SVC_PLAYERINFO => self.parse_playerinfo(&mut m)?,
                SVC_NAILS => self.parse_nails(&mut m),
                SVC_CHOKECOUNT => {
                    m.byte();
                }
                SVC_MODELLIST | SVC_SOUNDLIST => self.parse_list(&mut m, cmd == SVC_MODELLIST, &mut out)?,
                SVC_PACKETENTITIES | SVC_DELTAPACKETENTITIES => {
                    self.parse_packet_entities(&mut m, cmd == SVC_DELTAPACKETENTITIES, inseq)?;
                    got_entities = true;
                }
                SVC_MAXSPEED => self.movevars.maxspeed = m.float(),
                SVC_ENTGRAVITY => self.movevars.entgravity = m.float(),
                SVC_SETPAUSE => {
                    self.paused = m.byte() != 0;
                    body.byte(nq::SVC_SETPAUSE);
                    body.byte(self.paused as u8);
                }
                _ => return err(format!("CL_ParseServerMessage: Illegible server message {cmd}")),
            }
        }
        if m.badread {
            return err("CL_ParseServerMessage: Bad server message");
        }
        let frame = got_entities && self.nq_ready && self.nq_signon >= 3;
        if frame && self.stage == Stage::Signon {
            self.stage = Stage::Active;
        }
        if frame || !body.0.is_empty() {
            let mut msg = Nq::new();
            if frame {
                msg.byte(nq::SVC_TIME);
                msg.0.float(realtime as f32);
            }
            msg.0.write(&body.0.data);
            if frame {
                self.emit_frame(&mut msg);
            }
            out.nq.push(msg.0.data);
        }
        Ok(out)
    }
}
