use super::*;

impl System {
    /// PScript_RunParticleEffect: classic `particle` messages via `pe_<color>`
    /// or the PE_* fallbacks. Returns false if nothing handled it.
    #[allow(clippy::too_many_arguments)]
    pub fn run_classic_effect(
        &mut self,
        reg: &mut Registry,
        files: &mut dyn FileSource,
        world: &dyn World,
        org: Vec3,
        dir: Vec3,
        color: i32,
        count: i32,
    ) -> bool {
        if let Some(t) = reg.find(&format!("pe_{color}"), files)
            && self.run_effect(reg, world, org, Some(dir), count as f32, t, None)
        {
            return true;
        }
        let pick = if count > 130 && self.pe.size3.is_some() {
            self.pe.size3
        } else if count > 20 && self.pe.size2.is_some() {
            self.pe.size2
        } else {
            self.pe.default
        };
        let Some(t) = pick else { return false };
        reg.types[t].colorindex = color & !7;
        reg.types[t].colorrand = 8;
        self.run_effect(reg, world, org, Some(dir), count as f32, t, None)
    }

    /// PScript_RunParticleWeather (te_particlerain / te_particlesnow).
    #[allow(clippy::too_many_arguments)]
    pub fn run_weather(
        &mut self,
        reg: &mut Registry,
        files: &mut dyn FileSource,
        world: &dyn World,
        minb: Vec3,
        maxb: Vec3,
        dir: Vec3,
        count: f32,
        colour: i32,
        efname: &str,
    ) {
        let t = match reg.find(&format!("te_{efname}_{colour}"), files) {
            Some(t) => t,
            None => {
                let Some(t) = reg.find(&format!("te_{efname}"), files).or(self.pe.default) else { return };
                reg.types[t].colorindex = colour;
                t
            }
        };
        let pc = reg.types[t].count;
        let invcount = 1.0 / pc;
        let count = count * pc;
        let mut i = 0.0;
        while i < count {
            if !self.has_free_particle() {
                return;
            }
            let mut org = Vec3::ZERO;
            for j in 0..3 {
                let num = self.rng.frandom();
                org[j] = minb[j] + num * (maxb[j] - minb[j]);
            }
            self.run_effect(reg, world, org, Some(dir), invcount, t, None);
            i += 1.0;
        }
    }

    // ---- update ----

    /// P_AddRainParticles for one emitter.
    pub(super) fn add_rain(&mut self, reg: &Registry, world: &dyn World, em: &RainEmitter, vieworg: Vec3) {
        if self.settings.rain_quantity == 0.0 {
            return;
        }
        let Some(mut sm) = self.sky.remove(&em.model) else { return };
        sm.skytime += self.frametime as f64;
        'tris: for st in &mut sm.tris {
            let Some(t) = reg.types.get(st.ptype) else { continue };
            if !t.is_loaded() {
                continue;
            }
            let mut guard = 0;
            while st.nexttime < sm.skytime {
                if !self.has_free_particle() || guard > 4096 {
                    break 'tris;
                }
                guard += 1;
                st.nexttime += 10000.0 / (st.area * self.settings.rain_quantity * t.rainfrequency) as f64;
                let x = self.rng.frandom() * self.rng.frandom();
                let y = self.rng.frandom() * (1.0 - x);
                let o = st.org.mul_add(x, st.x).mul_add(y, st.y);
                let a = &em.axis;
                let worg = Vec3::new(o.dot(a[0]), -o.dot(a[1]), o.dot(a[2])) + em.origin;
                if (worg - vieworg).length() > (1024.0 + 512.0) * self.rng.frandom() {
                    continue;
                }
                let n = st.normal;
                let wnorm = Vec3::new(n.dot(a[0]), -n.dot(a[1]), n.dot(a[2]));
                let worg = worg.mul_add(0.5, wnorm);
                if world.point_contents(worg) & FTECONTENTS_SOLID == 0 {
                    self.run_effect(reg, world, worg, Some(wnorm), 1.0, st.ptype, None);
                }
            }
        }
        self.sky.insert(em.model, sm);
    }
}
