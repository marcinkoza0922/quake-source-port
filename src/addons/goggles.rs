//! Add-on equipment: things the player switches on and off (from the
//! equipment wheel or the `equip`/`goggles` commands) rather than holds.
//! Night-vision and thermal goggles share one slot, so turning one on turns
//! the other off. The look itself is a qrender post effect
//! (`qrender::Goggles`); thermal also tags each entity with a heat class.

use std::collections::HashMap;

use qrender::{HEAT_BODY, HEAT_COLD, HEAT_FIRE, HEAT_WARM, RenderEntity};

/// Toggleable equipment, in wheel order (clockwise from the top).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Equip {
    NightVision,
    Thermal,
}

impl Equip {
    pub const ALL: [Equip; 2] = [Equip::NightVision, Equip::Thermal];

    pub fn name(self) -> &'static str {
        match self {
            Equip::NightVision => "Night Vision Goggles",
            Equip::Thermal => "Thermal Goggles",
        }
    }
    /// Short label drawn in the wheel slot.
    pub fn short(self) -> &'static str {
        match self {
            Equip::NightVision => "NV",
            Equip::Thermal => "IR",
        }
    }
    /// Console name (`equip nv`, `goggles thermal`).
    pub fn from_key(s: &str) -> Option<Equip> {
        let s = s.to_ascii_lowercase();
        match s.as_str() {
            "nv" | "night" | "nightvision" => Some(Equip::NightVision),
            "thermal" | "ir" | "heat" => Some(Equip::Thermal),
            _ => None,
        }
    }
    /// Items in the same group exclude each other (the goggle slot).
    fn group(self) -> u8 {
        match self {
            Equip::NightVision | Equip::Thermal => 1,
        }
    }
    fn index(self) -> usize {
        Self::ALL.iter().position(|&e| e == self).unwrap_or(0)
    }
}

/// Which equipment one player has switched on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Equipment {
    on: [bool; Equip::ALL.len()],
}

