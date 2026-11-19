//! Sound categories and per-category volume mixing.

/// Broad mix bucket for a playing sound channel.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SoundCategory {
    /// BSP leaf ambient loops, static looping world sounds, ambience/*.
    Ambient,
    /// Player weapons, player actions, footsteps, add-on toys.
    Weapons,
    /// Enemy vocals, attacks, pain and deaths.
    Monsters,
    /// Doors, lifts, pickups, triggers and unrecognized mod sounds.
    #[default]
    World,
    /// Menu clicks and hit/kill feedback cues (kept dry and un-occluded).
    Ui,
}

/// Per-category linear volume multipliers (0..1, default 1.0).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CategoryVolumes {
    pub ambient: f32,
    pub weapons: f32,
    pub monsters: f32,
    pub world: f32,
}

impl Default for CategoryVolumes {
    fn default() -> Self {
        Self {
            ambient: 1.0,
            weapons: 1.0,
            monsters: 1.0,
            world: 1.0,
        }
    }
}

impl CategoryVolumes {
    pub fn clamped(self) -> Self {
        Self {
            ambient: self.ambient.clamp(0.0, 2.0),
            weapons: self.weapons.clamp(0.0, 2.0),
            monsters: self.monsters.clamp(0.0, 2.0),
            world: self.world.clamp(0.0, 2.0),
        }
    }

    pub fn gain(self, cat: SoundCategory) -> f32 {
        match cat {
            SoundCategory::Ambient => self.ambient,
            SoundCategory::Weapons => self.weapons,
            SoundCategory::Monsters => self.monsters,
            SoundCategory::World => self.world,
            SoundCategory::Ui => 1.0,
        }
    }
}

const MONSTER_PREFIXES: &[&str] = &[
    "soldier/",
    "ogre/",
    "shambler/",
    "demon/",
    "dog/",
    "enforcer/",
    "fish/",
    "hknight/",
    "knight/",
    "knight2/",
    "shalrath/",
    "tarbaby/",
    "wizard/",
    "zombie/",
    "boss1/",
    "boss2/",
    "blob/",
    "vore/",
    "spid/",
    "spider/",
    "wrath/",
    "golem/",
    "gaunt/",
    "pyro/",
    "dguard/",
    "sentinel/",
    "minotaur/",
    "hellknight/",
    "scourge/",
    "gremlin/",
    "armagon/",
    "dragon/",
    "eel/",
    "statue/",
    "morph/",
    "mummy/",
    "wraith/",
    "scorpion/",
    "jim/",
    "baron/",
    "freddie/",
    "fumigator/",
    "justice/",
    "lostsoul/",
    "swamp/",
    "monster/",
    "monsters/",
    "enemies/",
];

const WEAPON_PREFIXES: &[&str] = &[
    "weapons/",
    "player/",
    "footsteps/",
    "goggles/",
    "portal/",
    "rail/",
    "gel/",
    "hookshot/",
    "boomerang/",
    "cable/",
    "hipweap/",
    "rogue/weapons/",
];

/// Classify a sound asset name into its volume-mix category.
pub fn classify_sound(name: &str) -> SoundCategory {
    let lower = name.to_ascii_lowercase();
    let s = lower.strip_prefix("sound/").unwrap_or(&lower);
    if s.starts_with("misc/menu") || s.starts_with("feedback/") || s.starts_with("hitfeedback") {
        return SoundCategory::Ui;
    }
    if s.starts_with("ambience/") || s.starts_with("ambient/") {
        return SoundCategory::Ambient;
    }
    if WEAPON_PREFIXES.iter().any(|p| s.starts_with(p)) {
        return SoundCategory::Weapons;
    }
    if MONSTER_PREFIXES.iter().any(|p| s.starts_with(p)) {
        return SoundCategory::Monsters;
    }
    SoundCategory::World
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_standard_and_mod_paths() {
        assert_eq!(classify_sound("ambience/water1.wav"), SoundCategory::Ambient);
        assert_eq!(classify_sound("sound/ambience/wind2.wav"), SoundCategory::Ambient);
        assert_eq!(classify_sound("weapons/r_exp3.wav"), SoundCategory::Weapons);
        assert_eq!(classify_sound("player/pain1.wav"), SoundCategory::Weapons);
        assert_eq!(classify_sound("footsteps/stone_0"), SoundCategory::Weapons);
        assert_eq!(classify_sound("shambler/sattck1.wav"), SoundCategory::Monsters);
        assert_eq!(classify_sound("ogre/ogwake.wav"), SoundCategory::Monsters);
        assert_eq!(classify_sound("doors/drclos4.wav"), SoundCategory::World);
        assert_eq!(classify_sound("items/health1.wav"), SoundCategory::World);
        assert_eq!(classify_sound("misc/menu1.wav"), SoundCategory::Ui);
        assert_eq!(classify_sound("feedback/hit"), SoundCategory::Ui);
    }
}
