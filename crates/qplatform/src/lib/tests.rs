use super::*;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

const HEADLESS: &[(&str, &str)] = &[("SDL_VIDEO_DRIVER", "dummy"), ("SDL_AUDIO_DRIVER", "dummy")];

#[test]
fn window_and_events_headless() {
    let mut p = Platform::init_with_hints(HEADLESS).unwrap();
    let w = p.create_window("t", 320, 200, false).unwrap();
    let (dw, dh) = w.drawable_size();
    assert!(dw > 0 && dh > 0);
    w.set_title("t2").unwrap();
    let _ = p.poll_events();
    drop(w);
    assert!(p.rumble(12345, 1, 1, 1).is_err());
    assert!(!p.has_gyro(12345));
}

#[test]
fn audio_callback_runs() {
    let p = Platform::init_with_hints(HEADLESS).unwrap();
    let n = Arc::new(AtomicUsize::new(0));
    let n2 = n.clone();
    let s = p
        .open_audio(48000, move |buf| {
            assert_eq!(buf.len() % 2, 0);
            buf.fill(0);
            n2.fetch_add(buf.len(), Ordering::Relaxed);
        })
        .unwrap();
    s.resume().unwrap();
    let t0 = ticks_ns();
    while n.load(Ordering::Relaxed) == 0 && ticks_ns() - t0 < 3_000_000_000 {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    drop(s);
    assert!(n.load(Ordering::Relaxed) > 0);
}

#[test]
fn timer_monotonic() {
    let _p = Platform::init_with_hints(HEADLESS).unwrap();
    let a = ticks_ns();
    let (c, f) = performance_counter();
    assert!(f > 0 && c > 0);
    assert!(ticks_ns() >= a);
}
