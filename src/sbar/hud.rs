/// Modern HUD elements, in stacking order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudElem {
    Health,
    Armor,
    Ammo,
    AmmoList,
    Weapons,
    Items,
}

impl HudElem {
    pub const ALL: [HudElem; 6] =
        [HudElem::Health, HudElem::Armor, HudElem::Ammo, HudElem::AmmoList, HudElem::Weapons, HudElem::Items];
    /// Cvar suffix: `hud_<name>` (mode) and `hud_<name>_pos` (anchor).
    pub fn name(self) -> &'static str {
        ["health", "armor", "ammo", "ammolist", "weapons", "items"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudMode {
    Off,
    On,
    /// Shown only briefly after the element's value changes.
    Immersive,
}

impl HudMode {
    pub fn from_cvar(v: f32) -> Self {
        match v as i32 {
            0 => HudMode::Off,
            2 => HudMode::Immersive,
            _ => HudMode::On,
        }
    }
}

/// Screen anchor; cvar value is the discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAnchor {
    BottomLeft,
    BottomCenter,
    BottomRight,
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
}

impl HudAnchor {
    pub const COUNT: usize = 8;
    pub fn from_cvar(v: f32) -> Self {
        use HudAnchor::*;
        [BottomLeft, BottomCenter, BottomRight, TopLeft, TopCenter, TopRight, MiddleLeft, MiddleRight]
            [(v.max(0.0) as usize).min(7)]
    }
    /// 0 left, 1 center, 2 right.
    pub(super) fn col(self) -> u8 {
        use HudAnchor::*;
        match self {
            BottomLeft | TopLeft | MiddleLeft => 0,
            BottomCenter | TopCenter => 1,
            _ => 2,
        }
    }
    /// 0 top, 1 middle, 2 bottom.
    pub(super) fn row(self) -> u8 {
        use HudAnchor::*;
        match self {
            TopLeft | TopCenter | TopRight => 0,
            MiddleLeft | MiddleRight => 1,
            _ => 2,
        }
    }
}

/// Modern HUD settings (from `hud_*` cvars).
#[derive(Clone, Debug)]
pub struct HudConfig {
    pub mode: [HudMode; 6],
    pub anchor: [HudAnchor; 6],
    pub face: bool,
}
