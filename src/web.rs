//! Browser entry points (wasm32): `start(args)` builds the host on the
//! `<canvas id="quake">` and drives `Host::frame` from requestAnimationFrame;
//! `vfs_mount` / `vfs_take_dirty` / `vfs_read` let `web/main.js` populate the
//! in-memory file system and persist written user files. See docs/WEB_PORT.md.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use crate::{Opts, apply_rumble, host, open_game, open_sound, platform};

/// Inserts a game or user file into the virtual file system (not marked dirty).
#[wasm_bindgen]
pub fn vfs_mount(path: &str, bytes: Vec<u8>) {
    qcore::vfs::mount_file(Path::new(path), bytes);
}

/// Paths written since the last call. JS reads each with [`vfs_read`] and
/// stores it; a path that no longer reads was removed.
#[wasm_bindgen]
pub fn vfs_take_dirty() -> Vec<String> {
    qcore::vfs::take_dirty().into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

/// File contents, or `undefined` if the file does not exist.
#[wasm_bindgen]
pub fn vfs_read(path: &str) -> Option<Vec<u8>> {
    qcore::vfs::read(Path::new(path)).ok()
}

fn err(e: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&e.to_string())
}

type FrameCb = Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>>;

fn request_frame(cb: &FrameCb) {
    if let (Some(w), Some(c)) = (web_sys::window(), cb.borrow().as_ref()) {
        let _ = w.request_animation_frame(c.as_ref().unchecked_ref());
    }
}

/// Builds the host from command-line style `args` (e.g. `["+map", "e1m1"]`;
/// the base dir defaults to `/quake`) and starts the frame loop. Resolves
/// once the first frame is scheduled.
#[wasm_bindgen]
pub async fn start(args: Vec<String>) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let o = Opts { args };
    let (fs, basedir, pal) = open_game(&o).map_err(err)?;
    let mut p = qplatform::Platform::init().map_err(err)?;
    let win = p.create_window("Quake", 1280, 720, false).map_err(err)?;
    let renderer = qrender::Renderer::new_web(win.canvas(), &pal).await.map_err(|e| err(format!("{e:?}")))?;
    let (audio, snd) = if o.has("-nosound") { (None, None) } else { open_sound(&p, &basedir, &fs) };
    let mut host = host::Host::new(fs, renderer, snd, o.args.clone());
    let _ = win.set_text_input(true);
    host.window = Some(win);

    let mut audio = audio;
    let mut last = qplatform::ticks_ns();
    let cb: FrameCb = Rc::new(RefCell::new(None));
    let next = cb.clone();
    *cb.borrow_mut() = Some(Closure::new(move |_ts: f64| {
        let now = qplatform::ticks_ns();
        let el = now.saturating_sub(last) as f64 * 1e-9;
        // host_maxfps below the display rate: skip this animation frame
        let maxfps = if host.timedemo_active() { 0.0 } else { host.server.core.cvars.value_of("host_maxfps") };
        if maxfps > 0.0 && el + 0.002 < 1.0 / maxfps as f64 {
            request_frame(&next);
            return;
        }
        last = now;
        let events = p.poll_events().into_iter().flat_map(platform::convert).collect();
        host.frame(el.min(0.25), events);
        apply_rumble(&mut host, &p);
        if host.quit {
            // the page stays; persist config and stop the loop
            host.write_config();
            audio.take();
            return; // no further frames (the closure cannot drop itself while running)
        }
        request_frame(&next);
    }));
    request_frame(&cb);
    Ok(())
}
