use super::{Window, check, sys};
use crate::Event;
use std::collections::VecDeque;
use std::sync::Mutex;
use raw_window_handle::{AndroidNdkWindowHandle, DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, WindowHandle};
use std::ffi::{CString, c_void};
use std::ptr::NonNull;

#[link(name = "android")]
unsafe extern "C" {
    fn ANativeWindow_acquire(window: *mut c_void);
    fn ANativeWindow_release(window: *mut c_void);
}

/// An owning reference to the current Android surface's native window.
pub struct AndroidWindow(NonNull<c_void>);

// SAFETY: the Android native window API allows ownership and reference counting
// across threads. No Rust allocation or SDL window is accessed through this handle.
unsafe impl Send for AndroidWindow {}
// SAFETY: the reference keeps the native window alive for every shared borrower.
unsafe impl Sync for AndroidWindow {}

impl Clone for AndroidWindow {
    fn clone(&self) -> Self {
        // SAFETY: self keeps the native window alive while the new reference is acquired.
        unsafe { ANativeWindow_acquire(self.0.as_ptr()) };
        Self(self.0)
    }
}

impl Drop for AndroidWindow {
    fn drop(&mut self) {
        // SAFETY: this owns a reference acquired by Window::android_window.
        unsafe { ANativeWindow_release(self.0.as_ptr()) };
    }
}

impl HasWindowHandle for AndroidWindow {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        // SAFETY: the native window reference is held for the lifetime of the borrow.
        Ok(unsafe { WindowHandle::borrow_raw(AndroidNdkWindowHandle::new(self.0).into()) })
    }
}

impl HasDisplayHandle for AndroidWindow {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        Ok(DisplayHandle::android())
    }
}

impl AndroidWindow {
    pub fn id(&self) -> usize {
        self.0.as_ptr().addr()
    }
}

impl Window {
    pub fn android_window(&self) -> Option<AndroidWindow> {
        // SAFETY: the SDL window is live. Its Android surface callback clears
        // the property before releasing the native window. Holding the property
        // lock lets us take a reference before that release can happen.
        unsafe {
            let props = sys::SDL_GetWindowProperties(self.raw);
            if !sys::SDL_LockProperties(props) {
                return None;
            }
            let window = NonNull::new(sys::SDL_GetPointerProperty(
                props,
                sys::SDL_PROP_WINDOW_ANDROID_WINDOW_POINTER,
                std::ptr::null_mut(),
            ));
            if let Some(window) = window {
                ANativeWindow_acquire(window.as_ptr());
            }
            sys::SDL_UnlockProperties(props);
            window.map(AndroidWindow)
        }
    }
}

pub fn android_log(message: &str) {
    if let Ok(message) = CString::new(message) {
        // SAFETY: the format and its single string argument are valid C strings.
        unsafe { sys::SDL_Log(c"%s".as_ptr(), message.as_ptr()) };
    }
}

/// The built-in Android gyro. Closed when aiming is disabled or the app is idle.
#[derive(Default)]
pub(super) struct PhoneGyro {
    sensor: Option<NonNull<sys::SDL_Sensor>>,
    tried: bool,
}

impl PhoneGyro {
    fn open() -> Option<NonNull<sys::SDL_Sensor>> {
        // SAFETY: the sensor subsystem is initialized and used on the SDL thread.
        // SDL owns the sensor IDs and transfers the enumeration array to us.
        unsafe {
            let mut count = 0;
            let ids = NonNull::new(sys::SDL_GetSensors(&mut count))?;
            let sensor = std::slice::from_raw_parts(ids.as_ptr(), count.max(0) as usize)
                .iter()
                .copied()
                .filter(|&id| sys::SDL_GetSensorTypeForID(id) == sys::SDL_SENSOR_GYRO)
                .find_map(|id| NonNull::new(sys::SDL_OpenSensor(id)));
            sys::SDL_free(ids.as_ptr().cast());
            sensor
        }
    }
}

impl Drop for PhoneGyro {
    fn drop(&mut self) {
        if let Some(sensor) = self.sensor {
            // SAFETY: this handle was opened by PhoneGyro and is still live.
            unsafe { sys::SDL_CloseSensor(sensor.as_ptr()) };
        }
    }
}

impl super::Platform {
    /// Deliver lifecycle notifications from the SDL pump before its queued input.
    pub fn poll_event(&mut self) -> Option<Event> {
        if let Some(event) = self.lifecycle.pop().or_else(|| self.pending_input.take()) {
            return Some(event);
        }
        let input = self.poll_input();
        if let Some(event) = self.lifecycle.pop() {
            self.pending_input = input;
            return Some(event);
        }
        input
    }

