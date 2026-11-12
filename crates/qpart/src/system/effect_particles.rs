use super::*;

impl System {
    /// Spawns the particles of one effect stage. Returns false when the rest
    /// of the effect chain must be skipped (spawn time limit or failed
    /// spawn chance).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn spawn_effect_particles(
        &mut self,
        reg: &Registry,
        ty: TypeId,
        org: Vec3,
        dir: Option<Vec3>,
        count: f32,
        axis: &[Vec3; 3],
        ts: Option<usize>,
    ) -> bool {
        let t = &reg.types[ty];
        let mut spawnspc: i32 = 8;
        let (mut j, mut k, mut l): (i32, i32, i32) = (0, 0, 0);
        let mut m: f32 = 0.0;
        let mut pcount = t.countextra + self.settings.density * count * (t.count + t.countrand * self.rng.frandom());
        if t.flags & PT_INVFRAMETIME != 0 {
            pcount /= self.frametime.max(1e-6);
        }
        if let Some(ts) = ts {
            pcount += self.trails[ts].state2;
        }
        match t.spawnmode {
            SpawnMode::UniCircle => {
                m = pcount;
                if t.looks.kind == RenderType::Beam {
                    m -= 1.0;
                }
                m = if m < 1.0 { 0.0 } else { PI * 2.0 / m };
                if t.spawnparam1 != 0.0 {
                    m *= t.spawnparam1;
                }
            }
            SpawnMode::TeleBox | SpawnMode::LavaSplash => {
                if t.spawnmode == SpawnMode::TeleBox {
                    spawnspc = 4;
                    l = -t.areaspreadvert as i32;
                }
                j = -t.areaspread as i32;
                k = j;
                m = if t.spawnparam1 != 0.0 { t.spawnparam1 } else { 0.55752 };
                if t.spawnparam2 != 0.0 {
                    spawnspc = t.spawnparam2 as i32;
                }
            }
            SpawnMode::Field => {
                if self.avelocities.is_empty() {
                    for _ in 0..NUMVERTEXNORMALS {
                        let a = (self.rng.rand() & 255) as f32 * 0.01;
                        let b = (self.rng.rand() & 255) as f32 * 0.01;
                        self.avelocities.push([a, b]);
                    }
                }
                j = 0;
                m = 0.0;
            }
            _ => {}
        }
        let spawnspc = spawnspc.max(1);

        if t.spawntime != 0.0
            && let Some(ts) = ts
        {
            if self.trails[ts].state1 > self.particletime {
                return false;
            }
            self.trails[ts].state1 = self.particletime + t.spawntime;
        }

        let mut i: i32 = 0;
        // C `break`s out of the whole effect loop here.
        if t.spawnchance < self.rng.frandom() {
            return false;
        }
        {
            if t.die == 0.0 && t.count == 1.0 && t.countrand == 0.0 && pcount < 1.0 {
                pcount = 1.0;
            }
            let mut b = NIL;
            let mut bfirst = NIL;
            let cl_time = self.cl_time as f32;
            let mut ofsvec: Vec3;
            while (i as f32) < pcount {
                if !self.has_free_particle() {
                    break;
                }
                if t.looks.kind == RenderType::Beam && !self.has_free_beam() {
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
                    self.beams[b as usize] = BeamSeg { next: NIL, p, flags: 0, dir: Vec3::ZERO, texture_s: i as f32 };
                }
                self.init_particle(reg, t, p);
                if t.randsmax != 1 {
                    let o = t.texsstride * (self.rng.rand() % t.randsmax.max(1) as u32) as f32;
                    let pp = &mut self.parts[p as usize];
                    pp.s1 += o;
                    pp.s2 += o;
                }
                self.apply_colour(reg, t, p, 0);

                let rng = &mut self.rng;
                let arsvec;
                match t.spawnmode {
                    SpawnMode::Box => {
                        ofsvec = Vec3::new(rng.crandom(), rng.crandom(), rng.crandom());
                        arsvec = Vec3::new(ofsvec.x * t.areaspread, ofsvec.y * t.areaspread, ofsvec.z * t.areaspreadvert);
                    }
                    SpawnMode::TeleBox => {
                        ofsvec = Vec3::new(k as f32, j as f32, (l + 4) as f32).normalize();
                        ofsvec *= 1.0 - rng.frandom() * m;
                        arsvec = Vec3::new(
                            (j + (rng.rand() % spawnspc as u32) as i32) as f32,
                            (k + (rng.rand() % spawnspc as u32) as i32) as f32,
                            (l + (rng.rand() % spawnspc as u32) as i32) as f32,
                        );
                        j += spawnspc;
                        if j as f32 >= t.areaspread {
                            j = -t.areaspread as i32;
                            k += spawnspc;
                            if k as f32 >= t.areaspread {
                                k = -t.areaspread as i32;
                                l += spawnspc;
                                if l as f32 >= t.areaspreadvert {
                                    l = -t.areaspreadvert as i32;
                                }
                            }
                        }
                    }
                    SpawnMode::LavaSplash => {
                        let o = Vec3::new(
                            (k + (rng.rand() % spawnspc as u32) as i32) as f32,
                            (j + (rng.rand() % spawnspc as u32) as i32) as f32,
                            256.0,
                        );
                        arsvec = Vec3::new(o.x, o.y, rng.frandom() * t.areaspreadvert);
                        ofsvec = o.normalize() * (1.0 - rng.frandom() * m);
                        j += spawnspc;
                        if j as f32 >= t.areaspread {
                            j = -t.areaspread as i32;
                            k += spawnspc;
                            if k as f32 >= t.areaspread {
                                k = -t.areaspread as i32;
                            }
                        }
                    }
                    SpawnMode::UniCircle => {
                        ofsvec = Vec3::new(pcos(m * i as f32), psin(m * i as f32), 0.0);
                        arsvec = ofsvec * t.areaspread;
                    }
                    SpawnMode::Field => {
                        let av = self.avelocities[i as usize % NUMVERTEXNORMALS];
                        let a0 = cl_time * av[0] + m;
                        let a1 = cl_time * av[1] + m;
                        let a2 = pcos(a1);
                        ofsvec = Vec3::new(a2 * pcos(a0), a2 * psin(a0), -psin(a1));
                        let orgadd = t.spawnparam2 * psin(cl_time + j as f32 + m);
                        let n = AVERTEXNORMALS[j as usize];
                        arsvec = Vec3::new(
                            n[0] * (t.areaspread + orgadd) + ofsvec.x * t.spawnparam1,
                            n[1] * (t.areaspread + orgadd) + ofsvec.y * t.spawnparam1,
                            n[2] * (t.areaspreadvert + orgadd) + ofsvec.z * t.spawnparam1,
                        );
                        ofsvec = ofsvec.normalize();
                        j += 1;
                        if j as usize >= NUMVERTEXNORMALS {
                            j = 0;
                            m += 0.176_289_1;
                        }
                    }
                    SpawnMode::DistBall => {
                        let rdist = t.spawnparam2 - rng.crandom() * (1.0 - rng.crandom() * t.spawnparam1);
                        let z = if t.areaspreadvert != 0.0 { None } else { Some(0.0) };
                        let x = rng.hrandom();
                        let y = rng.hrandom();
                        let z = z.unwrap_or_else(|| rng.hrandom());
                        ofsvec = Vec3::new(x, y, z).normalize() * rdist;
                        arsvec = Vec3::new(ofsvec.x * t.areaspread, ofsvec.y * t.areaspread, ofsvec.z * t.areaspreadvert);
                    }
                    _ => {
                        let x = rng.hrandom();
                        let y = rng.hrandom();
                        let z = if t.areaspreadvert != 0.0 { rng.hrandom() } else { 0.0 };
                        ofsvec = Vec3::new(x, y, z).normalize();
                        if t.spawnmode != SpawnMode::Circle {
                            ofsvec *= rng.frandom();
                        }
                        arsvec = Vec3::new(ofsvec.x * t.areaspread, ofsvec.y * t.areaspread, ofsvec.z * t.areaspreadvert);
                    }
                }

                let orgadd = t.orgadd + rng.frandom() * t.randomorgadd;
                let mut veladd = t.veladd + rng.frandom() * t.randomveladd;
                if let Some(d) = dir {
                    veladd *= d.length();
                }
                let pp = &mut self.parts[p as usize];
                pp.vel = Vec3::ZERO
                    .mul_add(ofsvec.x * t.spawnvel, axis[0])
                    .mul_add(ofsvec.y * t.spawnvel, axis[1])
                    .mul_add(veladd + ofsvec.z * t.spawnvelvert, axis[2]);
                pp.org = org.mul_add(arsvec.x, axis[0]).mul_add(arsvec.y, axis[1]).mul_add(orgadd + arsvec.z, axis[2]);
                self.worldspace_rand(t, p);
                let pp = &mut self.parts[p as usize];
                pp.org += Vec3::from(t.orgbias);
                pp.die = self.particletime + t.die - pp.die;
                pp.oldorg = pp.org;
                i += 1;
            }

            if t.looks.kind == RenderType::Beam && b != NIL {
                if t.spawnmode == SpawnMode::UniCircle {
                    let ars = Vec3::new(pcos(m * (i - 2) as f32), psin(m * (i - 2) as f32), 0.0);
                    let po = self.parts[self.beams[b as usize].p as usize].org;
                    self.beams[bfirst as usize].dir = (po - ars).normalize();
                }
                self.beams[b as usize].flags |= BS_NODRAW;
                self.beams[b as usize].next = self.rt[ty].beams;
                self.rt[ty].beams = bfirst;
            }
        }

        if let Some(ts) = ts {
            self.trails[ts].state2 = pcount - i as f32;
        }
        if self.rt[ty].particles != NIL || self.rt[ty].decals != NIL {
            self.add_to_run_list(ty, true);
        }
        true
    }

    /// The PT_CDECAL branch of RunParticleEffectState. Returns the
    /// direction used for the decal (C reassigns `dir`), None if no model.
    pub(super) fn spawn_cdecal(&mut self, reg: &Registry, world: &dyn World, ty: TypeId, org: Vec3, dir: Option<Vec3>) -> Option<Vec3> {
        let t = &reg.types[ty];
        let mut center = org;
        let mut entity = 0;
        let bestdir;
        match dir {
            Some(d) if d != Vec3::ZERO => {
                let tr = world.trace_line(org - d, org + d);
                center = tr.endpos;
                bestdir = tr.normal;
                entity = tr.entity;
            }
            _ => {
                let mut best = Vec3::new(0.0, 0.73, 0.73).normalize();
                let mut bestfrac = 1.0;
                for n in 0..6 {
                    let mut e = Vec3::ZERO;
                    e[n % 3] = if n >= 3 { 16.0 } else { -16.0 };
                    let tr = world.trace_line(org - e, org + e);
                    if bestfrac > tr.frac {
                        bestfrac = tr.frac;
                        best = tr.normal;
                        center = tr.endpos;
                        entity = tr.entity;
                    }
                }
                bestdir = best;
            }
        }
        let used_dir = match dir {
            Some(d) if d != Vec3::ZERO => d,
            _ => bestdir,
        };
        if entity != 0 {
            match world.entity_origin(entity) {
                Some(o) => center -= o,
                None => entity = 0,
            }
        }
        if entity == 0 && !world.has_world() {
            return None;
        }
        let normal = (used_dir * -1.0).normalize();
        let vec = Vec3::splat(0.5).normalize();
        let tangent1 = normal.cross(vec);
        let tangent2 = rotate_around(normal, tangent1, self.rng.frandom() * 360.0);
        let tangent1 = normal.cross(tangent2).normalize();
        let tangent2 = tangent2.normalize();
        let m = t.scale + self.rng.frandom() * t.scalerand;
        let scale1 = t.s2 - t.s1;
        let scale2 = t.t2 - t.t1;
        let mut bias1 = t.s1 + scale1 / 2.0;
        if t.randsmax != 1 {
            bias1 += t.texsstride * (self.rng.rand() % t.randsmax.max(1) as u32) as f32;
        }
        let ctx = DecalCtx {
            ty,
            entity,
            center,
            normal,
            tangent1,
            tangent2,
            scale0: 2.0 / m,
            scale1: scale1 / m,
            scale2: scale2 / m,
            bias1,
            bias2: t.t1 + scale2 / 2.0,
        };
        self.clip_decal(reg, world, &ctx, m);
        Some(used_dir)
    }

    /// Mod_ClipDecal + PScript_AddDecals.
    pub(super) fn clip_decal(&mut self, reg: &Registry, world: &dyn World, ctx: &DecalCtx, size: f32) {
        let t = &reg.types[ctx.ty];
        let mut planes = [
            (ctx.tangent1, 0.0),
            (ctx.tangent1 * -1.0, 0.0),
            (ctx.tangent2, 0.0),
            (ctx.tangent2 * -1.0, 0.0),
            (ctx.normal, 0.0),
            (ctx.normal * -1.0, 0.0),
        ];
        let mut radius = 0.0f32;
        for (n, d) in &mut planes {
            let (nn, r) = n.normalize_len();
            *n = nn;
            let r = r * size / 2.0;
            radius = radius.max(r);
            *d = -(r - ctx.center.dot(*n));
        }
        let noperp = self.settings.decal_noperpendicular;
        let mut tris: Vec<[Vec3; 3]> = Vec::new();
        world.decal_polys(ctx.entity, ctx.center, radius, ctx.normal, noperp, &mut |poly| {
            fragment_clip_poly(poly, &planes, ctx.normal, noperp, &mut tris);
        });
        let _ = t;
        self.add_decals(reg, ctx, &tris);
    }

    pub(super) fn add_decals(&mut self, reg: &Registry, ctx: &DecalCtx, tris: &[[Vec3; 3]]) {
        let t = &reg.types[ctx.ty];
        for tri in tris {
            if !self.has_free_decal() {
                break;
            }
            let di = self.alloc_decal(ctx.ty);
            let noperp = self.settings.decal_noperpendicular;
            let die = t.randdie * self.rng.frandom();
            let mut a = if t.die != 0.0 { t.alpha + die * t.alphachange } else { t.alpha };
            a += t.alpharand * self.rng.frandom();
            let mut rgba = [0.0; 4];
            rgba[3] = a;
            if t.colorindex >= 0 {
                let r = if t.colorrand > 0 { (self.rng.rand() % t.colorrand as u32) as i32 } else { 0 };
                let cidx = t.colorindex + r;
                if cidx > 255 {
                    rgba[3] /= 2.0;
                }
                rgba[..3].copy_from_slice(&reg.palette_rgb(cidx as usize));
            } else {
                rgba[..3].copy_from_slice(&t.rgb);
            }
            let v = rgb_rand(&mut self.rng, t);
            for k in 0..3 {
                rgba[k] += v[k] * t.rgbrand[k] + t.rgbchange[k] * die;
            }
            let d = &mut self.decals[di as usize];
            d.vertex = *tri;
            for (i, &v) in tri.iter().enumerate() {
                let vec = v - ctx.center;
                d.texcoords[i] = [vec.dot(ctx.tangent1) * ctx.scale1 + ctx.bias1, vec.dot(ctx.tangent2) * ctx.scale2 + ctx.bias2];
                d.valpha[i] = if noperp { 1.0 } else { 1.0 - (vec.dot(ctx.normal) * ctx.scale0).abs() };
            }
            d.entity = ctx.entity;
            d.rgba = rgba;
            d.die = self.particletime + t.die - die;
            if t.looks.kind != RenderType::CDecal {
                d.die += 20.0;
            }
            self.add_to_run_list(ctx.ty, false);
        }
    }

    /// Wall-splat decal from a particle hit (deferred decal queue).
    pub(super) fn spawn_hit_decal(&mut self, reg: &Registry, world: &dyn World, dd: &DeferredDecal) {
        let t = &reg.types[dd.ty];
        let mut center = dd.center;
        let mut entity = dd.entity;
        if entity != 0 {
            match world.entity_origin(entity) {
                Some(o) => center -= o,
                None => entity = 0,
            }
        }
        let normal = (dd.normal * -1.0).normalize();
        let vec = Vec3::new(0.5, 0.5, 0.431).normalize();
        let tangent1 = normal.cross(vec);
        let tangent2 = rotate_around(normal, tangent1, self.rng.frandom() * 360.0);
        let tangent1 = normal.cross(tangent2).normalize();
        let tangent2 = tangent2.normalize();
        let m = dd.scale * (1.5 + self.rng.frandom() * 0.5) * 0.5;
        let scale1 = t.s2 - t.s1;
        let scale2 = t.t2 - t.t1;
        let ctx = DecalCtx {
            ty: dd.ty,
            entity,
            center,
            normal,
            tangent1,
            tangent2,
            scale0: 2.0 / m,
            scale1: scale1 / m,
            scale2: scale2 / m,
            bias1: t.s1 + scale1 * 0.5,
            bias2: t.t1 + scale2 * 0.5,
        };
        self.clip_decal(reg, world, &ctx, m);
    }
}
