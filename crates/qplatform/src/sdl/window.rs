use super::*;
use crate::DisplayMode;

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

    /// Left, top, right and bottom safe-area insets, as window fractions.
    pub fn safe_insets(&self) -> [f32; 4] {
        let (w, h) = self.window_size();
        let mut rect = sys::SDL_Rect { x: 0, y: 0, w: 0, h: 0 };
        // SAFETY: raw is a live window and rect is writable.
        let ok = unsafe { sys::SDL_GetWindowSafeArea(self.raw, &mut rect) };
        if !ok || w == 0 || h == 0 || rect.w <= 0 || rect.h <= 0 {
            return [0.0; 4];
        }
        [
            (rect.x as f32 / w as f32).max(0.0),
            (rect.y as f32 / h as f32).max(0.0),
            (1.0 - (rect.x + rect.w) as f32 / w as f32).max(0.0),
            (1.0 - (rect.y + rect.h) as f32 / h as f32).max(0.0),
        ]
    }

    /// Drawable pixels per platform UI unit (Android dp, Windows/Linux
    /// logical pixel, macOS point): the OS scale its own touch UI uses.
    /// 1 when SDL can't tell.
    pub fn display_scale(&self) -> f32 {
        // SAFETY: raw is a live window.
        let s = unsafe { sys::SDL_GetWindowDisplayScale(self.raw) };
        if s.is_finite() && s > 0.0 { s } else { 1.0 }
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

    /// Fullscreen resolutions of the window's display, largest first,
    /// deduplicated by size (refresh rates collapsed).
    pub fn display_modes(&self) -> Vec<(u32, u32)> {
        let mut out = Vec::new();
        // SAFETY: raw is live; the returned array is freed with SDL_free.
        unsafe {
            let id = sys::SDL_GetDisplayForWindow(self.raw);
            let mut n: c_int = 0;
            let modes = sys::SDL_GetFullscreenDisplayModes(id, &mut n);
            if modes.is_null() {
                return out;
            }
            for i in 0..n.max(0) as usize {
                let m = *modes.add(i);
                if !m.is_null() {
                    let s = ((*m).w.max(0) as u32, (*m).h.max(0) as u32);
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
            sys::SDL_free(modes as *mut std::ffi::c_void);
        }
        out
    }

    /// Desktop resolution of the window's display.
    pub fn desktop_size(&self) -> Option<(u32, u32)> {
        // SAFETY: raw is live; returned pointer owned by SDL.
        unsafe {
            let m = sys::SDL_GetDesktopDisplayMode(sys::SDL_GetDisplayForWindow(self.raw));
            (!m.is_null()).then(|| ((*m).w.max(0) as u32, (*m).h.max(0) as u32))
        }
    }

    /// Switches between windowed (`size` = window size in points),
    /// exclusive fullscreen at the closest mode to `size`, and borderless
    /// fullscreen at desktop resolution.
    pub fn set_display(&self, mode: DisplayMode, size: (u32, u32)) -> Result<()> {
        let (w, h) = (size.0.max(320) as c_int, size.1.max(200) as c_int);
        // SAFETY: raw is live; mode struct is valid for the calls.
        unsafe {
            match mode {
                DisplayMode::Windowed => {
                    check(sys::SDL_SetWindowFullscreen(self.raw, false))?;
                    // leaving fullscreen is asynchronous on some platforms
                    let _ = sys::SDL_SyncWindow(self.raw);
                    check(sys::SDL_SetWindowSize(self.raw, w, h))?;
                    let c = sys::SDL_WINDOWPOS_CENTERED;
                    let _ = sys::SDL_SetWindowPosition(self.raw, c, c);
                }
                DisplayMode::Fullscreen => {
                    let id = sys::SDL_GetDisplayForWindow(self.raw);
                    let mut m = std::mem::MaybeUninit::<sys::SDL_DisplayMode>::zeroed();
                    check(sys::SDL_GetClosestFullscreenDisplayMode(id, w, h, 0.0, false, m.as_mut_ptr()))?;
                    check(sys::SDL_SetWindowFullscreenMode(self.raw, m.as_ptr()))?;
                    check(sys::SDL_SetWindowFullscreen(self.raw, true))?;
                }
                DisplayMode::Borderless => {
                    check(sys::SDL_SetWindowFullscreenMode(self.raw, ptr::null()))?;
                    check(sys::SDL_SetWindowFullscreen(self.raw, true))?;
                }
            }
        }
        // SAFETY: raw is live.
        let _ = unsafe { sys::SDL_SyncWindow(self.raw) };
        Ok(())
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

    /// Copy UTF-8 text to the system clipboard.
    pub fn set_clipboard_text(&self, text: &str) -> Result<()> {
        let t = cstring(text)?;
        // SAFETY: SDL video is initialised while Window is alive; t is a valid C string.
        check(unsafe { sys::SDL_SetClipboardText(t.as_ptr()) })
    }

    /// Read UTF-8 text from the system clipboard, if any.
    pub fn clipboard_text(&self) -> Option<String> {
        // SAFETY: SDL video is initialised while Window is alive; returned buffer is freed with SDL_free.
        unsafe {
            let ptr = sys::SDL_GetClipboardText();
            if ptr.is_null() {
                return None;
            }
            let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
            sys::SDL_free(ptr as *mut c_void);
            (!s.is_empty()).then_some(s)
        }
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
