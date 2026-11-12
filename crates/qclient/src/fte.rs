//! Hooks for FTE scripted particles (vkQuake's `PScript_*` calls in
//! cl_tent.c / cl_main.c). The client only decides whether an effect is
//! scripted; the host owns the particle system and drains
//! [`ClientState::fte_queue`].
use std::collections::HashSet;

use crate::state::ClientState;
use qnet::MsgReader;

/// An effect for the FTE particle system to run.
#[derive(Clone, Debug, PartialEq)]
pub enum FteRequest {
    /// PScript_RunParticleEffectTypeString.
    Effect { name: String, org: [f32; 3], dir: Option<[f32; 3]>, count: f32 },
    /// PScript_EntParticleTrail: `entity` keys the trail state and dlight.
    /// `interval` overrides the frame time (trailparticles uses 1).
    Trail { name: String, start: [f32; 3], end: [f32; 3], entity: i32, interval: Option<f32> },
    /// te_particlerain / te_particlesnow (PScript_RunParticleWeather).
    Weather { min: [f32; 3], max: [f32; 3], dir: [f32; 3], count: f32, colour: i32, snow: bool },
}

pub const SVCDP_PRECACHE: u8 = 54;
pub const SVCDP_TRAILPARTICLES: u8 = 60;
pub const SVCDP_POINTPARTICLES: u8 = 61;
pub const SVCDP_POINTPARTICLES1: u8 = 62;
pub const TEDP_PARTICLERAIN: u8 = 55;
pub const TEDP_PARTICLESNOW: u8 = 56;
const MAX_PARTICLETYPES: usize = 2048;

/// Lowercased effect names that resolve, set by the host whenever the
/// loaded effects change. Empty: everything is classic.
#[derive(Clone, Debug, Default)]
pub struct FteNames {
    names: HashSet<String>,
}

impl FteNames {
    pub fn new(names: impl IntoIterator<Item = String>) -> Self {
        FteNames { names: names.into_iter().map(|n| n.to_ascii_lowercase()).collect() }
    }
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
    /// Whether `name` would resolve (`TE_EXPLOSION2_<c>_<n>` retints
    /// `TE_EXPLOSION2`).
    pub fn has(&self, name: &str) -> bool {
        let l = name.to_ascii_lowercase();
        self.names.contains(&l) || (l.starts_with("te_explosion2_") && self.names.contains("te_explosion2"))
    }
}

impl ClientState {
    /// Queues `name` if it is scripted; false means "use the classic effect".
    pub(crate) fn fte_effect(&mut self, name: &str, org: [f32; 3], count: f32) -> bool {
        if !self.fte_names.has(name) {
            return false;
        }
        self.fte_queue.push(FteRequest::Effect { name: name.to_string(), org, dir: None, count });
        true
    }

    pub(crate) fn fte_trail(&mut self, name: &str, start: [f32; 3], end: [f32; 3], entity: i32) -> bool {
        if !self.fte_names.has(name) {
            return false;
        }
        self.fte_queue.push(FteRequest::Trail { name: name.to_string(), start, end, entity, interval: None });
        true
    }

    fn precached(&self, ef: i32) -> Option<String> {
        self.particle_precache.get(ef as usize).filter(|n| !n.is_empty()).cloned()
    }

    /// svcdp_precache (CL_ParsePrecache): only particle names are used.
    pub(crate) fn parse_dp_precache(&mut self, m: &mut MsgReader<'_>) {
        let code = m.short() as u16;
        let name = m.string_lossy();
        let index = (code & 0x3fff) as usize;
        if code >> 14 == 1 && index < MAX_PARTICLETYPES {
            if self.particle_precache.len() <= index {
                self.particle_precache.resize(index + 1, String::new());
            }
            self.particle_precache[index] = name;
        }
    }

