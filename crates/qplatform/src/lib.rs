//! qplatform — safe SDL3 wrapper (window, Vulkan surface, events, gamepads,
//! audio stream, timer). See docs/DESIGN.md "Milestone 4 crate boundaries".
//!
//! All SDL calls must happen on the thread that created [`Platform`]
//! (enforced by `!Send`), except the audio callback which SDL runs on its
//! own thread.

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_int, c_void};
use std::ptr;
use std::rc::Rc;

use sdl3_sys::everything as sys;

/// SDL error string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn sdl_error() -> Error {
    // SAFETY: SDL_GetError returns a valid NUL-terminated thread-local string (never null).
    let s = unsafe { CStr::from_ptr(sys::SDL_GetError()) };
    Error(s.to_string_lossy().into_owned())
}

fn check(ok: bool) -> Result<()> {
    if ok { Ok(()) } else { Err(sdl_error()) }
}

fn cstring(s: &str) -> Result<CString> {
    CString::new(s).map_err(|_| Error("string contains NUL".into()))
}

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
    /// Gamepad opened (hotplug or present at startup).
    GamepadAdded { id: u32, name: String },
    GamepadRemoved { id: u32 },
    /// Gyro in rad/s (x pitch, y yaw, z roll), only if enabled.
    GamepadGyro { id: u32, data: [f32; 3], timestamp_ns: u64 },
    /// Drawable (pixel) size changed.
    Resize { width: u32, height: u32 },
    FocusGained,
    FocusLost,
}

/// SDL context. Owns subsystems and opened gamepads. `!Send`/`!Sync`.
pub struct Platform {
    gamepads: HashMap<u32, Gamepad>,
    sdl: Rc<SdlGuard>,
}

/// Shared SDL lifetime token: SDL_Quit runs when the last holder drops.
struct SdlGuard;

impl Drop for SdlGuard {
    fn drop(&mut self) {
        // SAFETY: matches the SDL_Init in Platform::init_with_hints; all
        // windows/streams/gamepads hold this guard and are gone by now.
        unsafe { sys::SDL_Quit() };
    }
}

struct Gamepad {
    raw: *mut sys::SDL_Gamepad,
    has_gyro: bool,
}

impl Platform {
    /// Initialise video, events, gamepad and audio.
    pub fn init() -> Result<Self> {
        Self::init_with_hints(&[])
    }

    /// Like [`init`](Self::init) but sets SDL hints first, e.g.
    /// `("SDL_VIDEO_DRIVER", "dummy")`, `("SDL_AUDIO_DRIVER", "dummy")`.
    pub fn init_with_hints(hints: &[(&str, &str)]) -> Result<Self> {
        for (k, v) in hints {
            let (k, v) = (cstring(k)?, cstring(v)?);
            // SAFETY: both pointers are valid NUL-terminated strings for the call.
            unsafe { sys::SDL_SetHint(k.as_ptr(), v.as_ptr()) };
        }
        let flags = sys::SDL_INIT_VIDEO | sys::SDL_INIT_EVENTS | sys::SDL_INIT_GAMEPAD | sys::SDL_INIT_AUDIO;
        // SAFETY: plain init call; paired with SDL_Quit in Drop.
        check(unsafe { sys::SDL_Init(flags) })?;
        // Gamepad add events for already-connected devices arrive via the
        // event queue, so open happens lazily in poll_event.
        Ok(Self { gamepads: HashMap::new(), sdl: Rc::new(SdlGuard) })
    }

    /// Add mappings from a gamecontrollerdb.txt (SDL's built-in DB is always
    /// active). Returns number of mappings added.
    pub fn add_gamepad_mappings_from_file(&self, path: &str) -> Result<u32> {
        let p = cstring(path)?;
        // SAFETY: valid C string; SDL initialised.
        let n = unsafe { sys::SDL_AddGamepadMappingsFromFile(p.as_ptr()) };
        if n < 0 { Err(sdl_error()) } else { Ok(n as u32) }
    }

