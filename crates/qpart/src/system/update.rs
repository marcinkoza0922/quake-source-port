use super::*;

impl System {
    /// PScript_UpdateParticle.
    pub(super) fn update_particle(&mut self, reg: &Registry, world: &dyn World, pi: u32, ty: TypeId) {
        let t = &reg.types[ty];
        let ft = self.frametime;
        let pt = self.particletime;
        let p = &mut self.parts[pi as usize];
        let oldorg = p.org;
        if t.flags & PT_VELOCITY != 0 {
            p.org += p.vel * ft;
            p.vel.z -= t.gravity * ft;
            if t.flags & PT_FRICTION != 0 {
                p.vel = p.vel.mul_elem(Vec3::new(1.0 - t.friction[0] * ft, 1.0 - t.friction[1] * ft, 1.0 - t.friction[2] * ft));
            }
            if t.flurry != 0.0 && self.doflurry {
                p.vel.x += self.urng.ucrandom() * t.flurry;
                p.vel.y += self.urng.ucrandom() * t.flurry;
            }
        }
        p.angle += p.rotationspeed * ft;
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
            _ => {
                if pt < p.die - t.die + t.rgbchangetime {
                    for k in 0..3 {
                        p.rgba[k] += ft * t.rgbchange[k];
                    }
                }
                p.rgba[3] += ft * t.alphachange;
                p.scale += ft * t.scaledelta;
            }
        }

        if let Some(e) = t.emit {
            if t.emittime < 0.0 {
                self.q_trails.push(DeferredTrail { start: oldorg, end: p.org, ty: e, owner: Some(pi) });
            } else if p.nextemit < pt {
                p.nextemit = pt + t.emittime + self.urng.ufrandom() * t.emitrand;
                self.q_effects.push((p.org, p.vel, 1.0, e));
            }
        }

