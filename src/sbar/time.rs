//! HUD → Time & Stats: a clock (`hud_clock`), the level timer with the
//! personal best beside it (`hud_timer`, `hud_pb`), and kills and secrets on
//! one item (`hud_kills`, `hud_secrets`). Each of the three items has its own
//! anchor; items sharing one flow onto the same line and wrap into rows
//! when the line gets too wide. They stack after the modern HUD's boxes at
//! that anchor.

use super::HudAnchor;
use crate::draw::Canvas;

/// Row height and spacing, in canvas units (8-unit text).
const ROW_H: f32 = 12.0;
const PAD: f32 = 3.0;
const GAP: f32 = 4.0;
/// Space between items on one line.
const ITEM_GAP: f32 = 10.0;
const MARGIN: f32 = 8.0;
const LABEL: [f32; 4] = [0.85, 0.7, 0.35, 1.0];
const VALUE: [f32; 4] = crate::draw::WHITE;

/// Settings from the `hud_*` time cvars.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TimeConfig {
    /// 0 off, 1 24-hour, 2 24-hour with seconds, 3 12-hour, 4 12-hour with seconds.
    pub clock: u8,
    pub clock_pos: HudAnchor,
    /// 0 off, else the decimals shown plus one (1 whole seconds .. 3 hundredths).
    pub timer: u8,
    pub pb: bool,
    /// The PB shown (and preferred on save cards) is 100% rather than any%.
    pub pb_full: bool,
    pub pb_color: [f32; 4],
    pub timer_pos: HudAnchor,
    /// 0 off, 1 count, 2 percent.
    pub kills: u8,
    pub secrets: u8,
    pub stats_pos: HudAnchor,
}

impl TimeConfig {
    pub fn from_cvars(cv: &qcore::cvar::Cvars) -> Self {
        let n = |name: &str, max: f32| cv.value_of(name).clamp(0.0, max) as u8;
        TimeConfig {
            clock: n("hud_clock", 4.0),
            clock_pos: HudAnchor::from_cvar(cv.value_of("hud_clock_pos")),
            timer: n("hud_timer", 3.0),
            pb: cv.value_of("hud_pb") != 0.0,
            pb_full: cv.value_of("hud_pb_100") != 0.0,
            pb_color: crate::reticle::color_of(cv, "hud_pb_color"),
            timer_pos: HudAnchor::from_cvar(cv.value_of("hud_timer_pos")),
            kills: n("hud_kills", 2.0),
            secrets: n("hud_secrets", 2.0),
            stats_pos: HudAnchor::from_cvar(cv.value_of("hud_stats_pos")),
        }
    }
}

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
    /// Deathmatch: no kills, secrets or personal best.
    pub deathmatch: bool,
    /// Local wall clock: hour, minute, second.
    pub clock: (u32, u32, u32),
}

/// A run of text in one colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Span {
    pub text: String,
    pub color: [f32; 4],
}

/// One HUD item: spans drawn side by side at an anchor.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub anchor: HudAnchor,
    pub spans: Vec<Span>,
}

fn span(text: impl Into<String>, color: [f32; 4]) -> Span {
    Span { text: text.into(), color }
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

/// The clock in `fmt` (see [`TimeConfig::clock`]).
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

/// `12/40` or `30%` (a level with nothing to find counts as complete).
fn count(fmt: u8, (n, total): (i32, i32)) -> String {
    if fmt == 2 {
        let pct = if total <= 0 { 100 } else { (n.max(0) as i64 * 100 / total as i64).min(100) };
        format!("{pct}%")
    } else {
        format!("{n}/{total}")
    }
}

/// The items to draw, in layout order: clock, timer + PB, kills + secrets.
pub fn items(cfg: &TimeConfig, info: &LevelInfo) -> Vec<Item> {
    let mut out = Vec::new();
    if cfg.clock > 0 {
        out.push(Item { anchor: cfg.clock_pos, spans: vec![span(format_clock(info.clock, cfg.clock), VALUE)] });
    }
    // the personal best rides on the timer's line
    if cfg.timer > 0 {
        let decimals = cfg.timer - 1;
        let mut timer = vec![span(format_timer(info.time, decimals), VALUE)];
        if let (true, false, Some(pb)) = (cfg.pb, info.deathmatch, info.pb) {
            let label = if cfg.pb_full { "  PB 100% " } else { "  PB " };
            let t = pb.map_or_else(|| "--:--".to_string(), |t| format_timer(f64::from(t), decimals));
            timer.push(span(format!("{label}{t}"), cfg.pb_color));
        }
        out.push(Item { anchor: cfg.timer_pos, spans: timer });
    }
    let mut stats = Vec::new();
    if !info.deathmatch {
        for (fmt, label, v) in [(cfg.kills, "K ", info.kills), (cfg.secrets, "S ", info.secrets)] {
            if fmt > 0 {
                let gap = if stats.is_empty() { "" } else { "  " };
                stats.push(span(format!("{gap}{label}"), LABEL));
                stats.push(span(count(fmt, v), VALUE));
            }
        }
    }
    if !stats.is_empty() {
        out.push(Item { anchor: cfg.stats_pos, spans: stats });
    }
    out
}

/// A laid-out line: its anchor, the items on it and its width.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub anchor: HudAnchor,
    pub items: Vec<usize>,
    pub width: f32,
}

