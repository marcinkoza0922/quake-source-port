//! What frames that can't be drawn do to the game.

use super::super::*;
use qrender::{Error, Lost};

fn fresh() -> Option<Host> {
    let fs = qcore::fs::Fs::new(std::path::Path::new("."));
    let r = Renderer::headless(64, 64, &[[0u8; 3]; 256]).ok()?;
    Some(Host::new(fs, r, None, Vec::new()))
}

#[test]
fn a_lost_device_ends_the_game_with_a_message() {
    let Some(mut h) = fresh() else { return };
    h.after_render(Err(Error::lost(Lost::Device, "gone")));
    assert!(h.quit);
    let (_, message) = h.fatal.clone().unwrap();
    assert!(message.contains("gone"), "{message}");
    // the frames until the loop ends don't change the reason
    h.after_render(Err(Error::lost(Lost::Device, "later")));
    assert_eq!(h.fatal.unwrap().1, message);
}

#[test]
fn a_surface_that_cannot_be_replaced_ends_the_game() {
    let Some(mut h) = fresh() else { return };
    h.after_render(Err(Error::lost(Lost::Surface, "gone")));
    assert!(h.quit && h.fatal.is_some(), "no window to make one for");
}

#[test]
fn an_error_in_every_frame_is_printed_once() {
    let Some(mut h) = fresh() else { return };
    h.frame(1.0 / 72.0, Vec::new());
    let count = |h: &Host| h.con.lines.iter().filter(|l| l.windows(20).any(|w| w == b"render error: broken")).count();
    for _ in 0..5 {
        h.after_render(Err(Error::new("broken")));
    }
    assert_eq!(count(&h), 1);
    assert!(!h.quit && h.fatal.is_none());
    h.after_render(Ok(()));
    h.after_render(Err(Error::new("broken")));
    assert_eq!(count(&h), 2, "it came back");
}