    /// Create a resizable, high-DPI window, optionally Vulkan-capable.
    pub fn create_window(&self, title: &str, width: u32, height: u32, vulkan: bool) -> Result<Window> {
        let t = cstring(title)?;
        let mut flags = sys::SDL_WINDOW_RESIZABLE | sys::SDL_WINDOW_HIGH_PIXEL_DENSITY;
        if vulkan {
            flags |= sys::SDL_WINDOW_VULKAN;
        }
        // SAFETY: valid title pointer; SDL video initialised.
        let raw = unsafe { sys::SDL_CreateWindow(t.as_ptr(), width as c_int, height as c_int, flags) };
        if raw.is_null() {
            return Err(sdl_error());
        }
        Ok(Window { raw, _sdl: self.sdl.clone() })
    }

    /// Poll one event. Gamepad hotplug opens/closes devices internally.
    /// Unhandled SDL events are skipped.
    pub fn poll_event(&mut self) -> Option<Event> {
        loop {
            let mut ev = std::mem::MaybeUninit::<sys::SDL_Event>::zeroed();
            // SAFETY: ev points to writable SDL_Event storage.
            if !unsafe { sys::SDL_PollEvent(ev.as_mut_ptr()) } {
                return None;
            }
            // SAFETY: SDL_PollEvent returned true, so ev is initialised.
            let ev = unsafe { ev.assume_init() };
            if let Some(e) = self.translate(&ev) {
                return Some(e);
            }
        }
    }

    /// Drain all pending events.
    pub fn poll_events(&mut self) -> Vec<Event> {
        std::iter::from_fn(|| self.poll_event()).collect()
    }

