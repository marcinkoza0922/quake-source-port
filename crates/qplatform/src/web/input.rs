//! DOM keyboard and mouse listeners feeding the event queue.

use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlCanvasElement, KeyboardEvent, MouseEvent, WheelEvent};

use super::{Listener, Shared, keymap};
use crate::{Event, Result};

/// keydown/keyup on the window: [`Event::Key`] plus [`Event::Text`] while
/// text input is on.
pub(super) fn keyboard_listeners(w: &EventTarget, shared: &Rc<Shared>) -> Result<Vec<Listener>> {
    let mut out = Vec::new();
    for (name, down) in [("keydown", true), ("keyup", false)] {
        let s = shared.clone();
        out.push(Listener::new(w, name, move |e| {
            let Ok(k) = e.dyn_into::<KeyboardEvent>() else { return };
            if k.is_composing() {
                return;
            }
            // typing into the touch keyboard's field: its `input` events
            // carry the text (`touch.rs`), so only editing keys go on
            let field = k.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).is_some();
            if field && k.key().chars().count() == 1 {
                return;
            }
            let code = k.code();
            let key = k.key();
            let scancode = keymap::scancode(&code);
            let keycode = keymap::keycode(&key, scancode);
            if scancode == 0 && keycode == 0 {
                return;
            }
            let shortcut = k.ctrl_key() || k.meta_key();
            // keep browser reload / devtools / fullscreen and Ctrl/Cmd shortcuts working
            if !shortcut && !matches!(code.as_str(), "F5" | "F11" | "F12") {
                k.prevent_default();
            }
            s.push(Event::Key { scancode, key: keycode, modifiers: keymap::modifiers(&k), down, repeat: k.repeat() });
            if down && s.text_input.get() && !shortcut {
                let mut ch = key.chars();
                if let (Some(c), None) = (ch.next(), ch.next()) {
                    s.push(Event::Text(c.to_string()));
                }
            }
        })?);
    }
    Ok(out)
}

/// Canvas position of a mouse event in backing pixels.
fn canvas_pos(c: &HtmlCanvasElement, m: &MouseEvent) -> (f32, f32) {
    let r = c.get_bounding_client_rect();
    let sx = if r.width() > 0.0 { c.width() as f64 / r.width() } else { 1.0 };
    let sy = if r.height() > 0.0 { c.height() as f64 / r.height() } else { 1.0 };
    (((m.client_x() as f64 - r.left()) * sx) as f32, ((m.client_y() as f64 - r.top()) * sy) as f32)
}

/// Mouse motion, buttons (re-requesting pointer lock on click), wheel and
/// context-menu suppression on the canvas.
pub(super) fn mouse_listeners(c: &HtmlCanvasElement, shared: &Rc<Shared>) -> Result<Vec<Listener>> {
    let t: &EventTarget = c.as_ref();
    let mut out = Vec::new();
    let (s, cv) = (shared.clone(), c.clone());
    out.push(Listener::new(t, "mousemove", move |e| {
        let Ok(m) = e.dyn_into::<MouseEvent>() else { return };
        let (x, y) = canvas_pos(&cv, &m);
        s.push(Event::MouseMotion { x, y, dx: m.movement_x() as f32, dy: m.movement_y() as f32 });
    })?);
    for (name, down) in [("mousedown", true), ("mouseup", false)] {
        let (s, cv) = (shared.clone(), c.clone());
        out.push(Listener::new(t, name, move |e| {
            let Ok(m) = e.dyn_into::<MouseEvent>() else { return };
            m.prevent_default();
            if down {
                let _ = cv.focus();
                let locked = web_sys::window().and_then(|w| w.document()).and_then(|d| d.pointer_lock_element());
                if s.want_lock.get() && locked.is_none() {
                    cv.request_pointer_lock();
                }
            }
            // DOM 0 left, 1 middle, 2 right, 3/4 back/forward -> SDL 1..5
            let button = (m.button() as i32 + 1).clamp(1, 5) as u8;
            let (x, y) = canvas_pos(&cv, &m);
            s.push(Event::MouseButton { button, down, x, y });
        })?);
    }
    let s = shared.clone();
    out.push(Listener::new(t, "wheel", move |e| {
        let Ok(w) = e.dyn_into::<WheelEvent>() else { return };
        w.prevent_default();
        // DOM: positive deltaY scrolls down; SDL: positive y is away from the user
        let step = |d: f64| if d > 0.0 { -1.0 } else if d < 0.0 { 1.0 } else { 0.0 };
        s.push(Event::MouseWheel { dx: -step(w.delta_x()), dy: step(w.delta_y()) });
    })?);
    out.push(Listener::new(t, "contextmenu", |e| e.prevent_default())?);
    Ok(out)
}
