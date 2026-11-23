use super::*;
use crate::view::ViewCvars;
use qrender::MapperSettings;

pub(crate) type RenderCvarsKey = (u64, bool, bool, u32);

/// Per-frame host cvars cached under [`Cvars::generation`], avoiding dozens of
/// hash lookups in `Host::render` when no cvar changed.
#[derive(Clone, Copy)]
pub(crate) struct RenderCvars {
    pub(crate) con_notify_time: f64,
    pub(crate) con_modern: bool,
    pub(crate) con_alpha: f32,
    pub(crate) mapper: MapperSettings,
    pub(crate) view: ViewCvars,
    pub(crate) third_person: bool,
    pub(crate) reflections_on: bool,
    pub(crate) vwep_enabled: bool,
    pub(crate) debris_on: bool,
    pub(crate) debris_set: crate::debris::Settings,
    pub(crate) gravity: f32,
    pub(crate) blood: crate::blood::Settings,
    pub(crate) impacts_on: bool,
    pub(crate) track_shooters_cvars: bool,
    pub(crate) teleport_windows: bool,
    pub(crate) render_scale: f32,
    pub(crate) draw_entities: bool,
    pub(crate) particles_on: bool,
    pub(crate) cable_metal: bool,
    pub(crate) gel_quality: u32,
    pub(crate) washer_quality: u32,
    pub(crate) bullet_meter: bool,
    pub(crate) teamplay: f32,
    pub(crate) captions: bool,
    pub(crate) viewsize: f32,
    pub(crate) damage_indicator: bool,
    pub(crate) flash_scale: f32,
    pub(crate) deathmatch: bool,
    pub(crate) fog_on: bool,
    pub(crate) portal_view: bool,
    pub(crate) showspeed_ofs: f32,
    pub(crate) printspeed: f32,
}

impl Host {
    pub(super) fn render_cvars(&mut self) -> RenderCvars {
        let key: RenderCvarsKey = (
            self.server.core.cvars.generation(),
            self.mapper_allowed(),
            self.freecam.cam.is_some(),
            self.zoom.0.to_bits(),
        );
        if let Some((k, cached)) = self.render_cvars_cache
            && k == key
        {
            return cached;
        }
        let v = |n: &str| self.cv(n);
        let gravity = v("sv_gravity");
        let gravity = if gravity > 0.0 { gravity } else { 800.0 };
        let impacts_on = crate::impacts::impacts_on(&self.server.core.cvars);
        let teleport_windows = v("r_teleportwindows") != 0.0;
        let blood = crate::blood::Settings::read(&self.server.core.cvars);
        let track_shooters_cvars = blood.enabled
            || (impacts_on
                && (v("r_impact_nails") != 0.0
                    || v("r_impact_bullets") != 0.0
                    || v("r_impact_scorches") != 0.0
                    || teleport_windows));
        let rc = RenderCvars {
            con_notify_time: v("con_notifytime").max(0.0) as f64,
            con_modern: v("con_modern") != 0.0,
            con_alpha: v("scr_conalpha"),
            mapper: self.mapper_settings(),
            view: self.view_cvars(),
            third_person: self.camera().third_person(),
            reflections_on: self.reflections_on(),
            vwep_enabled: v("cl_vwep") != 0.0,
            debris_on: v("r_debris") != 0.0,
            debris_set: crate::debris::Settings {
                quality: crate::debris::Quality::from_cvar(v("r_debris_quality")),
                ragdolls: false, // feature disabled (hidden from menus)
                start: crate::debris::Start::from_cvar(v("r_debris_ragdollstart")),
                max_ragdolls: v("r_debris_maxragdolls").clamp(1.0, 128.0) as usize,
                gibs: v("r_debris_gibs") != 0.0,
                shrapnel: v("r_debris_shrapnel") != 0.0,
                movers: v("r_debris_movers") != 0.0,
                force: v("r_debris_force").clamp(0.0, 6.0),
                kill_lift: false,
            },
            gravity,
            blood,
            impacts_on,
            track_shooters_cvars,
            teleport_windows,
            render_scale: v("r_renderscale"),
            draw_entities: v("r_drawentities") != 0.0,
            particles_on: v("r_particles") != 0.0,
            cable_metal: v("addon_cable_metal") != 0.0,
            gel_quality: (v("addon_gel_quality").round() as i32).clamp(0, 1) as u32,
            washer_quality: (v("addon_washer_quality").round() as i32).clamp(0, 1) as u32,
            bullet_meter: v("addon_bullet_time_meter") != 0.0,
            teamplay: v("teamplay"),
            captions: v("cl_captions") != 0.0,
            viewsize: v("viewsize"),
            damage_indicator: v("hud_damageindicator") != 0.0,
            flash_scale: v("v_flashscale"),
            deathmatch: v("deathmatch") != 0.0,
            fog_on: v("r_fog") != 0.0,
            portal_view: v("addon_portal_view") != 0.0,
            showspeed_ofs: v("scr_showspeed_ofs"),
            printspeed: v("scr_printspeed"),
        };
        self.render_cvars_cache = Some((key, rc));
        rc
    }
}
