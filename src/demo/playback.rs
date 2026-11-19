//! Playback controls state: speed (ironwail's `basedemospeed` with held
//! fast-forward / rewind), the timeline index, snapshots for seeking and
//! the clip marks.

use std::sync::Arc;
use std::sync::mpsc::Receiver;

use super::index::DemoIndex;
use super::keyframes::Keyframes;

/// Fast-forward / rewind multiplier while Left/Right is held (ironwail).
pub const SCRUB: f32 = 5.0;
/// Shift/Ctrl while scrubbing (ironwail).
pub const SLOW: f32 = 0.25;
pub const MIN_SPEED: f32 = 0.25;
pub const MAX_SPEED: f32 = 8.0;

pub struct Playback {
    pub base_speed: f32,
    /// Held Left (-1) / Right (+1).
    pub scrub: i8,
    pub slow: bool,
    /// Maps entered so far minus one (`None` before the first serverinfo).
    pub ord: Option<usize>,
    pub index: Option<Arc<DemoIndex>>,
    pub index_rx: Option<Receiver<DemoIndex>>,
    pub keyframes: Keyframes,
    /// Clip in/out points as running demo time.
    pub marks: [Option<f64>; 2],
    /// Realtime until which the playback bar stays up after a control.
    pub shown_until: f64,
    /// The file as given to `playdemo`, and whether it is an auto-demo.
    pub name: String,
    pub auto: bool,
    /// A seek asked for before the index was ready.
    pub pending_seek: Option<f64>,
}

impl Default for Playback {
    fn default() -> Self {
        Self {
            base_speed: 1.0,
            scrub: 0,
            slow: false,
            ord: None,
            index: None,
            index_rx: None,
            keyframes: Keyframes::default(),
            marks: [None; 2],
            shown_until: 0.0,
            name: String::new(),
            auto: false,
            pending_seek: None,
        }
    }
}

impl Playback {
    /// CL_UpdateDemoSpeed: the speed this frame (negative rewinds).
    pub fn speed(&self, paused: bool) -> f32 {
        let mut s = if self.scrub != 0 {
            f32::from(self.scrub) * SCRUB * self.base_speed
        } else if paused {
            0.0
        } else {
            self.base_speed
        };
        if self.slow {
            s *= SLOW;
        }
        s
    }

    /// Up arrow: double the speed (ironwail; from a pause it only resumes).
    pub fn faster(&mut self, paused: &mut bool) {
        if !*paused {
            self.base_speed = (self.base_speed * 2.0).clamp(MIN_SPEED, MAX_SPEED);
        }
        *paused = false;
    }

    /// Down arrow: halve the speed, pausing below the minimum.
    pub fn slower(&mut self, paused: &mut bool) {
        self.base_speed *= 0.5;
        if self.base_speed < MIN_SPEED {
            self.base_speed = MIN_SPEED;
            *paused = true;
        }
    }

    /// Picks up a finished background index.
    pub fn poll_index(&mut self) {
        if let Some(rx) = &self.index_rx
            && let Ok(idx) = rx.try_recv()
        {
            self.index = Some(Arc::new(idx));
            self.index_rx = None;
        }
    }

    /// Running demo time, if the index is ready and a map is playing.
    pub fn clock(&self, mtime: f64) -> Option<f64> {
        Some(self.index.as_ref()?.clock(self.ord?, mtime))
    }

    pub fn duration(&self) -> Option<f64> {
        self.index.as_ref().map(|i| i.duration)
    }

    /// The clip to export: the marks in order, defaulting to the whole demo.
    pub fn clip(&self) -> (f64, Option<f64>) {
        match self.marks {
            [Some(a), Some(b)] => (a.min(b), Some(a.max(b))),
            [Some(a), None] => (a, None),
            [None, Some(b)] => (0.0, Some(b)),
            [None, None] => (0.0, None),
        }
    }
}

/// `m:ss` (or `h:mm:ss`) for the bar and the Demos menu.
pub fn format_time(t: f64) -> String {
    let s = t.max(0.0) as u64;
    if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) }
}

/// Parses `90`, `1:30`, `1:02:03` (seconds); a leading `+`/`-` is relative.
pub fn parse_time(s: &str) -> Option<(f64, i8)> {
    let (sign, body) = match s.as_bytes().first()? {
        b'+' => (1, &s[1..]),
        b'-' => (-1, &s[1..]),
        _ => (0, s),
    };
    let mut t = 0.0;
    for part in body.split(':') {
        let v: f64 = part.parse().ok().filter(|v: &f64| v.is_finite() && *v >= 0.0)?;
        t = t * 60.0 + v;
    }
    Some((t, sign))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speed_follows_ironwail() {
        let mut p = Playback::default();
        let mut paused = false;
        assert_eq!(p.speed(paused), 1.0);
        p.faster(&mut paused);
        assert_eq!(p.speed(paused), 2.0);
        p.scrub = -1;
        assert_eq!(p.speed(paused), -10.0);
        p.slow = true;
        assert_eq!(p.speed(paused), -2.5);
        p.scrub = 0;
        p.slow = false;
        for _ in 0..4 {
            p.slower(&mut paused);
        }
        assert!(paused);
        assert_eq!(p.base_speed, MIN_SPEED);
        assert_eq!(p.speed(paused), 0.0);
        // scrubbing works while paused
        p.scrub = 1;
        assert_eq!(p.speed(paused), SCRUB * MIN_SPEED);
        // Up from a pause resumes at the same speed
        p.scrub = 0;
        p.faster(&mut paused);
        assert!(!paused);
        assert_eq!(p.base_speed, MIN_SPEED);
    }

    #[test]
    fn clip_orders_marks() {
        let mut p = Playback::default();
        assert_eq!(p.clip(), (0.0, None));
        p.marks = [Some(9.0), Some(3.0)];
        assert_eq!(p.clip(), (3.0, Some(9.0)));
        p.marks = [None, Some(4.0)];
        assert_eq!(p.clip(), (0.0, Some(4.0)));
    }

    #[test]
    fn times() {
        assert_eq!(format_time(65.9), "1:05");
        assert_eq!(format_time(3725.0), "1:02:05");
        assert_eq!(parse_time("90"), Some((90.0, 0)));
        assert_eq!(parse_time("1:30"), Some((90.0, 0)));
        assert_eq!(parse_time("-10"), Some((10.0, -1)));
        assert_eq!(parse_time("+1:00:00"), Some((3600.0, 1)));
        assert_eq!(parse_time("x"), None);
        assert_eq!(parse_time(""), None);
    }
}
