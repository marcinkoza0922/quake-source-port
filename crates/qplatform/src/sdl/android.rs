use super::{Window, sys};
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
