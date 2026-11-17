//! perfkit: the shared runner behind qbench (Part 1) and qprof (Part 2) of
//! the performance plan (`docs/perf/README.md`).
//!
//! - [`engine`]: which engines exist and where their binaries are.
//! - [`sandbox`]: a base dir of symlinks to the game data, without anyone's
//!   saved engine config, that runs may write into.
//! - [`guard`]: snapshots the configs a run must never change.
//! - [`runner`]: one windowed timedemo, directly or inside gamescope,
//!   optionally with a MangoHud per-frame log of exactly the demo's frames.
//! - [`mangohud`]: the MangoHud control socket and log parsing.
//! - [`settings`]: Part 1's "vanilla ultra" pins in each engine's cvars.
//! - [`stats`]: medians, spreads and percentiles.
//! - [`machine`]: the test machine's description for report headers.

pub mod engine;
pub mod guard;
pub mod machine;
pub mod mangohud;
pub mod runner;
pub mod sandbox;
pub mod settings;
pub mod stats;

/// Minimal JSON string escaping for the hand-written reports.
pub fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    #[test]
    fn json_escape() {
        assert_eq!(super::json_str("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }
}