    /// svcdp_trailparticles / svcdp_pointparticles(1) (CL_ParseParticles).
    pub(crate) fn parse_dp_particles(&mut self, m: &mut MsgReader<'_>, cmd: u8) {
        let pf = self.protocolflags;
        if cmd == SVCDP_TRAILPARTICLES {
            let entity = m.short();
            let ef = m.short();
            let (start, end) = (m.vec3_coord(pf), m.vec3_coord(pf));
            if let Some(name) = self.precached(ef) {
                self.fte_queue.push(FteRequest::Trail { name, start, end, entity, interval: Some(1.0) });
            }
            return;
        }
        let ef = m.short();
        let org = m.vec3_coord(pf);
        let (dir, count) =
            if cmd == SVCDP_POINTPARTICLES1 { ([0.0; 3], 1) } else { (m.vec3_coord(pf), m.short()) };
        if let Some(name) = self.precached(ef) {
            self.fte_queue.push(FteRequest::Effect { name, org, dir: Some(dir), count: count as f32 });
        }
    }

    /// TEDP_PARTICLERAIN / TEDP_PARTICLESNOW.
    pub(crate) fn parse_dp_weather(&mut self, m: &mut MsgReader<'_>, snow: bool) {
        let pf = self.protocolflags;
        let (min, max, dir) = (m.vec3_coord(pf), m.vec3_coord(pf), m.vec3_coord(pf));
        let count = m.short() as u16 as f32;
        let colour = m.byte() as u8 as i32;
        self.fte_queue.push(FteRequest::Weather { min, max, dir, count, colour, snow });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qnet::msg::SizeBuf;

    #[test]
    fn parses_dp_particle_messages() {
        let mut cl = ClientState::new();
        let mut m = SizeBuf::new(1024);
        m.byte(SVCDP_PRECACHE as i32);
        m.short(0x4000 | 3);
        m.string(b"effectinfo.fire");
        m.byte(SVCDP_POINTPARTICLES1 as i32);
        m.short(3);
        for v in [1.0, 2.0, 3.0] {
            m.coord(v, 0);
        }
        m.byte(SVCDP_POINTPARTICLES as i32);
        m.short(3);
        for v in [1.0, 2.0, 3.0, 0.0, 0.0, 10.0] {
            m.coord(v, 0);
        }
        m.short(5);
        m.byte(SVCDP_TRAILPARTICLES as i32);
        m.short(7);
        m.short(3);
        for v in [0.0, 0.0, 0.0, 8.0, 0.0, 0.0] {
            m.coord(v, 0);
        }
        // unknown effect index: ignored
        m.byte(SVCDP_POINTPARTICLES1 as i32);
        m.short(9);
        for v in [0.0; 3] {
            m.coord(v, 0);
        }
        m.byte(qnet::protocol::SVC_TEMP_ENTITY as i32);
        m.byte(TEDP_PARTICLESNOW as i32);
        for v in [0.0, 0.0, 0.0, 64.0, 64.0, 64.0, 0.0, 0.0, -30.0] {
            m.coord(v, 0);
        }
        m.short(100);
        m.byte(12);
        cl.parse_server_message(&m.data).unwrap();
        assert_eq!(cl.particle_precache[3], "effectinfo.fire");
        let n = "effectinfo.fire".to_string();
        assert_eq!(
            cl.fte_queue,
            vec![
                FteRequest::Effect { name: n.clone(), org: [1.0, 2.0, 3.0], dir: Some([0.0; 3]), count: 1.0 },
                FteRequest::Effect { name: n.clone(), org: [1.0, 2.0, 3.0], dir: Some([0.0, 0.0, 10.0]), count: 5.0 },
                FteRequest::Trail { name: n, start: [0.0; 3], end: [8.0, 0.0, 0.0], entity: 7, interval: Some(1.0) },
                FteRequest::Weather {
                    min: [0.0; 3],
                    max: [64.0; 3],
                    dir: [0.0, 0.0, -30.0],
                    count: 100.0,
                    colour: 12,
                    snow: true
                },
            ]
        );
    }

    #[test]
    fn scripted_names_replace_classic_effects() {
        let mut cl = ClientState::new();
        cl.fte_names = FteNames::new(["TE_EXPLOSION2".to_string(), "te_gunshot".into()]);
        assert!(cl.fte_names.has("te_explosion2_10_8"));
        assert!(cl.fte_effect("TE_GUNSHOT", [0.0; 3], 20.0));
        assert!(!cl.fte_effect("TE_SPIKE", [0.0; 3], 1.0));
        assert_eq!(cl.fte_queue.len(), 1);
    }
}
