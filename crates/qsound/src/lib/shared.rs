//! The mixer shared between the audio device and video capture. Normally
//! the device callback paints; while capturing, the game thread paints
//! exactly one video frame of samples per host frame (so audio and video
//! stay in step at any speed) and the device plays those samples back from
//! a short queue, or silence when exporting faster than real time.

use std::collections::VecDeque;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::Mixer;

/// Most monitor samples kept (interleaved), about half a second at 48 kHz.
const MONITOR_MAX: usize = 48_000;

pub struct SharedMixer {
    mixer: Mutex<Mixer>,
    capturing: AtomicBool,
    monitor_on: AtomicBool,
    monitor: Mutex<VecDeque<i16>>,
}

impl SharedMixer {
    pub fn new(m: Mixer) -> Self {
        Self { mixer: Mutex::new(m), capturing: AtomicBool::new(false), monitor_on: AtomicBool::new(false), monitor: Mutex::default() }
    }

    pub fn rate(&self) -> u32 {
        self.mixer.lock().unwrap().rate
    }

    /// The audio device callback.
    pub fn paint_device(&self, out: &mut [i16]) {
        if !self.capturing.load(Ordering::Acquire) {
            self.mixer.lock().unwrap().paint(out);
            return;
        }
        let mut q = self.monitor.lock().unwrap();
        for o in out.iter_mut() {
            *o = q.pop_front().unwrap_or(0);
        }
    }

    /// Game thread takes over painting; `monitor` plays the captured audio.
    pub fn capture_begin(&self, monitor: bool) {
        self.monitor.lock().unwrap().clear();
        self.monitor_on.store(monitor, Ordering::Release);
        self.capturing.store(true, Ordering::Release);
    }

    pub fn capture_end(&self) {
        self.capturing.store(false, Ordering::Release);
        self.monitor.lock().unwrap().clear();
    }

    /// Paints `frames` stereo frames for the capture (interleaved).
    pub fn capture_paint(&self, frames: usize) -> Vec<i16> {
        let mut buf = vec![0i16; frames * 2];
        self.mixer.lock().unwrap().paint(&mut buf);
        if self.monitor_on.load(Ordering::Acquire) {
            let mut q = self.monitor.lock().unwrap();
            q.extend(&buf);
            let over = q.len().saturating_sub(MONITOR_MAX);
            q.drain(..over);
        }
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SoundSystem;

    #[test]
    fn capture_paints_on_the_game_thread_and_feeds_the_monitor() {
        let (_sys, mixer) = SoundSystem::new(48000, |_| None);
        let m = SharedMixer::new(mixer);
        assert_eq!(m.rate(), 48000);
        m.capture_begin(true);
        assert_eq!(m.capture_paint(800).len(), 1600);
        // the device plays back what was captured, then silence
        let mut out = [7i16; 2000];
        m.paint_device(&mut out);
        assert!(out.iter().all(|&s| s == 0));
        assert!(m.monitor.lock().unwrap().is_empty());
        // the queue stays bounded when nothing drains it
        for _ in 0..200 {
            m.capture_paint(800);
        }
        assert!(m.monitor.lock().unwrap().len() <= MONITOR_MAX);
        m.capture_end();
        assert!(m.monitor.lock().unwrap().is_empty());
    }
}
