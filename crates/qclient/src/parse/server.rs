use super::*;

impl ClientState {
    /// CL_ParseServerInfo.
    pub(super) fn parse_serverinfo(&mut self, m: &mut MsgReader<'_>) -> R {
        self.clear_state();
        let mut p = m.long() as u32;
        // FTE extension prefixes ahead of the base protocol.
        loop {
            match p {
                crate::delta::PROTOCOL_FTE_PEXT1 => {
                    if m.long() != 0 {
                        return err("Server returned FTE1 protocol extensions that are not supported");
                    }
                }
                crate::delta::PROTOCOL_FTE_PEXT2 => {
                    self.pext2 = m.long() as u32;
                    if self.pext2 & !crate::delta::PEXT2_SUPPORTED != 0 {
                        return err(format!("Server returned FTE2 protocol extensions that are not supported ({:#x})", self.pext2));
                    }
                }
                _ => break,
            }
            p = m.long() as u32;
        }
        if self.pext2 & crate::delta::PEXT2_REPLACEMENTDELTAS != 0 {
            self.ack_frames.push(-1);
        }
        check_protocol(p)?;
        self.protocol = p;
        self.protocolflags = if p == PROTOCOL_RMQ { m.long() as u32 } else { 0 };
        let mc = m.byte();
        if !(1..=MAX_SCOREBOARD as i32).contains(&mc) {
            return err(format!("Bad maxclients ({mc}) from server"));
        }
        self.maxclients = mc as usize;
        self.scores = vec![Score::default(); self.maxclients];
        self.gametype = m.byte() as u8;
        self.levelname = m.string_lossy();
        for (list, max, what) in [(&mut self.model_precache, MAX_MODELS, "model"), (&mut self.sound_precache, MAX_SOUNDS, "sound")] {
            list.truncate(1);
            loop {
                let s = m.string();
                if s.is_empty() || m.badread {
                    break;
                }
                if list.len() == max {
                    return err(format!("Server sent too many {what} precaches"));
                }
                list.push(String::from_utf8_lossy(s).into_owned());
            }
        }
        if m.badread {
            return err("bad serverinfo");
        }
        self.model_flags = vec![0; self.model_precache.len()];
        let world = self.model_precache.get(1).cloned().unwrap_or_default();
        let base = world.rsplit('/').next().unwrap_or("");
        self.mapname = base.rsplit_once('.').map_or(base, |(a, _)| a).to_string();
        self.entities[0].model = 1;
        // A new serverinfo starts a fresh signon (changelevel / reconnect).
        self.signon = 0;
        self.events.push(ClientEvent::NewMap);
        Ok(())
    }

    /// CL_ParseBaseline.
    pub(super) fn parse_baseline(&self, m: &mut MsgReader<'_>, version: i32) -> EntityState {
        let pf = self.protocolflags;
        let bits = if version == 2 { m.byte() as u32 } else { 0 };
        let modelindex = if bits & B_LARGEMODEL != 0 { m.short() as u16 as usize } else { m.byte() as u8 as usize };
        let frame = if bits & B_LARGEFRAME != 0 { m.short() as u16 as i32 } else { m.byte() };
        let colormap = m.byte();
        let skin = m.byte();
        let mut origin = [0.0; 3];
        let mut angles = [0.0; 3];
        for i in 0..3 {
            origin[i] = m.coord(pf);
            angles[i] = m.angle(pf);
        }
        let alpha = if bits & B_ALPHA != 0 { m.byte() as u8 } else { ENTALPHA_DEFAULT };
        let scale = if bits & B_SCALE != 0 { m.byte() as u8 } else { ENTSCALE_DEFAULT };
        EntityState { origin, angles, modelindex, frame, colormap, skin, effects: 0, alpha, scale, ..EntityState::default() }
    }

