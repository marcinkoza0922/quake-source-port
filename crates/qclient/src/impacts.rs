//! Temp-entity impact visuals. The host can resolve the struck surface
//! before emitting either the scripted effect or its classic fallback.
use crate::ClientState;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImpactKind {
    Gunshot,
    Spike,
    SuperSpike,
    WizardSpike,
    KnightSpike,
    Explosion,
    TarExplosion,
    ColoredExplosion { start: i32, len: i32 },
}

impl ImpactKind {
    pub fn mark(self) -> Option<u8> {
        match self {
            Self::Gunshot => Some(0),
            Self::Spike | Self::SuperSpike => Some(1),
            Self::Explosion | Self::TarExplosion | Self::ColoredExplosion { .. } => Some(2),
            Self::WizardSpike | Self::KnightSpike => None,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ImpactEffect {
    pub kind: ImpactKind,
    pub pos: [f32; 3],
}

impl ClientState {
    pub fn defer_impact_effects(&mut self) {
        self.defer_impact_effects = true;
    }

    pub(crate) fn impact_effect(&mut self, kind: ImpactKind, pos: [f32; 3]) {
        let impact = ImpactEffect { kind, pos };
        if self.defer_impact_effects {
            self.impact_effects.push(impact);
        } else {
            self.emit_impact_effect(impact);
        }
    }

    pub fn emit_impact_effect(&mut self, ImpactEffect { kind, pos }: ImpactEffect) {
        use ImpactKind::*;
        let mark = kind.mark();
        self.shot_impacts.push((pos, mark == Some(2)));
        if let Some(mark) = mark {
            self.weapon_impacts.push((mark, pos));
        }
        let (name, color, count) = match kind {
            Gunshot => ("TE_GUNSHOT", 0, 20),
            Spike => ("TE_SPIKE", 0, 10),
            SuperSpike => ("TE_SUPERSPIKE", 0, 20),
            WizardSpike => ("TE_WIZSPIKE", 20, 30),
            KnightSpike => ("TE_KNIGHTSPIKE", 226, 20),
            Explosion => {
                if !self.fte_effect("TE_EXPLOSION", pos, 1.0) {
                    self.particle_explosion(pos);
                }
                self.explosion_light(pos);
                return;
            }
            TarExplosion => {
                if !self.fte_effect("TE_TAREXPLOSION", pos, 1.0) {
                    self.blob_explosion(pos);
                }
                return;
            }
            ColoredExplosion { start, len } => {
                if !self.fte_effect(&format!("TE_EXPLOSION2_{start}_{len}"), pos, 1.0) {
                    self.particle_explosion2(pos, start, len);
                }
                self.explosion_light(pos);
                return;
            }
        };
        let scripted_count = if kind == Gunshot { 20.0 } else { 1.0 };
        if !self.fte_effect(name, pos, scripted_count) {
            self.run_particle_effect(pos, [0.0; 3], color, count);
        }
    }

    fn explosion_light(&mut self, pos: [f32; 3]) {
        let die = self.time + 0.5;
        let dl = self.alloc_dlight(0);
        dl.origin = pos;
        dl.radius = 350.0;
        dl.die = die;
        dl.decay = 300.0;
    }
}
