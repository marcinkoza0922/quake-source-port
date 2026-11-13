//! FTE replacement deltas, client side (vkQuake cl_parse.c
//! CLFTE_ParseEntitiesUpdate / CL_EntitiesDeltaed / CLFTE_ParseBaseline).

pub use qnet::delta::*;
use qnet::MsgReader;
use qnet::protocol::{MAX_MODELS, SIGNONS};

use crate::parse::{R, err};
use crate::state::*;

/// The delta state an entity resets to.
pub(crate) fn net_state(b: &EntityState) -> EntState {
    EntState {
        origin: b.origin,
        angles: b.angles,
        modelindex: b.modelindex as u32,
        frame: b.frame as u32,
        skin: b.skin as u32,
        colormap: b.colormap as u8,
        effects: b.effects as u32,
        alpha: b.alpha,
        scale: b.scale,
        traileffect: b.traileffect,
        emiteffect: b.emiteffect,
        ..EntState::default()
    }
}

fn entity_state(s: &EntState) -> EntityState {
    EntityState {
        origin: s.origin,
        angles: s.angles,
        modelindex: s.modelindex as usize,
        frame: s.frame as i32,
        colormap: s.colormap as i32,
        skin: s.skin as i32,
        effects: s.effects as i32,
        alpha: s.alpha,
        scale: s.scale,
        traileffect: s.traileffect,
        emiteffect: s.emiteffect,
    }
}

impl ClientState {
    /// CLFTE_ParseBaseline (svcfte_spawnbaseline2 / svcfte_spawnstatic2).
    pub(crate) fn parse_baseline_fte(&mut self, m: &mut MsgReader<'_>) -> R<EntityState> {
        let mut s = EntState::default();
        if read_delta(m, &mut s, &EntState::default(), self.protocolflags, self.pext2).is_none() {
            return err("unsupported entity delta info");
        }
        Ok(entity_state(&s))
    }

    /// CLFTE_ParseEntitiesUpdate (svcfte_updateentities): replaces svc_time
    /// and the U_* entity updates.
    pub(crate) fn parse_entities_update(&mut self, m: &mut MsgReader<'_>) -> R {
        // The server learns which frames arrived (and which to resend).
        if let Some(seq) = self.net_sequence
            && self.ack_frames.len() < 8
        {
            self.ack_frames.push(seq as i32);
        }
        if self.pext2 & PEXT2_PREDINFO != 0 {
            m.short();
        }
        let newtime = m.float() as f64;
        // A snapshot split over several datagrams shares one time.
        if newtime != self.mtime[0] {
            self.mtime[1] = self.mtime[0];
            self.mtime[0] = newtime;
            self.fixangle = false;
        }
        if self.signon == SIGNONS - 1 {
            self.signon = SIGNONS;
            self.signon_reply();
        }
        let (pf, pext2, mtime) = (self.protocolflags, self.pext2, self.mtime);
        while let Some((num, remove)) = read_record_num(m) {
            if m.badread {
                break;
            }
            if remove {
                if num == 0 {
                    // Removing the world: forget every entity (full resend).
                    for e in self.entities.iter_mut().skip(1) {
                        e.delta_live = false;
                        e.model = 0;
                    }
                } else if let Some(e) = self.entities.get_mut(num as usize) {
                    e.delta_live = false;
                    e.model = 0;
                }
                continue;
            }
            let e = self.entity_num(num as i32)?;
            let base = net_state(&e.baseline);
            let mut s = if e.delta_live { e.netstate } else { base };
            if read_delta(m, &mut s, &base, pf, pext2).is_none() {
                return err("unsupported entity delta info");
            }
            e.netstate = s;
            if !e.delta_live {
                e.delta_live = true;
                // The slot's stale time must not defeat the forcelink check.
                e.msgtime = -1.0;
            } else if e.msgtime == mtime[0] {
                // Updated again within this time: process it again.
                e.msgtime = mtime[1];
            }
        }
        if m.badread {
            return err("svcfte_updateentities: truncated");
        }
        self.entities_deltaed();
        Ok(())
    }

    /// CL_EntitiesDeltaed: every entity in the snapshot takes its net state
    /// like an svc_update would (the lerp history shifts once per new time).
    fn entities_deltaed(&mut self) {
        let (mtime, nmodels) = (self.mtime, self.model_precache.len());
        for e in self.entities.iter_mut().skip(1).filter(|e| e.delta_live) {
            let s = e.netstate;
            let mut forcelink = false;
            if e.msgtime != mtime[0] {
                forcelink = e.msgtime != mtime[1];
                if e.msgtime + 0.2 < mtime[0] {
                    e.lerpflags |= LERP_RESETANIM;
                }
                e.msgtime = mtime[0];
                e.msg_origins[1] = e.msg_origins[0];
                e.msg_angles[1] = e.msg_angles[0];
                e.msg_origins[0] = s.origin;
                e.msg_angles[0] = s.angles;
            }
            e.skin = s.skin as i32;
            e.colormap = s.colormap as i32;
            e.effects = s.effects as i32;
            e.alpha = s.alpha;
            e.scale = s.scale;
            e.traileffect = s.traileffect;
            e.emiteffect = s.emiteffect;
            if s.eflags & EFLAGS_STEP != 0 {
                e.lerpflags |= LERP_MOVESTEP;
                e.forcelink = true;
            } else {
                e.lerpflags &= !LERP_MOVESTEP;
            }
            e.lerpflags &= !LERP_FINISH;
            let modnum = s.modelindex as usize;
            // Unknown precache index behaves like a NULL model.
            let model = if modnum < nmodels && modnum < MAX_MODELS { modnum } else { 0 };
            if model != e.model {
                e.model = model;
                if model == 0 {
                    forcelink = true;
                } else {
                    e.syncbase = 0.0;
                }
                e.lerpflags |= LERP_RESETANIM;
            }
            e.frame = s.frame as i32;
            if forcelink {
                // New or reappearing entities must also snap renderer movement history.
                e.lerpflags |= LERP_RESETMOVE;
                e.msg_origins[1] = e.msg_origins[0];
                e.origin = e.msg_origins[0];
                e.msg_angles[1] = e.msg_angles[0];
                e.angles = e.msg_angles[0];
                e.forcelink = true;
            }
        }
    }
}

#[cfg(test)]
mod movement_tests {
    use super::*;

    #[test]
    fn replacement_delta_appearance_resets_movement_but_steps_do_not() {
        let mut cl = ClientState::new();
        cl.model_precache = vec![String::new(), "progs/ogre.mdl".into()];
        cl.entities.resize(3, ClientEntity::default());
        cl.entities[2].delta_live = true;
        cl.entities[2].netstate = EntState {
            modelindex: 1, origin: [256.0, 128.0, 24.0],
            eflags: EFLAGS_STEP, ..Default::default()
        };
        for stale_time in [-1.0, 0.5] {
            cl.mtime = [1.0, 0.9];
            cl.entities[2].msgtime = stale_time;
            cl.entities_deltaed();
            let e = &mut cl.entities[2];
            assert_ne!(e.lerpflags & LERP_RESETMOVE, 0);
            assert_eq!(e.origin, e.netstate.origin);
            e.lerpflags &= !LERP_RESETMOVE;
            cl.mtime = [1.1, 1.0];
            cl.entities[2].netstate.origin[0] += 20.0;
            cl.entities_deltaed();
            assert_eq!(cl.entities[2].lerpflags & LERP_RESETMOVE, 0);
        }
    }
}
