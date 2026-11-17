//! SDL3 backend (every target except wasm32).

use std::collections::HashMap;
use std::ffi::{CStr, CString, c_int, c_void};
use std::ptr;
use std::rc::Rc;

use sdl3_sys::everything as sys;

use crate::*;

mod window;
pub use window::vulkan_instance_extensions;

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

/// Label family from SDL's gamepad type, then vendor id and name.
pub fn pad_family(sdl_type: i32, vendor: u16, name: &str) -> u8 {
    use sys::SDL_GamepadType as G;
    let t = G(sdl_type);
    if [G::PS3, G::PS4, G::PS5].contains(&t) {
        return family::PLAYSTATION;
    }
    if [G::NINTENDO_SWITCH_PRO, G::NINTENDO_SWITCH_JOYCON_LEFT, G::NINTENDO_SWITCH_JOYCON_RIGHT, G::NINTENDO_SWITCH_JOYCON_PAIR]
        .contains(&t)
    {
        return family::NINTENDO;
    }
    family_from_vendor_name(vendor, name)
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
        // touches arrive as Event::Finger; synthesised mouse clicks would
        // fire and turn the view behind the on-screen controls
        for (k, v) in [("SDL_TOUCH_MOUSE_EVENTS", "0")].iter().chain(hints) {
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
            T::FINGER_DOWN | T::FINGER_MOTION | T::FINGER_UP | T::FINGER_CANCELED => {
                // SAFETY: tag is a touch finger event.
                let f = unsafe { ev.tfinger };
                let phase = match ty {
                    T::FINGER_DOWN => FingerPhase::Down,
                    T::FINGER_MOTION => FingerPhase::Move,
                    _ => FingerPhase::Up,
                };
                let id = f.touchID.0.rotate_left(32) ^ f.fingerID.0;
                Event::Finger { id, phase, x: f.x, y: f.y }
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
        // SAFETY: raw is a valid open gamepad.
        let (ty, vendor) = unsafe { (sys::SDL_GetGamepadType(raw).0, sys::SDL_GetGamepadVendor(raw)) };
        let family = pad_family(ty, vendor, &name);
        self.gamepads.insert(id.0, Gamepad { raw, has_gyro });
        Some(Event::GamepadAdded { id: id.0, name, family })
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

    /// Rumble every opened gamepad (errors from pads without motors ignored).
    pub fn rumble_all(&self, low: u16, high: u16, duration_ms: u32) {
        for g in self.gamepads.values() {
            // SAFETY: g.raw is a valid open gamepad.
            unsafe { sys::SDL_RumbleGamepad(g.raw, low, high, duration_ms) };
        }
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

/// Local calendar time of a Unix timestamp: (year, month, day, hour, minute).
pub fn local_time(unix_secs: i64) -> Option<(i32, i32, i32, i32, i32)> {
    let mut dt = sys::SDL_DateTime::default();
    // SAFETY: dt is a valid out pointer; SDL needs no init for time conversion.
    let ok = unsafe { sys::SDL_TimeToDateTime(unix_secs.saturating_mul(1_000_000_000), &mut dt, true) };
    ok.then_some((dt.year, dt.month, dt.day, dt.hour, dt.minute))
}

/// A modal error box (works before SDL video init); false if it couldn't show.
pub fn error_box(title: &str, message: &str) -> bool {
    let (Ok(t), Ok(m)) = (CString::new(title), CString::new(message)) else { return false };
    // SAFETY: valid C strings; a null window is allowed.
    unsafe { sys::SDL_ShowSimpleMessageBox(sys::SDL_MESSAGEBOX_ERROR, t.as_ptr(), m.as_ptr(), ptr::null_mut()) }
}

/// The user's preferred UI languages, most preferred first, as ISO codes
/// ("de", "pt-BR"). Works before SDL init.
pub fn preferred_languages() -> Vec<String> {
    let mut n: c_int = 0;
    // SAFETY: n is a valid out pointer; the result is freed below.
    let list = unsafe { sys::SDL_GetPreferredLocales(&mut n) };
    if list.is_null() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..n.max(0) as usize {
        // SAFETY: SDL returns an array of `n` locale pointers.
        let loc = unsafe { *list.add(i) };
        if loc.is_null() {
            break;
        }
        // SAFETY: `loc` is a valid SDL_Locale from that array.
        let lang = unsafe { (*loc).language };
        if !lang.is_null() {
            // SAFETY: `language` is a NUL-terminated C string owned by the list.
            out.push(unsafe { CStr::from_ptr(lang) }.to_string_lossy().into_owned());
        }
    }
    // SAFETY: one SDL allocation holds the whole list.
    unsafe { sys::SDL_free(list.cast()) };
    out
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
mod tests;
