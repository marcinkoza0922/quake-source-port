//! Brief confirmation around the selected crosshair, independent of its visibility.

use crate::draw::Canvas;
use qcore::cvar::Cvars;

type Segment = ([f32; 2], [f32; 2]);

/// `remaining` is the unexpired fraction of the confirmation's lifetime.
pub(super) fn draw_hitmarker(c: &mut Canvas, scene_h: f32, cv: &Cvars, remaining: f32, kill: bool) {
    if cv.value_of("hud_hitmarkers") == 0.0 || !remaining.is_finite() || remaining <= 0.0 {
        return;
    }
    let remaining = remaining.min(1.0);
    let base = c.scale;
    let size = cv.value_of("crosshair_size").clamp(0.5, 4.0);
    let center = [(c.w * 0.5).floor(), (scene_h * base * 0.5).floor()];
    let unit = size * base;
    // An outward drift helps separate each confirmation from the static reticle.
    let radius = 6.0 + (1.0 - remaining) * 2.0;
    let segments = marker_segments(cv.value_of("crosshair_shape") as i32, radius);
    let thickness = (0.65 * unit).round().max(1.0);
    let outline = (0.5 * base).round().max(1.0);
    let alpha = (remaining * 3.0).min(1.0);
    let color = if kill {
        [1.0, 0.22, 0.08, alpha]
    } else {
        [1.0, 1.0, 1.0, alpha]
    };
    c.scale = 1.0;
    // Paint every outline before the foreground, so adjacent strokes join cleanly.
    for (width, color) in [
        (thickness + 2.0 * outline, [0.0, 0.0, 0.0, alpha * 0.85]),
        (thickness, color),
    ] {
        for &(a, b) in &segments {
            let a = [center[0] + a[0] * unit, center[1] + a[1] * unit];
            let b = [center[0] + b[0] * unit, center[1] + b[1] * unit];
            pixel_line(c, a, b, width, color);
        }
    }
    c.scale = base;
}

fn marker_segments(shape: i32, r: f32) -> Vec<Segment> {
    let mut lines = Vec::with_capacity(8);
    match shape {
        // Gap cross: four compact outward-facing corner brackets.
        1 => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    lines.push(([x * r, y * (r + 3.0)], [x * (r + 3.0), y * (r + 3.0)]));
                    lines.push(([x * (r + 3.0), y * r], [x * (r + 3.0), y * (r + 3.0)]));
                }
            }
        }
        // Dot: a diamond surrounding the aiming point.
        2 => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    lines.push(([x * (r + 2.0), 0.0], [0.0, y * (r + 2.0)]));
                }
            }
        }
        // Cross + dot: a pair of diagonal ticks at each corner.
        3 => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    for offset in [0.0, 4.0] {
                        lines.push((
                            [x * (r + offset), y * (r + offset)],
                            [x * (r + offset + 2.0), y * (r + offset + 2.0)],
                        ));
                    }
                }
            }
        }
        // T: two chevrons below the otherwise open top of the reticle.
        4 => {
            for offset in [0.0, 3.0] {
                for x in [-1.0, 1.0] {
                    lines.push(([x * 3.0, r + offset], [0.0, r + offset + 2.0]));
                }
            }
        }
        // Box: a larger broken square, with gaps at all four corners.
        5 => {
            for sign in [-1.0, 1.0] {
                lines.push(([-r + 2.0, sign * r], [r - 2.0, sign * r]));
                lines.push(([sign * r, -r + 2.0], [sign * r, r - 2.0]));
            }
        }
        // Cross: the familiar four diagonal hit-confirmation ticks.
        _ => {
            for x in [-1.0, 1.0] {
                for y in [-1.0, 1.0] {
                    lines.push(([x * r, y * r], [x * (r + 3.0), y * (r + 3.0)]));
                }
            }
        }
    }
    lines
}

/// Native-pixel strokes, including crisp diagonal stair steps, need no new atlas.
fn pixel_line(c: &mut Canvas, a: [f32; 2], b: [f32; 2], width: f32, color: [f32; 4]) {
    let half = (width * 0.5).floor();
    let a = a.map(f32::round);
    let b = b.map(f32::round);
    if a[1] == b[1] {
        c.fill(
            a[0].min(b[0]) - half,
            a[1] - half,
            (b[0] - a[0]).abs() + width,
            width,
            color,
        );
        return;
    }
    let (a, b) = if a[1] < b[1] { (a, b) } else { (b, a) };
    if a[0] == b[0] {
        c.fill(a[0] - half, a[1] - half, width, b[1] - a[1] + width, color);
        return;
    }
    let slope = (b[0] - a[0]) / (b[1] - a[1]);
    let x_at = |y: f32| (a[0] + (y.clamp(a[1], b[1]) - a[1]) * slope).round();
    // Each scanline is emitted once: overlapping translucent pixel stamps
    // would make thick strokes refuse to fade out uniformly.
    let top = (a[1] - half) as i32;
    let bottom = (b[1] - half + width) as i32;
    for y in top..bottom {
        let x0 = x_at(y as f32 - width + half + 1.0);
        let x1 = x_at(y as f32 + half);
        c.fill(
            x0.min(x1) - half,
            y as f32,
            (x1 - x0).abs() + width,
            1.0,
            color,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_crosshair_has_a_distinct_marker() {
        let shapes: Vec<_> = (0..6).map(|shape| marker_segments(shape, 6.0)).collect();
        for (i, a) in shapes.iter().enumerate() {
            for b in &shapes[i + 1..] {
                assert_ne!(a, b);
            }
        }
    }

    #[test]
    fn confirmation_respects_scene_center_and_restores_scale() {
        let mut cv = Cvars::new();
        cv.register("hud_hitmarkers", "1", 0);
        cv.register("crosshair_size", "1", 0);
        cv.register("crosshair_shape", "0", 0);
        let mut c = Canvas::new(960, 720);
        let scale = c.scale;
        draw_hitmarker(&mut c, 192.0, &cv, 1.0, true);
        assert_eq!(c.scale, scale);
        let mut bounds = [f32::MAX, f32::MAX, f32::MIN, f32::MIN];
        for d in &c.list {
            if let qrender::Draw2D::Fill { x, y, w, h, .. } = d {
                bounds[0] = bounds[0].min(*x);
                bounds[1] = bounds[1].min(*y);
                bounds[2] = bounds[2].max(x + w);
                bounds[3] = bounds[3].max(y + h);
            }
        }
        assert!(((bounds[0] + bounds[2]) * 0.5 - 480.0).abs() <= 1.0);
        assert!(((bounds[1] + bounds[3]) * 0.5 - 288.0).abs() <= 1.0);
        c.list.clear();
        draw_hitmarker(&mut c, 192.0, &cv, 0.0, false);
        assert!(c.list.is_empty());
    }
}