impl Equipment {
    pub fn is_on(&self, e: Equip) -> bool {
        self.on[e.index()]
    }
    /// Switches `e` on or off; on turns off the rest of its group.
    pub fn set(&mut self, e: Equip, on: bool) {
        if on {
            for o in Equip::ALL {
                if o.group() == e.group() {
                    self.on[o.index()] = false;
                }
            }
        }
        self.on[e.index()] = on;
    }
    /// Flips `e`, returning its new state.
    pub fn toggle(&mut self, e: Equip) -> bool {
        let on = !self.is_on(e);
        self.set(e, on);
        on
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn any(&self) -> bool {
        self.on.iter().any(|&b| b)
    }
    /// `qrender::Goggles::mode`: 0 off, 1 night vision, 2 thermal.
    pub fn goggles_mode(&self) -> u32 {
        if self.is_on(Equip::Thermal) {
            2
        } else if self.is_on(Equip::NightVision) {
            1
        } else {
            0
        }
    }
}

/// Heat class of a model by its file name.
pub fn heat_of(name: &str) -> u8 {
    let n = name.rsplit('/').next().unwrap_or(name).to_ascii_lowercase();
    let stem = n.split('.').next().unwrap_or("");
    const FIRE: &[&str] = &["flame", "flame2", "lavaball", "s_explod", "explode", "s_expl", "fireball", "bolt", "bolt2", "bolt3", "lspike", "laser", "s_light", "candle", "torch"];
    const BODY: &[&str] = &[
        "player", "soldier", "dog", "ogre", "knight", "hknight", "demon", "shambler", "zombie", "wizard", "fish",
        "enforcer", "shalrath", "tarbaby", "oldone", "boss", "eyes", "scrag", "mummy", "dragon", "lavaman", "grem",
        "armagon", "arma", "voreling", "drole", "fiend", "golem", "ogre_m", "ogre_h", "zom", "sword",
    ];
    if FIRE.contains(&stem) || stem.starts_with("flame") || stem.contains("explo") || stem.contains("fire") {
        HEAT_FIRE
    } else if BODY.contains(&stem) || stem.starts_with("mon_") || stem.starts_with("m_") {
        HEAT_BODY
    } else if stem.starts_with("h_") || stem.starts_with("gib") || stem == "missile" || stem == "grenade" || stem.contains("spike") {
        HEAT_WARM
    } else {
        HEAT_COLD
    }
}

/// Model id → heat class for this level's models, built while thermal is on.
#[derive(Default)]
pub struct HeatTable {
    map: String,
    count: usize,
    heat: HashMap<u32, u8>,
}

impl HeatTable {
    /// Rebuilds when the level or its model list changed.
    pub fn refresh(&mut self, map: &str, names: &[String], lv: &crate::assets::Level) {
        let count = names.len() + lv.by_name.len();
        if self.map == map && self.count == count {
            return;
        }
        self.map = map.to_string();
        self.count = count;
        self.heat.clear();
        for (i, m) in lv.models.iter().enumerate() {
            if let (Some(m), Some(n)) = (m, names.get(i)) {
                self.heat.insert(m.id.0, heat_of(n));
            }
        }
        for (n, m) in &lv.by_name {
            if let Some(m) = m {
                self.heat.entry(m.id.0).or_insert_with(|| heat_of(n));
            }
        }
    }
    /// Sets each entity's heat; the view model stays cold, flames stay hot.
    pub fn tag(&self, ents: &mut [RenderEntity]) {
        for e in ents.iter_mut().filter(|e| !e.viewmodel) {
            let h = self.heat.get(&e.model.0).copied().unwrap_or(HEAT_COLD);
            e.heat = if e.fullbright { HEAT_FIRE } else { h };
        }
    }
}

/// Original goggle switch cue: rising (on) or falling (off) electronic
/// whine over a relay click, at `rate` Hz.
pub fn cue(on: bool, rate: u32) -> qsound::Sfx {
    let dur = if on { 0.55 } else { 0.32 };
    let n = (dur * rate as f32) as usize;
    let mut data = Vec::with_capacity(n);
    let (mut ph, mut seed) = (0.0f32, 0x1234_5679u32);
    for i in 0..n {
        let t = i as f32 / rate as f32;
        let u = t / dur;
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let noise = (seed as i32 as f32) / i32::MAX as f32;
        let f = if on { 900.0 + 5200.0 * u * u } else { 4200.0 * (1.0 - u) + 300.0 };
        ph += std::f32::consts::TAU * f / rate as f32;
        let whine = ph.sin() * if on { (1.0 - u).powf(0.6) * 0.35 } else { (1.0 - u).powi(2) * 0.4 };
        let click = noise * (-t * 300.0).exp() * 0.9;
        let tail = ((dur - t) / 0.02).min(1.0);
        let attack = (t / 0.002).min(1.0);
        data.push(((whine + click) * attack * tail * 20000.0) as i16);
    }
    qsound::Sfx { name: format!("goggles/{}", if on { "on" } else { "off" }), data, loop_start: None }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn goggles_are_mutually_exclusive() {
        let mut e = Equipment::default();
        assert_eq!(e.goggles_mode(), 0);
        assert!(e.toggle(Equip::NightVision));
        assert_eq!(e.goggles_mode(), 1);
        assert!(e.toggle(Equip::Thermal));
        assert!(!e.is_on(Equip::NightVision));
        assert_eq!(e.goggles_mode(), 2);
        assert!(!e.toggle(Equip::Thermal));
        assert!(!e.any());
    }

    #[test]
    fn heat_classes_by_model_name() {
        assert_eq!(heat_of("progs/ogre.mdl"), HEAT_BODY);
        assert_eq!(heat_of("progs/flame2.mdl"), HEAT_FIRE);
        assert_eq!(heat_of("progs/s_explod.spr"), HEAT_FIRE);
        assert_eq!(heat_of("progs/h_ogre.mdl"), HEAT_WARM);
        assert_eq!(heat_of("progs/armor.mdl"), HEAT_COLD);
        assert_eq!(heat_of("maps/b_bh25.bsp"), HEAT_COLD);
        assert_eq!(Equip::from_key("IR"), Some(Equip::Thermal));
    }

    #[test]
    fn cues_click_and_fade() {
        for on in [true, false] {
            let s = cue(on, 22050);
            assert!(s.data.iter().any(|x| x.abs() > 3000));
            assert!(s.data.last().unwrap().abs() < 200);
        }
    }
}
