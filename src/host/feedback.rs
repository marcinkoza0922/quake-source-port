//! Combat feedback state and game-thread synthesis cache.
use super::*;
use qsound::feedback::{SoundKind, SoundSpec};
use std::sync::Arc;

#[derive(Default)]
pub(crate) struct Feedback {
    until: f64,
    pub(crate) kill: bool,
    next_sound: f64,
}

impl Feedback {
    /// Keep a kill visible through following pellets/rapid-fire hits.
    pub(crate) fn trigger(&mut self, now: f64, kill: bool) -> bool {
        let was_kill = self.kill && self.until > now;
        self.kill = kill || was_kill;
        if kill || !was_kill {
            self.until = now + if kill { 0.35 } else { 0.22 };
        }
        let play = kill || (!was_kill && now >= self.next_sound);
        if play { self.next_sound = now + 0.045; }
        play
    }

    pub(crate) fn remaining(&self, now: f64) -> f32 {
        ((self.until - now) / if self.kill { 0.35 } else { 0.22 }).clamp(0.0, 1.0) as f32
    }
}

#[derive(Default)]
pub(super) struct CueCache {
    cues: [Option<(SoundSpec, u32, Arc<qsound::Sfx>)>; 2],
}

fn spec(cv: &Cvars, kill: bool) -> SoundSpec {
    let prefix = if kill { "s_kill_" } else { "s_hit_" };
    let v = |name: &str| cv.value_of(&format!("{prefix}{name}"));
    SoundSpec {
        kind: SoundKind::ALL[(v("kind").max(0.0) as usize).min(SoundKind::ALL.len() - 1)],
        volume: v("volume"), length: v("length"), speed: v("speed"), pitch: v("pitch"),
        bass: v("bass"), treble: v("treble"), drive: v("drive"), echo: v("echo"), reverb: v("reverb"),
    }.sanitized()
}

impl Host {
    pub(super) fn receive_feedback(&mut self, kill: bool) {
        if self.feedback.trigger(self.realtime, kill) && self.cv("s_hitfeedback") != 0.0 {
            self.play_feedback(kill);
        }
    }

    pub(super) fn guest_feedback(&mut self) {
        for i in 0..self.guests.len() {
            if let Some(kill) = self.guests[i].pending_feedback.take()
                && self.guests[i].feedback.trigger(self.realtime, kill)
                && self.cv("s_hitfeedback") != 0.0
            {
                self.play_feedback(kill);
            }
        }
    }

    /// Preview bypasses the gameplay toggle; master SFX volume still applies.
    pub(super) fn play_feedback(&mut self, kill: bool) {
        let spec = spec(&self.server.core.cvars, kill);
        let Some(snd) = &self.snd else { return };
        let rate = snd.rate();
        let entry = &mut self.feedback_sounds.cues[usize::from(kill)];
        if entry.as_ref().is_none_or(|(old, old_rate, _)| *old != spec || *old_rate != rate) {
            *entry = Some((spec, rate, Arc::new(spec.render(rate))));
        }
        if let Some((_, _, cue)) = entry {
            snd.event(SoundEvent::Local(cue.clone()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kills_win_over_following_pellets_then_expire() {
        let mut f = Feedback::default();
        assert!(f.trigger(1.0, true));
        assert!(!f.trigger(1.01, false));
        assert!(f.kill);
        assert!(f.remaining(1.1) > 0.0);
        assert_eq!(f.remaining(2.0), 0.0);
        assert!(f.trigger(2.0, false));
        assert!(!f.kill);
    }

    #[test]
    fn rapid_hits_throttle_sound_but_refresh_marker() {
        let mut f = Feedback::default();
        assert!(f.trigger(1.0, false));
        assert!(!f.trigger(1.01, false));
        assert!(f.remaining(1.22) > 0.0);
        assert!(f.trigger(1.05, false));
        assert!(f.trigger(1.06, true));
    }
}
