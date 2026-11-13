use super::*;

/// Crosshair colours (`crosshair_color` index).
pub(super) const CROSSHAIR_COLORS: [[f32; 4]; 6] = [
    [1.0, 1.0, 1.0, 1.0],
    [1.0, 0.85, 0.1, 1.0],
    [0.2, 1.0, 0.3, 1.0],
    [0.2, 0.9, 1.0, 1.0],
    [1.0, 0.3, 0.9, 1.0],
    [1.0, 0.25, 0.2, 1.0],
];

/// A crosshair at the scene centre: shape from `crosshair_shape` (0 cross,
/// 1 gap cross, 2 dot, 3 cross + dot, 4 T, 5 box), sized by `crosshair_size`
/// with an optional dark outline (`crosshair_outline`) so it reads on any wall.
pub(super) fn draw_crosshair(c: &mut Canvas, scene_h: f32, cv: &qcore::cvar::Cvars, locked: bool) {
    let size = cv.value_of("crosshair_size").clamp(0.5, 4.0);
    let color = CROSSHAIR_COLORS[(cv.value_of("crosshair_color").max(0.0) as usize).min(CROSSHAIR_COLORS.len() - 1)];
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
    let bars: Vec<(f32, f32, f32, f32)> = match cv.value_of("crosshair_shape") as i32 {
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
    if cv.value_of("crosshair_outline") != 0.0 {
        let o = (base * 0.5).round().max(1.0);
        for &(x, y, w, h) in &bars {
            c.fill(x - o, y - o, w + 2.0 * o, h + 2.0 * o, [0.0, 0.0, 0.0, 0.85]);
        }
    }
    for (x, y, w, h) in bars {
        c.fill(x, y, w, h, color);
    }
    if locked {
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
}
