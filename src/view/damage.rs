//! Directional damage vignette, in each player's HUD coordinates.
use super::ViewState;
use crate::draw::Canvas;

impl ViewState {
    pub fn draw_damage(&self, c: &mut Canvas, height: f32, enabled: bool, flash: f32) {
        if !enabled {
            return;
        }
        let width = c.vw();
        let depth = width.min(height) * 0.12;
        // Thin bands approximate a smooth fade while leaving the center clear.
        for (side, strength) in self.edges.iter().enumerate() {
            for band in 0..24 {
                let t = band as f32 / 24.0;
                let alpha = strength * flash.clamp(0.0, 1.0) * (1.0 - t).powi(2);
                if alpha <= 0.001 {
                    continue;
                }
                let offset = depth * t;
                let step = depth / 24.0;
                let (x, y, w, h) = match side {
                    0 => (0.0, offset, width, step),
                    1 => (width - offset - step, 0.0, step, height),
                    2 => (0.0, height - offset - step, width, step),
                    _ => (offset, 0.0, step, height),
                };
                c.fill(x, y, w, h, [0.9, 0.0, 0.0, alpha]);
            }
        }
    }
}
