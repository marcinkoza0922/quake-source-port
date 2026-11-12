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
        let bsp_ext = name.to_ascii_lowercase().ends_with(".bsp");
        let data = if cached.is_some() || !bsp_ext { None } else { self.c.fs.open(name) };
        let is_brush = cached.is_some()
            || data.as_deref().is_some_and(|d| !matches!(d.get(..4), Some(b"IDPO" | b"IDSP" | b"IDP3")));
        if is_brush {
            let cm = match cached {
                Some(cm) => cm,
                None => {
                    let data = data.unwrap_or_default();
                    let bsp = qformats::Bsp::parse(&data)
                        .map_err(|e| VmError::new(format!("Mod_LoadModel: {name}: {e:?}")))?;
                    let cm = Arc::new(CollisionModel::from_bsp(&bsp));
                    self.c.sv.brush_cache.insert(name.to_string(), cm.clone());
                    cm
                }
            };
            let (mins, maxs) = cm.submodels.first().map_or((Vec3::ZERO, Vec3::ZERO), |m| (m.mins, m.maxs));
            self.w.set_model(index, Some((cm, 0)));
            return Ok(Some(ModelInfo { name: name.into(), brush: true, mins, maxs }));
        }
        if if bsp_ext { data.is_none() } else { !self.c.fs.exists(name) } {
            return Err(vm.run_error(format!("Mod_LoadModel: {name} not found")));
        }
        Ok(Some(ModelInfo { name: name.into(), brush: false, mins: Vec3::splat(-16.0), maxs: Vec3::splat(16.0) }))
    }

    pub(super) fn precache_model(&mut self, vm: &mut Vm) -> Result<()> {
        if !self.c.sv.loading {
            return Err(vm.run_error("PF_Precache_*: Precache can only be done in spawn functions"));
        }
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
        let info = self.load_model(vm, &s, n)?;
        self.c.sv.model_precache.push(s);
        self.c.sv.models.push(info);
        Ok(())
    }

    pub(super) fn precache_sound(&mut self, vm: &mut Vm) -> Result<()> {
        if !self.c.sv.loading {
            return Err(vm.run_error("PF_Precache_*: Precache can only be done in spawn functions"));
        }
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
        self.c.sv.sound_precache.push(s);
        Ok(())
    }

    pub(super) fn setmodel(&mut self, vm: &mut Vm) -> Result<()> {
        let e = ent_arg(vm, 0)?;
        let m = str_arg(vm, 1)?;
        let Some(i) = self.c.sv.model_precache.iter().position(|s| s.as_bytes() == m.as_slice()) else {
            return Err(vm.run_error(format!("no precache: {}", lossy(&m))));
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
