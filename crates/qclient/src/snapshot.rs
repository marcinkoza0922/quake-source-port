//! Demo seeking: copies of the client state without the effects that only
//! live for a moment (particles, beams, lights, queued events).

use crate::state::*;

impl ClientState {
    /// A copy for a seek snapshot, transient effects left out.
    pub fn demo_snapshot(&mut self) -> ClientState {
        let particles = std::mem::take(&mut self.particles);
        let events = std::mem::take(&mut self.events);
        let mut s = self.clone();
        self.particles = particles;
        self.events = events;
        s.clear_transient();
        s
    }

    /// Restores `snap`, keeping this state's effect pools (emptied) and the
    /// host-owned settings.
    pub fn demo_restore(&mut self, snap: ClientState) {
        let mut particles = std::mem::take(&mut self.particles);
        particles.clear();
        let keep = (self.cl_nolerp, self.name.clone(), self.color);
        *self = snap;
        self.particles = particles;
        (self.cl_nolerp, self.name, self.color) = keep;
        self.oldtime = self.time;
    }

    /// Drops what a seek would leave behind half-finished.
    pub fn clear_transient(&mut self) {
        self.particles.clear();
        self.beams.clear();
        self.dlights.clear();
        self.temp_entities.clear();
        self.events.clear();
        self.weapon_impacts.clear();
        self.impact_effects.clear();
        self.shot_impacts.clear();
        self.blood_hits.clear();
        self.fte_queue.clear();
        self.ack_frames.clear();
        // ragdolls own entities by number; the host's debris is reset too
        self.debris_owned.clear();
    }

    /// Approximate heap size of a snapshot (for the keyframe budget).
    pub fn snapshot_bytes(&self) -> usize {
        use std::mem::size_of;
        size_of::<Self>()
            + (self.entities.len() + self.static_entities.len()) * size_of::<ClientEntity>()
            + self.model_precache.iter().chain(&self.sound_precache).chain(&self.lightstyles).map(|s| s.len() + 24).sum::<usize>()
    }
}
