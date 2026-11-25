//! What a frame that couldn't be drawn means for the game.

use super::*;
use qrender::{Error, Lost};

/// Failures of the latest frames, reset by one that was drawn.
#[derive(Default)]
pub(super) struct RenderErrors {
    /// Printed already: one that repeats every frame is told once.
    last: String,
    surface_losses: u32,
}

/// Frames in a row a surface may be lost in before it's no use trying again.
const MAX_SURFACE_LOSSES: u32 = 5;

impl Host {
    /// A lost surface is replaced. Nothing can be drawn without the device, so
    /// the game is saved and ends instead of running on unseen (Sys_Error in
    /// vkQuake). Other errors go to the console.
    pub(super) fn after_render(&mut self, drawn: qrender::Result<()>) {
        let e = match drawn {
            Ok(()) => return self.render_errors = Default::default(),
            Err(e) => e,
        };
        match e.lost {
            Some(Lost::Device) => self.graphics_lost(&e),
            Some(Lost::Surface) => {
                self.render_errors.surface_losses += 1;
                let again = if self.render_errors.surface_losses > MAX_SURFACE_LOSSES { Err(e) } else { self.new_surface() };
                if let Err(e) = again {
                    self.graphics_lost(&e);
                }
            }
            None => {
                let text = format!("render error: {e}\n");
                if text != self.render_errors.last {
                    self.con.print(&text);
                    self.render_errors.last = text;
                }
            }
        }
    }

    #[cfg(not(any(target_arch = "wasm32", target_os = "android")))]
    fn new_surface(&mut self) -> qrender::Result<()> {
        match &self.window {
            Some(w) => self.r.recreate_surface(w),
            None => Err(Error::new("no window")),
        }
    }

    /// (the wgpu backend replaces its own)
    #[cfg(any(target_arch = "wasm32", target_os = "android"))]
    fn new_surface(&mut self) -> qrender::Result<()> {
        Err(Error::new("no window surface"))
    }

    fn graphics_lost(&mut self, e: &Error) {
        if self.fatal.is_some() {
            return;
        }
        if !self.headless {
            self.background_autosave();
        }
        self.fatal = Some(("Quake lost the graphics device", format!("The graphics driver reset or stopped, so the game has to end: {e}")));
        self.quit = true;
    }
}