    /// CL_ParseUpdate.
    pub(super) fn parse_update(&mut self, m: &mut MsgReader<'_>, mut bits: u32) -> R {
        if self.signon == SIGNONS - 1 {
            self.signon = SIGNONS;
            self.signon_reply();
        }
        if bits & U_MOREBITS != 0 {
            bits |= (m.byte() as u32 & 0xff) << 8;
        }
        let fitz = self.protocol != PROTOCOL_NETQUAKE;
        if fitz {
            if bits & U_EXTEND1 != 0 {
                bits |= (m.byte() as u32 & 0xff) << 16;
            }
            if bits & U_EXTEND2 != 0 {
                bits |= (m.byte() as u32 & 0xff) << 24;
            }
        }
        let num = if bits & U_LONGENTITY != 0 { m.short() as u16 as i32 } else { m.byte() };
        let (pf, protocol, mtime, nmodels) = (self.protocolflags, self.protocol, self.mtime, self.model_precache.len());
        let ent = self.entity_num(num)?;
        let mut forcelink = ent.msgtime != mtime[1];
        if ent.msgtime + 0.2 < mtime[0] {
            ent.lerpflags |= LERP_RESETANIM;
        }
        ent.msgtime = mtime[0];
        let b = ent.baseline;
        let mut modnum = if bits & U_MODEL != 0 { m.byte() as u8 as usize } else { b.modelindex };
        let prevframe = ent.frame;
        ent.frame = if bits & U_FRAME != 0 { m.byte() } else { b.frame };
        ent.colormap = if bits & U_COLORMAP != 0 { m.byte() } else { b.colormap };
        ent.skin = if bits & U_SKIN != 0 { m.byte() } else { b.skin };
        ent.effects = if bits & U_EFFECTS != 0 { m.byte() } else { b.effects };
        ent.msg_origins[1] = ent.msg_origins[0];
        ent.msg_angles[1] = ent.msg_angles[0];
        let (o1, a1, o2, a2, o3, a3) = (U_ORIGIN1, U_ANGLE1, U_ORIGIN2, U_ANGLE2, U_ORIGIN3, U_ANGLE3);
        for (i, (ob, ab)) in [(o1, a1), (o2, a2), (o3, a3)].into_iter().enumerate() {
            ent.msg_origins[0][i] = if bits & ob != 0 { m.coord(pf) } else { b.origin[i] };
            ent.msg_angles[0][i] = if bits & ab != 0 { m.angle(pf) } else { b.angles[i] };
        }
        if bits & U_STEP != 0 {
            ent.lerpflags |= LERP_MOVESTEP;
            ent.forcelink = true;
        } else {
            ent.lerpflags &= !LERP_MOVESTEP;
        }
        if fitz {
            ent.alpha = if bits & U_ALPHA != 0 { m.byte() as u8 } else { b.alpha };
            ent.scale = if bits & U_SCALE != 0 { m.byte() as u8 } else { b.scale };
            if bits & U_FRAME2 != 0 {
                ent.frame = (ent.frame & 0xff) | ((m.byte() & 0xff) << 8);
            }
            if bits & U_MODEL2 != 0 {
                modnum = (modnum & 0xff) | (((m.byte() & 0xff) as usize) << 8);
            }
            if bits & U_LERPFINISH != 0 {
                ent.lerpfinish = ent.msgtime + m.byte() as f64 / 255.0;
                ent.lerpflags |= LERP_FINISH;
            } else {
                ent.lerpflags &= !LERP_FINISH;
            }
        } else if protocol == PROTOCOL_NETQUAKE {
            if bits & U_TRANS != 0 {
                // Nehahra alpha.
                let a = m.float();
                let alpha = m.float();
                if a == 2.0 {
                    m.float();
                }
                ent.alpha = entalpha_encode(alpha);
            } else {
                ent.alpha = b.alpha;
            }
            ent.scale = b.scale;
        }
        if modnum >= MAX_MODELS {
            return err("CL_ParseModel: bad modnum");
        }
        // Unknown precache index behaves like a NULL model.
        let model = if modnum < nmodels { modnum } else { 0 };
        if model != ent.model {
            ent.model = model;
            if model == 0 {
                forcelink = true;
            } else {
                ent.syncbase = 0.0;
            }
            ent.lerpflags |= LERP_RESETANIM;
        }
        let _ = prevframe; // syncbase for ST_FRAMETIME models is the renderer's concern
        if forcelink {
            // New or reappearing entities must also snap renderer movement history.
            ent.lerpflags |= LERP_RESETMOVE;
            ent.msg_origins[1] = ent.msg_origins[0];
            ent.origin = ent.msg_origins[0];
            ent.msg_angles[1] = ent.msg_angles[0];
            ent.angles = ent.msg_angles[0];
            ent.forcelink = true;
        }
        if m.badread {
            return err("CL_ParseUpdate: truncated");
        }
        Ok(())
    }

