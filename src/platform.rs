//! qplatform glue: SurfaceProvider for the window, platform event to
//! `qinput::InputEvent` conversion (SDL keycodes to Quake key numbers).

use std::ffi::CString;

use qinput::keys::*;
use qinput::{GamepadAxis, GamepadButton, InputEvent};
use qplatform::{Event, Window};

pub struct Surface<'a>(pub &'a Window);

impl qrender::SurfaceProvider for Surface<'_> {
    fn instance_extensions(&self) -> Vec<CString> {
        self.0.vulkan_instance_extensions()
    }
    fn create_surface(&self, instance: u64) -> u64 {
        self.0.create_vulkan_surface(instance).unwrap_or(0)
    }
    fn drawable_size(&self) -> (u32, u32) {
        self.0.drawable_size()
    }
}

/// SDL scancode to Quake key for non-character keys.
fn scancode_key(sc: u32) -> Option<i32> {
    Some(match sc {
        41 => K_ESCAPE,
        40 => K_ENTER,
        42 => K_BACKSPACE,
        43 => K_TAB,
        44 => K_SPACE,
        53 => b'`' as i32,
        57 => K_CAPSLOCK,
        58..=69 => K_F1 + (sc - 58) as i32,
        70 => K_PRINTSCREEN,
        71 => K_SCROLLLOCK,
        72 => K_PAUSE,
        73 => K_INS,
        74 => K_HOME,
        75 => K_PGUP,
        76 => K_DEL,
        77 => K_END,
        78 => K_PGDN,
        79 => K_RIGHTARROW,
        80 => K_LEFTARROW,
        81 => K_DOWNARROW,
        82 => K_UPARROW,
        83 => K_KP_NUMLOCK,
        84 => K_KP_SLASH,
        85 => K_KP_STAR,
        86 => K_KP_MINUS,
        87 => K_KP_PLUS,
        88 => K_KP_ENTER,
        89 => K_KP_END,
        90 => K_KP_DOWNARROW,
        91 => K_KP_PGDN,
        92 => K_KP_LEFTARROW,
        93 => K_KP_5,
        94 => K_KP_RIGHTARROW,
        95 => K_KP_HOME,
        96 => K_KP_UPARROW,
        97 => K_KP_PGUP,
        98 => K_KP_INS,
        99 => K_KP_DEL,
        224 | 228 => K_CTRL,
        225 | 229 => K_SHIFT,
        226 | 230 => K_ALT,
        227 | 231 => K_COMMAND,
        _ => return None,
    })
}

fn pad_button(b: u8) -> Option<GamepadButton> {
    use GamepadButton::*;
    Some(match b {
        0 => South,
        1 => East,
        2 => West,
        3 => North,
        4 => Back,
        5 => Guide,
        6 => Start,
        7 => LeftStick,
        8 => RightStick,
        9 => LeftShoulder,
        10 => RightShoulder,
        11 => DpadUp,
        12 => DpadDown,
        13 => DpadLeft,
        14 => DpadRight,
        15 => Misc1,
        16 => Paddle1,
        17 => Paddle2,
        18 => Paddle3,
        19 => Paddle4,
        20 => Touchpad,
        _ => return None,
    })
}

/// Host-level event after conversion.
pub enum HostEvent {
    Input(InputEvent),
    Quit,
    Resize,
    Focus,
    /// Absolute cursor position in window points (menus).
    MousePos { x: f32, y: f32 },
    /// Button-label family of a newly connected gamepad (`qplatform::family`).
    PadFamily(u8),
}

pub fn convert(e: Event) -> Vec<HostEvent> {
    let one = |e| vec![HostEvent::Input(e)];
    match e {
        Event::Quit => vec![HostEvent::Quit],
        Event::Key { scancode, key, down, .. } => {
            let k = scancode_key(scancode).or_else(|| (1..127).contains(&key).then(|| (key as u8).to_ascii_lowercase() as i32));
            match k {
                Some(k) => one(InputEvent::Key { key: k, down }),
                None => vec![],
            }
        }
        Event::Text(s) => s.chars().map(|c| HostEvent::Input(InputEvent::Text(c))).collect(),
        Event::MouseMotion { x, y, dx, dy } => vec![HostEvent::MousePos { x, y }, HostEvent::Input(InputEvent::MouseMotion { dx, dy })],
        Event::MouseButton { button, down, x, y } => {
            // SDL: 1 left, 2 middle, 3 right; Quake: MOUSE1 left, MOUSE2 right, MOUSE3 middle
            let b = match button {
                2 => 3,
                3 => 2,
                b => b,
            };
            vec![HostEvent::MousePos { x, y }, HostEvent::Input(InputEvent::MouseButton { button: b, down })]
        }
        Event::MouseWheel { dy, .. } => one(InputEvent::MouseWheel { y: dy }),
        Event::GamepadAxis { axis, value, .. } => {
            let a = match axis {
                qplatform::axis::LEFTX => GamepadAxis::LeftX,
                qplatform::axis::LEFTY => GamepadAxis::LeftY,
                qplatform::axis::RIGHTX => GamepadAxis::RightX,
                qplatform::axis::RIGHTY => GamepadAxis::RightY,
                qplatform::axis::LEFT_TRIGGER => GamepadAxis::LeftTrigger,
                qplatform::axis::RIGHT_TRIGGER => GamepadAxis::RightTrigger,
                _ => return vec![],
            };
            one(InputEvent::GamepadAxis { axis: a, value: (value as f32 / 32767.0).clamp(-1.0, 1.0) })
        }
        Event::GamepadButton { button, down, .. } => match pad_button(button) {
            Some(b) => one(InputEvent::GamepadButton { button: b, down }),
            None => vec![],
        },
        Event::GamepadAdded { family, .. } => {
            vec![HostEvent::Input(InputEvent::GamepadConnected(true)), HostEvent::PadFamily(family)]
        }
        Event::GamepadRemoved { .. } => one(InputEvent::GamepadConnected(false)),
        Event::GamepadGyro { data, .. } => one(InputEvent::Gyro { pitch: data[0], yaw: data[1] }),
        Event::Resize { .. } => vec![HostEvent::Resize],
        Event::FocusGained | Event::FocusLost => vec![HostEvent::Focus],
    }
}
