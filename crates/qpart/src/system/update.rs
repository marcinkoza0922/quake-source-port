use super::*;

#[inline(always)]
fn step_kinematics(p: &mut Particle, ft: f32, has_vel: bool, has_fric: bool, fric_mul: Vec3, grav_ft: f32) {
    if has_vel {
        p.org += p.vel * ft;
        p.vel.z -= grav_ft;
        if has_fric {
            p.vel = p.vel.mul_elem(fric_mul);
        }
    }
    p.angle += p.rotationspeed * ft;
}

#[inline(always)]
fn step_linear_ramp(t: &PartType, p: &mut Particle, pt: f32, d_rgb: [f32; 3], d_alpha: f32, d_scale: f32) {
    if pt < p.die - t.die + t.rgbchangetime {
        p.rgba[0] += d_rgb[0];
        p.rgba[1] += d_rgb[1];
        p.rgba[2] += d_rgb[2];
    }
    p.rgba[3] += d_alpha;
    p.scale += d_scale;
}

impl System {
    /// PScript_UpdateParticle.
    pub(super) fn update_particle(&mut self, reg: &Registry, world: &dyn World, pi: PartRef, ty: TypeId) {
        let t = &reg.types[ty];
        let ft = self.frametime;
        let pt = self.particletime;
        let has_vel = t.flags & PT_VELOCITY != 0;
        let has_fric = t.flags & PT_FRICTION != 0;
        let fric_mul = Vec3::new(1.0 - t.friction[0] * ft, 1.0 - t.friction[1] * ft, 1.0 - t.friction[2] * ft);
        let grav_ft = t.gravity * ft;
        let d_rgb = [ft * t.rgbchange[0], ft * t.rgbchange[1], ft * t.rgbchange[2]];
        let d_alpha = ft * t.alphachange;
        let d_scale = ft * t.scaledelta;

        // Pre-sample flurry RNG before borrowing the particle element.
        let flurry = if has_vel && t.flurry != 0.0 && self.doflurry {
            Some([self.urng.ucrandom() * t.flurry, self.urng.ucrandom() * t.flurry])
        } else {
            None
        };
        let p = self.part_mut(pi);
        let oldorg = p.org;
        step_kinematics(p, ft, has_vel, has_fric, fric_mul, grav_ft);
        if let Some([fx, fy]) = flurry {
            p.vel.x += fx;
            p.vel.y += fy;
        }
        let n = t.ramp.len();
        let age_frac = |die: f32| n as f32 * (t.die - (die - pt)) / t.die;
        match t.rampmode {
            RampMode::Nearest if n > 0 => {
                let r = &t.ramp[(age_frac(p.die).max(0.0) as usize).min(n - 1)];
                p.rgba = [r.rgb[0], r.rgb[1], r.rgb[2], r.alpha];
                p.scale = r.scale;
            }
            RampMode::Lerp if n > 0 => {
                let mut frac = age_frac(p.die);
                let s1 = (frac.max(0.0) as usize).min(n - 1);
                let s2 = (s1 + 1).min(n - 1);
                frac -= s1 as f32;
                let (a, b) = (&t.ramp[s1], &t.ramp[s2]);
                for k in 0..3 {
                    p.rgba[k] = a.rgb[k] + (b.rgb[k] - a.rgb[k]) * frac;
                }
                p.rgba[3] = a.alpha + (b.alpha - a.alpha) * frac;
                p.scale = a.scale + (b.scale - a.scale) * frac;
            }
            RampMode::Delta if n > 0 => {
                let r = &t.ramp[(age_frac(p.die).max(0.0) as usize).min(n - 1)];
                for k in 0..3 {
                    p.rgba[k] += ft * r.rgb[k];
                }
                p.rgba[3] -= ft * r.alpha;
                p.scale += ft * r.scale;
            }
            _ => step_linear_ramp(t, p, pt, d_rgb, d_alpha, d_scale),
        }

        if let Some(e) = t.emit {
            let p_org = p.org;
            let p_vel = p.vel;
            let p_nextemit = p.nextemit;
            if t.emittime < 0.0 {
                self.q_trails.push(DeferredTrail { start: oldorg, end: p_org, ty: e, owner: Some(pi) });
            } else if p_nextemit < pt {
                let nextemit = pt + t.emittime + self.urng.ufrandom() * t.emitrand;
                self.part_mut(pi).nextemit = nextemit;
                self.q_effects.push((p_org, p_vel, 1.0, e));
            }
        }

        let Some(ct) = t.cliptype else { return };
        if !self.settings.bouncysparks {
            return;
        }
        let p = self.part_mut(pi);
        let moved = p.org - p.oldorg;
        if t.clipbounce == 0.0 || moved.dot(moved) > 100.0 {
            let p_oldorg = p.oldorg;
            let p_org = p.org;
            self.traces_used += 1;
            let tr = if self.traces_used <= self.settings.trace_limit {
                world.trace_line(p_oldorg, p_org)
            } else {
                Trace { frac: 1.0, endpos: p_org, normal: Vec3::ZERO, entity: 0 }
            };
            if tr.frac < 1.0 {
                if t.clipbounce < 0.0 {
                    let p_scale;
                    {
                        let p = self.part_mut(pi);
                        p.die = -1.0;
                        p_scale = p.scale;
                    }
                    if t.clipbounce == -2.0 {
                        self.q_decals.push(DeferredDecal {
                            ty,
                            entity: tr.entity,
                            center: p_org,
                            normal: tr.normal,
                            scale: p_scale,
                        });
                    }
                    return;
                } else if ct == ty {
                    let p = self.part_mut(pi);
                    let dist = p.vel.dot(tr.normal) * -t.clipbounce;
                    p.vel = p.vel.mul_add(dist, tr.normal);
                    p.org = tr.endpos;
                    if t.texname.is_empty() && p.vel.length() < 1000.0 * ft && t.looks.kind == RenderType::Normal {
                        p.die = -1.0;
                        return;
                    }
                } else {
                    let cc = t.clipcount / reg.types[ct].count;
                    let d = {
                        let p = self.part_mut(pi);
                        p.die = -1.0;
                        p.vel = p.vel.normalize();
                        if t.clipbounce != 0.0 { tr.normal * t.clipbounce } else { p.vel }
                    };
                    self.q_effects.push((tr.endpos, d, cc, ct));
                    return;
                }
            }
            let p = self.part_mut(pi);
            p.oldorg = p.org;
        }
    }