    pub fn phone_gyro(&mut self, window: &Window, enabled: bool) -> Option<[f32; 3]> {
        // SAFETY: the window is live and this runs on the SDL thread.
        let flags = unsafe { sys::SDL_GetWindowFlags(window.raw) };
        let focused = (flags & sys::SDL_WINDOW_INPUT_FOCUS) != sys::SDL_WindowFlags(0);
        if !enabled || !focused {
            if self.phone_gyro.sensor.is_some() {
                self.phone_gyro = PhoneGyro::default();
            }
            return None;
        }
        if !self.phone_gyro.tried {
            self.phone_gyro.tried = true;
            self.phone_gyro.sensor = PhoneGyro::open();
        }
        let sensor = self.phone_gyro.sensor?;
        let mut data = [0.0; 3];
        // SAFETY: sensor and window are live; the buffer holds three floats.
        unsafe {
            if !sys::SDL_GetSensorData(sensor.as_ptr(), data.as_mut_ptr(), 3) {
                return None;
            }
            let display = sys::SDL_GetDisplayForWindow(window.raw);
            let natural = orientation_turns(sys::SDL_GetNaturalDisplayOrientation(display));
            let current = orientation_turns(sys::SDL_GetCurrentDisplayOrientation(display));
            Some(rotate_gyro(data, current - natural))
        }
    }
}

fn orientation_turns(orientation: sys::SDL_DisplayOrientation) -> i32 {
    match orientation {
        sys::SDL_ORIENTATION_LANDSCAPE => 1,
        sys::SDL_ORIENTATION_PORTRAIT_FLIPPED => 2,
        sys::SDL_ORIENTATION_LANDSCAPE_FLIPPED => 3,
        _ => 0,
    }
}

fn rotate_gyro([x, y, z]: [f32; 3], turns: i32) -> [f32; 3] {
    // The Android sensor uses the display's natural axes. Match SDL's
    // SDLSurface rotation for sensor vectors before turning the game camera.
    match turns.rem_euclid(4) {
        1 => [-y, x, z],
        2 => [-x, -y, z],
        3 => [y, -x, z],
        _ => [x, y, z],
    }
}

impl Window {
    pub fn physical_size_cm(&self) -> Option<[f32; 2]> {
        use std::sync::OnceLock;
        static DPI: OnceLock<Option<[f32; 2]>> = OnceLock::new();
        let [xdpi, ydpi] = (*DPI.get_or_init(|| {
            let dpi = |name| {
                std::env::var(name).ok()?.parse::<f32>().ok()
                    .filter(|d| (70.0..=1000.0).contains(d))
            };
            let x = dpi("QUAKE_DISPLAY_DPI_X")?;
            let y = dpi("QUAKE_DISPLAY_DPI_Y")?;
            (0.75..=1.33).contains(&(x / y)).then_some([x, y])
        }))?;
        let (w, h) = self.drawable_size();
        let cm = [w as f32 * 2.54 / xdpi, h as f32 * 2.54 / ydpi];
        cm.iter().all(|d| (3.0..=100.0).contains(d)).then_some(cm)
    }
}


/// SDL calls watchers for app lifecycle events without adding them to its
/// poll queue. Record the notifications before returning to the game thread.
pub(super) struct Lifecycle(Box<Mutex<VecDeque<Event>>>);

impl Lifecycle {
    pub(super) fn new() -> crate::Result<Self> {
        let this = Self(Box::default());
        // SAFETY: the boxed queue has a stable address and Drop removes the
        // watch before the queue or its SDL context goes away.
        check(unsafe { sys::SDL_AddEventWatch(Some(watch_lifecycle), this.userdata()) })?;
        Ok(this)
    }

    fn userdata(&self) -> *mut c_void {
        std::ptr::from_ref(self.0.as_ref()).cast_mut().cast()
    }

    pub(super) fn pop(&self) -> Option<Event> {
        self.0.lock().unwrap_or_else(std::sync::PoisonError::into_inner).pop_front()
    }
}

impl Drop for Lifecycle {
    fn drop(&mut self) {
        // SAFETY: the callback and userdata match the registration.
        unsafe { sys::SDL_RemoveEventWatch(Some(watch_lifecycle), self.userdata()) };
    }
}

unsafe extern "C" fn watch_lifecycle(userdata: *mut c_void, event: *mut sys::SDL_Event) -> bool {
    // SAFETY: SDL passes the live queue and a valid event for this callback.
    let ty = unsafe { (*event).r#type };
    let event = match sys::SDL_EventType(ty) {
        sys::SDL_EventType::WILL_ENTER_BACKGROUND => Event::WillEnterBackground,
        sys::SDL_EventType::DID_ENTER_FOREGROUND => Event::DidEnterForeground,
        _ => return true,
    };
    // SAFETY: userdata is a pointer to the boxed queue registered in Lifecycle.
    let queue = unsafe { &*userdata.cast::<Mutex<VecDeque<Event>>>() };
    queue.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push_back(event);
    true
}
