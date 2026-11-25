//! Room-probing environmental reverb and multi-ray BSP sound occlusion.

use qclient::ClientState;
use qcore::math::{Vec3, angle_vectors};
use qpart::World;
use qsound::{ReverbParams, SoundSystem};

use crate::assets::Level;
use crate::fte::world::{BspWorld, BrushEnt, cached_world};
use crate::hitfx::Material;

const PROBE_DIST: f32 = 2048.0;
const REVERB_INTERVAL: f64 = 0.08;
const OCCLUSION_INTERVAL: f64 = 0.05;
const MAX_TRACKED_DYNAMIC: usize = 64;

/// 14 evenly distributed probe directions (6 cardinal + 8 diagonal).
const PROBE_DIRS: [[f32; 3]; 14] = [
    [1.0, 0.0, 0.0],
    [-1.0, 0.0, 0.0],
    [0.0, 1.0, 0.0],
    [0.0, -1.0, 0.0],
    [0.0, 0.0, 1.0],
    [0.0, 0.0, -1.0],
    [0.577_350_26, 0.577_350_26, 0.577_350_26],
    [-0.577_350_26, 0.577_350_26, 0.577_350_26],
    [0.577_350_26, -0.577_350_26, 0.577_350_26],
    [-0.577_350_26, -0.577_350_26, 0.577_350_26],
    [0.577_350_26, 0.577_350_26, -0.577_350_26],
    [-0.577_350_26, 0.577_350_26, -0.577_350_26],
    [0.577_350_26, -0.577_350_26, -0.577_350_26],
    [-0.577_350_26, -0.577_350_26, -0.577_350_26],
];

#[derive(Clone, Copy)]
struct TrackedSound {
    entity: i32,
    channel: i32,
    origin: Vec3,
    expires: f64,
}

#[derive(Default)]
pub struct AcousticState {
    last_reverb_t: f64,
    last_reverb_pos: Option<Vec3>,
    smooth_room: f32,
    smooth_damp: f32,
    smooth_wet: f32,
    reverb_primed: bool,
    last_occ_t: f64,
    tracked: Vec<TrackedSound>,
}

impl AcousticState {
    pub fn clear_map(&mut self) {
        self.last_reverb_t = 0.0;
        self.last_reverb_pos = None;
        self.reverb_primed = false;
        self.last_occ_t = 0.0;
        self.tracked.clear();
    }

    pub fn track_dynamic(&mut self, entity: i32, channel: i32, origin: Vec3, expires: f64) {
        if channel != 0
            && let Some(existing) = self.tracked.iter_mut().find(|s| s.entity == entity && s.channel == channel)
        {
            existing.origin = origin;
            existing.expires = expires;
            return;
        }
        if self.tracked.len() >= MAX_TRACKED_DYNAMIC {
            self.tracked.remove(0);
        }
        self.tracked.push(TrackedSound { entity, channel, origin, expires });
    }

    /// Periodically probe the room geometry around `origin` and return updated `ReverbParams`.
    #[allow(clippy::too_many_arguments)]
    pub fn update_reverb(
        &mut self,
        cl: &ClientState,
        level: &Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        origin: Vec3,
        underwater: bool,
        amount: f32,
        now: f64,
    ) -> Option<ReverbParams> {
        let moved = self.last_reverb_pos.is_none_or(|p| {
            let d = p - origin;
            d.dot(d) > 24.0 * 24.0
        });
        if self.reverb_primed && !moved && (now - self.last_reverb_t) < REVERB_INTERVAL {
            return None;
        }
        self.last_reverb_t = now;
        self.last_reverb_pos = Some(origin);

        let world = cached_world(cl, level, brushes)?;
        let raw = probe_room(&world, origin, underwater, amount);
        if !self.reverb_primed {
            self.smooth_room = raw.room_size;
            self.smooth_damp = raw.damping;
            self.smooth_wet = raw.wet;
            self.reverb_primed = true;
        } else {
            let alpha = 0.35;
            self.smooth_room += (raw.room_size - self.smooth_room) * alpha;
            self.smooth_damp += (raw.damping - self.smooth_damp) * alpha;
            self.smooth_wet += (raw.wet - self.smooth_wet) * alpha;
        }
        Some(ReverbParams {
            enabled: true,
            room_size: self.smooth_room,
            damping: self.smooth_damp,
            wet: self.smooth_wet,
        })
    }

    /// Compute occlusion for a single sound immediately on start.
    pub fn probe_single_occlusion(
        &self,
        cl: &ClientState,
        level: &Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        view: &qrender::View,
        target: Vec3,
    ) -> Option<f32> {
        let world = cached_world(cl, level, brushes)?;
        let (_, right, up) = angle_vectors(Vec3::from_array(view.angles));
        Some(ray_occlusion(&world, Vec3::from_array(view.origin), right, up, target))
    }