    fn recycle_particles(&mut self) {
        let mut left = 256usize;
        let n_types = self.run_list.len();
        if n_types > 0 {
            for step in 0..n_types {
                let ty = self.run_list[(self.particle_recycle + step) % n_types];
                let v = &mut self.rt[ty].vec;
                let take = left.min(v.len());
                for p in &mut v[..take] {
                    p.die = -1.0;
                }
                left -= take;
                if left == 0 {
                    self.particle_recycle = (self.particle_recycle + step + 1) % n_types;
                    return;
                }
            }
            self.particle_recycle = (self.particle_recycle + 1) % n_types;
        }
        for _ in 0..left {
            if self.parts.is_empty() {
                break;
            }
            let idx = self.particle_recycle % self.parts.len();
            self.parts[idx].die = -1.0;
            self.particle_recycle = (idx + 1) % self.parts.len();
        }
    }

    fn reap_vec(&mut self, t: &PartType, ty: TypeId, pt: f32) {
        let len = self.rt[ty].vec.len();
        let mut w = 0;
        {
            let v = &self.rt[ty].vec;
            while w < len && v[w].die >= pt {
                w += 1;
            }
        }
        if w == len {
            return;
        }
        if t.emittime < 0.0 {
            let mut v = std::mem::take(&mut self.rt[ty].vec);
            let start = w;
            for r in start..len {
                if v[r].die < pt {
                    let mut tr = v[r].trail.take();
                    self.delink_trailstate(&mut tr);
                } else {
                    v[w] = v[r];
                    w += 1;
                }
            }
            v.truncate(w);
            self.rt[ty].vec = v;
        } else {
            let v = &mut self.rt[ty].vec;
            let start = w + 1;
            for r in start..len {
                if v[r].die >= pt {
                    v[w] = v[r];
                    w += 1;
                }
            }
            v.truncate(w);
        }
        self.total_particles -= len - w;
    }

    fn reap_arena(&mut self, reg: &Registry, ty: TypeId) {
        let pt = self.particletime;
        let emit_trail = reg.types[ty].emittime < 0.0;
        let mut prev = NIL;
        let mut cur = self.rt[ty].particles;
        let mut reaped = 0usize;
        while cur != NIL {
            let next = self.parts[cur as usize].next;
            if self.parts[cur as usize].die < pt {
                if emit_trail {
                    let mut tr = self.parts[cur as usize].trail.take();
                    self.delink_trailstate(&mut tr);
                }
                if prev == NIL {
                    self.rt[ty].particles = next;
                } else {
                    self.parts[prev as usize].next = next;
                }
                self.kill_list.push(cur);
                reaped += 1;
            } else {
                prev = cur;
            }
            cur = next;
        }
        self.total_particles -= reaped;
    }