/// Flow layout: items sharing an anchor share a line while it stays within
/// `max_w`, then wrap to a new row. `widths` holds each item's anchor and
/// width; rows come out grouped by anchor in item order.
pub fn flow(widths: &[(HudAnchor, f32)], max_w: f32) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    for (i, &(anchor, w)) in widths.iter().enumerate() {
        match rows.iter_mut().rev().find(|r| r.anchor == anchor) {
            Some(r) if r.width + ITEM_GAP + w <= max_w => {
                r.width += ITEM_GAP + w;
                r.items.push(i);
            }
            _ => rows.push(Row { anchor, items: vec![i], width: w }),
        }
    }
    rows.sort_by_key(|r| r.anchor as usize);
    rows
}

/// Space already taken at each anchor before the time items are placed.
#[derive(Clone, Copy, Debug, Default)]
pub struct Reserved {
    /// Per anchor, from the screen edge (the modern HUD's stacks).
    pub used: [f32; HudAnchor::COUNT],
    /// Taken at the bottom of the screen (the classic status bar).
    pub bottom: f32,
}

/// Draws the items with a dark backing per row.
pub fn draw(c: &mut Canvas, items: &[Item], reserved: Reserved) {
    if items.is_empty() {
        return;
    }
    c.reset_origin();
    let (vw, vh) = (c.vw(), c.vh());
    let item_w = |c: &Canvas, it: &Item| it.spans.iter().map(|s| c.ptext_w(&s.text)).sum::<f32>();
    let widths: Vec<_> = items.iter().map(|it| (it.anchor, item_w(c, it))).collect();
    let rows = flow(&widths, (vw * 0.4).max(120.0));
    for anchor_rows in rows.chunk_by(|a, b| a.anchor == b.anchor) {
        let a = anchor_rows[0].anchor;
        let total = anchor_rows.len() as f32 * (ROW_H + GAP) - GAP;
        let used = reserved.used[a as usize];
        let mut y = match a.row() {
            0 => MARGIN + used,
            1 if used > 0.0 => ((vh + used) / 2.0).floor() + GAP,
            1 => ((vh - total) / 2.0).floor(),
            _ => vh - reserved.bottom - MARGIN - used - total,
        };
        for row in anchor_rows {
            let w = row.width + 2.0 * PAD;
            let x = match a.col() {
                0 => MARGIN,
                1 => ((vw - w) / 2.0).floor(),
                _ => vw - MARGIN - w,
            };
            c.fill(x, y, w, ROW_H, [0.0, 0.0, 0.0, 0.45]);
            let mut tx = x + PAD;
            for (k, &i) in row.items.iter().enumerate() {
                if k > 0 {
                    tx += ITEM_GAP;
                }
                for s in &items[i].spans {
                    tx += c.ptext(tx, y + 2.0, &s.text, s.color);
                }
            }
            y += ROW_H + GAP;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use HudAnchor::*;

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

    #[test]
    fn items_put_pb_with_timer_and_stats_together() {
        let cfg = TimeConfig {
            clock: 0,
            clock_pos: TopRight,
            timer: 3,
            pb: true,
            pb_full: false,
            pb_color: [0.0, 1.0, 0.0, 1.0],
            timer_pos: TopLeft,
            kills: 1,
            secrets: 2,
            stats_pos: TopLeft,
        };
        let info = LevelInfo { time: 61.0, pb: Some(Some(55.5)), kills: (4, 10), secrets: (1, 2), ..Default::default() };
        let it = items(&cfg, &info);
        assert_eq!(it.len(), 2);
        let text = |i: &Item| i.spans.iter().map(|s| s.text.as_str()).collect::<String>();
        assert_eq!(text(&it[0]), "1:01.00  PB 0:55.50");
        assert_eq!(it[0].spans[1].color, cfg.pb_color);
        assert_eq!(text(&it[1]), "K 4/10  S 50%");
        // no PB yet; demos (None) hide it; deathmatch drops PB and stats
        assert_eq!(text(&items(&cfg, &LevelInfo { pb: Some(None), ..info.clone() })[0]), "1:01.00  PB --:--");
        assert_eq!(text(&items(&cfg, &LevelInfo { pb: None, ..info.clone() })[0]), "1:01.00");
        assert_eq!(items(&cfg, &LevelInfo { deathmatch: true, ..info.clone() }).len(), 1);
        // no timer, no personal best
        assert_eq!(items(&TimeConfig { timer: 0, ..cfg }, &info).len(), 1);
        assert_eq!(format_timer(f64::from(65.53f32), 2), "1:05.53");
    }

    #[test]
    fn flow_inlines_then_wraps_per_anchor() {
        let rows = flow(&[(TopRight, 40.0), (TopLeft, 50.0), (TopRight, 60.0), (TopRight, 80.0)], 120.0);
        let shape: Vec<_> = rows.iter().map(|r| (r.anchor, r.items.clone(), r.width)).collect();
        assert_eq!(shape, [(TopLeft, vec![1], 50.0), (TopRight, vec![0, 2], 40.0 + ITEM_GAP + 60.0), (TopRight, vec![3], 80.0)]);
    }
}
