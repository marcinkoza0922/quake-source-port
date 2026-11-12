use super::*;

impl System {
    /// PScript_ParticleTrail: spawns a trail between two points.
    /// `dlkey` keys any dynamic light (usually the entity number).
    #[allow(clippy::too_many_arguments)]
    pub fn trail(
        &mut self,
        reg: &Registry,
        world: &dyn World,
        start: Vec3,
        end: Vec3,
        ty: TypeId,
        timeinterval: f32,
        dlkey: i32,
        tsk: Option<&mut Option<TrailHandle>>,
    ) -> bool {
        let Some(t) = reg.types.get(ty) else { return false };
        if !t.is_loaded() {
            return false;
        }
        self.ensure_rt(reg.types.len());
        let mut ty = ty;
        if self.settings.contentswitch
            && let Some(iw) = t.inwater
            && world.has_world()
            && self.contents(world, start) & FTECONTENTS_FLUID != 0
        {
            ty = iw;
        }
        self.trail_spawn(reg, world, start, end, ty, timeinterval, dlkey, tsk, 0);
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn trail_spawn(
        &mut self,
        reg: &Registry,
        world: &dyn World,
        start0: Vec3,
        end: Vec3,
        ty: TypeId,
        timeinterval: f32,
        dlkey: i32,
        tsk: Option<&mut Option<TrailHandle>>,
        depth: usize,
    ) {
        let t = &reg.types[ty];
        let tsk = if t.flags & PT_NOSTATE != 0 { None } else { tsk };
        let mut ts = tsk.map(|s| self.trailstate(s));

        self.effect_spawned(t, start0, dlkey);

        if let Some(a) = t.assoc
            && depth < reg.types.len()
        {
            match ts {
                Some(cur) => {
                    let mut slot = self.trails[cur].assoc;
                    self.trail(reg, world, start0, end, a, timeinterval, dlkey, Some(&mut slot));
                    self.trails[cur].assoc = slot;
                }
                None => {
                    self.trail(reg, world, start0, end, a, timeinterval, dlkey, None);
                }
            }
        }

        if self.settings.contentswitch && t.flags & (PT_TRUNDERWATER | PT_TROVERWATER) != 0 && world.has_world() {
            let cont = self.contents(world, start0);
            if (t.flags & PT_TROVERWATER != 0 && cont & t.fluidmask != 0)
                || (t.flags & PT_TRUNDERWATER != 0 && cont & t.fluidmask == 0)
            {
                return;
            }
        }

        if t.spawntime != 0.0
            && let Some(cur) = ts
        {
            if self.trails[cur].state1 > self.particletime {
                return;
            }
            self.trails[cur].state1 = self.particletime + t.spawntime;
            ts = None;
        }

        if t.spawnchance < self.rng.frandom() {
            return;
        }
        if t.die == 0.0 {
            ts = None;
        }

        let mut start = start0;
        let (vec, mut len) = (end - start).normalize_len();
        let mut step;
        let mut count: f32;
        if t.countspacing != 0.0 {
            step = t.countspacing / self.settings.density;
            if t.countextra != 0.0 {
                count = t.countextra;
                if step > 0.0 {
                    count += len / step;
                }
                step = len / count;
            }
        } else {
            let s = t.count * self.settings.density * timeinterval + t.countextra + self.rt[ty].countoverflow;
            count = (s as i32) as f32;
            self.rt[ty].countoverflow = s - count;
            if count <= 0.0 {
                return;
            }
            step = len / count;
        }
        if t.flags & PT_AVERAGETRAIL != 0 {
            let mut tavg = len / step;
            tavg /= tavg.ceil();
            step *= tavg;
            len += step;
        }
        let mut vstep = vec * step;

        let (mut right, mut up) = (Vec3::ZERO, Vec3::ZERO);
        let mut tdegree = 2.0 * PI / 256.0;
        let mut sdegree = 0.0;
        if t.spawnmode == SpawnMode::Spiral {
            (right, up) = vector_vectors(vec);
            if t.spawnparam1 != 0.0 {
                tdegree = 2.0 * PI / t.spawnparam1;
            }
            sdegree = t.spawnparam2 * (PI / 180.0);
        } else if t.spawnmode == SpawnMode::Circle {
            (right, up) = vector_vectors(vec);
        }

        let stop;
        if let Some(cur) = ts {
            stop = self.trails[cur].state2 + len;
            self.trails[cur].state2 = stop;
            len = self.trails[cur].state1;
        } else {
            stop = len;
            len = 0.0;
        }
        let nrfirst = if t.flags & PT_NOSPREADFIRST != 0 { len + step * 1.5 } else { len };
        let nrlast = if t.flags & PT_NOSPREADLAST != 0 { stop } else { stop + step };

        let mut b = NIL;
        let mut bfirst = NIL;
        if len < stop {
            count = (stop - len) / step;
        } else {
            count = 0.0;
            step = 0.0;
            vstep = Vec3::ZERO;
        }
        let veladd = -t.veladd;

        while count > 0.0 {
            count -= 1.0;
            len += step;
            if !self.has_free_particle() {
                len = stop;
                break;
            }
            if t.looks.kind == RenderType::Beam && !self.has_free_beam() {
                len = stop;
                break;
            }
            let p = self.alloc_particle(ty);
            if t.looks.kind == RenderType::Beam {
                let nb = self.alloc_beam();
                if b != NIL {
                    self.beams[b as usize].next = nb;
                } else {
                    bfirst = nb;
                }
                b = nb;
                self.beams[b as usize] = BeamSeg { next: NIL, p, flags: 0, dir: vec, texture_s: len };
            }
            self.init_particle(reg, t, p);
            let tcount =
                if t.spawnparam1 != 0.0 { (len * t.count / t.spawnparam1) as i32 } else { (len * t.count) as i32 };
            let extra = if t.flags & PT_CITRACER != 0 { (tcount & 4) << 1 } else { 0 };
            self.apply_colour(reg, t, p, extra);
            if t.randsmax != 1 {
                let o = t.texsstride * (self.rng.rand() % t.randsmax.max(1) as u32) as f32;
                let pp = &mut self.parts[p as usize];
                pp.s1 += o;
                pp.s2 += o;
                let mut guard = 0;
                while pp.s1 >= 1.0 && guard < 64 {
                    pp.s1 -= 1.0;
                    pp.s2 -= 1.0;
                    pp.t1 += t.texsstride;
                    pp.t2 += t.texsstride;
                    guard += 1;
                }
            }

            let rng = &mut self.rng;
            let (org, vel);
            if len < nrfirst || len >= nrlast {
                vel = vec * veladd;
                org = start;
            } else {
                match t.spawnmode {
                    SpawnMode::Tracer => {
                        let s = if tcount & 1 != 0 { 1.0 } else { -1.0 };
                        let side = Vec3::new(vec.y * s, -vec.x * s, 0.0);
                        vel = Vec3::new(side.x * t.spawnvel + vec.x * veladd, side.y * t.spawnvel + vec.y * veladd, vec.z * veladd);
                        org = Vec3::new(side.x * t.areaspread + start.x, side.y * t.areaspread + start.y, start.z);
                    }
                    SpawnMode::Spiral => {
                        let (tc, tsn) = (pcos(len * tdegree + sdegree), psin(len * tdegree + sdegree));
                        org = start + right * (tc * t.areaspread) + up * (tsn * t.areaspread);
                        vel = vec * veladd + right * (tc * t.spawnvel) + up * (tsn * t.spawnvel);
                    }
                    SpawnMode::Ball => {
                        let o = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom()).normalize() * rng.frandom();
                        vel = vec * veladd + Vec3::new(o.x * t.spawnvel, o.y * t.spawnvel, o.z * t.spawnvelvert);
                        org = Vec3::new(o.x * t.areaspread, o.y * t.areaspread, o.z * t.areaspreadvert) + start;
                    }
                    SpawnMode::Circle => {
                        let (tc, tsn) = (pcos(len * tdegree) * t.areaspread, psin(len * tdegree) * t.areaspread);
                        let a = len * tdegree;
                        org = Vec3::new(
                            start.x + right.x * tc + up.x * tsn + vstep.x * a,
                            start.y + right.y * tc + up.y * tsn + vstep.y * a,
                            start.z + right.z * tc + up.z * tsn + vstep.z * a * 50.0,
                        );
                        let (tc, tsn) = (pcos(len * tdegree) * t.spawnvel, psin(len * tdegree) * t.spawnvel);
                        vel = vec * veladd + right * tc + up * tsn;
                    }
                    SpawnMode::DistBall => {
                        let rdist = t.spawnparam2 - rng.crandom() * (1.0 - rng.crandom() * t.spawnparam1);
                        let o = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom()).normalize() * rdist;
                        vel = vec * veladd + Vec3::new(o.x * t.spawnvel, o.y * t.spawnvel, o.z * t.spawnvelvert);
                        org = Vec3::new(o.x * t.areaspread, o.y * t.areaspread, o.z * t.areaspreadvert) + start;
                    }
                    _ => {
                        let o = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom());
                        vel = vec * veladd + Vec3::new(o.x * t.spawnvel, o.y * t.spawnvel, o.z * t.spawnvelvert);
                        org = Vec3::new(o.x * t.areaspread, o.y * t.areaspread, o.z * t.areaspreadvert) + start;
                    }
                }
            }
            let pp = &mut self.parts[p as usize];
            pp.vel = vel;
            pp.org = if len >= nrfirst && len < nrlast && t.orgadd != 0.0 { org + vec * t.orgadd } else { org };
            self.worldspace_rand(t, p);
            let pp = &mut self.parts[p as usize];
            pp.org += Vec3::from(t.orgbias);

