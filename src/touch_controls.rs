use crate::draw::Canvas;
use qinput::touch::{BUTTONS, PAUSE_BUTTON, TouchLayout, TouchOverlay};

/// Paint the same controls in gameplay and in the settings preview.
pub fn draw(c: &mut Canvas, layout: &TouchLayout, input: Option<&TouchOverlay>, area: [f32; 4], opacity: f32, menu_only: bool) {
    let [x, y, w, h] = area;
    let a = opacity.clamp(0.05, 1.0);
    let fill = [0.04, 0.035, 0.03, 0.85 * a];
    let edge = [0.96, 0.81, 0.60, 0.85 * a];
    let disc = |c: &mut Canvas, p: [f32; 2], r: f32, fill: [f32; 4], edge| {
        let (x, y, r) = (x + p[0] * w, y + p[1] * h, r * h);
        c.round_box(x - r, y - r, r * 2.0, r * 2.0, r, fill, Some(edge));
    };
    for (i, button) in BUTTONS.iter().enumerate() {
        if !layout.button_enabled(i) || (menu_only && i != PAUSE_BUTTON) { continue; }
        let (p, r) = layout.button(i);
        let held = input.is_some_and(|input| input.pressed(i));
        let bg = if held { [0.65, 0.40, 0.15, 0.9 * a] } else { fill };
        disc(c, p, r, bg, edge);
        let label = qcore::lang::tr(button.label);
        let size = (r * h * 0.44).min(16.0);
        let size = size * (r * h * 1.75 / c.text_w_raw(&label, size).max(1.0)).min(1.0);
        let tx = x + p[0] * w - c.text_w_raw(&label, size) * 0.5;
        let ty = y + p[1] * h - size * 0.5;
        c.text_raw(tx, ty, &label, size, [1.0, 0.95, 0.86, 0.9 * a.max(0.5)]);
    }
    if menu_only { return; }
    let r = layout.stick_radius();
    if let Some((origin, pos)) = input.and_then(TouchOverlay::stick) {
        disc(c, origin, r, fill, edge);
        let dx = (pos[0] - origin[0]) * layout.aspect;
        let dy = pos[1] - origin[1];
        let k = (r / dx.hypot(dy).max(1e-6)).min(1.0);
        let knob = [origin[0] + dx * k / layout.aspect, origin[1] + dy * k];
        disc(c, knob, r * 0.46, [0.96, 0.81, 0.60, 0.65 * a], edge);
    } else {
        let origin = layout.resting_stick();
        disc(c, origin, r, [0.04, 0.035, 0.03, 0.45 * a], edge);
        disc(c, origin, r * 0.46, [0.96, 0.81, 0.60, 0.28 * a], edge);
    }
}
