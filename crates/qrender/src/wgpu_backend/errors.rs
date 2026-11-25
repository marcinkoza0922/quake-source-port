//! Errors wgpu reports outside the call that caused them. Its own handler
//! panics on native, which ends the game; here [`Renderer::render`] returns them.
//!
//! [`Renderer::render`]: super::Renderer::render

use std::sync::{Arc, Mutex};

use crate::{Error, Lost, Result};

#[derive(Clone, Default)]
pub(super) struct DeviceErrors(Arc<Mutex<State>>);

#[derive(Default)]
struct State {
    /// The device is gone for good.
    lost: Option<String>,
    /// First error since the last [`DeviceErrors::take`].
    first: Option<String>,
}

impl DeviceErrors {
    pub(super) fn watch(dev: &wgpu::Device) -> Self {
        let errors = Self::default();
        let e = errors.clone();
        dev.on_uncaptured_error(Arc::new(move |err: wgpu::Error| {
            e.with(|s| {
                s.first.get_or_insert_with(|| err.to_string());
            });
        }));
        let e = errors.clone();
        dev.set_device_lost_callback(move |reason, message| {
            e.with(|s| {
                s.lost.get_or_insert_with(|| if message.is_empty() { format!("{reason:?}") } else { message.clone() });
            });
        });
        errors
    }

    fn with<T>(&self, f: impl FnOnce(&mut State) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }

    /// Errors while the device is gone for good are only ever about that.
    pub(super) fn take(&self) -> Result<()> {
        self.with(|s| match (&s.lost, s.first.take()) {
            (Some(why), _) => Err(Error::lost(Lost::Device, format!("wgpu: {why}"))),
            (None, Some(e)) => Err(Error::new(format!("wgpu: {e}"))),
            (None, None) => Ok(()),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Lost, RenderFrame, Renderer};

    fn headless() -> Option<Renderer> {
        Renderer::headless(64, 64, &[[0; 3]; 256]).ok()
    }

    #[test]
    fn a_lost_device_is_reported_by_every_frame() {
        let Some(mut r) = headless() else { return };
        r.render(&RenderFrame::default()).unwrap();
        r.dev.destroy();
        // (a destroyed device is lost once its queue is empty; a driver's loss at once)
        let _ = r.dev.poll(wgpu::PollType::wait_indefinitely());
        for _ in 0..3 {
            assert_eq!(r.render(&RenderFrame::default()).unwrap_err().lost, Some(Lost::Device));
        }
    }

    #[test]
    fn an_invalid_call_fails_one_frame() {
        let Some(mut r) = headless() else { return };
        r.render(&RenderFrame::default()).unwrap();
        // more than any device allows
        r.dev.create_buffer(&wgpu::BufferDescriptor { label: None, size: u64::MAX / 2, usage: wgpu::BufferUsages::VERTEX, mapped_at_creation: false });
        let e = r.render(&RenderFrame::default()).unwrap_err();
        assert_eq!(e.lost, None, "{e}");
        r.render(&RenderFrame::default()).unwrap();
    }
}
