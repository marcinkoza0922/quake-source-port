//! Browser backend (wasm32): DOM keyboard/mouse events on `<canvas id="quake">`,
//! pointer lock, the Gamepad API and WebAudio, behind the same API as the
//! SDL backend. Events are queued by DOM listeners and drained by
//! [`Platform::poll_events`], which also polls gamepads and tracks the
//! canvas size (backing size = CSS size × devicePixelRatio).

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{EventTarget, HtmlCanvasElement};

use crate::*;

mod audio;
mod gamepad;
mod input;
mod keymap;
mod touch;

pub use audio::AudioStream;

fn js_err(e: JsValue) -> Error {
    Error(e.as_string().unwrap_or_else(|| format!("{e:?}")))
}

fn window() -> Result<web_sys::Window> {
    web_sys::window().ok_or_else(|| Error("no window".into()))
}

fn document() -> Result<web_sys::Document> {
    window()?.document().ok_or_else(|| Error("no document".into()))
}

/// A DOM event listener; removed from its target when dropped.
pub(crate) struct Listener {
    target: EventTarget,
    name: &'static str,
    cb: Closure<dyn FnMut(web_sys::Event)>,
}

impl Listener {
    pub(crate) fn new(target: &EventTarget, name: &'static str, f: impl FnMut(web_sys::Event) + 'static) -> Result<Self> {
        let cb = Closure::<dyn FnMut(web_sys::Event)>::new(f);
        target.add_event_listener_with_callback(name, cb.as_ref().unchecked_ref()).map_err(js_err)?;
        Ok(Self { target: target.clone(), name, cb })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback(self.name, self.cb.as_ref().unchecked_ref());
    }
}

/// State shared by the platform, its window and the DOM listeners.
#[derive(Default)]
pub(crate) struct Shared {
    pub(crate) queue: RefCell<VecDeque<Event>>,
    /// Game wants pointer lock (taken on the next click: browsers need a gesture).
    pub(crate) want_lock: Cell<bool>,
    pub(crate) text_input: Cell<bool>,
    pub(crate) canvas: RefCell<Option<HtmlCanvasElement>>,
    /// A finger touched the canvas (the device has a touchscreen).
    pub(crate) touch_used: Cell<bool>,
    /// Hidden field that raises the on-screen keyboard (`touch.rs`).
    pub(crate) text_field: RefCell<Option<(web_sys::HtmlInputElement, Listener)>>,
}

impl Shared {
    pub(crate) fn push(&self, e: Event) {
        self.queue.borrow_mut().push_back(e);
    }
}

/// Browser context. `!Send`/`!Sync`.
pub struct Platform {
    shared: Rc<Shared>,
    listeners: RefCell<Vec<Listener>>,
    pads: gamepad::Pads,
}

impl Platform {
    pub fn init() -> Result<Self> {
        Self::init_with_hints(&[])
    }

    /// Hints are SDL-specific and ignored in the browser.
    pub fn init_with_hints(_hints: &[(&str, &str)]) -> Result<Self> {
        let shared = Rc::new(Shared::default());
        let w: EventTarget = window()?.into();
        let mut l = input::keyboard_listeners(&w, &shared)?;
        let s = shared.clone();
        l.push(Listener::new(&w, "focus", move |_| s.push(Event::FocusGained))?);
        let s = shared.clone();
        l.push(Listener::new(&w, "blur", move |_| s.push(Event::FocusLost))?);
        Ok(Self { shared, listeners: RefCell::new(l), pads: gamepad::Pads::default() })
    }

    /// SDL mapping files do not apply to the Gamepad API.
    pub fn add_gamepad_mappings_from_file(&self, _path: &str) -> Result<u32> {
        Ok(0)
    }

    /// Finds `<canvas id="quake">` (or creates one of `width`×`height` CSS
    /// pixels in `<body>`) and attaches mouse listeners to it.
    pub fn create_window(&self, title: &str, width: u32, height: u32, _vulkan: bool) -> Result<Window> {
        let doc = document()?;
        doc.set_title(title);
        let canvas = match doc.get_element_by_id("quake") {
            Some(e) => e.dyn_into::<HtmlCanvasElement>().map_err(|_| Error("#quake is not a canvas".into()))?,
            None => {
                let c: HtmlCanvasElement = doc.create_element("canvas").map_err(js_err)?.unchecked_into();
                c.set_id("quake");
                let st = c.style();
                let _ = st.set_property("width", &format!("{width}px"));
                let _ = st.set_property("height", &format!("{height}px"));
                doc.body().ok_or_else(|| Error("no body".into()))?.append_child(&c).map_err(js_err)?;
                c
            }
        };
        canvas.set_tab_index(0);
        let _ = canvas.focus();
        self.listeners.borrow_mut().extend(input::mouse_listeners(&canvas, &self.shared)?);
        self.listeners.borrow_mut().extend(touch::touch_listeners(&canvas, &self.shared)?);
        *self.shared.canvas.borrow_mut() = Some(canvas.clone());
        let win = Window { canvas, shared: self.shared.clone() };
        sync_canvas_size(&win.canvas);
        Ok(win)
    }

