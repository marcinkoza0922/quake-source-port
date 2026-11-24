//! The red dot: drawn where the bullets go (a collimated sight's dot stays on
//! the aim point whatever the rifle does) while the tube shows that spot.
use crate::draw::Canvas;
use qcore::math::{Vec3, angle_vectors};
use qrender::{PicId, View};

/// `addon_rifle_dot` choices.
const COLOURS: [[f32; 3]; 3] = [[1.0, 0.10, 0.06], [0.15, 1.0, 0.25], [1.0, 0.8, 0.15]];
/// Radius of the dot (tangent).
const SIZE: f32 = 0.0065;

/// Where a direction falls in a view: pixels right of and below its centre,
/// for a scene `h` pixels tall.
fn project(view: &View, dir: Vec3, h: f32) -> Option<(f32, f32)> {
    let (f, r, u) = angle_vectors(view.angles.into());
    let z = dir.dot(f);
    if z < 0.1 {
        return None;
    }
    let k = h * 0.5 / half_tan(view);
    Some((dir.dot(r) / z * k, -dir.dot(u) / z * k))
}

/// Vertical half FOV tangent (the FOV is given at 4:3).
fn half_tan(view: &View) -> f32 {
    (view.fov_x.clamp(1.0, 179.0).to_radians() * 0.5).tan() * 0.75
}

/// A raised sight in one view.
#[derive(Clone, Copy, Debug)]
pub struct Sight {
    pub view: View,
    /// Where the player aims.
    pub angles: [f32; 3],
    /// How visible the dot is.
    pub alpha: f32,
    pub colour: usize,
    pub pic: Option<PicId>,
}

impl Sight {
    /// Draws the dot in a scene `scene_h` canvas units tall.
    pub fn draw(&self, c: &mut Canvas, scene_h: f32) {
        let Some(pic) = self.pic.filter(|_| self.alpha > 0.0) else { return };
        let h = scene_h * c.scale;
        let Some((dx, dy)) = project(&self.view, angle_vectors(self.angles.into()).0, h) else { return };
        let (x, y) = (c.w / 2.0 + dx, h / 2.0 + dy);
        let [r, g, b] = COLOURS[self.colour.min(COLOURS.len() - 1)];
        let size = (SIZE / half_tan(&self.view) * h * 0.5).max(1.5);
        // the picture falls off from the middle: the halo, then the dot itself
        for (k, rgb, a) in [(5.0, [r, g, b], 0.45), (2.2, [r, g, b], 1.0), (1.7, [r, g, b], 1.0), (0.9, [1.0, 0.8 + g * 0.2, 0.75 + b * 0.2], 0.8)] {
            let s = size * k;
            c.pic_stretch_px_color([x - s, y - s, s * 2.0, s * 2.0], pic, [rgb[0], rgb[1], rgb[2], a * self.alpha]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dot_follows_the_aim_not_the_kicked_view() {
        let view = View { angles: [0.0, 90.0, 0.0], fov_x: 90.0, ..Default::default() };
        let at = |angles: [f32; 3]| project(&view, angle_vectors(angles.into()).0, 480.0).unwrap();
        let (x, y) = at([0.0, 90.0, 0.0]);
        assert!(x.abs() < 0.01 && y.abs() < 0.01);
        // aiming below and right of the view: 10 degrees down, 5 right
        let (x, y) = at([10.0, 85.0, 0.0]);
        assert!(x > 20.0 && y > 50.0, "{x} {y}");
        // the top of a 480 pixel scene is the vertical FOV
        let (_, y) = at([-(0.75f32.atan().to_degrees()), 90.0, 0.0]);
        assert!((y + 240.0).abs() < 0.1, "{y}");
    }
}
