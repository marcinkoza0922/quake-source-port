use super::{CStr, sys};
use std::cell::Cell;
use std::ptr::NonNull;

pub(super) struct PhoneHaptics {
    raw: NonNull<sys::SDL_Haptic>,
    pulse_until: Cell<u64>,
}

impl PhoneHaptics {
    pub(super) fn open() -> Option<Self> {
        // SAFETY: Platform owns the SDL lifetime. Its guard shuts down all subsystems.
        unsafe {
            if !sys::SDL_InitSubSystem(sys::SDL_INIT_HAPTIC) {
                return None;
            }
            let mut count = 0;
            let ids = NonNull::new(sys::SDL_GetHaptics(&mut count))?;
            let phone = std::slice::from_raw_parts(ids.as_ptr(), count as usize).iter().copied().find(|&id| {
                let name = sys::SDL_GetHapticNameForID(id);
                // SDL's Android backend exposes the system vibrator under this name.
                !name.is_null() && CStr::from_ptr(name) == c"VIBRATOR_SERVICE"
            });
            sys::SDL_free(ids.as_ptr().cast());
            let raw = NonNull::new(sys::SDL_OpenHaptic(phone?))?;
            if !sys::SDL_InitHapticRumble(raw.as_ptr()) {
                sys::SDL_CloseHaptic(raw.as_ptr());
                return None;
            }
            Some(Self { raw, pulse_until: Cell::new(0) })
        }
    }

    pub(super) fn play(&self, strength: f32, duration_ms: u32) {
        // SAFETY: The timer is always available and raw is open for this object's lifetime.
        let now = unsafe { sys::SDL_GetTicks() };
        if now < self.pulse_until.get() && strength < 0.35 {
            return;
        }
        // SAFETY: raw is an open haptic initialized for rumble.
        if unsafe { sys::SDL_PlayHapticRumble(self.raw.as_ptr(), strength, duration_ms) } && strength >= 0.35 {
            self.pulse_until.set(now + u64::from(duration_ms));
        }
    }
}

impl Drop for PhoneHaptics {
    fn drop(&mut self) {
        // SAFETY: the haptic is owned here and dropped before the SDL guard.
        unsafe { sys::SDL_CloseHaptic(self.raw.as_ptr()) };
    }
}