    fn translate(&mut self, ev: &sys::SDL_Event) -> Option<Event> {
        use sys::SDL_EventType as T;
        // SAFETY: the `type` tag is valid for every event and selects which
        // union member SDL initialised (relied on by the reads below).
        let ty = T(unsafe { ev.r#type });
        Some(match ty {
            T::QUIT => Event::Quit,
            T::KEY_DOWN | T::KEY_UP => {
                // SAFETY: tag is a keyboard event.
                let k = unsafe { ev.key };
                Event::Key { scancode: k.scancode.0 as u32, key: k.key.0, modifiers: k.r#mod.0, down: k.down, repeat: k.repeat }
            }
            T::TEXT_INPUT => {
                // SAFETY: tag is text input; `text` is a valid C string owned by SDL for this event.
                let s = unsafe { CStr::from_ptr(ev.text.text) };
                Event::Text(s.to_string_lossy().into_owned())
            }
            T::MOUSE_MOTION => {
                // SAFETY: tag is mouse motion.
                let m = unsafe { ev.motion };
                Event::MouseMotion { x: m.x, y: m.y, dx: m.xrel, dy: m.yrel }
            }
            T::MOUSE_BUTTON_DOWN | T::MOUSE_BUTTON_UP => {
                // SAFETY: tag is mouse button.
                let b = unsafe { ev.button };
                Event::MouseButton { button: b.button, down: b.down, x: b.x, y: b.y }
            }
            T::MOUSE_WHEEL => {
                // SAFETY: tag is mouse wheel.
                let w = unsafe { ev.wheel };
                let s = if w.direction == sys::SDL_MOUSEWHEEL_FLIPPED { -1.0 } else { 1.0 };
                Event::MouseWheel { dx: w.x * s, dy: w.y * s }
            }
            T::GAMEPAD_AXIS_MOTION => {
                // SAFETY: tag is gamepad axis.
                let a = unsafe { ev.gaxis };
                Event::GamepadAxis { id: a.which.0, axis: a.axis, value: a.value }
            }
            T::GAMEPAD_BUTTON_DOWN | T::GAMEPAD_BUTTON_UP => {
                // SAFETY: tag is gamepad button.
                let b = unsafe { ev.gbutton };
                Event::GamepadButton { id: b.which.0, button: b.button, down: b.down }
            }
            T::GAMEPAD_ADDED => {
                // SAFETY: tag is gamepad device.
                let id = unsafe { ev.gdevice.which };
                return self.open_gamepad(id);
            }
            T::GAMEPAD_REMOVED => {
                // SAFETY: tag is gamepad device.
                let id = unsafe { ev.gdevice.which }.0;
                if let Some(g) = self.gamepads.remove(&id) {
                    // SAFETY: g.raw was opened by us and not yet closed.
                    unsafe { sys::SDL_CloseGamepad(g.raw) };
                }
                Event::GamepadRemoved { id }
            }
            T::GAMEPAD_SENSOR_UPDATE => {
                // SAFETY: tag is gamepad sensor.
                let s = unsafe { ev.gsensor };
                if s.sensor != sys::SDL_SENSOR_GYRO.0 {
                    return None;
                }
                Event::GamepadGyro { id: s.which.0, data: s.data, timestamp_ns: s.sensor_timestamp }
            }
            T::WINDOW_PIXEL_SIZE_CHANGED => {
                // SAFETY: tag is a window event.
                let w = unsafe { ev.window };
                Event::Resize { width: w.data1.max(0) as u32, height: w.data2.max(0) as u32 }
            }
            T::WINDOW_FOCUS_GAINED => Event::FocusGained,
            T::WINDOW_FOCUS_LOST => Event::FocusLost,
            _ => return None,
        })
    }

    fn open_gamepad(&mut self, id: sys::SDL_JoystickID) -> Option<Event> {
        if self.gamepads.contains_key(&id.0) {
            return None;
        }
        // SAFETY: id comes from an SDL gamepad-added event.
        let raw = unsafe { sys::SDL_OpenGamepad(id) };
        if raw.is_null() {
            return None;
        }
        // SAFETY: raw is a valid open gamepad; name pointer may be null.
        let name_ptr = unsafe { sys::SDL_GetGamepadName(raw) };
        let name = if name_ptr.is_null() {
            String::new()
        } else {
            // SAFETY: non-null, NUL-terminated, valid while gamepad is open.
            unsafe { CStr::from_ptr(name_ptr) }.to_string_lossy().into_owned()
        };
        // SAFETY: raw is a valid open gamepad.
        let has_gyro = unsafe { sys::SDL_GamepadHasSensor(raw, sys::SDL_SENSOR_GYRO) };
        self.gamepads.insert(id.0, Gamepad { raw, has_gyro });
        Some(Event::GamepadAdded { id: id.0, name })
    }

    fn pad(&self, id: u32) -> Result<&Gamepad> {
        self.gamepads.get(&id).ok_or_else(|| Error(format!("no gamepad {id}")))
    }

    /// Ids of currently opened gamepads.
    pub fn gamepads(&self) -> Vec<u32> {
        self.gamepads.keys().copied().collect()
    }

    /// Rumble low/high-frequency motors (0..=0xFFFF) for `duration_ms`.
    pub fn rumble(&self, id: u32, low: u16, high: u16, duration_ms: u32) -> Result<()> {
        let g = self.pad(id)?;
        // SAFETY: g.raw is a valid open gamepad.
        check(unsafe { sys::SDL_RumbleGamepad(g.raw, low, high, duration_ms) })
    }

    /// Whether the gamepad has a gyroscope.
    pub fn has_gyro(&self, id: u32) -> bool {
        self.gamepads.get(&id).is_some_and(|g| g.has_gyro)
    }

    /// Enable/disable gyro reports ([`Event::GamepadGyro`]).
    pub fn set_gyro_enabled(&self, id: u32, enabled: bool) -> Result<()> {
        let g = self.pad(id)?;
        if !g.has_gyro {
            return Err(Error("gamepad has no gyro".into()));
        }
        // SAFETY: g.raw is a valid open gamepad.
        check(unsafe { sys::SDL_SetGamepadSensorEnabled(g.raw, sys::SDL_SENSOR_GYRO, enabled) })
    }

    /// Open default playback device as interleaved stereo i16 at `freq`
    /// (44100 or 48000). `fill` runs on SDL's audio thread.
    pub fn open_audio<F>(&self, freq: u32, fill: F) -> Result<AudioStream>
    where
        F: FnMut(&mut [i16]) + Send + 'static,
    {
        AudioStream::open(freq, Box::new(fill), self.sdl.clone())
    }

    /// Nanoseconds since SDL init (monotonic).
    pub fn ticks_ns(&self) -> u64 {
        ticks_ns()
    }
}

/// Monotonic nanoseconds since SDL init.
pub fn ticks_ns() -> u64 {
    // SAFETY: no preconditions.
    unsafe { sys::SDL_GetTicksNS() }
}

/// High-resolution counter and its frequency (ticks/second).
pub fn performance_counter() -> (u64, u64) {
    // SAFETY: no preconditions.
    unsafe { (sys::SDL_GetPerformanceCounter(), sys::SDL_GetPerformanceFrequency()) }
}

impl Drop for Platform {
    fn drop(&mut self) {
        for (_, g) in self.gamepads.drain() {
            // SAFETY: each raw gamepad is open and closed exactly once here.
            unsafe { sys::SDL_CloseGamepad(g.raw) };
        }
    }
}

/// Window; keeps SDL alive while it exists.
pub struct Window {
    raw: *mut sys::SDL_Window,
    _sdl: Rc<SdlGuard>,
}

impl Window {
    /// Drawable size in pixels.
    pub fn drawable_size(&self) -> (u32, u32) {
        let (mut w, mut h) = (0, 0);
        // SAFETY: raw is a live window; out pointers valid.
        unsafe { sys::SDL_GetWindowSizeInPixels(self.raw, &mut w, &mut h) };
        (w.max(0) as u32, h.max(0) as u32)
    }

    /// Window size in points (the coordinate space of mouse events).
    pub fn window_size(&self) -> (u32, u32) {
        let (mut w, mut h) = (0, 0);
        // SAFETY: raw is a live window; out pointers valid.
        unsafe { sys::SDL_GetWindowSize(self.raw, &mut w, &mut h) };
        (w.max(0) as u32, h.max(0) as u32)
    }

    /// Relative mouse mode (hidden cursor, unbounded deltas).
    pub fn set_relative_mouse(&self, on: bool) -> Result<()> {
        // SAFETY: raw is a live window.
        check(unsafe { sys::SDL_SetWindowRelativeMouseMode(self.raw, on) })
    }

    /// Start/stop [`Event::Text`] delivery.
    pub fn set_text_input(&self, on: bool) -> Result<()> {
        // SAFETY: raw is a live window.
        check(unsafe { if on { sys::SDL_StartTextInput(self.raw) } else { sys::SDL_StopTextInput(self.raw) } })
    }

    pub fn set_title(&self, title: &str) -> Result<()> {
        let t = cstring(title)?;
        // SAFETY: raw is live; t valid for the call (SDL copies it).
        check(unsafe { sys::SDL_SetWindowTitle(self.raw, t.as_ptr()) })
    }

    pub fn set_fullscreen(&self, on: bool) -> Result<()> {
        // SAFETY: raw is a live window.
        check(unsafe { sys::SDL_SetWindowFullscreen(self.raw, on) })
    }

    /// Vulkan instance extensions SDL needs (window must have the Vulkan flag).
    pub fn vulkan_instance_extensions(&self) -> Vec<CString> {
        vulkan_instance_extensions()
    }

    /// Create a `VkSurfaceKHR` (as u64) for `instance` (a `VkInstance` handle).
    pub fn create_vulkan_surface(&self, instance: u64) -> Result<u64> {
        let mut surface: sys::VkSurfaceKHR = ptr::null_mut();
        // SAFETY: raw is a live Vulkan window; caller passes a valid VkInstance
        // handle (dispatchable handle = pointer); out pointer valid.
        check(unsafe {
            sys::SDL_Vulkan_CreateSurface(self.raw, instance as usize as sys::VkInstance, ptr::null(), &mut surface)
        })?;
        Ok(surface as usize as u64)
    }

    /// Raw SDL window pointer (escape hatch).
    pub fn raw(&self) -> *mut sys::SDL_Window {
        self.raw
    }
}

/// Vulkan instance extensions required by SDL (empty on error).
pub fn vulkan_instance_extensions() -> Vec<CString> {
    let mut n = 0u32;
    // SAFETY: out pointer valid; returned array is owned by SDL.
    let arr = unsafe { sys::SDL_Vulkan_GetInstanceExtensions(&mut n) };
    if arr.is_null() {
        return Vec::new();
    }
    (0..n as usize)
        // SAFETY: arr has n valid C-string pointers.
        .map(|i| unsafe { CStr::from_ptr(*arr.add(i)) }.to_owned())
        .collect()
}

impl Drop for Window {
    fn drop(&mut self) {
        // SAFETY: raw is live and destroyed exactly once.
        unsafe { sys::SDL_DestroyWindow(self.raw) };
    }
}

type Fill = Box<dyn FnMut(&mut [i16]) + Send>;

struct AudioState {
    fill: Fill,
    buf: Vec<i16>,
}

/// Playback stream; the callback is called with interleaved stereo i16
/// frames to fill. Keeps SDL alive while it exists.
pub struct AudioStream {
    raw: *mut sys::SDL_AudioStream,
    state: *mut AudioState,
    freq: u32,
    _sdl: Rc<SdlGuard>,
}

unsafe extern "C" fn audio_cb(user: *mut c_void, stream: *mut sys::SDL_AudioStream, additional: c_int, _total: c_int) {
    // SAFETY: user is the AudioState box pointer, alive until the stream is
    // destroyed; SDL serialises callback invocations so &mut is unique.
    let st = unsafe { &mut *(user as *mut AudioState) };
    let samples = (additional.max(0) as usize / 4) * 2; // 4 bytes per stereo frame
    if samples == 0 {
        return;
    }
    st.buf.clear();
    st.buf.resize(samples, 0);
    (st.fill)(&mut st.buf);
    // SAFETY: stream is valid within its callback; buf holds samples*2 bytes.
    unsafe { sys::SDL_PutAudioStreamData(stream, st.buf.as_ptr().cast(), (samples * 2) as c_int) };
}

impl AudioStream {
    fn open(freq: u32, fill: Fill, sdl: Rc<SdlGuard>) -> Result<Self> {
        let spec = sys::SDL_AudioSpec { format: sys::SDL_AUDIO_S16, channels: 2, freq: freq as c_int };
        let state = Box::into_raw(Box::new(AudioState { fill, buf: Vec::new() }));
        // SAFETY: spec valid for the call; state outlives the stream (freed in Drop after destroy).
        let raw = unsafe {
            sys::SDL_OpenAudioDeviceStream(sys::SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK, &spec, Some(audio_cb), state.cast())
        };
        if raw.is_null() {
            // SAFETY: state came from Box::into_raw and SDL never saw it.
            drop(unsafe { Box::from_raw(state) });
            return Err(sdl_error());
        }
        Ok(Self { raw, state, freq, _sdl: sdl })
    }

    /// Start (or resume) playback; streams open paused.
    pub fn resume(&self) -> Result<()> {
        // SAFETY: raw is a live stream.
        check(unsafe { sys::SDL_ResumeAudioStreamDevice(self.raw) })
    }

    pub fn pause(&self) -> Result<()> {
        // SAFETY: raw is a live stream.
        check(unsafe { sys::SDL_PauseAudioStreamDevice(self.raw) })
    }

    pub fn freq(&self) -> u32 {
        self.freq
    }
}

impl Drop for AudioStream {
    fn drop(&mut self) {
        // SAFETY: destroy stops the callback before returning; then no one
        // else references state, so it can be freed.
        unsafe {
            sys::SDL_DestroyAudioStream(self.raw);
            drop(Box::from_raw(self.state));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const HEADLESS: &[(&str, &str)] = &[("SDL_VIDEO_DRIVER", "dummy"), ("SDL_AUDIO_DRIVER", "dummy")];

    #[test]
    fn window_and_events_headless() {
        let mut p = Platform::init_with_hints(HEADLESS).unwrap();
        let w = p.create_window("t", 320, 200, false).unwrap();
        let (dw, dh) = w.drawable_size();
        assert!(dw > 0 && dh > 0);
        w.set_title("t2").unwrap();
        let _ = p.poll_events();
        drop(w);
        assert!(p.rumble(12345, 1, 1, 1).is_err());
        assert!(!p.has_gyro(12345));
    }

    #[test]
    fn audio_callback_runs() {
        let p = Platform::init_with_hints(HEADLESS).unwrap();
        let n = Arc::new(AtomicUsize::new(0));
        let n2 = n.clone();
        let s = p
            .open_audio(48000, move |buf| {
                assert_eq!(buf.len() % 2, 0);
                buf.fill(0);
                n2.fetch_add(buf.len(), Ordering::Relaxed);
            })
            .unwrap();
        s.resume().unwrap();
        let t0 = ticks_ns();
        while n.load(Ordering::Relaxed) == 0 && ticks_ns() - t0 < 3_000_000_000 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        drop(s);
        assert!(n.load(Ordering::Relaxed) > 0);
    }

    #[test]
    fn timer_monotonic() {
        let _p = Platform::init_with_hints(HEADLESS).unwrap();
        let a = ticks_ns();
        let (c, f) = performance_counter();
        assert!(f > 0 && c > 0);
        assert!(ticks_ns() >= a);
    }
}
