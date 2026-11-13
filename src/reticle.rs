use crate::draw::Canvas;
use qcore::cvar::Cvars;
pub(crate) mod hitmarker;

/// Crosshair colours (`crosshair_color` index).
pub(crate) const CROSSHAIR_COLORS: [[f32; 4]; 7] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 0.85, 0.1, 1.0],
    [0.2, 1.0, 0.3, 1.0],
    [0.2, 0.9, 1.0, 1.0],
    [1.0, 0.3, 0.9, 1.0],
    [1.0, 0.0, 0.0, 1.0],
    [0.65, 0.65, 0.65, 1.0],
];

/// A crosshair at the scene centre: shape from `crosshair_shape` (0 cross,
/// 1 gap cross, 2 dot, 3 cross + dot, 4 T, 5 box), sized by `crosshair_size`
/// with an optional dark outline (`crosshair_outline`) so it reads on any wall.
pub(crate) fn draw_crosshair(c: &mut Canvas, scene_h: f32, cv: &qcore::cvar::Cvars, locked: bool, active: bool, color: [f32; 4]) {
    let size = cv.value_of("crosshair_size").clamp(0.5, 4.0);
    let base = c.scale;
    // Work in whole pixels so thin strokes stay crisp at any size.
    c.scale = 1.0;
    let (cx, cy) = ((c.w / 2.0).floor(), (scene_h * base / 2.0).floor());
    let arm = (3.5 * size * base).round().max(2.0);
    let t = (size * base * 0.5).round().max(1.0);
    let h = (t / 2.0).floor();
    let gap = (arm * 0.45).round().max(t + 1.0);
    let dot = (t + 1.0).max((size * base).round());
    let dh = (dot / 2.0).floor();
    let full_h = (cx - arm, cy - h, arm * 2.0 + t, t);
    let full_v = (cx - h, cy - arm, t, arm * 2.0 + t);
    let center_dot = (cx - dh, cy - dh, dot, dot);
    let gapped = |down: bool| {
        let mut v = vec![
            (cx - arm, cy - h, arm - gap, t),
            (cx + gap + t, cy - h, arm - gap, t),
            (cx - h, cy + gap + t, t, arm - gap),
        ];
        if !down {
            v.push((cx - h, cy - arm, t, arm - gap));
        }
        v
    };
    let mut bars: Vec<(f32, f32, f32, f32)> = match cv.value_of("crosshair_shape") as i32 {
        1 => gapped(false),
        2 => vec![center_dot],
        3 => {
            let mut v = gapped(false);
            v.push(center_dot);
            v
        }
        4 => {
            let mut v = gapped(true);
            v.push(center_dot);
            v
        }
        5 => {
            let b = (arm * 0.6).round().max(t + 2.0);
            vec![
                (cx - b, cy - b, b * 2.0 + t, t),
                (cx - b, cy + b, b * 2.0 + t, t),
                (cx - b, cy - b + t, t, b * 2.0 - t),
                (cx + b, cy - b + t, t, b * 2.0 - t),
            ]
        }
        _ => vec![full_h, full_v],
    };
    if cv.value_of("crosshair") == 0.0 { bars.clear(); }
    if cv.value_of("crosshair_outline") != 0.0 {
        let o = (base * 0.5).round().max(1.0);
        for &(x, y, w, h) in &bars {
            c.fill(x - o, y - o, w + 2.0 * o, h + 2.0 * o, [0.0, 0.0, 0.0, 0.85]);
        }
    }
    for (x, y, w, h) in bars {
        c.fill(x, y, w, h, color);
    }
    if locked && cv.value_of("assist_autoaim_indicator") != 0.0 {
        // Four small corner ticks preserve the chosen shape and colour.
        let r = arm + 3.0 * base;
        let tick = (2.0 * base).round().max(2.0);
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                let x = cx + sx * r;
                let y = cy + sy * r;
                c.fill(x - if sx > 0.0 { tick } else { 0.0 }, y, tick + t, t, color);
                c.fill(x, y - if sy > 0.0 { tick } else { 0.0 }, t, tick + t, color);
            }
        }
    }
    c.scale = base;
    if active && cv.value_of("assist_autoaim") != 0.0 && cv.value_of("assist_autoaim_mode") == 1.0 {
        // Keep the armed state visible between targets, independently of lock ticks.
        let x = cx / base;
        let y = (cy + arm + 8.0 * base) / base;
        c.text_center(x + 0.5, y + 0.5, "AUTO AIM", 6.0, [0.0, 0.0, 0.0, 0.9]);
        c.text_center(x, y, "AUTO AIM", 6.0, [1.0, 0.85, 0.1, 1.0]);
    }
}

