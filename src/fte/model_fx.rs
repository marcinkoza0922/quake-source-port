use super::*;

impl Fte {
    pub(super) fn model_fx_of(&mut self, name: &str, f: &mut dyn qpart::FileSource) -> qpart::ModelEffects {
        if let Some(m) = self.model_fx.get(name) {
            return *m;
        }
        let m = self.reg.model_effects(name, f);
        self.model_fx.insert(name.to_string(), m);
        m
    }

    /// Per model index: drawn by its `replace` emitter instead.
    pub fn replaced_models(&mut self, cl: &ClientState, fs: &qcore::fs::Fs) -> Vec<bool> {
        if self.reg.associated.iter().all(|a| a.flags & qpart::EMIT_REPLACE == 0) {
            return Vec::new();
        }
        let mut f = files(fs);
        (0..cl.model_precache.len())
            .map(|i| {
                cl.model_name(i).is_some_and(|n| {
                    let m = self.model_fx_of(n, &mut f);
                    m.emit.is_some() && m.flags & qpart::EMIT_REPLACE != 0
                })
            })
            .collect()
    }

    /// `r_effect` / `r_trail` effects of visible entities (CL_RelinkEntities).
    /// Rain emitters keyed by BSP submodel: the world at the origin plus
    /// every active inline brush entity with sky triangles (P_AddRainParticles
    /// over cl.entities, with the entity's angles and origin).
    pub(super) fn rain_emitters(&self, cl: &ClientState) -> Vec<qpart::RainEmitter> {
        let mut out = Vec::new();
        if self.sky_models.contains(&0) {
            let (f, r, u) = qcore::math::angle_vectors(Vec3::ZERO);
            out.push(qpart::RainEmitter { model: 0, axis: [f, r, u], origin: Vec3::ZERO });
        }
        if self.sky_models.len() <= usize::from(self.sky_models.contains(&0)) {
            return out;
        }
        for (_, e) in cl.active_entities() {
            let Some(sub) = cl.model_name(e.model).and_then(|n| n.strip_prefix('*')).and_then(|n| n.parse::<usize>().ok())
            else {
                continue;
            };
            if sub != 0 && self.sky_models.contains(&sub) {
                let (f, r, u) = qcore::math::angle_vectors(Vec3::from(e.angles));
                out.push(qpart::RainEmitter { model: sub, axis: [f, r, u], origin: Vec3::from(e.origin) });
            }
        }
        out
    }

    /// Per-entity effects (CL_RelinkEntities / R_StoreEfrags): `r_trail` /
    /// `r_effect` model associations, overridden by the entity's
    /// `.traileffectnum` / `.emiteffectnum` (DP_ENT_TRAILEFFECTNUM). Netstate
    /// trails run in the client's relink. Static entities only emit their
    /// netstate effect.
    pub(super) fn model_effects(&mut self, cl: &ClientState, world: &BspWorld<'_>, f: &mut dyn qpart::FileSource, ft: f32) {
        let net_type = |reg: &mut Registry, f: &mut dyn qpart::FileSource, n: u16| {
            let name = cl.particle_precache.get(n as usize).filter(|s| !s.is_empty())?;
            reg.find(name, f)
        };
        let has_net = cl.entities.iter().any(|e| e.emiteffect > 0 || e.traileffect > 0);
        if self.reg.associated.is_empty() && !has_net && !cl.static_entities.iter().any(|e| e.emiteffect > 0) {
            return;
        }
        for &i in &cl.visedicts {
            let Some(e) = cl.entities.get(i) else { continue };
            let Some(name) = cl.model_name(e.model) else { continue };
            let me = if self.reg.associated.is_empty() { qpart::ModelEffects::default() } else { self.model_fx_of(name, f) };
            let trail = if e.traileffect > 0 { None } else { me.trail };
            let (fwd, _, up) = qcore::math::angle_vectors(e.angles.into());
            // the "stupid vanilla bug": alias models pitch the other way
            let forward = if name.ends_with(".mdl") { Vec3::new(fwd.x, fwd.y, -fwd.z) } else { fwd };
            let emit = match e.emiteffect {
                0 => me.emit.map(|t| (t, if me.flags & qpart::EMIT_FORWARDS != 0 { forward } else { up * -1.0 })),
                n => net_type(&mut self.reg, f, n).map(|t| (t, forward)),
            };
            if trail.is_none() && emit.is_none() {
                continue;
            }
            let key = i as i32;
            let entry = self.ent_fx.entry(key).or_insert((e.origin, None));
            let oldorg = std::mem::replace(&mut entry.0, e.origin);
            if let Some(t) = trail {
                let slot = self.trails.entry(key).or_insert(None);
                self.sys.trail(&self.reg, world, oldorg.into(), e.origin.into(), t, ft, key, Some(slot));
            }
            if let Some((t, dir)) = emit {
                self.emit(world, key, e.origin.into(), dir, ft, t);
            }
        }
        // Static entities, keyed below the dynamic ones.
        let ft = ft.clamp(0.0, 0.1);
        for (k, e) in cl.static_entities.iter().enumerate().filter(|(_, e)| e.emiteffect > 0) {
            let Some(t) = net_type(&mut self.reg, f, e.emiteffect) else { continue };
            let (fwd, _, _) = qcore::math::angle_vectors(e.angles.into());
            let alias = cl.model_name(e.model).is_some_and(|n| n.ends_with(".mdl"));
            let dir = if alias { Vec3::new(fwd.x, fwd.y, -fwd.z) } else { fwd };
            self.emit(world, -2 - k as i32, e.origin.into(), dir, ft, t);
        }
    }

    /// PScript_RunParticleEffectState with the emit state kept per `key`.
    pub(super) fn emit(&mut self, world: &BspWorld<'_>, key: i32, org: Vec3, dir: Vec3, ft: f32, t: qpart::TypeId) {
        let mut slot = self.ent_fx.get(&key).and_then(|x| x.1);
        self.sys.run_effect(&self.reg, world, org, Some(dir), ft, t, Some(&mut slot));
        self.ent_fx.entry(key).or_insert(([org.x, org.y, org.z], None)).1 = slot;
    }
}
