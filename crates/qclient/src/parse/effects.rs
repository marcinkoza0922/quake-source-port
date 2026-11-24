use super::*;
use crate::ImpactKind;

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
        let b = Beam { entity, model, start, end, starttime: self.time - 0.001, endtime: self.time + 0.2, path: None };
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
                self.impact_effect(ImpactKind::WizardSpike, pos);
                self.tsound("wizard/hit.wav", pos);
            }
            TE_KNIGHTSPIKE => {
                let pos = pos_of(m);
                self.impact_effect(ImpactKind::KnightSpike, pos);
                self.tsound("hknight/hit.wav", pos);
            }
            TE_SPIKE | TE_SUPERSPIKE => {
                let pos = pos_of(m);
                let kind = if ty == TE_SPIKE { ImpactKind::Spike } else { ImpactKind::SuperSpike };
                self.impact_effect(kind, pos);
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
                self.impact_effect(ImpactKind::Gunshot, pos);
            }
            TE_EXPLOSION => {
                let pos = pos_of(m);
                self.impact_effect(ImpactKind::Explosion, pos);
                self.tsound("weapons/r_exp3.wav", pos);
            }
            TE_TAREXPLOSION => {
                let pos = pos_of(m);
                self.impact_effect(ImpactKind::TarExplosion, pos);
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
                self.impact_effect(ImpactKind::ColoredExplosion { start, len }, pos);
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
}

#[cfg(test)]
mod impact_tests {
    use super::*;
    #[test]
    fn weapon_kinds_survive_scripted_particle_replacement() {
        let mut cl = ClientState::new();
        cl.fte_names = crate::FteNames::new(["TE_GUNSHOT".into(),"TE_SPIKE".into(),"TE_EXPLOSION".into()]);
        for (ty, kind) in [(TE_GUNSHOT,Some(0)),(TE_SPIKE,Some(1)),(TE_SUPERSPIKE,Some(1)),
            (TE_EXPLOSION,Some(2)),(TE_WIZSPIKE,None),(TE_KNIGHTSPIKE,None)] {
            cl.weapon_impacts.clear();
            let bytes=[ty,0,0,0,0,0,0];
            cl.parse_tent(&mut MsgReader::new(&bytes)).unwrap();
            assert_eq!(cl.weapon_impacts.first().map(|e| e.0),kind);
        }
    }

    #[test]
    fn deferred_weapon_impacts_wait_for_surface_routing() {
        let mut cl = ClientState::new();
        cl.defer_impact_effects();
        cl.clear_state();
        for ty in [
            TE_GUNSHOT, TE_SPIKE, TE_SUPERSPIKE, TE_WIZSPIKE, TE_KNIGHTSPIKE,
            TE_EXPLOSION, TE_TAREXPLOSION, TE_EXPLOSION2,
        ] {
            let bytes = [ty, 0, 0, 0, 0, 0, 0, 4, 8];
            cl.parse_tent(&mut MsgReader::new(&bytes)).unwrap();
        }
        assert_eq!(cl.impact_effects.len(), 8);
        assert!(cl.weapon_impacts.is_empty() && cl.shot_impacts.is_empty());
        assert!(cl.particles.is_empty() && cl.fte_queue.is_empty() && cl.dlights.is_empty());

        let impact = cl.impact_effects[5];
        cl.emit_impact_effect(impact);
        assert_eq!(cl.weapon_impacts, [(2, [0.0; 3])]);
        assert!(!cl.particles.is_empty() && !cl.dlights.is_empty());
    }
}
