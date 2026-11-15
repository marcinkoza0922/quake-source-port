//! User-facing `r_debris_*` settings and `r_debris_quality` tier budgets.
use super::{MAX_GIBS, MAX_SHARDS, MAX_STEPS, STEP, rig};

/// `r_debris_ragdollstart`: when a dying monster becomes a ragdoll.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Start {
    /// After the death animation (0).
    Animation,
    /// On the first death frame (1).
    AtDeath,
    /// At death for a hard killing hit; a weaker one may play the animation (2).
    Mixed,
}

impl Start {
    pub fn from_cvar(v: f32) -> Start {
        match v as i32 {
            1 => Start::AtDeath,
            2 => Start::Mixed,
            _ => Start::Animation,
        }
    }
}

/// `r_debris_quality`: simulation detail, step rate and caps.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
}

impl Quality {
    pub fn from_cvar(v: f32) -> Quality {
        match v.round() as i32 {
            ..=0 => Quality::Low,
            1 => Quality::Medium,
            _ => Quality::High,
        }
    }
}

/// Resolved per-tier simulation budgets and rates.
#[derive(Clone, Copy)]
pub(super) struct Tier {
    pub max_bones: usize,
    pub iters: usize,
    pub step: f32,
    pub max_steps: usize,
    pub skip_down: bool,
    pub movers: bool,
    pub max_ragdolls: usize,
    pub max_gibs: usize,
    pub max_shards: usize,
    pub shard_burst: usize,
    pub gib_pieces: usize,
    pub corpse_life: f32,
    pub gib_life: f32,
    pub client_gib_life: (f32, f32),
    pub shard_life: (f32, f32),
}

/// The `r_debris_*` sub-options.
#[derive(Clone, Copy)]
pub struct Settings {
    pub quality: Quality,
    pub ragdolls: bool,
    /// When a dying monster goes limp.
    pub start: Start,
    pub max_ragdolls: usize,
    pub gibs: bool,
    pub shrapnel: bool,
    /// Collide with and ride on moving brush entities.
    pub movers: bool,
    /// Scale for shot and explosion pushes.
    pub force: f32,
    /// A killing volley also lifts the body.
    pub kill_lift: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            quality: Quality::Medium,
            ragdolls: true,
            start: Start::Mixed,
            max_ragdolls: 48,
            gibs: true,
            shrapnel: true,
            movers: true,
            force: 1.0,
            kill_lift: false,
        }
    }
}

pub(super) fn tier_for(set: &Settings) -> Tier {
    match set.quality {
        Quality::Low => Tier {
            max_bones: 6,
            iters: 3,
            step: 1.0 / 30.0,
            max_steps: 3,
            skip_down: true,
            movers: false,
            max_ragdolls: set.max_ragdolls.min(16),
            max_gibs: 32,
            max_shards: 24,
            shard_burst: 6,
            gib_pieces: 3,
            corpse_life: 12.0,
            gib_life: 8.0,
            client_gib_life: (6.0, 4.0),
            shard_life: (4.0, 2.0),
        },
        Quality::Medium => Tier {
            max_bones: 8,
            iters: 4,
            step: 1.0 / 45.0,
            max_steps: 5,
            skip_down: false,
            movers: set.movers,
            max_ragdolls: set.max_ragdolls.min(32),
            max_gibs: 64,
            max_shards: 48,
            shard_burst: 10,
            gib_pieces: 4,
            corpse_life: 24.0,
            gib_life: 16.0,
            client_gib_life: (8.0, 6.0),
            shard_life: (6.0, 3.0),
        },
        Quality::High => Tier {
            max_bones: rig::MAX_BONES,
            iters: 6,
            step: STEP,
            max_steps: MAX_STEPS,
            skip_down: false,
            movers: set.movers,
            max_ragdolls: set.max_ragdolls,
            max_gibs: MAX_GIBS,
            max_shards: MAX_SHARDS,
            shard_burst: 14,
            gib_pieces: 6,
            corpse_life: f32::INFINITY,
            gib_life: f32::INFINITY,
            client_gib_life: (10.0, 10.0),
            shard_life: (8.0, 4.0),
        },
    }
}
