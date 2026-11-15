use super::*;

impl System {
    pub(super) fn free_beam(&mut self, b: u32) {
        self.free_beams.push(b);
    }

    /// PScript_UpdateParticleTypes minus geometry: decals, draining of
    /// zero-lifetime types, beam bookkeeping, run-list maintenance.
    pub(super) fn update_types(&mut self, reg: &Registry, sink: &mut dyn Sink) {
        let pt = self.particletime;
        let ft = self.frametime;
        for i in 0..self.run_scratch.len() {
            let ty = self.run_scratch[i];
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
                let v = &self.rt[ty].vec;
                if !v.is_empty() {
                    sink.particles_rev(ty, v);
                }
            } else if t.die == 0.0 {
                // Zero-lifetime: draw once, then kill.
                if !self.rt[ty].vec.is_empty() {
                    if draws_points {
                        sink.particles_rev(ty, &self.rt[ty].vec);
                    }
                    if let Some(e) = t.emit
                        && t.emitstart <= 0.0
                    {
                        for p in self.rt[ty].vec.iter().rev() {
                            self.q_effects.push((p.org, p.vel, 1.0, e));
                        }
                    }
                    self.total_particles -= self.rt[ty].vec.len();
                    self.rt[ty].vec.clear();
                }
                let mut i = self.rt[ty].particles;
                let mut arena_killed = 0usize;
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
                    arena_killed += 1;
                    i = next;
                }
                self.total_particles -= arena_killed;
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
            if r.vec.is_empty() && r.particles == NIL && r.beams == NIL && r.decals == NIL {
                r.in_run = false;
            } else {
                self.run_list.push(ty);
            }
        }
    }

    pub(super) fn draw_beam(&mut self, ty: TypeId, b: u32, nb: u32, sink: &mut dyn Sink) {
        let po = self.parts[self.beams[b as usize].p as usize].org;
        let qo = self.parts[self.beams[nb as usize].p as usize].org;
        self.beams[nb as usize].dir = (qo - po).normalize();
        let (seg, next) = (&self.beams[b as usize], &self.beams[nb as usize]);
        sink.beam(ty, seg, next, &self.parts[seg.p as usize], &self.parts[next.p as usize]);
    }
}
