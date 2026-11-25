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