    pub fn poll_event(&mut self) -> Option<Event> {
        if self.shared.queue.borrow().is_empty() {
            self.refresh();
        }
        self.shared.queue.borrow_mut().pop_front()
    }

    /// Drain all pending events (also polls gamepads and the canvas size).
    pub fn poll_events(&mut self) -> Vec<Event> {
        self.refresh();
        self.shared.queue.borrow_mut().drain(..).collect()
    }

    fn refresh(&mut self) {
        let canvas = self.shared.canvas.borrow().clone();
        if let Some(c) = canvas
            && let Some((width, height)) = sync_canvas_size(&c)
        {
            self.shared.push(Event::Resize { width, height });
        }
        self.pads.poll(&self.shared);
    }

    /// Indices of connected gamepads.
    pub fn gamepads(&self) -> Vec<u32> {
        self.pads.ids()
    }

    pub fn rumble(&self, id: u32, low: u16, high: u16, duration_ms: u32) -> Result<()> {
        gamepad::rumble(id, low, high, duration_ms)
    }

    pub fn rumble_all(&self, low: u16, high: u16, duration_ms: u32) {
        for id in self.pads.ids() {
            let _ = gamepad::rumble(id, low, high, duration_ms);
        }
    }

    /// The Gamepad API exposes no gyro.
    pub fn has_gyro(&self, _id: u32) -> bool {
        false
    }

    pub fn set_gyro_enabled(&self, _id: u32, _enabled: bool) -> Result<()> {
        Err(Error("gamepad has no gyro".into()))
    }

    /// WebAudio output of interleaved stereo i16 at `freq`. `fill` runs on
    /// the main thread from the audio node's callback. Playback starts on
    /// the first user gesture (browser autoplay policy).
    pub fn open_audio<F>(&self, freq: u32, fill: F) -> Result<AudioStream>
    where
        F: FnMut(&mut [i16]) + Send + 'static,
    {
        AudioStream::open(freq, Box::new(fill))
    }

    pub fn ticks_ns(&self) -> u64 {
        ticks_ns()
    }
}

/// Monotonic nanoseconds since page load (`performance.now()`).
pub fn ticks_ns() -> u64 {
    let ms = web_sys::window().and_then(|w| w.performance()).map_or(0.0, |p| p.now());
    (ms * 1e6) as u64
}

/// High-resolution counter (nanoseconds) and its frequency.
/// The browser's preferred languages (`navigator.languages`).
pub fn preferred_languages() -> Vec<String> {
    let Some(nav) = web_sys::window().map(|w| w.navigator()) else { return Vec::new() };
    let list = js_sys::Reflect::get(&nav, &"languages".into()).ok();
    let arr: Option<js_sys::Array> = list.and_then(|v| v.dyn_into().ok());
    arr.map(|a| a.iter().filter_map(|v| v.as_string()).collect()).unwrap_or_default()
}

pub fn performance_counter() -> (u64, u64) {
    (ticks_ns(), 1_000_000_000)
}

/// Local calendar time of a Unix timestamp: (year, month, day, hour, minute).
pub fn local_time(unix_secs: i64) -> Option<(i32, i32, i32, i32, i32)> {
    let d = js_sys::Date::new(&JsValue::from_f64(unix_secs as f64 * 1000.0));
    let y = d.get_full_year();
    // an out-of-range timestamp gives an Invalid Date, whose fields are NaN (0 here)
    (!d.get_time().is_nan()).then_some((y as i32, d.get_month() as i32 + 1, d.get_date() as i32, d.get_hours() as i32, d.get_minutes() as i32))
}

/// Shows `title` and `message` with `window.alert`; false if there is no window.
pub fn error_box(title: &str, message: &str) -> bool {
    web_sys::window().is_some_and(|w| w.alert_with_message(&format!("{title}\n\n{message}")).is_ok())
}