            start += vstep;
            if t.countrand != 0.0 {
                let rstep = self.rng.frandom() / t.countrand;
                start = start.mul_add(rstep, vec);
                step += rstep;
            }
            let pp = &mut self.parts[p as usize];
            pp.die = self.particletime + t.die - pp.die;
            pp.oldorg = pp.org;
        }

        if let Some(cur) = ts {
            self.trails[cur].state1 = len;
            if t.looks.kind == RenderType::Beam {
                if b != NIL {
                    let lb = self.trails[cur].lastbeam;
                    if self.rt[ty].beams != NIL {
                        if lb != NIL {
                            self.beams[b as usize].next = self.beams[lb as usize].next;
                            self.beams[lb as usize].next = bfirst;
                            self.beams[lb as usize].flags &= !BS_LASTSEG;
                        } else {
                            self.beams[b as usize].next = self.rt[ty].beams;
                            self.rt[ty].beams = bfirst;
                        }
                    } else {
                        self.rt[ty].beams = bfirst;
                        self.beams[b as usize].next = NIL;
                    }
                    self.beams[b as usize].flags |= BS_LASTSEG;
                    self.trails[cur].lastbeam = b;
                }
                let lb = self.trails[cur].lastbeam;
                if (!self.has_free_particle() || !self.has_free_beam()) && lb != NIL {
                    self.beams[lb as usize].flags &= !BS_LASTSEG;
                    self.beams[lb as usize].flags |= BS_NODRAW;
                    self.trails[cur].lastbeam = NIL;
                }
            }
        } else if t.looks.kind == RenderType::Beam && b != NIL {
            self.beams[b as usize].flags |= BS_NODRAW;
            self.beams[b as usize].next = self.rt[ty].beams;
            self.rt[ty].beams = bfirst;
        }
        self.add_to_run_list(ty, false);
    }
}