/// One palette and state selection for both reticle layers. Kills take priority.
pub(crate) fn color(cv: &Cvars, target: bool, kill: bool) -> [f32; 4] {
    let name = if kill && cv.value_of("crosshair_onkill") != 0.0 {
        "crosshair_killcolor"
    } else if target && cv.value_of("crosshair_ontarget") != 0.0 {
        "crosshair_targetcolor"
    } else {
        "crosshair_color"
    };
    CROSSHAIR_COLORS[(cv.value_of(name).max(0.0) as usize).min(CROSSHAIR_COLORS.len() - 1)]
}

/// Static comparisons for each shared color state, using the gameplay paths.
pub(crate) fn preview(c: &mut Canvas, cv: &Cvars) {
    let (x, y, w, h) = (508.0, 72.0, 128.0, 266.0);
    c.fill(x, y, w, h, [0.035, 0.04, 0.045, 0.95]);
    c.frame(x, y, w, h, crate::menu::BORDER);
    c.text_center(x + w * 0.25, y + 8.0, "Crosshair", 8.0, crate::menu::GOLD_FG);
    c.text_center(x + w * 0.75, y + 8.0, "+ Marker", 8.0, crate::menu::GOLD_FG);
    for (row, (label, target, kill)) in [("Normal", false, false), ("On target", true, false), ("On kill", false, true)].into_iter().enumerate() {
        let top = y + 26.0 + row as f32 * 76.0;
        c.text_center(x + w * 0.5, top, label, 8.0, crate::menu::FG);
        let tint = color(cv, target, kill);
        for with_marker in [false, true] {
            let mut p = Canvas::new((w * 0.5 * c.scale) as u32, (60.0 * c.scale) as u32);
            p.scale = c.scale;
            draw_crosshair(&mut p, 60.0, cv, false, false, tint);
            if with_marker {
                hitmarker::draw_preview(&mut p, 60.0, cv, 1.0, tint);
            }
            let left = x + if with_marker { w * 0.5 } else { 0.0 };
            crate::draw::translate(&mut p.list, (c.ox + left) * c.scale, (c.oy + top + 8.0) * c.scale);
            c.list.append(&mut p.list);
        }
    }
    if cv.value_of("hud_hitmarkers") == 0.0 {
        c.text_center(x + w * 0.5, y + h - 12.0, "Hit markers off", 8.0, crate::menu::MUTED);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_colors_respect_toggles_and_kill_priority() {
        let mut cv = Cvars::new();
        for (name, value) in [("crosshair_color", "6"), ("crosshair_targetcolor", "0"),
            ("crosshair_killcolor", "5"), ("crosshair_ontarget", "1"), ("crosshair_onkill", "1")] {
            cv.register(name, value, 0);
        }
        assert_eq!(color(&cv, false, false), [0.65, 0.65, 0.65, 1.0]);
        assert_eq!(color(&cv, true, false), [1.0; 4]);
        assert_eq!(color(&cv, true, true), [1.0, 0.0, 0.0, 1.0]);
        cv.set("crosshair_onkill", "0");
        assert_eq!(color(&cv, true, true), [1.0; 4]);
        cv.set("crosshair_ontarget", "0");
        assert_eq!(color(&cv, true, true), color(&cv, false, false));
    }
}
