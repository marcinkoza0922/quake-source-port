//! Silent looping gameplay recordings; playback never touches the live game.
use super::*;

pub(super) fn draw(c: &mut Canvas, pic: &mut PicFn, id: &str, realtime: f64, rect: [f32; 4]) {
    if matches!(id, "projectileshotguns" | "bullettime") {
        draw_simulation(c, id, realtime, rect);
        return;
    }
    let frame = (realtime.max(0.0) * crate::draw::addon_video::FPS as f64) as usize % crate::draw::addon_video::FRAMES;
    let [x, y, w, h] = rect;
    c.round_box(x, y, w, h, 2.0, [0.015, 0.012, 0.01, 1.0], Some(BORDER));
    if let Some((p, pw, ph)) = pic(&format!("addons/previews/{id}/{frame:03}.tga")) {
        let scale = (w / pw.max(1) as f32).min(h / ph.max(1) as f32);
        let (width, height) = (pw as f32 * scale, ph as f32 * scale);
        c.pic_stretch_px([(x + (w - width) / 2.0 + c.ox) * c.scale, (y + (h - height) / 2.0 + c.oy) * c.scale, width * c.scale, height * c.scale], p);
        c.text(x + 6.0, y + 6.0, "GAMEPLAY", 7.0, MUTED);
        c.text(x + 6.0, y + h - 12.0, "LOOP", 7.0, MUTED);
    } else {
        c.text_center(x + w / 2.0, y + h / 2.0, "Preview unavailable", TXT, MUTED);
    }
}

/// Procedural demonstrations for gameplay modifiers: no map or live state needed.
fn draw_simulation(c: &mut Canvas, id: &str, time: f64, [x, y, w, h]: [f32; 4]) {
    c.round_box(x, y, w, h, 2.0, [0.025, 0.035, 0.045, 1.0], Some(BORDER));
    let bullet_time = id == "bullettime";
    c.text(x + 8.0, y + 8.0, "DEMONSTRATION", 7.0, MUTED);
    for row in 0..2 {
        let cy = y + 40.0 + row as f32 * 45.0;
        c.fill(x + 20.0, cy + 12.0, w - 40.0, 1.0, BORDER);
        c.fill(x + 24.0, cy - 5.0, 18.0, 15.0, [0.5, 0.6, 0.65, 1.0]);
        c.fill(x + w - 48.0, cy - 9.0, 14.0, 20.0, [0.6, 0.3, 0.2, 1.0]);
        let speed = if row == 1 { 0.2 } else { 1.0 };
        let t = (time * speed).rem_euclid(1.5) as f32 / 1.5;
        let px = x + 46.0 + t * (w - 100.0);
        for pellet in -2i32..=2 {
            let py = cy + pellet as f32 * (1.0 + t * 3.0);
            c.fill(px - pellet.abs() as f32 * 2.0, py, 5.0, 1.5, [0.9, 0.75, 0.35, 1.0]);
        }
        let label = if row == 0 { "NORMAL SPEED" } else if bullet_time { "BULLET TIME: 20%" } else { "SLOW MOTION: PHYSICAL PELLETS" };
        c.text(x + 50.0, cy - 19.0, label, 7.0, MUTED);
    }
}
