use super::*;

impl System {
    pub(super) fn effect_spawned(&mut self, t: &PartType, org: Vec3, dlkey: i32) {
        if t.dl_radius[0] != 0.0 || t.dl_radius[1] != 0.0 {
            let tick = (self.cl_time * 20.0) as i64;
            if self.flicker.0 != tick {
                self.flicker = (tick, self.rng.rand());
            }
            let f = if self.settings.lightflicker {
                ((self.flicker.1 as i64 + dlkey as i64 * 2000) & 0xffff) as f32 * (1.0 / 65535.0)
            } else {
                0.5
            };
            self.dlights.push(DlightSpawn {
                key: dlkey,
                org,
                radius: t.dl_radius[0] + f * t.dl_radius[1],
                die: self.cl_time + t.dl_time as f64,
                decay: t.dl_decay[3],
                rgb: t.dl_rgb,
            });
        }
        if !t.sounds.is_empty() {
            let total: f32 = t.sounds.iter().map(|s| s.weight).sum();
            let w = self.rng.frandom() * total;
            let mut acc = 0.0;
            for s in &t.sounds {
                acc += s.weight;
                if w <= acc {
                    if !s.name.is_empty() && s.vol > 0.0 {
                        self.sounds.push(SoundSpawn { name: s.name.clone(), org, vol: s.vol, atten: s.atten });
                    }
                    break;
                }
            }
        }
    }

    /// Shared initial state of a new particle (die offset kept in `p.die`).
    pub(super) fn init_particle(&mut self, reg: &Registry, t: &PartType, i: u32) {
        let rng = &mut self.rng;
        let die = t.randdie * rng.frandom();
        let scale = t.scale + t.scalerand * rng.frandom();
        let mut a = if t.die != 0.0 { t.alpha + die * t.alphachange } else { t.alpha };
        a += t.alpharand * rng.frandom();
        let p = &mut self.parts[i as usize];
        p.die = die;
        p.scale = scale;
        p.rgba[3] = a;
        if t.emittime >= 0.0 {
            p.nextemit = self.particletime + t.emitstart - die;
        }
        p.rotationspeed = t.rotationmin + rng.frandom() * t.rotationrand;
        p.angle = t.rotationstartmin + rng.frandom() * t.rotationstartrand;
        p.s1 = t.s1;
        p.t1 = t.t1;
        p.s2 = t.s2;
        p.t2 = t.t2;
        let _ = reg;
    }

    pub(super) fn apply_colour(&mut self, reg: &Registry, t: &PartType, i: u32, extra_cidx: i32) {
        let p = &mut self.parts[i as usize];
        if t.colorindex >= 0 {
            let r = if t.colorrand > 0 { (self.rng.rand() % t.colorrand as u32) as i32 } else { 0 };
            let cidx = t.colorindex + r + extra_cidx;
            if cidx > 255 {
                p.rgba[3] /= 2.0;
            }
            let c = reg.palette_rgb(cidx as usize);
            p.rgba[..3].copy_from_slice(&c);
        } else {
            p.rgba[..3].copy_from_slice(&t.rgb);
        }
        let v = rgb_rand(&mut self.rng, t);
        let die = p.die;
        for (k, c) in p.rgba[..3].iter_mut().enumerate() {
            *c += v[k] * t.rgbrand[k] + t.rgbchange[k] * die;
        }
    }

    pub(super) fn worldspace_rand(&mut self, t: &PartType, i: u32) {
        if t.flags & PT_WORLDSPACERAND == 0 {
            return;
        }
        let mut v;
        loop {
            v = Vec3::new(self.rng.crand(), self.rng.crand(), self.rng.crand());
            if v.dot(v) <= 1.0 {
                break;
            }
        }
        let p = &mut self.parts[i as usize];
        p.org += v.mul_elem(Vec3::from(t.orgwrand));
        p.vel += v.mul_elem(Vec3::from(t.velwrand));
        p.vel += Vec3::from(t.velbias);
    }

    /// PScript_RunParticleEffectState. Returns false if the type is invalid
    /// or not loaded (the C return value 1).
    #[allow(clippy::too_many_arguments)]
    pub fn run_effect(
        &mut self,
        reg: &Registry,
        world: &dyn World,
        org: Vec3,
        dir: Option<Vec3>,
        count: f32,
        ty: TypeId,
        tsk: Option<&mut Option<TrailHandle>>,
    ) -> bool {
        let Some(t0) = reg.types.get(ty) else { return false };
        if !t0.is_loaded() {
            return false;
        }
        self.ensure_rt(reg.types.len());
        let mut ty = ty;
        if self.settings.contentswitch
            && let Some(iw) = t0.inwater
            && world.has_world()
            && self.contents(world, org) & FTECONTENTS_FLUID != 0
        {
            ty = iw;
        }
        let mut tsk = if reg.types[ty].flags & PT_NOSTATE != 0 { None } else { tsk };
        let mut ts = tsk.as_mut().map(|s| self.trailstate(s));
        let mut axis = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, -1.0)];
        let mut dir = dir;
        let mut guard = 0;

        loop {
            guard += 1;
            if guard > reg.types.len() + 1 {
                break;
            }
            let t = &reg.types[ty];
            let mut skip = false;
            if self.settings.contentswitch && t.flags & (PT_TRUNDERWATER | PT_TROVERWATER) != 0 && world.has_world() {
                let cont = self.contents(world, org);
                if (t.flags & PT_TROVERWATER != 0 && cont & t.fluidmask != 0)
                    || (t.flags & PT_TRUNDERWATER != 0 && cont & t.fluidmask == 0)
                {
                    skip = true;
                }
            }
            if !skip {
                if let Some(d) = dir
                    && d != Vec3::ZERO
                {
                    axis[2] = d.normalize();
                    axis[0] = perpendicular(axis[2]).normalize();
                    axis[1] = axis[2].cross(axis[0]).normalize();
                }
                self.effect_spawned(t, org, 0);

                if t.looks.kind == RenderType::CDecal {
                    if !self.has_free_decal() {
                        return true;
                    }
                    let d = self.spawn_cdecal(reg, world, ty, org, dir);
                    if d.is_none() {
                        return true;
                    }
                    dir = d;
                    match t.assoc {
                        Some(a) => {
                            ty = a;
                            continue;
                        }
                        None => break,
                    }
                }
                if !self.spawn_effect_particles(reg, ty, org, dir, count, &axis, ts) {
                    return true;
                }
            }
            // next associated effect
            let Some(a) = t.assoc else { break };
            if let Some(cur) = ts {
                let mut slot = self.trails[cur].assoc;
                let n = self.trailstate(&mut slot);
                self.trails[cur].assoc = slot;
                ts = Some(n);
            }
            ty = a;
        }
        true
    }
}
