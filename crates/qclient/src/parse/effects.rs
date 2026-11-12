use super::*;

impl ClientState {
    /// CL_ParseStartSoundPacket.
    pub(super) fn parse_start_sound(&mut self, m: &mut MsgReader<'_>) -> R {
        let mut fm = m.byte() as u32;
        let fte = self.pext2 & crate::delta::PEXT2_REPLACEMENTDELTAS != 0;
        if fte && fm & SND_FTE_MOREFLAGS != 0 {
            fm |= (m.byte() as u32 & 0xff) << 8;
        }
        let volume = if fm & SND_VOLUME != 0 { m.byte() } else { DEFAULT_SOUND_PACKET_VOLUME };
        let attenuation = if fm & SND_ATTENUATION != 0 { m.byte() as f32 / 64.0 } else { DEFAULT_SOUND_PACKET_ATTENUATION };
        if fte {
            // FTE/DP sound extras the mixer doesn't use: pitch, offset, velocity.
            if fm & SND_FTE_PITCHADJ != 0 {
                m.byte();
            }
            if fm & SND_FTE_TIMEOFS != 0 {
                m.short();
            }
            if fm & SND_FTE_VELOCITY != 0 {
                m.short();
                m.short();
                m.short();
            }
            if fm & SND_DP_PITCH != 0 {
                m.short();
            }
        }
        let (ent, channel) = if fm & SND_LARGEENTITY != 0 {
            let e = m.short() as u16 as i32;
            (e, m.byte())
        } else {
            let c = m.short() as u16 as i32;
            (c >> 3, c & 7)
        };
        let sound = if fm & SND_LARGESOUND != 0 { m.short() as u16 as usize } else { m.byte() as u8 as usize };
        if sound >= MAX_SOUNDS {
            return err(format!("CL_ParseStartSoundPacket: {sound} > MAX_SOUNDS"));
        }
        if ent as usize > MAX_EDICTS {
            return err(format!("CL_ParseStartSoundPacket: ent = {ent}"));
        }
        let origin = m.vec3_coord(self.protocolflags);
        self.events.push(ClientEvent::Sound { ent, channel, sound, origin, volume: volume as f32 / 255.0, attenuation });
        Ok(())
    }

    /// CL_ParseBeam.
    pub(super) fn parse_beam(&mut self, m: &mut MsgReader<'_>, model: &'static str) {
        let pf = self.protocolflags;
        let entity = m.short();
        let start = m.vec3_coord(pf);
        let end = m.vec3_coord(pf);
        let b = Beam { entity, model, start, end, starttime: self.time - 0.001, endtime: self.time + 0.2 };
        let t = self.time;
        let slice = self.beams.as_mut_slice();
        if let Some(x) = slice.iter_mut().find(|x| x.entity == entity) {
            *x = b;
        } else if let Some(x) = slice.iter_mut().find(|x| x.starttime > t || x.endtime < t) {
            *x = b;
        } else {
            let _ = self.beams.insert(b); // "Beam list overflow" when full
        }
    }

    /// CL_ParseTEnt.
    pub(super) fn parse_tent(&mut self, m: &mut MsgReader<'_>) -> R {
        let pf = self.protocolflags;
        let ty = m.byte() as u8;
        let pos_of = |m: &mut MsgReader<'_>| m.vec3_coord(pf);
        match ty {
            TE_WIZSPIKE => {
                let pos = pos_of(m);
                self.shot_impacts.push((pos, false));
                if !self.fte_effect("TE_WIZSPIKE", pos, 1.0) {
                    self.run_particle_effect(pos, [0.0; 3], 20, 30);
                }
                self.tsound("wizard/hit.wav", pos);
            }
            TE_KNIGHTSPIKE => {
                let pos = pos_of(m);
                self.shot_impacts.push((pos, false));
                if !self.fte_effect("TE_KNIGHTSPIKE", pos, 1.0) {
                    self.run_particle_effect(pos, [0.0; 3], 226, 20);
                }
                self.tsound("hknight/hit.wav", pos);
            }
            TE_SPIKE | TE_SUPERSPIKE => {
                let pos = pos_of(m);
                let name = if ty == TE_SPIKE { "TE_SPIKE" } else { "TE_SUPERSPIKE" };
                self.shot_impacts.push((pos, false));
                if !self.fte_effect(name, pos, 1.0) {
                    self.run_particle_effect(pos, [0.0; 3], 0, if ty == TE_SPIKE { 10 } else { 20 });
                }
                if self.rng.rand() % 5 != 0 {
                    self.tsound("weapons/tink1.wav", pos);
                } else {
                    let s = match self.rng.rand() & 3 {
                        1 => "weapons/ric1.wav",
                        2 => "weapons/ric2.wav",
                        _ => "weapons/ric3.wav",
                    };
                    self.tsound(s, pos);
                }
            }
            TE_GUNSHOT => {
                let pos = pos_of(m);
                self.shot_impacts.push((pos, false));
                if !self.fte_effect("TE_GUNSHOT", pos, 20.0) {
                    self.run_particle_effect(pos, [0.0; 3], 0, 20);
                }
            }
            TE_EXPLOSION => {
                let pos = pos_of(m);
                self.shot_impacts.push((pos, true));
                if !self.fte_effect("TE_EXPLOSION", pos, 1.0) {
                    self.particle_explosion(pos);
                }
                self.explosion_light(pos);
                self.tsound("weapons/r_exp3.wav", pos);
            }
            TE_TAREXPLOSION => {
                let pos = pos_of(m);
                self.shot_impacts.push((pos, true));
                if !self.fte_effect("TE_TAREXPLOSION", pos, 1.0) {
                    self.blob_explosion(pos);
                }
                self.tsound("weapons/r_exp3.wav", pos);
            }
            TE_LIGHTNING1 => self.parse_beam(m, "progs/bolt.mdl"),
            TE_LIGHTNING2 => self.parse_beam(m, "progs/bolt2.mdl"),
            TE_LIGHTNING3 => self.parse_beam(m, "progs/bolt3.mdl"),
            TE_BEAM => self.parse_beam(m, "progs/beam.mdl"),
            TE_LAVASPLASH => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_LAVASPLASH", pos, 1.0) {
                    self.lava_splash(pos);
                }
            }
            TE_TELEPORT => {
                let pos = pos_of(m);
                if !self.fte_effect("TE_TELEPORT", pos, 1.0) {
                    self.teleport_splash(pos);
                }
            }
            TE_EXPLOSION2 => {
                let pos = pos_of(m);
                let start = m.byte();
                let len = m.byte();
                self.shot_impacts.push((pos, true));
                if !self.fte_effect(&format!("TE_EXPLOSION2_{start}_{len}"), pos, 1.0) {
                    self.particle_explosion2(pos, start, len);
                }
                self.explosion_light(pos);
                self.tsound("weapons/r_exp3.wav", pos);
            }
            crate::fte::TEDP_PARTICLERAIN => self.parse_dp_weather(m, false),
            crate::fte::TEDP_PARTICLESNOW => self.parse_dp_weather(m, true),
            _ => return err(format!("CL_ParseTEnt: bad type {ty}")),
        }
        Ok(())
    }

    pub(super) fn tsound(&mut self, name: &'static str, origin: [f32; 3]) {
        self.events.push(ClientEvent::TempSound { name, origin });
    }

    pub(super) fn explosion_light(&mut self, pos: [f32; 3]) {
        let die = self.time + 0.5;
        let dl = self.alloc_dlight(0);
        dl.origin = pos;
        dl.radius = 350.0;
        dl.die = die;
        dl.decay = 300.0;
    }
}