    /// Periodically refresh occlusion for active dynamic sounds and nearby static sounds.
    pub fn update_occlusions(
        &mut self,
        cl: &ClientState,
        level: &Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        view: &qrender::View,
        now: f64,
        snd: &SoundSystem,
    ) {
        if now - self.last_occ_t < OCCLUSION_INTERVAL {
            return;
        }
        self.last_occ_t = now;
        self.tracked.retain(|s| s.expires > now);

        let Some(world) = cached_world(cl, level, brushes) else { return };
        let eye = Vec3::from_array(view.origin);
        let (_, right, up) = angle_vectors(Vec3::from_array(view.angles));

        let mut dyn_out = Vec::with_capacity(self.tracked.len());
        for s in &self.tracked {
            // Follow live entity position when available so moving monsters update around corners.
            let pos = if s.entity > 0
                && let Some(e) = cl.entities.get(s.entity as usize)
                && e.model != 0
            {
                Vec3::from_array(e.origin)
            } else {
                s.origin
            };
            let occ = ray_occlusion(&world, eye, right, up, pos);
            dyn_out.push((s.entity, s.channel, occ));
        }

        let mut static_out = Vec::new();
        // (the mixer has channels for the sounds that loaded)
        let loaded = cl.static_sounds.iter().filter(|s| matches!(level.sounds.get(s.sound), Some(Some(_))));
        for (idx, st) in loaded.enumerate() {
            let pos = Vec3::from_array(st.origin);
            let d = pos - eye;
            if d.dot(d) <= 1050.0 * 1050.0 {
                let occ = ray_occlusion(&world, eye, right, up, pos);
                static_out.push((idx, occ));
            }
        }

        if !dyn_out.is_empty() || !static_out.is_empty() {
            snd.update_occlusions(dyn_out, static_out);
        }
    }
}

fn material_damping(m: Material) -> f32 {
    match m {
        Material::Metal => 0.20,
        Material::Stone => 0.30,
        Material::Water | Material::Slime | Material::Lava => 0.42,
        Material::Wood => 0.58,
        Material::Dirt => 0.74,
        Material::None => 0.50,
    }
}

/// Probe room geometry around `origin` to estimate reverb room size, damping, and wet gain.
fn probe_room(world: &BspWorld<'_>, origin: Vec3, underwater: bool, amount: f32) -> ReverbParams {
    let mut dist_sum = 0.0f32;
    let mut damp_sum = 0.0f32;
    let mut hit_count = 0.0f32;
    let mut sky_count = 0.0f32;

    for dir in PROBE_DIRS {
        let d = Vec3::from_array(dir);
        let tr = world.trace_line(origin, origin + d * PROBE_DIST);
        let dist = tr.frac * PROBE_DIST;
        if tr.frac >= 0.99 {
            sky_count += 1.0;
            dist_sum += PROBE_DIST * 0.65;
            continue;
        }
        let mat = crate::impacts::surfaces::texture_at(world, &tr)
            .map_or(Material::Stone, |t| crate::hitfx::classify(&t.name));
        if mat == Material::None {
            sky_count += 1.0;
            dist_sum += dist.min(PROBE_DIST * 0.65);
        } else {
            dist_sum += dist;
            damp_sum += material_damping(mat);
            hit_count += 1.0;
        }
    }

    let n = PROBE_DIRS.len() as f32;
    let mean_dist = dist_sum / n;
    let sky_ratio = sky_count / n;
    let indoor = (1.0 - sky_ratio).clamp(0.0, 1.0).powf(1.35);

    // Map mean free path (~80u tight hallway .. ~1100u cathedral/cavern) to room_size 0.22..0.90.
    let room_size = ((mean_dist - 80.0) / 1020.0).clamp(0.0, 1.0) * 0.68 + 0.22;
    let base_damp = if hit_count > 0.0 { damp_sum / hit_count } else { 0.45 };
    let damping = if underwater { 0.88 } else { base_damp };
    // Subtle enough to never muddy combat, noticeable in halls/caverns, nearly dry outdoors.
    let wet = ((0.09 + room_size * 0.26) * indoor * amount).clamp(0.0, 0.72);

    ReverbParams {
        enabled: true,
        room_size,
        damping,
        wet,
    }
}

/// Multi-ray soft occlusion test between `eye` and `target` (0.0 = clear, 1.0 = fully blocked).
fn ray_occlusion(world: &BspWorld<'_>, eye: Vec3, right: Vec3, up: Vec3, target: Vec3) -> f32 {
    let delta = target - eye;
    let dist_sq = delta.dot(delta);
    if dist_sq < 24.0 * 24.0 || dist_sq > 1150.0 * 1150.0 {
        return 0.0;
    }
    let offsets = [
        Vec3::ZERO,
        right * 14.0,
        right * -14.0,
        up * 14.0,
        up * -14.0,
    ];
    let mut blocked = 0u32;
    let mut tested = 0u32;
    for ofs in offsets {
        let p = target + ofs;
        // Skip offset probes whose endpoint lies inside solid geometry.
        if ofs != Vec3::ZERO && world.point_contents(p) == qpart::FTECONTENTS_SOLID {
            continue;
        }
        tested += 1;
        let tr = world.trace_line(eye, p);
        if tr.frac < 0.96 {
            blocked += 1;
        }
    }
    if tested == 0 {
        return 0.0;
    }
    blocked as f32 / tested as f32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_damping_orders_metal_stone_wood_dirt() {
        assert!(material_damping(Material::Metal) < material_damping(Material::Stone));
        assert!(material_damping(Material::Stone) < material_damping(Material::Wood));
        assert!(material_damping(Material::Wood) < material_damping(Material::Dirt));
    }
}
