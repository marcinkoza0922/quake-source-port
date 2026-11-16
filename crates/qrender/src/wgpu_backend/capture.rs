//! Reading frames back to the CPU (screenshots, save thumbnails, tests).
//!
//! Headless renderers copy every frame into a readback buffer and map it
//! blocking. A presented frame (the web canvas) is copied only after
//! `request_capture`, and the buffer is mapped asynchronously: in the browser
//! the map completes only once control returns to the event loop, so
//! [`Renderer::take_capture`] reports it as pending until a later frame.

use super::Renderer;
use crate::{Error, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const MAPPING: u8 = 0;
const MAPPED: u8 = 1;
const FAILED: u8 = 2;

/// A presented frame copied into `buf`, being mapped for reading.
pub(super) struct Pending {
    buf: wgpu::Buffer,
    /// Padded bytes per row in `buf`.
    bpr: u32,
    width: u32,
    height: u32,
    /// The surface stores BGRA; swizzle to RGBA.
    bgra: bool,
    /// `map_async` was called (after the copy was submitted).
    mapping: bool,
    state: Arc<AtomicU8>,
}

/// Unpads `rows` rows of `width` pixels and forces opaque alpha.
fn unpad(data: &[u8], bpr: u32, width: u32, height: u32, bgra: bool) -> Vec<u8> {
    let row = (width * 4) as usize;
    let mut px = Vec::with_capacity(row * height as usize);
    for y in 0..height as usize {
        let ofs = y * bpr as usize;
        px.extend_from_slice(&data[ofs..ofs + row]);
    }
    for p in px.as_chunks_mut::<4>().0 {
        if bgra {
            p.swap(0, 2);
        }
        p[3] = 255;
    }
    px
}

impl Renderer {
    /// Records a copy of the presented frame `tex` when a capture was
    /// requested; [`Renderer::start_capture_map`] maps it after submit.
    pub(super) fn record_capture(&mut self, enc: &mut wgpu::CommandEncoder, tex: &wgpu::Texture) {
        if !self.capture_req || self.capture.is_some() {
            return;
        }
        self.capture_req = false;
        if !tex.usage().contains(wgpu::TextureUsages::COPY_SRC) {
            return;
        }
        let (width, height) = (tex.width(), tex.height());
        let bpr = (width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let buf = self.dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("capture"),
            size: bpr as u64 * height as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(bpr), rows_per_image: None },
            },
            tex.size(),
        );
        let bgra = matches!(tex.format(), wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb);
        self.capture = Some(Pending { buf, bpr, width, height, bgra, mapping: false, state: Arc::new(AtomicU8::new(MAPPING)) });
    }

    /// Starts mapping the capture recorded this frame (after submit).
    pub(super) fn start_capture_map(&mut self) {
        let Some(c) = &mut self.capture else { return };
        if std::mem::replace(&mut c.mapping, true) {
            return;
        }
        let state = c.state.clone();
        c.buf.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            state.store(if r.is_ok() { MAPPED } else { FAILED }, Ordering::Release);
        });
    }

    /// The pixels of a captured frame as (width, height, RGBA8): `Ok(None)`
    /// while the copy requested with `request_capture` is still in flight
    /// (try again next frame). Headless renderers return the last frame.
    pub fn take_capture(&mut self) -> Result<Option<(u32, u32, Vec<u8>)>> {
        if self.surface.is_none() {
            let px = self.read_back()?;
            return Ok(Some((self.width, self.height, px)));
        }
        let Some(c) = &self.capture else {
            if self.capture_req {
                // the frame was skipped (surface timeout); it is copied next time
                return Ok(None);
            }
            return Err(Error::new("this display cannot be captured"));
        };
        #[cfg(not(target_arch = "wasm32"))]
        let _ = self.dev.poll(wgpu::PollType::Poll);
        match c.state.load(Ordering::Acquire) {
            MAPPING => Ok(None),
            FAILED => {
                self.capture = None;
                Err(Error::new("reading the frame back failed"))
            }
            _ => {
                let c = self.capture.take().expect("checked above");
                let px = {
                    let data = c.buf.slice(..).get_mapped_range().map_err(|e| Error::new(e.to_string()))?;
                    unpad(&data, c.bpr, c.width, c.height, c.bgra)
                };
                c.buf.destroy();
                Ok(Some((c.width, c.height, px)))
            }
        }
    }

    /// Maps the headless readback buffer (blocking) and returns RGBA8 rows.
    pub(super) fn read_back(&mut self) -> Result<Vec<u8>> {
        let (w, h) = (self.width, self.height);
        let t = self.targets.as_ref().ok_or_else(|| Error::new("no frame rendered"))?;
        let (_, buf, bpr) = t.out.as_ref().ok_or_else(|| Error::new("screen capture needs a headless renderer"))?;
        if !self.frame_ready {
            return Err(Error::new("no frame rendered"));
        }
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        let _ = self.dev.poll(wgpu::PollType::wait_indefinitely());
        rx.recv()
            .map_err(|e| Error::new(e.to_string()))?
            .map_err(|e| Error::new(e.to_string()))?;
        let px = {
            let data = slice.get_mapped_range().map_err(|e| Error::new(e.to_string()))?;
            unpad(&data, *bpr, w, h, false)
        };
        buf.unmap();
        Ok(px)
    }
}

#[cfg(test)]
mod tests {
    use super::unpad;

    #[test]
    fn unpad_drops_row_padding_swizzles_and_forces_alpha() {
        // 2x2, rows padded to 12 bytes
        let mut data = vec![0u8; 24];
        data[0..8].copy_from_slice(&[1, 2, 3, 0, 4, 5, 6, 7]);
        data[12..20].copy_from_slice(&[8, 9, 10, 11, 12, 13, 14, 15]);
        assert_eq!(unpad(&data, 12, 2, 2, false), [1, 2, 3, 255, 4, 5, 6, 255, 8, 9, 10, 255, 12, 13, 14, 255]);
        assert_eq!(unpad(&data, 12, 2, 2, true)[..8], [3, 2, 1, 255, 6, 5, 4, 255]);
    }
}
