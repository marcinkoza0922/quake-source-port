//! qplatform — window, events, gamepads, audio stream and timer behind one
//! API with two backends: SDL3 natively ([`sdl`]) and the browser DOM /
//! WebAudio / Gamepad API on wasm32 ([`web`]). See docs/DESIGN.md
//! "Milestone 4 crate boundaries" and docs/WEB_PORT.md.
//!
//! All calls must happen on the thread that created [`Platform`]
//! (enforced by `!Send`), except the audio callback which SDL runs on its
//! own thread.

#[cfg(not(target_arch = "wasm32"))]
mod sdl;
#[cfg(not(target_arch = "wasm32"))]
pub use sdl::*;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::*;

/// Platform (SDL or browser) error string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

/// Gamepad axis ids (SDL_GamepadAxis values).
pub mod axis {
    pub const LEFTX: u8 = 0;
    pub const LEFTY: u8 = 1;
    pub const RIGHTX: u8 = 2;
    pub const RIGHTY: u8 = 3;
    pub const LEFT_TRIGGER: u8 = 4;
    pub const RIGHT_TRIGGER: u8 = 5;
}

/// Gamepad button ids (SDL_GamepadButton values).
pub mod button {
    pub const SOUTH: u8 = 0;
    pub const EAST: u8 = 1;
    pub const WEST: u8 = 2;
    pub const NORTH: u8 = 3;
    pub const BACK: u8 = 4;
    pub const GUIDE: u8 = 5;
    pub const START: u8 = 6;
    pub const LEFT_STICK: u8 = 7;
    pub const RIGHT_STICK: u8 = 8;
    pub const LEFT_SHOULDER: u8 = 9;
    pub const RIGHT_SHOULDER: u8 = 10;
    pub const DPAD_UP: u8 = 11;
    pub const DPAD_DOWN: u8 = 12;
    pub const DPAD_LEFT: u8 = 13;
    pub const DPAD_RIGHT: u8 = 14;
}

/// Gamepad button-label families (see [`Event::GamepadAdded`]).
pub mod family {
    pub const XBOX: u8 = 0;
    pub const PLAYSTATION: u8 = 1;
    pub const NINTENDO: u8 = 2;
}

/// Platform event. Key codes are raw SDL values: `scancode` is
/// `SDL_Scancode` (layout independent), `key` is `SDL_Keycode`, `modifiers`
/// is `SDL_Keymod`. Mouse buttons: 1 left, 2 middle, 3 right, 4/5 X1/X2.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Quit,
    Key { scancode: u32, key: u32, modifiers: u16, down: bool, repeat: bool },
    Text(String),
    MouseMotion { x: f32, y: f32, dx: f32, dy: f32 },
    MouseButton { button: u8, down: bool, x: f32, y: f32 },
    MouseWheel { dx: f32, dy: f32 },
    /// Gamepad axis; value in -32768..=32767 (triggers 0..=32767).
    GamepadAxis { id: u32, axis: u8, value: i16 },
    GamepadButton { id: u32, button: u8, down: bool },
    /// Gamepad opened (hotplug or present at startup). `family` is the
    /// button-label family: [`family::XBOX`], [`family::PLAYSTATION`] or
    /// [`family::NINTENDO`].
    GamepadAdded { id: u32, name: String, family: u8 },
    GamepadRemoved { id: u32 },
    /// Gyro in rad/s (x pitch, y yaw, z roll), only if enabled.
    GamepadGyro { id: u32, data: [f32; 3], timestamp_ns: u64 },
    /// Touchscreen finger; `x`/`y` normalised to the window (0..1). Touches
    /// no longer also arrive as mouse events.
    Finger { id: u64, phase: FingerPhase, x: f32, y: f32 },
    /// Drawable (pixel) size changed.
    Resize { width: u32, height: u32 },
    FocusGained,
    FocusLost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FingerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

/// Label family from a vendor id (0 if unknown) and the device name.
pub fn family_from_vendor_name(vendor: u16, name: &str) -> u8 {
    match vendor {
        0x054c => return family::PLAYSTATION,
        0x057e => return family::NINTENDO,
        _ => {}
    }
    let n = name.to_lowercase();
    let has = |l: &[&str]| l.iter().any(|w| n.contains(w));
    if has(&["dualsense", "dualshock", "playstation", "sony", "ps4", "ps5"]) {
        family::PLAYSTATION
    } else if has(&["nintendo", "pro controller", "joy-con", "switch"]) {
        family::NINTENDO
    } else {
        family::XBOX
    }
}


/// Window presentation mode for `Window::set_display`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DisplayMode {
    Windowed,
    /// Exclusive fullscreen (changes the display mode).
    Fullscreen,
    /// Fullscreen window at desktop resolution.
    Borderless,
}
