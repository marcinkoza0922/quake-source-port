//! Text of the HUD's Time & Stats elements (`crate::hud`): the clock, the
//! level timer and personal best, and kills and secrets.

/// What the items show this frame.
#[derive(Clone, Debug, Default)]
pub struct LevelInfo {
    /// Level time in seconds (frozen at the exit).
    pub time: f64,
    /// Personal best for this map, skill and category, if any (None also
    /// hides it, e.g. in demos).
    pub pb: Option<Option<f32>>,
    pub kills: (i32, i32),
    pub secrets: (i32, i32),
    /// Horizontal speed in units per second (50 ms peak hold).
    pub speed: i32,
    /// Deathmatch: no kills, secrets or personal best.
    pub deathmatch: bool,
    /// Local wall clock: hour, minute, second.
    pub clock: (u32, u32, u32),
}

/// `m:ss`, `m:ss.d` or `m:ss.cc` (`h:mm:ss` past an hour); `decimals` 0..=2.
pub fn format_timer(t: f64, decimals: u8) -> String {
    let scale = 10u64.pow(u32::from(decimals));
    // truncate, as speedrun timers do (the slack absorbs f32 storage error)
    let ticks = (t.max(0.0) * scale as f64 + 1e-3) as u64;
    let (s, frac) = (ticks / scale, ticks % scale);
    let mut out = if s >= 3600 { format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60) } else { format!("{}:{:02}", s / 60, s % 60) };
    if decimals > 0 {
        out.push_str(&format!(".{frac:0w$}", w = decimals as usize));
    }
    out
}

/// The clock in `fmt`: 1 24-hour, 2 with seconds, 3 12-hour, 4 with seconds.
pub fn format_clock((h, m, s): (u32, u32, u32), fmt: u8) -> String {
    let secs = matches!(fmt, 2 | 4);
    if fmt >= 3 {
        let (h12, pm) = (if h % 12 == 0 { 12 } else { h % 12 }, h >= 12);
        let ampm = if pm { "PM" } else { "AM" };
        if secs { format!("{h12}:{m:02}:{s:02} {ampm}") } else { format!("{h12}:{m:02} {ampm}") }
    } else if secs {
        format!("{h:02}:{m:02}:{s:02}")
    } else {
        format!("{h:02}:{m:02}")
    }
}

/// `12/40` (`fmt` 1) or `30%` (2; a level with nothing to find counts as complete).
pub fn count(fmt: u8, (n, total): (i32, i32)) -> String {
    if fmt == 2 {
        let pct = if total <= 0 { 100 } else { (n.max(0) as i64 * 100 / total as i64).min(100) };
        format!("{pct}%")
    } else {
        format!("{n}/{total}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_and_clock_formats() {
        assert_eq!(format_timer(83.456, 0), "1:23");
        assert_eq!(format_timer(83.456, 1), "1:23.4");
        assert_eq!(format_timer(83.456, 2), "1:23.45");
        assert_eq!(format_timer(3725.0, 2), "1:02:05.00");
        assert_eq!(format_timer(-1.0, 0), "0:00");
        assert_eq!(format_clock((14, 5, 9), 1), "14:05");
        assert_eq!(format_clock((14, 5, 9), 2), "14:05:09");
        assert_eq!(format_clock((14, 5, 9), 3), "2:05 PM");
        assert_eq!(format_clock((0, 30, 0), 4), "12:30:00 AM");
        assert_eq!(count(2, (3, 0)), "100%");
        assert_eq!(count(2, (1, 3)), "33%");
        assert_eq!(count(1, (1, 3)), "1/3");
    }
}
