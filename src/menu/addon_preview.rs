//! Silent looping gameplay recordings; playback never touches the live game.
use super::*;

pub(super) fn draw(c: &mut Canvas, pic: &mut PicFn, id: &str, realtime: f64, rect: [f32; 4]) {
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
