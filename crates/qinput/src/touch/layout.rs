use super::{BUTTONS, HIT_SCALE, MOVE_FIRE, QUICK_LOAD, QUICK_SAVE, STICK_RADIUS, SWIM_DOWN, TouchButtons};

pub const LAYOUT_CVAR: &str = "touch_layout";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ButtonPlacement {
    /// Safe-area fractions in the right-handed layout.
    pub center: [f32; 2],
    pub size: f32,
}

/// Window shape and control scale, from the host.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchLayout {
    /// Width / height.
    pub aspect: f32,
    /// `touch_size` (1 = default).
    pub size: f32,
    pub left_handed: bool,
    /// Left, top, right and bottom insets, normalised to the window.
    pub insets: [f32; 4],
    pub placements: [Option<ButtonPlacement>; BUTTONS.len()],
    pub buttons: TouchButtons,
    pub move_fire: bool,
}

impl TouchLayout {
    pub fn new(aspect: f32, size: f32, left_handed: bool, insets: [f32; 4]) -> Self {
        Self {
            aspect: aspect.max(0.1), size, left_handed, insets, placements: [None; BUTTONS.len()],
            buttons: TouchButtons::default(), move_fire: false,
        }
    }

    fn scale(&self) -> f32 {
        let [l, t, r, b] = self.insets;
        let half_width = (1.0 - l - r) * self.aspect * 0.5;
        let height = 1.0 - t - b;
        // Limit the scale so the buttons fit as the safe rectangle narrows.
        self.size.clamp(0.5, 2.0).min(half_width / 0.48).min(height / 0.60).max(0.01)
    }

    fn mid_x(&self) -> f32 {
        (self.insets[0] + 1.0 - self.insets[2]) * 0.5
    }

    pub(super) fn move_side(&self, x: f32) -> bool {
        if self.left_handed { x >= self.mid_x() } else { x < self.mid_x() }
    }

    pub(super) fn contains(&self, [x, y]: [f32; 2]) -> bool {
        let [l, t, r, b] = self.insets;
        (l..=1.0 - r).contains(&x) && (t..=1.0 - b).contains(&y)
    }

    pub fn from_cvars(cv: &qcore::cvar::Cvars, aspect: f32, insets: [f32; 4]) -> Self {
        let mut layout = Self::new(aspect, cv.value_of("touch_size"), cv.value_of("touch_left_handed") >= 0.5, insets);
        layout.move_fire = cv.value_of("touch_move_fire") != 0.0;
        if let Some(config) = cv.by_name(LAYOUT_CVAR) {
            for (slot, entry) in layout.placements.iter_mut().zip(config.string.split(';')) {
                let mut values = entry.split_whitespace().map(str::parse::<f32>);
                if let (Some(Ok(x)), Some(Ok(y)), Some(Ok(size)), None) = (values.next(), values.next(), values.next(), values.next())
                    && [x, y, size].iter().all(|v| v.is_finite())
                {
                    *slot = Some(ButtonPlacement { center: [x.clamp(0.0, 1.0), y.clamp(0.0, 1.0)], size: size.clamp(0.5, 2.0) });
                }
            }
        }
        layout
    }

    pub fn save(&self, cv: &mut qcore::cvar::Cvars) {
        let value = self.placements.map(|p| p.map_or_else(|| "-".into(), |p| {
            format!("{:.5} {:.5} {:.3}", p.center[0], p.center[1], p.size)
        })).join(";");
        cv.set(LAYOUT_CVAR, &value);
    }

    pub fn hits(&self, index: usize, pos: [f32; 2]) -> bool {
        let (center, radius) = self.button(index);
        self.dist(pos, center) <= radius * HIT_SCALE
    }

    pub fn button_at(&self, pos: [f32; 2]) -> Option<usize> {
        (0..BUTTONS.len()).filter(|&i| self.button_enabled(i)).filter_map(|i| {
            let (center, radius) = self.button(i);
            let d = self.dist(pos, center);
            (d <= radius * HIT_SCALE).then_some((i, d))
        }).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i)
    }

    pub fn button_enabled(&self, index: usize) -> bool {
        match index {
            SWIM_DOWN => self.buttons.submerged,
            QUICK_SAVE => self.buttons.quicksave,
            QUICK_LOAD => self.buttons.quickload,
            MOVE_FIRE => self.move_fire,
            _ => true,
        }
    }

    /// Centre in window fractions, radius in height units. Keep the full hit
    /// area inside the safe rectangle even after a size or aspect change.
    pub fn button(&self, index: usize) -> ([f32; 2], f32) {
        let b = &BUTTONS[index];
        let [l, t, r, bot] = self.insets;
        let (w, h) = (1.0 - l - r, 1.0 - t - bot);
        let s = self.scale();
        let (mut pos, size) = if let Some(p) = self.placements[index] {
            ([l + p.center[0] * w, t + p.center[1] * h], p.size)
        } else {
            let [x, y] = b.at;
            let x = if x >= 0.0 { 1.0 - r - x * s / self.aspect } else { l - x * s / self.aspect };
            let y = if y >= 0.0 { 1.0 - bot - y * s } else { t - y * s };
            ([x, y], 1.0)
        };
        if self.left_handed {
            pos[0] = l + 1.0 - r - pos[0];
        }
        let radius = (b.radius * s * size).min((w * self.aspect).min(h) * 0.49 / HIT_SCALE);
        let hy = radius * HIT_SCALE;
        let hx = hy / self.aspect;
        ([pos[0].clamp(l + hx, 1.0 - r - hx), pos[1].clamp(t + hy, 1.0 - bot - hy)], radius)
    }

    pub fn placement(&self, index: usize) -> ButtonPlacement {
        self.placements[index].unwrap_or_else(|| self.placement_at(self.button(index).0, 1.0))
    }

    pub fn placement_at(&self, [x, y]: [f32; 2], size: f32) -> ButtonPlacement {
        let [l, t, r, b] = self.insets;
        let x = ((x - l) / (1.0 - l - r)).clamp(0.0, 1.0);
        let x = if self.left_handed { 1.0 - x } else { x };
        let y = ((y - t) / (1.0 - t - b)).clamp(0.0, 1.0);
        ButtonPlacement { center: [x, y], size: size.clamp(0.5, 2.0) }
    }

    pub fn stick_radius(&self) -> f32 {
        STICK_RADIUS * self.scale()
    }

    pub fn resting_stick(&self) -> [f32; 2] {
        let [l, t, r, b] = self.insets;
        let dx = (self.stick_radius() + 0.08 * self.scale()) / self.aspect;
        let x = if self.left_handed { 1.0 - r - dx } else { l + dx };
        [x, t + (1.0 - t - b) * 0.58]
    }

    /// Distance between two normalised points in height units.
    pub(super) fn dist(&self, a: [f32; 2], b: [f32; 2]) -> f32 {
        ((a[0] - b[0]) * self.aspect).hypot(a[1] - b[1])
    }
}
