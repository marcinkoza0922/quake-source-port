//! Hooks for FTE scripted particles (vkQuake's `PScript_*` calls in
//! cl_tent.c / cl_main.c). The client only decides whether an effect is
//! scripted; the host owns the particle system and drains
//! [`ClientState::fte_queue`].
use std::collections::HashSet;

use crate::state::ClientState;

/// An effect for the FTE particle system to run.
#[derive(Clone, Debug, PartialEq)]
pub enum FteRequest {
    /// PScript_RunParticleEffectTypeString.
    Effect { name: String, org: [f32; 3], dir: Option<[f32; 3]>, count: f32 },
    /// PScript_EntParticleTrail: `entity` keys the trail state and dlight.
    Trail { name: String, start: [f32; 3], end: [f32; 3], entity: i32 },
}

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
        self.fte_queue.push(FteRequest::Trail { name: name.to_string(), start, end, entity });
        true
    }
}
