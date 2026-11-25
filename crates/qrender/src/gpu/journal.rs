//! Undoing half-done work: what is created inside a scope is recorded, so an
//! error in the middle doesn't leak the part before it.

use super::*;

/// Something created in an open scope, not handed to its owner yet.
pub(super) enum Made {
    Image(vk::Image, vk::ImageView, Mem),
    View(vk::ImageView),
    Buffer(vk::Buffer, Mem),
    Accel(vk::AccelerationStructureKHR),
    Pool(vk::DescriptorPool),
    Cmd(vk::CommandBuffer),
}

/// Open scopes nest: an inner one that fails undoes only its own part.
#[derive(Default)]
pub struct Journal {
    made: Vec<Made>,
    depth: u32,
    /// Fails the nth creation from now (0 = the next one).
    #[cfg(test)]
    pub fail_in: std::cell::Cell<Option<u32>>,
    /// Pipelines of the build in progress (`Pipelines::build`).
    pub pipelines: std::cell::RefCell<Vec<vk::Pipeline>>,
}

impl Journal {
    pub(super) fn add(&mut self, m: Made) {
        if self.depth > 0 {
            self.made.push(m);
        }
    }

    /// The owner freed it itself.
    fn forget(&mut self, gone: impl Fn(&Made) -> bool) {
        if let Some(i) = self.made.iter().rposition(gone) {
            self.made.remove(i);
        }
    }

    pub(super) fn forget_image(&mut self, i: vk::Image) {
        self.forget(|m| matches!(m, Made::Image(x, ..) if *x == i));
    }

    pub(super) fn forget_buffer(&mut self, b: vk::Buffer) {
        self.forget(|m| matches!(m, Made::Buffer(x, _) if *x == b));
    }

    pub(super) fn forget_accel(&mut self, a: vk::AccelerationStructureKHR) {
        self.forget(|m| matches!(m, Made::Accel(x) if *x == a));
    }

    pub(super) fn forget_cmd(&mut self, c: vk::CommandBuffer) {
        self.forget(|m| matches!(m, Made::Cmd(x) if *x == c));
    }

    #[cfg(test)]
    pub fn injected(&self) -> Result<()> {
        match self.fail_in.get() {
            Some(0) => {
                self.fail_in.set(None);
                Err(Error::new("injected"))
            }
            n => {
                self.fail_in.set(n.map(|n| n - 1));
                Ok(())
            }
        }
    }

    #[cfg(not(test))]
    #[inline(always)]
    pub fn injected(&self) -> Result<()> {
        Ok(())
    }
}

impl Gpu {
    /// Opens a scope; [`Self::end_scope`] takes the result.
    pub fn begin_scope(&mut self) -> usize {
        self.journal.depth += 1;
        self.journal.made.len()
    }

    /// Closes the scope opened at `mark`. With `undo`, destroys what it
    /// created (the device must be idle) and returns the images among it.
    pub fn end_scope(&mut self, mark: usize, undo: bool) -> Vec<vk::Image> {
        let mut images = Vec::new();
        if undo {
            for m in self.journal.made.split_off(mark).into_iter().rev() {
                // SAFETY: device idle (caller contract); created in this scope
                // and not destroyed since, or it would have been forgotten.
                unsafe {
                    match m {
                        Made::Image(image, view, mem) => {
                            self.device.destroy_image_view(view, None);
                            self.device.destroy_image(image, None);
                            self.alloc.free(&mem);
                            images.push(image);
                        }
                        Made::View(v) => self.device.destroy_image_view(v, None),
                        Made::Buffer(b, mem) => {
                            self.device.destroy_buffer(b, None);
                            self.alloc.free(&mem);
                        }
                        Made::Accel(a) => {
                            if let Some(rt) = &self.rt {
                                rt.accel.destroy_acceleration_structure(a, None);
                            }
                        }
                        Made::Pool(p) => self.device.destroy_descriptor_pool(p, None),
                        Made::Cmd(c) => self.device.free_command_buffers(self.upload_pool, &[c]),
                    }
                }
            }
        }
        self.journal.depth -= 1;
        if self.journal.depth == 0 {
            self.journal.made.clear();
        }
        images
    }

    /// Another view of an image made in this scope.
    pub fn image_view(&mut self, ci: &vk::ImageViewCreateInfo) -> Result<vk::ImageView> {
        self.journal.injected()?;
        // SAFETY: valid device and create info (callers pass live images).
        let v = unsafe { self.device.create_image_view(ci, None)? };
        self.journal.add(Made::View(v));
        Ok(v)
    }

    pub fn descriptor_pool(&mut self, ci: &vk::DescriptorPoolCreateInfo) -> Result<vk::DescriptorPool> {
        self.journal.injected()?;
        // SAFETY: valid device and create info.
        let p = unsafe { self.device.create_descriptor_pool(ci, None)? };
        self.journal.add(Made::Pool(p));
        Ok(p)
    }
}
