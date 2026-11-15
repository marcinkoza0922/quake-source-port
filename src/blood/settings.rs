//! User-facing `r_blood_*` settings and `r_blood_quality` tier budgets.
use super::droplets::MAX_DROPLETS;

/// `r_blood_quality`: mark caps, lifetimes, spray density and mesh wounds.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Quality {
    Low = 0,
    #[default]
    Medium = 1,
    High = 2,
}

impl Quality {
    pub fn from_cvar(v: f32) -> Self {
        match v.round() as i32 {
            ..=0 => Self::Low,
            1 => Self::Medium,
            _ => Self::High,
        }
    }
}

#[derive(Clone, Copy)]
pub struct Settings {
    pub enabled: bool,
    pub quality: Quality,
    pub amount: usize,
    pub size: f32,
    pub lifetime: f64,
    pub limit: usize,
    pub wounds: bool,
    pub gibs: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            quality: Quality::Medium,
            amount: 5,
            size: 1.0,
            lifetime: 120.0,
            limit: 512,
            wounds: true,
            gibs: true,
        }
    }
}

/// Resolved per-tier caps and feature toggles.
#[derive(Clone, Copy)]
pub(super) struct Tier {
    pub amount: usize,
    pub burst_base: usize,
    pub burst_per_amount: usize,
    pub max_droplets: usize,
    pub lifetime: f64,
    pub limit: usize,
    pub wounds: bool,
    pub gibs: bool,
}

impl Settings {
    pub fn read(cv: &qcore::cvar::Cvars) -> Self {
        Self {
            enabled: cv.value_of("r_blood") != 0.0,
            quality: Quality::from_cvar(cv.value_of("r_blood_quality")),
            amount: cv.value_of("r_blood_amount").clamp(1.0, 12.0) as usize,
            size: cv.value_of("r_blood_size").clamp(0.25, 3.0),
            lifetime: cv.value_of("r_blood_lifetime").clamp(5.0, 600.0) as f64,
            limit: cv.value_of("r_blood_maxmarks").clamp(32.0, 2048.0) as usize,
            wounds: cv.value_of("r_blood_wounds") != 0.0,
            gibs: cv.value_of("r_blood_gibs") != 0.0,
        }
    }

    pub(super) fn tier(self) -> Tier {
        match self.quality {
            Quality::Low => Tier {
                amount: self.amount.min(3),
                burst_base: 16,
                burst_per_amount: 4,
                max_droplets: 256,
                lifetime: self.lifetime.min(30.0),
                limit: self.limit.min(256),
                wounds: false,
                gibs: false,
            },
            Quality::Medium => Tier {
                amount: self.amount.min(6),
                burst_base: 32,
                burst_per_amount: 8,
                max_droplets: 768,
                lifetime: self.lifetime.min(120.0),
                limit: self.limit.min(512),
                wounds: self.wounds,
                gibs: self.gibs,
            },
            Quality::High => Tier {
                amount: self.amount,
                burst_base: 48,
                burst_per_amount: 12,
                max_droplets: MAX_DROPLETS,
                lifetime: self.lifetime,
                limit: self.limit,
                wounds: self.wounds,
                gibs: self.gibs,
            },
        }
    }
}
