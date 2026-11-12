use super::*;

impl QwState {
    /// The NQ entity frame: svc_clientdata, players, packet entities, nails.
    pub(super) fn emit_frame(&mut self, b: &mut Nq) {
        let fi = Self::frame_index(self.parsecount);
        let me = self.frames[fi].playerstate[self.playernum];
        self.emit_clientdata(b, &me);
        let muzzle = std::mem::take(&mut self.muzzle);
        for j in 0..MAX_CLIENTS {
            let st = self.frames[fi].playerstate[j];
            if st.messagenum != self.parsecount || !st.present {
                continue;
            }
            let local = j == self.playernum;
            if st.modelindex == 0 && !local {
                continue;
            }
            let mut origin = st.origin;
            if local && let Some((o, _)) = self.intermission {
                origin = o;
            }
            let pitch = -st.viewangles[0] / 3.0;
            let angles = [pitch, st.viewangles[1], 0.0];
            let mut effects = map_effects(st.effects);
            if muzzle.contains(&j) {
                effects |= crate::EF_MUZZLEFLASH;
            }
            let model = if local && self.intermission.is_some() { 0 } else { st.modelindex };
            let colormap = if st.modelindex == self.playerindex { j as i32 + 1 } else { 0 };
            emit_update(b, j + 1, model, st.frame, colormap, st.skinnum, effects, origin, angles);
        }
        if self.validsequence != 0 {
            let ents = &self.frames[fi].entities;
            for e in ents {
                if e.modelindex == 0 && map_effects(e.effects) == 0 {
                    continue;
                }
                emit_update(b, e.number, e.modelindex, e.frame, e.colormap, e.skinnum, map_effects(e.effects), e.origin, e.angles);
            }
        } else if let Some(prev) = self.last_valid_frame() {
            // No usable delta this packet: keep showing the last valid entities.
            for e in prev {
                emit_update(b, e.number, e.modelindex, e.frame, e.colormap, e.skinnum, map_effects(e.effects), e.origin, e.angles);
            }
        }
        for (k, (o, a)) in self.nails.iter().enumerate() {
            emit_update(b, NAIL_BASE + k, self.spikeindex, 0, 0, 0, 0, *o, *a);
        }
    }

    pub(super) fn last_valid_frame(&self) -> Option<Vec<QwEntity>> {
        self.frames.iter().filter_map(|f| f.entities_seq.map(|s| (s, &f.entities))).max_by_key(|(s, _)| *s).map(|(_, e)| e.clone())
    }

    /// svc_clientdata from the QW stats and the local player state.
    pub(super) fn emit_clientdata(&mut self, b: &mut Nq, me: &QwPlayerState) {
        use nq::*;
        let s = &self.stats;
        let viewheight: i32 = if me.flags & PF_GIB != 0 {
            8
        } else if me.flags & PF_DEAD != 0 {
            -16
        } else {
            DEFAULT_VIEWHEIGHT as i32
        };
        let mut bits = SU_ITEMS | SU_WEAPONFRAME | SU_ARMOR | SU_WEAPON;
        if viewheight != DEFAULT_VIEWHEIGHT as i32 {
            bits |= SU_VIEWHEIGHT;
        }
        if self.punch != 0.0 {
            bits |= SU_PUNCH1;
        }
        let vel: Vec<i32> = me.velocity.iter().map(|v| (*v / 16.0).clamp(-127.0, 127.0) as i32).collect();
        for (i, v) in vel.iter().enumerate() {
            if *v != 0 {
                bits |= SU_VELOCITY1 << i;
            }
        }
        let hi = |v: i32| (v >> 8) & 0xff != 0;
        for (bit, v) in [
            (SU_WEAPON2, s[STAT_WEAPON]),
            (SU_ARMOR2, s[STAT_ARMOR]),
            (SU_AMMO2, s[STAT_AMMO]),
            (SU_SHELLS2, s[STAT_SHELLS]),
            (SU_NAILS2, s[STAT_NAILS]),
            (SU_ROCKETS2, s[STAT_ROCKETS]),
            (SU_CELLS2, s[STAT_CELLS]),
            (SU_WEAPONFRAME2, me.weaponframe),
        ] {
            if hi(v) {
                bits |= bit;
            }
        }
        if bits & 0xff00_0000 != 0 {
            bits |= SU_EXTEND2;
        }
        if bits & 0x00ff_0000 != 0 {
            bits |= SU_EXTEND1;
        }
        b.byte(SVC_CLIENTDATA);
        b.0.short(bits as i32);
        if bits & SU_EXTEND1 != 0 {
            b.byte((bits >> 16) as u8);
        }
        if bits & SU_EXTEND2 != 0 {
            b.byte((bits >> 24) as u8);
        }
        if bits & SU_VIEWHEIGHT != 0 {
            b.byte(viewheight as i8 as u8);
        }
        for (i, v) in vel.iter().enumerate() {
            if i == 0 && bits & SU_PUNCH1 != 0 {
                b.byte(self.punch as i8 as u8);
            }
            if bits & (SU_VELOCITY1 << i) != 0 {
                b.byte(*v as i8 as u8);
            }
        }
        self.punch = 0.0;
        b.0.long(s[STAT_ITEMS]);
        b.byte(me.weaponframe as u8);
        b.byte(s[STAT_ARMOR] as u8);
        b.byte(s[STAT_WEAPON] as u8);
        b.0.short(s[STAT_HEALTH]);
        b.byte(s[STAT_AMMO] as u8);
        for i in 0..4 {
            b.byte(s[STAT_SHELLS + i] as u8);
        }
        b.byte(s[STAT_ACTIVEWEAPON] as u8);
        for (bit, v) in [
            (SU_WEAPON2, s[STAT_WEAPON]),
            (SU_ARMOR2, s[STAT_ARMOR]),
            (SU_AMMO2, s[STAT_AMMO]),
            (SU_SHELLS2, s[STAT_SHELLS]),
            (SU_NAILS2, s[STAT_NAILS]),
            (SU_ROCKETS2, s[STAT_ROCKETS]),
            (SU_CELLS2, s[STAT_CELLS]),
            (SU_WEAPONFRAME2, me.weaponframe),
        ] {
            if bits & bit != 0 {
                b.byte((v >> 8) as u8);
            }
        }
    }
}