/// Sets the canvas backing size to its CSS size × devicePixelRatio;
/// returns the new size if it changed.
fn sync_canvas_size(c: &HtmlCanvasElement) -> Option<(u32, u32)> {
    let dpr = web_sys::window().map_or(1.0, |w| w.device_pixel_ratio());
    let (cw, ch) = (c.client_width(), c.client_height());
    if cw <= 0 || ch <= 0 {
        return None;
    }
    let (w, h) = (((cw as f64) * dpr).round() as u32, ((ch as f64) * dpr).round() as u32);
    if (w, h) == (c.width(), c.height()) {
        return None;
    }
    c.set_width(w);
    c.set_height(h);
    Some((w, h))
}

/// The game canvas.
pub struct Window {
    canvas: HtmlCanvasElement,
    shared: Rc<Shared>,
}

impl Window {
    /// The `<canvas>` the renderer draws into.
    pub fn canvas(&self) -> HtmlCanvasElement {
        self.canvas.clone()
    }

    /// Canvas backing size in pixels.
    pub fn drawable_size(&self) -> (u32, u32) {
        (self.canvas.width(), self.canvas.height())
    }

    /// Same as [`drawable_size`](Self::drawable_size): mouse events are
    /// reported in backing pixels.
    pub fn window_size(&self) -> (u32, u32) {
        self.drawable_size()
    }

    pub fn safe_insets(&self) -> [f32; 4] {
        [0.0; 4]
    }

    /// Backing pixels per CSS pixel (`devicePixelRatio`).
    pub fn display_scale(&self) -> f32 {
        web_sys::window().map_or(1.0, |w| w.device_pixel_ratio() as f32).max(0.1)
    }

    /// Pointer lock. Locking needs a user gesture, so it is requested now
    /// and again on the next click inside the canvas.
    pub fn set_relative_mouse(&self, on: bool) -> Result<()> {
        self.shared.want_lock.set(on);
        let doc = document()?;
        if on {
            if doc.pointer_lock_element().is_none() {
                self.canvas.request_pointer_lock();
            }
        } else if doc.pointer_lock_element().is_some() {
            doc.exit_pointer_lock();
        }
        Ok(())
    }

    /// Start/stop [`Event::Text`] delivery.
    pub fn set_text_input(&self, on: bool) -> Result<()> {
        self.shared.text_input.set(on);
        touch::set_keyboard(&self.shared, on);
        Ok(())
    }

    pub fn set_title(&self, title: &str) -> Result<()> {
        document()?.set_title(title);
        Ok(())
    }

    /// Browsers cannot change the display mode.
    pub fn display_modes(&self) -> Vec<(u32, u32)> {
        Vec::new()
    }

    pub fn desktop_size(&self) -> Option<(u32, u32)> {
        let s = window().ok()?.screen().ok()?;
        let dpr = web_sys::window()?.device_pixel_ratio();
        let (w, h) = (s.width().ok()?, s.height().ok()?);
        Some(((w as f64 * dpr) as u32, (h as f64 * dpr) as u32))
    }

    /// Fullscreen modes map to the Fullscreen API on the canvas; `size` is ignored.
    pub fn set_display(&self, mode: DisplayMode, _size: (u32, u32)) -> Result<()> {
        self.set_fullscreen(mode != DisplayMode::Windowed)
    }

    pub fn set_fullscreen(&self, on: bool) -> Result<()> {
        let doc = document()?;
        match (on, doc.fullscreen_element().is_some()) {
            (true, false) => self.canvas.request_fullscreen().map_err(js_err),
            (false, true) => {
                doc.exit_fullscreen();
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Writes to the clipboard (async, best effort).
    pub fn set_clipboard_text(&self, text: &str) -> Result<()> {
        let nav = window()?.navigator();
        let clip = js_sys::Reflect::get(&nav, &"clipboard".into()).map_err(js_err)?;
        let f: js_sys::Function =
            js_sys::Reflect::get(&clip, &"writeText".into()).map_err(js_err)?.dyn_into().map_err(js_err)?;
        f.call1(&clip, &text.into()).map_err(js_err)?;
        Ok(())
    }

    /// Reading the clipboard is asynchronous and permission-gated on the
    /// web; pasting is not supported.
    pub fn clipboard_text(&self) -> Option<String> {
        None
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        self.shared.canvas.borrow_mut().take();
    }
}
