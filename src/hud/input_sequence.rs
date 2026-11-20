//! A bounded, chronological sequence of physical gameplay inputs.

use std::collections::VecDeque;

use qinput::{GamepadAxis, Input, InputEvent, keys::*};

use crate::draw::Canvas;
use crate::glyphs::{self, Family, Glyphs};

pub(super) const SIZE: (f32, f32) = (184.0, 18.0);
const LIMIT: usize = 8;
const GLYPH_H: f32 = 14.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Press {
    Key(i32, Family),
    Stick { right: bool, arrow: &'static str },
}

#[derive(Default)]
pub struct Sequence {
    presses: VecDeque<Press>,
    stick_directions: [i8; 4],
}

impl Sequence {
    pub(crate) fn reset_sticks(&mut self) {
        self.stick_directions = [0; 4];
    }

    pub(crate) fn record(
        &mut self,
        event: InputEvent,
        input: &Input,
        family: Family,
        trigger_threshold: f32,
    ) {
        let key = match event {
            InputEvent::Key { key, down: true }
                if key > 0 && key < MAX_KEYS as i32 && key != K_ESCAPE =>
            {
                key
            }
            InputEvent::MouseButton { button, down: true } if (1..=5).contains(&button) => {
                K_MOUSE1 + i32::from(button) - 1
            }
            InputEvent::MouseWheel { y } if y > 0.0 => K_MWHEELUP,
            InputEvent::MouseWheel { y } if y < 0.0 => K_MWHEELDOWN,
            InputEvent::GamepadButton { button, down: true } => button.key(),
            InputEvent::GamepadAxis { axis, value } => {
                let i = axis as usize;
                if i < self.stick_directions.len() {
                    self.record_stick(i, value);
                    return;
                }
                if value <= trigger_threshold {
                    return;
                }
                if axis == GamepadAxis::LeftTrigger {
                    K_LTRIGGER
                } else {
                    K_RTRIGGER
                }
            }
            _ => return,
        };
        if !input.key_down(key) {
            self.push(Press::Key(key, family));
        }
    }

    fn record_stick(&mut self, i: usize, value: f32) {
        let previous = self.stick_directions[i];
        let direction = if value >= 0.55 {
            1
        } else if value <= -0.55 {
            -1
        } else if value * f32::from(previous) <= 0.35 {
            0
        } else {
            previous
        };
        self.stick_directions[i] = direction;
        if direction != 0 && direction != previous {
            let arrow = match (i % 2, direction) {
                (0, -1) => "←",
                (0, _) => "→",
                (_, -1) => "↑",
                (_, _) => "↓",
            };
            self.push(Press::Stick {
                right: i >= 2,
                arrow,
            });
        }
    }

    fn push(&mut self, press: Press) {
        if self.presses.len() == LIMIT {
            self.presses.pop_front();
        }
        self.presses.push_back(press);
    }

    pub(super) fn draw(
        &self,
        c: &mut Canvas,
        (w, h): (f32, f32),
        bg: bool,
        preview: bool,
        glyph_override: f32,
    ) {
        let sample = [
            Press::Key(b'w' as i32, Family::Xbox),
            Press::Key(b'a' as i32, Family::Xbox),
            Press::Key(K_SPACE, Family::Xbox),
            Press::Key(K_MOUSE1, Family::Xbox),
            Press::Key(K_MWHEELUP, Family::Xbox),
            Press::Stick {
                right: false,
                arrow: "→",
            },
            Press::Key(K_ABUTTON, Family::Xbox),
            Press::Key(K_RTRIGGER, Family::Xbox),
        ];
        let mut presses = [None; LIMIT];
        let n = if preview {
            presses = sample.map(Some);
            sample.len()
        } else {
            for (slot, press) in presses.iter_mut().zip(&self.presses) {
                *slot = Some(*press);
            }
            self.presses.len()
        };
        if n == 0 {
            return;
        }
        if bg {
            c.fill(0.0, 0.0, w, h, super::draw::BACKING);
        }

        let measure = |c: &Canvas, press, height| match press {
            Press::Key(key, family) => {
                glyphs::width(c, key, family.with_override(glyph_override), height)
            }
            Press::Stick { .. } => height * 1.55,
        };
        let available = w - 6.0;
        let mut height = GLYPH_H;
        let total = |c: &Canvas, height| {
            presses
                .iter()
                .flatten()
                .map(|&p| measure(c, p, height))
                .sum::<f32>()
        };
        let width = total(c, height);
        if width > available {
            // Leave a few units for rounding glyph widths at the new size.
            height *= (available - 4.0) / width;
        }
        let mut x = w - 3.0;
        let y = (h - height) / 2.0;
        for &press in presses[..n].iter().rev().flatten() {
            x -= measure(c, press, height);
            if x < 2.0 {
                break;
            }
            match press {
                Press::Key(key, family) => {
                    Glyphs {
                        family: family.with_override(glyph_override),
                    }
                    .key_h(c, x, y, key, height);
                }
                Press::Stick { right, arrow } => {
                    let w = height * 1.55;
                    let s = height * 0.52;
                    let label = if right {
                        format!("RS{arrow}")
                    } else {
                        format!("LS{arrow}")
                    };
                    c.round_box(
                        x + 1.0,
                        y + 1.0,
                        w - 2.0,
                        height - 2.0,
                        2.0,
                        [0.36, 0.36, 0.4, 0.95],
                        None,
                    );
                    c.text_center(
                        x + w / 2.0,
                        glyphs::center_text_y(y, height, s),
                        &label,
                        s,
                        crate::draw::WHITE,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_latest_mixed_presses_and_filters_analog_jitter() {
        let mut sequence = Sequence::default();
        let mut cvars = qcore::cvar::Cvars::new();
        let input = Input::new(&mut cvars);
        for key in b'a'..=b'j' {
            sequence.record(
                InputEvent::Key {
                    key: i32::from(key),
                    down: true,
                },
                &input,
                Family::Xbox,
                0.2,
            );
        }
        assert_eq!(sequence.presses.len(), LIMIT);
        assert_eq!(
            sequence.presses.front(),
            Some(&Press::Key(b'c' as i32, Family::Xbox))
        );

        for value in [0.6, 0.51, 0.56, 0.4] {
            sequence.record(
                InputEvent::GamepadAxis {
                    axis: GamepadAxis::LeftX,
                    value,
                },
                &input,
                Family::PlayStation,
                0.2,
            );
        }
        assert_eq!(sequence.presses[6], Press::Key(b'j' as i32, Family::Xbox));
        assert_eq!(
            sequence.presses[7],
            Press::Stick {
                right: false,
                arrow: "→"
            }
        );

        sequence.record(
            InputEvent::MouseButton {
                button: 2,
                down: true,
            },
            &input,
            Family::Xbox,
            0.2,
        );
        sequence.record(
            InputEvent::GamepadAxis {
                axis: GamepadAxis::RightTrigger,
                value: 0.4,
            },
            &input,
            Family::PlayStation,
            0.2,
        );
        assert_eq!(sequence.presses[6], Press::Key(K_MOUSE2, Family::Xbox));
        assert_eq!(
            sequence.presses[7],
            Press::Key(K_RTRIGGER, Family::PlayStation)
        );
    }
}
