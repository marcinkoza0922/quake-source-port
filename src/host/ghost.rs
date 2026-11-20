//! Personal-best demo ghost (`cl_demo_ghost`: 0 off, 1 any%, 2 100%).
//! Loads the matching PB demo for the current map and skill once (or when the
//! setting changes mid-level) and emits a translucent fullbright player entity
//! synced to the level clock, faded out when overlapping the camera.

use std::path::PathBuf;

use qcore::math::Vec3;
use qrender::RenderEntity;

use super::*;
use crate::demo::ghost::{GhostTrack, find_pb_demo};

/// Base opacity of the ghost model once clear of the camera.
const GHOST_ALPHA: f32 = 0.45;
/// Distance fade around the camera so spawning on top of the ghost never clips.
const FADE_NEAR: f32 = 20.0;
const FADE_FAR: f32 = 64.0;

#[derive(Default)]
pub(super) struct GhostState {
    /// Key of the currently cached track: (user_dir, mapname, skill, full).
    key: Option<(PathBuf, String, usize, bool)>,
    track: Option<GhostTrack>,
}

impl Host {
    pub(super) fn ghost_reset(&mut self) {
        self.ghost.key = None;
        self.ghost.track = None;
    }

    /// Returns the translucent PB ghost entity for this frame, if enabled and
    /// a PB demo exists for the current map, skill and category.
    pub(super) fn ghost_entity(&mut self, view: &qrender::View, lerp_models: bool) -> Option<RenderEntity> {
        let mode = self.cv("cl_demo_ghost").round() as i32;
        let full = match mode {
            1 => false,
            2 => true,
            _ => {
                if self.ghost.key.is_some() {
                    self.ghost_reset();
                }
                return None;
            }
        };
        if !self.pb_allowed() || !self.guests.is_empty() || self.cl.signon != 4 || self.cl.intermission != 0 {
            return None;
        }
        let skill = self.pb_skill()?;
        let user_dir = self.user_dir();
        let mapname = self.level.mapname.clone();
        let want = (user_dir.clone(), mapname.clone(), skill, full);
        if self.ghost.key.as_ref() != Some(&want) {
            self.ghost.key = Some(want);
            self.ghost.track = find_pb_demo(&user_dir, &mapname, skill, full)
                .and_then(|p| qcore::vfs::read(&p).ok())
                .map(|data| GhostTrack::extract(&data, Some(&mapname)))
                .filter(|t| !t.samples.is_empty());
        }
        let pose = self.ghost.track.as_ref()?.sample(self.cl.time, lerp_models)?;
        let dist = (Vec3::from(pose.origin) - Vec3::from(view.origin)).length();
        let alpha = GHOST_ALPHA * ((dist - FADE_NEAR) / (FADE_FAR - FADE_NEAR)).clamp(0.0, 1.0);
        if alpha <= 0.01 {
            return None;
        }
        let fs = &self.server.core.fs;
        let model = self
            .level
            .named(fs, &mut self.r, &pose.model)
            .or_else(|| self.level.named(fs, &mut self.r, "progs/player.mdl"))?;
        Some(RenderEntity {
            model: model.id,
            origin: pose.origin,
            angles: pose.angles,
            frame: pose.frame,
            prev_frame: pose.prev_frame,
            lerp: pose.lerp,
            skin: pose.skin,
            colormap: pose.colormap,
            alpha,
            fullbright: true,
            player: true,
            key: 0,
            ..Default::default()
        })
    }
}