    /// CL_ParseClientdata.
    pub(super) fn parse_clientdata(&mut self, m: &mut MsgReader<'_>) {
        let mut bits = m.short() as u16 as u32;
        if bits & SU_EXTEND1 != 0 {
            bits |= (m.byte() as u32 & 0xff) << 16;
        }
        if bits & SU_EXTEND2 != 0 {
            bits |= (m.byte() as u32 & 0xff) << 24;
        }
        self.viewheight = if bits & SU_VIEWHEIGHT != 0 { m.char() as f32 } else { DEFAULT_VIEWHEIGHT };
        self.idealpitch = if bits & SU_IDEALPITCH != 0 { m.char() as f32 } else { 0.0 };
        self.mvelocity[1] = self.mvelocity[0];
        for i in 0..3 {
            self.punchangle[i] = if bits & (SU_PUNCH1 << i) != 0 { m.char() as f32 } else { 0.0 };
            self.mvelocity[0][i] = if bits & (SU_VELOCITY1 << i) != 0 { m.char() as f32 * 16.0 } else { 0.0 };
        }
        if self.punchangles[0] != self.punchangle {
            self.punchangles[1] = self.punchangles[0];
            self.punchangles[0] = self.punchangle;
            self.punchtime = self.time;
        }
        let items = m.long();
        if self.items != items {
            for j in 0..32 {
                if items & (1 << j) != 0 && self.items & (1 << j) == 0 {
                    self.item_gettime[j] = self.time;
                }
            }
            self.items = items;
            self.stats[STAT_ITEMS] = items;
        }
        self.onground = bits & SU_ONGROUND != 0;
        self.inwater = bits & SU_INWATER != 0;
        let s = &mut self.stats;
        s[STAT_WEAPONFRAME] = if bits & SU_WEAPONFRAME != 0 { m.byte() } else { 0 };
        s[STAT_ARMOR] = if bits & SU_ARMOR != 0 { m.byte() } else { 0 };
        s[STAT_WEAPON] = if bits & SU_WEAPON != 0 { m.byte() } else { 0 };
        s[STAT_HEALTH] = m.short();
        s[STAT_AMMO] = m.byte();
        for i in 0..4 {
            s[STAT_SHELLS + i] = m.byte();
        }
        let w = m.byte();
        s[STAT_ACTIVEWEAPON] = if self.standard_quake { w } else { 1i32.wrapping_shl(w as u32) };
        for (bit, stat) in [
            (SU_WEAPON2, STAT_WEAPON),
            (SU_ARMOR2, STAT_ARMOR),
            (SU_AMMO2, STAT_AMMO),
            (SU_SHELLS2, STAT_SHELLS),
            (SU_NAILS2, STAT_NAILS),
            (SU_ROCKETS2, STAT_ROCKETS),
            (SU_CELLS2, STAT_CELLS),
            (SU_WEAPONFRAME2, STAT_WEAPONFRAME),
        ] {
            if bits & bit != 0 {
                s[stat] |= (m.byte() & 0xff) << 8;
            }
        }
        self.viewent.alpha = if bits & SU_WEAPONALPHA != 0 { m.byte() as u8 } else { ENTALPHA_DEFAULT };
        let wm = self.stats[STAT_WEAPON].max(0) as usize;
        if self.viewent.model != wm {
            self.viewent.model = wm;
            self.viewent.lerpflags |= LERP_RESETANIM;
        }
        self.viewent.frame = self.stats[STAT_WEAPONFRAME];
    }
}
