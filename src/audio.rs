//! Host audio enhancements:
//! - Room-probing environmental reverb (`s_reverb`, `s_reverb_amount`)
//! - Multi-ray BSP sound occlusion (`s_occlusion`)
//! - 3D binaural spatialization (`s_hrtf`) & underwater muffling (`snd_waterfx`)
//! - Surface-aware footsteps (`s_footsteps`, `s_footsteps_volume`)
//! - Per-category volume mixing (`s_vol_ambient`, `s_vol_weapons`, `s_vol_monsters`, `s_vol_world`)
//! - Soundtrack switcher & music crossfade (`s_soundtrack`, `s_music_crossfade`)

pub mod acoustic;
pub mod footsteps;
pub mod soundtrack;

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::Vec3;
use qsound::{CategoryVolumes, ReverbParams, SoundSystem};

use crate::assets::Level;
use crate::fte::world::BrushEnt;
use crate::splitscreen::Guest;
use qclient::ClientState;

/// Host-owned state for enhanced audio features.
#[derive(Default)]
pub struct AudioFx {
    pub acoustic: acoustic::AcousticState,
    pub footsteps: footsteps::Footsteps,
    pub soundtracks: soundtrack::SoundtrackManager,
    last_cats: Option<CategoryVolumes>,
    last_hrtf: Option<bool>,
    last_occlusion: Option<bool>,
    last_water_fx: Option<bool>,
    last_crossfade: Option<f32>,
    last_reverb_on: Option<bool>,
    last_soundtrack: Option<String>,
}

impl AudioFx {
    /// Discover installed and custom user soundtrack packs.
    pub fn refresh_soundtracks(&mut self, fs: &Fs) {
        self.soundtracks.refresh(fs, &crate::userdata::root());
    }

    /// Reset per-map state on level load or disconnect.
    pub fn clear_map(&mut self) {
        self.acoustic.clear_map();
        self.footsteps.clear_map();
    }

    /// Sync audio cvars, run room reverb probe, occlusion updates and footsteps.
    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        cv: &Cvars,
        fs: &Fs,
        cl: &ClientState,
        guests: &[Guest],
        level: &Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        view: Option<&qrender::View>,
        underwater: bool,
        realtime: f64,
        paused: bool,
        snd: &mut SoundSystem,
    ) {
        // Per-category volume mix.
        let cats = CategoryVolumes {
            ambient: cv.value_of("s_vol_ambient").clamp(0.0, 1.0),
            weapons: cv.value_of("s_vol_weapons").clamp(0.0, 1.0),
            monsters: cv.value_of("s_vol_monsters").clamp(0.0, 1.0),
            world: cv.value_of("s_vol_world").clamp(0.0, 1.0),
        };
        if self.last_cats != Some(cats) {
            snd.set_category_volumes(cats);
            self.last_cats = Some(cats);
        }

        let hrtf = cv.value_of("s_hrtf") != 0.0;
        if self.last_hrtf != Some(hrtf) {
            snd.set_hrtf(hrtf);
            self.last_hrtf = Some(hrtf);
        }

        let occlusion = cv.value_of("s_occlusion") != 0.0;
        if self.last_occlusion != Some(occlusion) {
            snd.set_occlusion(occlusion);
            self.last_occlusion = Some(occlusion);
        }

        let water_fx = cv.value_of("snd_waterfx") != 0.0;
        if self.last_water_fx != Some(water_fx) {
            snd.set_water_fx(water_fx);
            self.last_water_fx = Some(water_fx);
        }

        let crossfade = cv.value_of("s_music_crossfade").clamp(0.0, 3.0);
        if self.last_crossfade != Some(crossfade) {
            snd.set_music_crossfade(crossfade);
            self.last_crossfade = Some(crossfade);
        }

        // Live soundtrack switcher: if `s_soundtrack` changed while a track is active, swap it.
        let st_cvar = cv.by_name("s_soundtrack").map_or("", |c| c.string.trim());
        if self.last_soundtrack.as_deref() != Some(st_cvar) {
            let had_prev = self.last_soundtrack.is_some();
            self.last_soundtrack = Some(st_cvar.to_owned());
            if had_prev {
                self.soundtracks.replay_current(st_cvar, fs, snd);
            }
        }

        // Environmental reverb probe.
        let reverb_on = cv.value_of("s_reverb") != 0.0;
        let reverb_amt = cv.value_of("s_reverb_amount").clamp(0.25, 2.0);
        if !reverb_on {
            if self.last_reverb_on != Some(false) {
                snd.set_reverb(ReverbParams::default());
                self.last_reverb_on = Some(false);
            }
        } else if let Some(v) = view {
            self.last_reverb_on = Some(true);
            if let Some(params) = self.acoustic.update_reverb(
                cl,
                level,
                brushes,
                Vec3::from_array(v.origin),
                underwater,
                reverb_amt,
                realtime,
            ) {
                snd.set_reverb(params);
            }
        } else if self.last_reverb_on != Some(false) {
            snd.set_reverb(ReverbParams::default());
            self.last_reverb_on = Some(false);
        }

        // Sound occlusion periodic refresh.
        if occlusion && let Some(v) = view {
            self.acoustic.update_occlusions(cl, level, brushes, v, realtime, snd);
        }

        // Optional surface-aware footsteps (client-side cosmetic).
        if !paused && cv.value_of("s_footsteps") != 0.0 {
            let step_vol = cv.value_of("s_footsteps_volume").clamp(0.1, 1.5);
            self.footsteps.frame(cl, guests, level, brushes, step_vol, realtime, snd);
        }
    }

    /// Record a newly started dynamic sound for occlusion tracking and return its initial occlusion.
    #[allow(clippy::too_many_arguments)]
    pub fn on_sound_start(
        &mut self,
        cv: &Cvars,
        cl: &ClientState,
        level: &Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        view: Option<&qrender::View>,
        entity: i32,
        channel: i32,
        origin: [f32; 3],
        duration_secs: f64,
        realtime: f64,
        snd: &SoundSystem,
    ) {
        if cv.value_of("s_occlusion") == 0.0 || entity == cl.viewentity as i32 {
            return;
        }
        self.acoustic.track_dynamic(entity, channel, Vec3::from_array(origin), realtime + duration_secs.clamp(0.25, 12.0));
        if let Some(v) = view
            && let Some(occ) = self.acoustic.probe_single_occlusion(cl, level, brushes, v, Vec3::from_array(origin))
        {
            snd.set_channel_occlusion(entity, channel, occ);
        }
    }
}
