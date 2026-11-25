//! Running out of video memory: nothing half-made is left behind, and scenes
//! that don't fit are drawn with less.
use super::*;

/// The settings (and the output size) that decide how much memory a scene takes.
pub(super) type MemoryKey = [u32; 16];

pub(super) fn memory_key(st: &Settings, (w, h): (u32, u32)) -> MemoryKey {
    let (lights, shadow) = crate::rtlight::shadow_budget(st);
    [
        w,
        h,
        st.msaa,
        st.render_scale.to_bits(),
        st.supersample.into(),
        (st.ssao > 0.0).into(),
        st.ssao_quality,
        st.portal_quality,
        st.motion_blur_quality,
        (st.bloom > 0.0).into(),
        st.bloom_quality,
        lights as u32,
        shadow,
        st.water_reflections,
        st.shader_scale,
        st.teleport_windows.into(),
    ]
}

/// `st` with everything off that only costs memory for looks.
pub(super) fn reduced(st: &Settings) -> Settings {
    Settings {
        msaa: 1,
        render_scale: st.render_scale.min(1.0),
        supersample: false,
        ssao: 0.0,
        ssao_quality: 0,
        portal_quality: 0,
        motion_blur_quality: 0,
        bloom: 0.0,
        dlight_shadows: false,
        water_reflections: 0,
        shader_scale: 0,
        ..*st
    }
}

impl Renderer {
    /// Runs `f`; when it fails, frees the textures, models, pictures and GPU
    /// objects it made. `f` must not destroy or replace what was there before.
    pub(crate) fn all_or_nothing<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        let before: Vec<bool> = self.textures.iter().map(Option::is_some).collect();
        let (models, pics, poses) = (self.models.len(), self.pics.len(), self.alias_cpu.len());
        let mark = self.gpu.begin_scope();
        let r = f(self);
        if r.is_err() {
            self.idle();
        }
        let gone = self.gpu.end_scope(mark, r.is_err());
        if r.is_err() {
            for (i, t) in self.textures.iter_mut().enumerate() {
                let image = t.as_ref().and_then(|t| t.image.as_ref()).map(|i| i.image);
                if !before.get(i).copied().unwrap_or(false) || image.is_some_and(|i| gone.contains(&i)) {
                    *t = None;
                }
            }
            self.models.truncate(models);
            self.pics.truncate(pics);
            self.alias_cpu.truncate(poses);
        }
        r
    }

    /// Creates what drawing `fr` needs, with [`reduced`] settings when the
    /// player's don't fit. Returns the frame to draw.
    pub(super) fn prepare<'a>(&mut self, fr: &RenderFrame<'a>) -> Result<RenderFrame<'a>> {
        let size = (self.width, self.height);
        let key = memory_key(&fr.settings, size);
        let less = RenderFrame { settings: reduced(&fr.settings), ..*fr };
        if self.reduced_for != Some(key) {
            self.reduced_for = None;
            match self.prepare_frame(fr) {
                Ok(()) => return Ok(*fr),
                Err(e) if e.lost.is_none() && memory_key(&less.settings, size) != key => {
                    self.reduced_for = Some(key);
                    self.notice = Some(Error::new(format!("{e}: not enough for these graphics settings, drawing with less")));
                }
                Err(e) => return Err(e),
            }
        }
        self.prepare_frame(&less)?;
        Ok(less)
    }

    fn prepare_frame(&mut self, fr: &RenderFrame) -> Result<()> {
        self.apply_settings(&fr.settings)?;
        // r_water_reflections: 1 planar (portal views), 2 ray traced
        let refl_mode = self.reflection_mode(&fr.settings);
        if self.ray_traced(fr) {
            self.ensure_rt()?;
        }
        self.gun_targets |= fr.gun_portals.iter().any(|g| g.live && g.exit.is_some()) || refl_mode == 1;
        let portals = self.gun_targets || self.world.as_ref().is_some_and(|w| !w.portals.is_empty());
        if self.targets.as_ref().is_none_or(|t| t.portals.is_empty() && portals) {
            self.create_targets()?;
        }
        if self.ray_traced(fr) {
            self.ensure_rt_refl()?;
        }
        if fr.settings.ssao > 0.0 && (fr.view.is_some() || !fr.splits.is_empty()) {
            self.ensure_ao_source()?;
        }
        if fr.view.is_some_and(|v| v.water_screen.drops_age() >= 0.0)
            || fr.splits.iter().any(|s| s.view.water_screen.drops_age() >= 0.0)
        {
            self.ensure_droplets()?;
        }
        self.prepare_shader(fr.settings.shader_scale != 0)
    }

    /// This frame traces water reflections.
    pub(super) fn ray_traced(&self, fr: &RenderFrame) -> bool {
        self.reflection_mode(&fr.settings) == 2
            && fr.settings.goggles.mode != 2
            && (fr.view.is_some() || !fr.splits.is_empty())
    }
}