        let Some(ct) = t.cliptype else { return };
        if !self.settings.bouncysparks {
            return;
        }
        let p = &mut self.parts[pi as usize];
        let moved = p.org - p.oldorg;
        if t.clipbounce == 0.0 || moved.dot(moved) > 100.0 {
            self.traces_used += 1;
            let tr = if self.traces_used <= self.settings.trace_limit {
                world.trace_line(p.oldorg, p.org)
            } else {
                Trace { frac: 1.0, endpos: p.org, normal: Vec3::ZERO, entity: 0 }
            };
            if tr.frac < 1.0 {
                if t.clipbounce < 0.0 {
                    p.die = -1.0;
                    if t.clipbounce == -2.0 {
                        self.q_decals.push(DeferredDecal {
                            ty,
                            entity: tr.entity,
                            center: p.org,
                            normal: tr.normal,
                            scale: p.scale,
                        });
                    }
                    return;
                } else if ct == ty {
                    let dist = p.vel.dot(tr.normal) * -t.clipbounce;
                    p.vel = p.vel.mul_add(dist, tr.normal);
                    p.org = tr.endpos;
                    if t.texname.is_empty() && p.vel.length() < 1000.0 * ft && t.looks.kind == RenderType::Normal {
                        p.die = -1.0;
                        return;
                    }
                } else {
                    p.die = -1.0;
                    p.vel = p.vel.normalize();
                    let cc = t.clipcount / reg.types[ct].count;
                    let d = if t.clipbounce != 0.0 { tr.normal * t.clipbounce } else { p.vel };
                    self.q_effects.push((tr.endpos, d, cc, ct));
                    return;
                }
            }
            p.oldorg = p.org;
        }
    }

    /// Unlinks expired particles of `ty` into the kill list and returns the
    /// live ones.
    pub(super) fn reap(&mut self, reg: &Registry, ty: TypeId, out: &mut Vec<(u32, TypeId)>) {
        let pt = self.particletime;
        let emit_trail = reg.types[ty].emittime < 0.0;
        let mut prev = NIL;
        let mut cur = self.rt[ty].particles;
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
            } else {
                out.push((cur, ty));
                prev = cur;
            }
            cur = next;
        }
    }

    pub(super) fn free_beam(&mut self, b: u32) {
        self.free_beams.push(b);
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
        if !self.has_free_particle() && !self.parts.is_empty() {
            for _ in 0..256 {
                self.parts[self.particle_recycle].die = -1.0;
                self.particle_recycle = (self.particle_recycle + 1) % self.parts.len();
            }
        }

        if self.settings.rain {
            for em in &input.rain {
                self.add_rain(reg, world, em, input.vieworg);
            }
        }

        // Setup: reap and flatten.
        let mut updates = Vec::new();
        let run = self.run_list.clone();
        for &ty in &run {
            if reg.types[ty].die == 0.0 {
                continue;
            }
            self.reap(reg, ty, &mut updates);
        }
        self.traces_used = 0;
        for &(p, ty) in &updates {
            self.update_particle(reg, world, p, ty);
        }

        self.update_types(reg, sink);

        // Deferred spawns.
        let ft = self.frametime;
        for tr in std::mem::take(&mut self.q_trails) {
            let mut slot = tr.owner.and_then(|p| self.parts[p as usize].trail);
            self.trail(reg, world, tr.start, tr.end, tr.ty, ft, 0, Some(&mut slot));
            if let Some(p) = tr.owner {
                self.parts[p as usize].trail = slot;
            }
        }
        for (org, dir, count, ty) in std::mem::take(&mut self.q_effects) {
            self.run_effect(reg, world, org, Some(dir), count, ty, None);
        }
        for dd in std::mem::take(&mut self.q_decals) {
            self.spawn_hit_decal(reg, world, &dd);
        }

        let kills = std::mem::take(&mut self.kill_list);
        self.free_parts.extend(kills.iter().rev());
        self.particletime += self.frametime;
    }

    /// PScript_UpdateParticleTypes minus geometry: decals, draining of
    /// zero-lifetime types, beam bookkeeping, run-list maintenance.
    pub(super) fn update_types(&mut self, reg: &Registry, sink: &mut dyn Sink) {
        let pt = self.particletime;
        let ft = self.frametime;
        let run = std::mem::take(&mut self.run_list);
        let mut keep = Vec::with_capacity(run.len());
        for ty in run {
            let t = &reg.types[ty];

            // Clipped decals.
            if self.rt[ty].decals != NIL {
                let mut prev = NIL;
                let mut cur = self.rt[ty].decals;
                while cur != NIL {
                    let next = self.decals[cur as usize].next;
                    if self.decals[cur as usize].die < pt {
                        if prev == NIL {
                            self.rt[ty].decals = next;
                        } else {
                            self.decals[prev as usize].next = next;
                        }
                        self.free_decals.push(cur);
                        cur = next;
                        continue;
                    }
                    let d = &mut self.decals[cur as usize];
                    if d.die - pt <= t.die {
                        ramp_decal(t, d, pt, ft);
                    }
                    sink.decal(ty, d);
                    prev = cur;
                    cur = next;
                }
            }

            let draws_points = matches!(
                t.looks.kind,
                RenderType::UDecal
                    | RenderType::Normal
                    | RenderType::Spark
                    | RenderType::SparkFan
                    | RenderType::TexturedSpark
            );

            if draws_points && t.die != 0.0 {
                let mut i = self.rt[ty].particles;
                while i != NIL {
                    let p = &self.parts[i as usize];
                    sink.particle(ty, p);
                    i = p.next;
                }
            } else if t.die == 0.0 {
                // Zero-lifetime: draw once, then kill.
                let mut i = self.rt[ty].particles;
                while i != NIL {
                    let p = &self.parts[i as usize];
                    if draws_points {
                        sink.particle(ty, p);
                    }
                    if let Some(e) = t.emit
                        && t.emitstart <= 0.0
                    {
                        self.q_effects.push((p.org, p.vel, 1.0, e));
                    }
                    let next = p.next;
                    self.kill_list.push(i);
                    i = next;
                }
                self.rt[ty].particles = NIL;

                while self.rt[ty].beams != NIL && self.beams[self.rt[ty].beams as usize].flags & BS_DEAD != 0 {
                    let b = self.rt[ty].beams;
                    self.rt[ty].beams = self.beams[b as usize].next;
                    self.free_beam(b);
                }
                let mut b = self.rt[ty].beams;
                while b != NIL {
                    if self.beams[b as usize].flags & BS_NODRAW == 0 {
                        let nb = self.beams[b as usize].next;
                        if nb != NIL {
                            self.draw_beam(ty, b, nb, sink);
                        }
                    }
                    loop {
                        let k = self.beams[b as usize].next;
                        if k != NIL && self.beams[k as usize].flags & BS_DEAD != 0 {
                            self.beams[b as usize].next = self.beams[k as usize].next;
                            self.free_beam(k);
                            continue;
                        }
                        break;
                    }
                    self.beams[b as usize].flags |= BS_DEAD;
                    b = self.beams[b as usize].next;
                }
            } else {
                // Beams of types with a lifetime.
                loop {
                    let k = self.rt[ty].beams;
                    if k == NIL {
                        break;
                    }
                    let bk = &self.beams[k as usize];
                    if (bk.flags & BS_DEAD != 0 || self.parts[bk.p as usize].die < pt) && bk.flags & BS_LASTSEG == 0 {
                        self.rt[ty].beams = bk.next;
                        self.free_beam(k);
                        continue;
                    }
                    break;
                }
                let mut b = self.rt[ty].beams;
                let mut guard = 0usize;
                while b != NIL && guard <= MAX_BEAMSEGS * 2 {
                    guard += 1;
                    let next = self.beams[b as usize].next;
                    if next == NIL {
                        if self.parts[self.beams[b as usize].p as usize].die < pt {
                            self.beams[b as usize].flags |= BS_DEAD;
                        }
                        break;
                    }
                    if self.beams[b as usize].flags & (BS_LASTSEG | BS_DEAD | BS_NODRAW) != 0 {
                        loop {
                            let k = self.beams[b as usize].next;
                            if k != NIL
                                && self.beams[k as usize].flags & BS_DEAD != 0
                                && self.beams[k as usize].flags & BS_LASTSEG == 0
                            {
                                self.beams[b as usize].next = self.beams[k as usize].next;
                                self.free_beam(k);
                                continue;
                            }
                            break;
                        }
                        if self.beams[b as usize].next == NIL {
                            continue;
                        }
                    } else {
                        if self.beams[next as usize].flags & BS_DEAD == 0 {
                            self.draw_beam(ty, b, next, sink);
                        }
                        if self.parts[self.beams[b as usize].p as usize].die < pt {
                            self.beams[b as usize].flags |= BS_DEAD;
                        }
                    }
                    if self.parts[self.beams[b as usize].p as usize].die < pt {
                        self.beams[b as usize].flags |= BS_DEAD;
                    }
                    b = self.beams[b as usize].next;
                }
            }

            let r = &mut self.rt[ty];
            if r.particles == NIL && r.beams == NIL && r.decals == NIL {
                r.in_run = false;
            } else {
                keep.push(ty);
            }
        }
        // Types added during this pass (none expected) go first.
        let added: Vec<_> = std::mem::take(&mut self.run_list);
        self.run_list = added.into_iter().chain(keep).collect();
    }

    pub(super) fn draw_beam(&mut self, ty: TypeId, b: u32, nb: u32, sink: &mut dyn Sink) {
        let po = self.parts[self.beams[b as usize].p as usize].org;
        let qo = self.parts[self.beams[nb as usize].p as usize].org;
        self.beams[nb as usize].dir = (qo - po).normalize();
        let (seg, next) = (&self.beams[b as usize], &self.beams[nb as usize]);
        sink.beam(ty, seg, next, &self.parts[seg.p as usize], &self.parts[next.p as usize]);
    }
}
