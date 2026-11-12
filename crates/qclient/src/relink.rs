//! CL_LerpPoint / CL_RelinkEntities and the per-frame client update.

use qcore::math::{Vec3, angle_mod, angle_vectors};

use crate::state::*;

impl ClientState {
    /// One client frame of `dt` seconds after parsing this frame's messages:
    /// advance time, relink entities, update beams, decay lights, run particles.
    pub fn frame(&mut self, dt: f64) {
        self.oldtime = self.time;
        if !self.paused {
            self.time += dt;
        }
        self.relink_entities();
        self.update_tents();
        self.decay_lights();
        self.run_particles();
    }

    /// CL_LerpPoint: interpolation fraction between mtime[1] and mtime[0];
    /// clamps `time` into the window.
    pub fn lerp_point(&mut self) -> f32 {
        let mut f = self.mtime[0] - self.mtime[1];
        if f == 0.0 || self.nolerp {
            self.time = self.mtime[0];
            return 1.0;
        }
        if f > 0.1 {
            self.mtime[1] = self.mtime[0] - 0.1;
            f = 0.1;
        }
        let mut frac = (self.time - self.mtime[1]) / f;
        if frac < 0.0 {
            if frac < -0.01 {
                self.time = self.mtime[1];
            }
            frac = 0.0;
        } else if frac > 1.0 {
            if frac > 1.01 {
                self.time = self.mtime[0];
            }
            frac = 1.0;
        }
        frac as f32
    }

    fn reset_trail(e: &mut ClientEntity) {
        e.traildelay = 1.0 / 72.0;
        e.trailorg = e.origin;
    }

    /// CL_RocketTrail rate limiter.
    fn ent_trail(&mut self, i: usize, ty: i32) {
        let dt = (self.time - self.oldtime) as f32;
        let e = &mut self.entities[i];
        e.traildelay -= dt;
        if e.traildelay > 0.0 {
            return;
        }
        let (a, b) = (e.trailorg, e.origin);
        self.rocket_trail(a, b, ty);
        Self::reset_trail(&mut self.entities[i]);
    }

