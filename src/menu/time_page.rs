//! Options → HUD → Time & Stats: the clock, the level timer with its
//! personal best, and kills and secrets (`sbar::time`); and HUD →
//! Performance Metrics (`host/metrics.rs`). Items at the same position share
//! a line.

use super::pages::{HUD_POS, Item, XHAIR_COLOR, toggle};

pub(super) const CLOCK: &[(&str, f32)] =
    &[("off", 0.0), ("24-hour", 1.0), ("24-hour + seconds", 2.0), ("12-hour", 3.0), ("12-hour + seconds", 4.0)];
pub(super) const TIMER: &[(&str, f32)] = &[("off", 0.0), ("1:23", 1.0), ("1:23.4", 2.0), ("1:23.45", 3.0)];
pub(super) const PB_KIND: &[(&str, f32)] = &[("any%", 0.0), ("100%", 1.0)];
pub(super) const COUNT: &[(&str, f32)] = &[("off", 0.0), ("count", 1.0), ("percent", 2.0)];

pub(super) const TIME: &[Item] = &[
    Item::Choice { label: "Clock", cvar: "hud_clock", choices: CLOCK },
    Item::Choice { label: "Clock Position", cvar: "hud_clock_pos", choices: HUD_POS },
    Item::Choice { label: "Level Timer", cvar: "hud_timer", choices: TIMER },
    toggle("Personal Best", "hud_pb"),
    Item::Choice { label: "Personal Best Type", cvar: "hud_pb_100", choices: PB_KIND },
    Item::Choice { label: "Personal Best Color", cvar: "hud_pb_color", choices: XHAIR_COLOR },
    Item::Choice { label: "Timer Position", cvar: "hud_timer_pos", choices: HUD_POS },
    Item::Choice { label: "Kills", cvar: "hud_kills", choices: COUNT },
    Item::Choice { label: "Secrets", cvar: "hud_secrets", choices: COUNT },
    Item::Choice { label: "Kills & Secrets Position", cvar: "hud_stats_pos", choices: HUD_POS },
];

pub(super) const METRICS: &[Item] = &[
    toggle("Show FPS", "scr_showfps"),
    toggle("Frame Time", "scr_showframetime"),
    toggle("CPU Usage", "scr_showcpu"),
    toggle("Memory Usage", "scr_showram"),
    toggle("GPU Time", "scr_showgpu"),
    toggle("Video Memory", "scr_showvram"),
    Item::Choice { label: "Metrics Position", cvar: "scr_metrics_pos", choices: HUD_POS },
];