    fn update_type_particles(&mut self, reg: &Registry, world: &dyn World, ty: TypeId) {
        let t = &reg.types[ty];
        let ft = self.frametime;
        let pt = self.particletime;
        if t.looks.kind == RenderType::Beam {
            self.reap_arena(reg, ty);
            let mut cur = self.rt[ty].particles;
            while cur != NIL {
                let next = self.parts[cur as usize].next;
                self.update_particle(reg, world, PartRef::Arena(cur), ty);
                cur = next;
            }
            return;
        }
        self.reap_vec(t, ty, pt);
        let len = self.rt[ty].vec.len();
        if len == 0 {
            return;
        }
        let has_vel = t.flags & PT_VELOCITY != 0;
        let has_fric = t.flags & PT_FRICTION != 0;
        let fric_mul = Vec3::new(1.0 - t.friction[0] * ft, 1.0 - t.friction[1] * ft, 1.0 - t.friction[2] * ft);
        let grav_ft = t.gravity * ft;
        let d_rgb = [ft * t.rgbchange[0], ft * t.rgbchange[1], ft * t.rgbchange[2]];
        let d_alpha = ft * t.alphachange;
        let d_scale = ft * t.scaledelta;
        let simple_ramp = matches!(t.rampmode, RampMode::None) || t.ramp.is_empty();
        if simple_ramp
            && t.emit.is_none()
            && (t.cliptype.is_none() || !self.settings.bouncysparks)
            && (t.flurry == 0.0 || !self.doflurry)
        {
            for p in self.rt[ty].vec.iter_mut().rev() {
                step_kinematics(p, ft, has_vel, has_fric, fric_mul, grav_ft);
                step_linear_ramp(t, p, pt, d_rgb, d_alpha, d_scale);
            }
            return;
        }
        for idx in (0..len).rev() {
            self.update_particle(reg, world, PartRef::Vec(ty, idx), ty);
        }
    }

    /// One frame: spawns rain, advances particles, feeds renderables to
    /// `sink`, runs effects queued by particles, and advances the clock.
    pub fn frame(&mut self, reg: &Registry, world: &dyn World, input: &FrameInput, sink: &mut dyn Sink) {
        self.ensure_rt(reg.types.len());
        self.cl_time = input.cl_time;
        self.frametime = ((input.cl_time - self.oldtime) as f32).clamp(0.0, 1.0);
        self.oldtime = input.cl_time;
        self.kill_list.clear();

        self.flurrytime -= self.frametime;
        if self.flurrytime < 0.0 {
            self.doflurry = true;
            self.flurrytime = 0.1 + self.rng.frandom() * 0.3;
        } else {
            self.doflurry = false;
        }

        // Full pools: mark some as dead so new ones can spawn next frame.
        if !self.has_free_decal() && !self.decals.is_empty() {
            for _ in 0..256 {
                self.decals[self.decal_recycle].die = -1.0;
                self.decal_recycle = (self.decal_recycle + 1) % self.decals.len();
            }
        }
        if !self.has_free_particle() {
            self.recycle_particles();
        }

        if self.settings.rain {
            for em in &input.rain {
                self.add_rain(reg, world, em, input.vieworg);
            }
        }

        // Snapshot active types without allocating and reap+update each non-zero type.
        std::mem::swap(&mut self.run_scratch, &mut self.run_list);
        self.run_list.clear();
        self.traces_used = 0;
        for i in 0..self.run_scratch.len() {
            let ty = self.run_scratch[i];
            if reg.types[ty].die == 0.0 {
                continue;
            }
            self.update_type_particles(reg, world, ty);
        }

        self.update_types(reg, sink);

        // Deferred spawns (preserve capacity of reusable queues).
        let ft = self.frametime;
        if !self.q_trails.is_empty() {
            let mut q = std::mem::take(&mut self.q_trails);
            for tr in q.drain(..) {
                let mut slot = tr.owner.and_then(|p| self.part(p).trail);
                self.trail(reg, world, tr.start, tr.end, tr.ty, ft, 0, Some(&mut slot));
                if let Some(p) = tr.owner {
                    self.part_mut(p).trail = slot;
                }
            }
            self.q_trails = q;
        }
        if !self.q_effects.is_empty() {
            let mut q = std::mem::take(&mut self.q_effects);
            for (org, dir, count, ty) in q.drain(..) {
                self.run_effect(reg, world, org, Some(dir), count, ty, None);
            }
            self.q_effects = q;
        }
        if !self.q_decals.is_empty() {
            let mut q = std::mem::take(&mut self.q_decals);
            for dd in q.drain(..) {
                self.spawn_hit_decal(reg, world, &dd);
            }
            self.q_decals = q;
        }

        if !self.kill_list.is_empty() {
            self.free_parts.extend(self.kill_list.iter().rev());
            self.kill_list.clear();
        }
        self.particletime += self.frametime;
    }
}