    /// CL_RelinkEntities: interpolate origins/angles, spawn effect lights
    /// and trails, fill [`ClientState::visedicts`].
    #[allow(clippy::needless_range_loop)]
    pub fn relink_entities(&mut self) {
        let frac = self.lerp_point();
        self.visedicts.clear();
        for i in 0..3 {
            self.velocity[i] = self.mvelocity[1][i] + frac * (self.mvelocity[0][i] - self.mvelocity[1][i]);
        }
        if self.demoplayback {
            for i in 0..3 {
                let mut d = self.mviewangles[0][i] - self.mviewangles[1][i];
                if d > 180.0 {
                    d -= 360.0;
                } else if d < -180.0 {
                    d += 360.0;
                }
                self.viewangles[i] = self.mviewangles[1][i] + frac * d;
            }
        }
        let bobjrotate = angle_mod(100.0 * self.time as f32);
        let t = self.time;
        for i in 1..self.entities.len() {
            let e = &mut self.entities[i];
            if e.model == 0 {
                continue;
            }
            if e.msgtime != self.mtime[0] {
                e.model = 0;
                e.lerpflags |= LERP_RESETMOVE | LERP_RESETANIM;
                continue;
            }
            let oldorg = e.origin;
            if e.forcelink {
                e.origin = e.msg_origins[0];
                e.angles = e.msg_angles[0];
            } else {
                let mut f = frac;
                let mut delta = [0.0; 3];
                for j in 0..3 {
                    delta[j] = e.msg_origins[0][j] - e.msg_origins[1][j];
                    if !(-100.0..=100.0).contains(&delta[j]) {
                        f = 1.0;
                        e.lerpflags |= LERP_RESETMOVE;
                    }
                }
                if e.lerpflags & LERP_MOVESTEP != 0 {
                    f = 1.0;
                }
                for j in 0..3 {
                    e.origin[j] = e.msg_origins[1][j] + f * delta[j];
                    let mut d = e.msg_angles[0][j] - e.msg_angles[1][j];
                    if d > 180.0 {
                        d -= 360.0;
                    } else if d < -180.0 {
                        d += 360.0;
                    }
                    e.angles[j] = e.msg_angles[1][j] + f * d;
                }
            }
            if e.forcelink || e.lerpflags & LERP_RESETMOVE != 0 {
                Self::reset_trail(e);
            }
            let mflags = self.model_flags.get(e.model).copied().unwrap_or(0);
            if mflags & MF_ROTATE != 0 {
                e.angles[1] = bobjrotate;
            }
            let (effects, origin, angles) = (e.effects, e.origin, e.angles);
            let key = i as i32;
            if effects & EF_BRIGHTFIELD != 0 {
                self.entity_particles(origin);
            }
            if effects & EF_MUZZLEFLASH != 0 {
                let (fv, _, _) = angle_vectors(Vec3::from_array(angles));
                let r = (self.rng.rand() & 31) as f32;
                let dl = self.alloc_dlight(key);
                dl.origin = [origin[0] + 18.0 * fv.x, origin[1] + 18.0 * fv.y, origin[2] + 16.0 + 18.0 * fv.z];
                dl.radius = 200.0 + r;
                dl.minlight = 32.0;
                dl.die = t + 0.1;
                if i == self.viewentity {
                    self.viewent.lerpflags |= LERP_RESETANIM | LERP_RESETANIM2;
                } else {
                    self.entities[i].lerpflags |= LERP_RESETANIM | LERP_RESETANIM2;
                }
            }
            for (bit, rad, dz, color) in [
                (EF_BRIGHTLIGHT, 400.0, 16.0, [1.0; 3]),
                (EF_DIMLIGHT, 200.0, 0.0, [1.0; 3]),
                (EF_QEX_QUADLIGHT, 200.0, 0.0, [0.25, 0.25, 1.0]),
                (EF_QEX_PENTALIGHT, 200.0, 0.0, [1.0, 0.25, 0.25]),
            ] {
                if effects & bit != 0 {
                    let r = (self.rng.rand() & 31) as f32;
                    let dl = self.alloc_dlight(key);
                    dl.origin = [origin[0], origin[1], origin[2] + dz];
                    dl.radius = rad + r;
                    dl.die = t + 0.001;
                    dl.color = color;
                }
            }
            // FTE trails run every frame from the previous origin
            // (PScript_EntParticleTrail); classic ones are rate limited.
            let fte_trail = [
                (MF_GIB, "TR_BLOOD"),
                (MF_ZOMGIB, "TR_SLIGHTBLOOD"),
                (MF_TRACER, "TR_WIZSPIKE"),
                (MF_TRACER2, "TR_KNIGHTSPIKE"),
                (MF_ROCKET, "TR_ROCKET"),
                (MF_GRENADE, "TR_GRENADE"),
                (MF_TRACER3, "TR_VORESPIKE"),
            ]
            .into_iter()
            .find(|(f, _)| mflags & f != 0)
            .map(|(_, n)| n);
            let relinked = self.entities[i].forcelink || self.entities[i].lerpflags & LERP_RESETMOVE != 0;
            if let Some(name) = fte_trail
                && self.fte_names.has(name)
            {
                // A teleported entity draws no trail this frame.
                if !relinked {
                    self.fte_trail(name, oldorg, origin, key);
                }
                Self::reset_trail(&mut self.entities[i]);
                if mflags & MF_ROCKET != 0 {
                    let dl = self.alloc_dlight(key);
                    dl.origin = origin;
                    dl.radius = 200.0;
                    dl.die = t + 0.01;
                }
            } else if mflags & MF_GIB != 0 {
                self.ent_trail(i, 2);
            } else if mflags & MF_ZOMGIB != 0 {
                self.ent_trail(i, 4);
            } else if mflags & MF_TRACER != 0 {
                self.ent_trail(i, 3);
            } else if mflags & MF_TRACER2 != 0 {
                self.ent_trail(i, 5);
            } else if mflags & MF_ROCKET != 0 {
                self.ent_trail(i, 0);
                let dl = self.alloc_dlight(key);
                dl.origin = origin;
                dl.radius = 200.0;
                dl.die = t + 0.01;
            } else if mflags & MF_GRENADE != 0 {
                self.ent_trail(i, 1);
            } else if mflags & MF_TRACER3 != 0 {
                self.ent_trail(i, 6);
            } else {
                Self::reset_trail(&mut self.entities[i]);
            }
            self.entities[i].forcelink = false;
            if i == self.viewentity {
                continue;
            }
            self.visedicts.push(i);
        }
    }
}
