use super::*;

impl Ctx<'_> {
    /// Mod_ForName(name, true) for a precache.
    pub(super) fn load_model(&mut self, vm: &Vm, name: &str, index: usize) -> Result<Option<ModelInfo>> {
        if name.starts_with('*') {
            return Ok(None);
        }
        // Mod_LoadModel dispatches on the file magic, not the extension
        // (some mods ship alias models named b_*.bsp).
        let cached = self.c.sv.brush_cache.get(name).cloned();
        let bsp_ext = name.trim_end_matches(' ').to_ascii_lowercase().ends_with(".bsp");
        let data = if cached.is_some() || !bsp_ext { None } else { self.c.fs.open(name) };
        let is_brush = cached.is_some()
            || data.as_deref().is_some_and(|d| {
                !matches!(d.get(..4), Some(b"IDPO" | b"IDSP" | b"IDP3"))
                    && d.get(..16) != Some(qformats::iqm::IQM_MAGIC.as_slice())
            });
        if is_brush {
            let (cm, sm) = match cached {
                Some(cm) => {
                    let sm = self.c.sv.surface_cache.get(name).cloned();
                    (cm, sm)
                }
                None => {
                    let data = data.unwrap_or_default();
                    let bsp = qformats::Bsp::parse(&data)
                        .map_err(|e| VmError::new(format!("Mod_LoadModel: {name}: {e:?}")))?;
                    let cm = Arc::new(CollisionModel::from_bsp(&bsp));
                    let sm = Arc::new(crate::surface::SurfaceModel::from_bsp(&bsp));
                    self.c.sv.brush_cache.insert(name.to_string(), cm.clone());
                    self.c.sv.surface_cache.insert(name.to_string(), sm.clone());
                    (cm, Some(sm))
                }
            };
            let (mins, maxs) = cm.submodels.first().map_or((Vec3::ZERO, Vec3::ZERO), |m| (m.mins, m.maxs));
            self.w.set_model(index, Some((cm, 0)));
            if self.c.sv.surface_models.len() <= index {
                self.c.sv.surface_models.resize(index + 1, None);
            }
            self.c.sv.surface_models[index] = sm.map(|s| (s, 0));
            return Ok(Some(ModelInfo { name: name.into(), brush: true, mins, maxs }));
        }
        if if bsp_ext { data.is_none() } else { !self.c.fs.exists(name) } {
            return Err(vm.run_error(format!("Mod_LoadModel: {name} not found")));
        }
        let (mins, maxs) = self.alias_bounds(name);
        Ok(Some(ModelInfo { name: name.into(), brush: false, mins, maxs }))
    }

    /// `mod->mins`/`maxs` of a non-brush model, which PF_setmodel gives
    /// entities. Like ironwail and vkQuake (`sv_gameplayfix_setmodelrealbox`
    /// 1), not vanilla's fixed ±16 box: an MDL's bounds over every pose
    /// (Mod_CalcAliasBounds), an MD3's over every frame, a sprite's
    /// ±width/2, ±height/2 (Mod_LoadSpriteModel, integer halves). An MDL
    /// with an `.md3`/`.md5mesh` replacement keeps the MDL's bounds, as in
    /// vkQuake, which loads the MDL last; ironwail takes the replacement's
    /// bind pose when `r_enhancedmodels` loads one.
    fn alias_bounds(&self, name: &str) -> (Vec3, Vec3) {
        let fallback = (Vec3::splat(-16.0), Vec3::splat(16.0));
        let Some(data) = self.c.fs.open(name) else { return fallback };
        match data.get(..4) {
            Some(b"IDPO") => {
                let Ok(mdl) = qformats::mdl::Mdl::parse(&data) else { return fallback };
                union(mdl.frames.iter().flat_map(|f| f.poses.iter()).flat_map(|p| p.verts.iter()).map(|&v| Vec3::from(mdl.decode(v)))).unwrap_or(fallback)
            }
            Some(b"IDP3") => qformats::md3::Md3::parse(&data)
                .ok()
                .and_then(|m| union(m.surfaces.iter().flat_map(|s| s.verts.iter()).map(|v| Vec3::from(v.xyz.map(|x| x as f32 * qformats::md3::XYZ_SCALE)))))
                .unwrap_or(fallback),
            Some(b"IDSP") => match qformats::spr::Spr::parse(&data) {
                Ok(s) => {
                    let (w, h) = ((s.width / 2) as f32, (s.height / 2) as f32);
                    (Vec3::new(-w, -w, -h), Vec3::new(w, w, h))
                }
                Err(_) => fallback,
            },
            _ if data.get(..16) == Some(qformats::iqm::IQM_MAGIC.as_slice()) => {
                match qformats::Iqm::parse(&data) {
                    Ok(iqm) => {
                        let (lo, hi, _) = iqm.compute_bounds();
                        (Vec3::from(lo), Vec3::from(hi))
                    }
                    Err(_) => fallback,
                }
            }
            _ => fallback,
        }
    }

    /// A precache outside spawn functions: an error in ironwail, a warning
    /// with extensions on (vkQuake), where clients are told of the new name.
    fn late_precache_ok(&mut self, vm: &Vm, what: &str, name: &str) -> Result<()> {
        if !self.extensions_enabled() {
            return Err(vm.run_error("PF_Precache_*: Precache can only be done in spawn functions"));
        }
        self.c.print(&format!("PF_precache_{what}(\"{name}\"): Precache should only be done in spawn functions\n"));
        Ok(())
    }

    /// svcdp_precache to the clients that know it.
    fn announce_precache(&mut self, code: usize, name: &str) {
        for cl in self.c.clients.iter_mut().filter(|c| c.active && (c.local || c.delta.is_some())) {
            cl.message.byte(crate::particles::SVCDP_PRECACHE);
            cl.message.short(code as i32);
            cl.message.string(name.as_bytes());
        }
    }

    /// SV_Precache_Model (vkQuake): adds a model after loading, or one QC sets
    /// without a precache. A missing file leaves an empty model.
    pub(crate) fn late_precache_model(&mut self, vm: &Vm, name: String) -> Result<usize> {
        let n = self.c.sv.model_precache.len();
        if n >= MAX_MODELS {
            return Err(vm.run_error("PF_precache_model: overflow"));
        }
        let info = match self.load_model(vm, &name, n) {
            Ok(info) => info,
            Err(_) => {
                self.c.print(&format!("Mod_LoadModel: {name} not found\n"));
                None
            }
        };
        if !self.c.sv.loading {
            self.announce_precache(n, &name);
        }
        self.c.sv.model_precache.push(name);
        self.c.sv.models.push(info);
        Ok(n)
    }

    pub(super) fn precache_model(&mut self, vm: &mut Vm) -> Result<()> {
        let h = vm.g_i(p(0));
        vm.ret_i(h);
        let s = str_arg(vm, 0)?;
        if s.first().is_none_or(|&c| c <= b' ') {
            return Err(vm.run_error("Bad string"));
        }
        let s = lossy(&s);
        if self.c.sv.model_precache.contains(&s) {
            return Ok(());
        }
        let n = self.c.sv.model_precache.len();
        if n >= MAX_MODELS {
            return Err(vm.run_error("PF_precache_model: overflow"));
        }
        if !self.c.sv.loading {
            self.late_precache_ok(vm, "model", &s)?;
            return self.late_precache_model(vm, s).map(|_| ());
        }
        let info = self.load_model(vm, &s, n)?;
        self.c.sv.model_precache.push(s);
        self.c.sv.models.push(info);
        Ok(())
    }

    pub(super) fn precache_sound(&mut self, vm: &mut Vm) -> Result<()> {
        let h = vm.g_i(p(0));
        vm.ret_i(h);
        let s = str_arg(vm, 0)?;
        if s.first().is_none_or(|&c| c <= b' ') {
            return Err(vm.run_error("Bad string"));
        }
        let s = lossy(&s);
        if self.c.sv.sound_precache.contains(&s) {
            return Ok(());
        }
        if self.c.sv.sound_precache.len() >= MAX_SOUNDS {
            return Err(vm.run_error("PF_precache_sound: overflow"));
        }
        if !self.c.sv.loading {
            self.late_precache_ok(vm, "sound", &s)?;
            self.announce_precache(self.c.sv.sound_precache.len() | 0x8000, &s);
        }
        self.c.sv.sound_precache.push(s);
        Ok(())
    }

    pub(super) fn setmodel(&mut self, vm: &mut Vm) -> Result<()> {
        let e = ent_arg(vm, 0)?;
        let m = str_arg(vm, 1)?;
        let i = match self.c.sv.model_precache.iter().position(|s| s.as_bytes() == m.as_slice()) {
            Some(i) => i,
            None if self.extensions_enabled() => {
                let name = lossy(&m);
                self.c.print(&format!("PF_setmodel(\"{name}\"): Model was not precached\n"));
                self.late_precache_model(vm, name)?
            }
            None => return Err(vm.run_error(format!("no precache: {}", lossy(&m)))),
        };
        let h = vm.strings.engine(&m);
        let f = &self.c.sv.fields;
        let (fm, fmi) = (f.model, f.modelindex);
        vm.set_ent_i(e, fm, h);
        vm.set_ent_f(e, fmi, i as f32);
        let (mn, mx) = match self.c.sv.models.get(i).and_then(|m| m.as_ref()) {
            Some(m) => (m.mins.to_array(), m.maxs.to_array()),
            None => ([0.0; 3], [0.0; 3]),
        };
        self.set_min_max_size(vm, e, mn, mx)
    }
}

/// Bounding box of `pts`, or None when empty.
fn union(pts: impl Iterator<Item = Vec3>) -> Option<(Vec3, Vec3)> {
    pts.fold(None, |acc, v| Some(acc.map_or((v, v), |(lo, hi): (Vec3, Vec3)| (lo.min(v), hi.max(v)))))
}
