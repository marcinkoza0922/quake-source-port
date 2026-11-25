//! A frame that ends early must not leave the next one waiting forever.
use super::*;

fn headless() -> Option<Renderer> {
    Renderer::headless(64, 64, &[[0; 3]; 256]).ok()
}

/// Runs `f` on a thread, so a hang fails the test instead of the run.
fn in_time(f: impl FnOnce() + Send + 'static) {
    let (tx, rx) = std::sync::mpsc::channel();
    let t = std::thread::spawn(move || {
        f();
        let _ = tx.send(());
    });
    if rx.recv_timeout(std::time::Duration::from_secs(30)) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
        panic!("render never came back");
    }
    t.join().unwrap();
}

#[test]
fn no_lightmap_draws_nothing_and_does_not_hang() {
    in_time(|| {
        let Some(mut r) = headless() else { return };
        let fr = RenderFrame::default();
        r.render(&fr).unwrap();
        r.idle();
        let lm = r.lightmap.take().unwrap();
        for _ in 0..4 {
            r.render(&fr).unwrap();
        }
        r.lightmap = Some(lm);
        for _ in 0..4 {
            r.render(&fr).unwrap();
        }
    });
}

#[test]
fn a_frame_abandoned_after_its_fence_was_reset_recovers() {
    in_time(|| {
        let Some(mut r) = headless() else { return };
        let fr = RenderFrame::default();
        for _ in 0..3 {
            r.render(&fr).unwrap();
            // as `render` does when it fails between the reset and the submit
            let fence = r.frames[r.cur].fence;
            // SAFETY: waited for, so not in use.
            unsafe {
                r.gpu.device.wait_for_fences(&[fence], true, u64::MAX).unwrap();
                r.gpu.device.reset_fences(&[fence]).unwrap();
            }
            r.abandon_frame();
        }
        r.render(&fr).unwrap();
    });
}

#[test]
fn images_without_pixels_become_one_texel() {
    let Some(mut r) = headless() else { return };
    r.register_pic_rgba(0, 8, &[]).unwrap();
    r.register_pic_rgba(0, 65535, &[]).unwrap();
    r.register_pic_rgba(4, 4, &[0; 10]).unwrap();
    let face = qformats::image::Rgba { width: 0, height: 4096, pixels: Vec::new() };
    r.set_skybox(Some(&std::array::from_fn(|_| face.clone()))).unwrap();
    r.render(&RenderFrame::default()).unwrap();
}

#[test]
fn losses_are_told_apart_from_other_errors() {
    for (r, lost) in [
        (vk::Result::ERROR_DEVICE_LOST, Some(Lost::Device)),
        (vk::Result::ERROR_SURFACE_LOST_KHR, Some(Lost::Surface)),
        (vk::Result::ERROR_OUT_OF_DEVICE_MEMORY, None),
    ] {
        assert_eq!(Error::from(r).lost, lost, "{r}");
    }
}

#[test]
fn a_minimised_window_has_no_swapchain_size() {
    let e = |width, height| vk::Extent2D { width, height };
    let caps = |current| vk::SurfaceCapabilitiesKHR { current_extent: current, min_image_extent: e(1, 1), max_image_extent: e(4096, 4096), ..Default::default() };
    let any = caps(e(u32::MAX, u32::MAX));
    for (c, window, size) in [
        (caps(e(0, 0)), (1280, 720), None), // Windows
        (caps(e(1280, 0)), (1280, 720), None),
        (caps(e(800, 600)), (0, 0), Some(e(800, 600))),
        (any, (0, 0), None),
        (any, (1280, 720), Some(e(1280, 720))),
        (any, (9000, 720), Some(e(4096, 720))),
    ] {
        assert_eq!(swapchain::swap_extent(&c, window), size, "{window:?}");
    }
}
