//! Touchscreens in the browser: DOM touch events become [`Event::Finger`]
//! (canvas-normalized coordinates and `performance.now()` nanoseconds, as
//! SDL reports them), and a hidden text field brings up a phone's
//! on-screen keyboard while text input is on (a canvas can't take focus
//! for typing). Its typing arrives as [`Event::Text`], and deletions and
//! line breaks as Backspace/Enter keys.

use std::rc::Rc;

use wasm_bindgen::JsCast;
use web_sys::{EventTarget, HtmlCanvasElement, HtmlInputElement, InputEvent, TouchEvent};

use super::{Listener, Shared, js_err, keymap};
use crate::{Event, FingerPhase, Result};

/// The field always holds this one character, so a deletion on an
/// otherwise empty field still fires an `input` event.
const SENTINEL: &str = " ";

/// touchstart/move/end/cancel on the canvas.
pub(super) fn touch_listeners(c: &HtmlCanvasElement, shared: &Rc<Shared>) -> Result<Vec<Listener>> {
    let t: &EventTarget = c.as_ref();
    let mut out = Vec::new();
    for (name, phase) in [
        ("touchstart", FingerPhase::Down),
        ("touchmove", FingerPhase::Move),
        ("touchend", FingerPhase::Up),
        ("touchcancel", FingerPhase::Cancel),
    ] {
        let (s, cv) = (shared.clone(), c.clone());
        out.push(Listener::new(t, name, move |e| {
            let Ok(te) = e.dyn_into::<TouchEvent>() else { return };
            // no page scrolling or zooming, and no emulated mouse events
            te.prevent_default();
            s.touch_used.set(true);
            let r = cv.get_bounding_client_rect();
            let (w, h) = (r.width().max(1.0), r.height().max(1.0));
            let timestamp_ns = (te.time_stamp() * 1e6) as u64;
            let list = te.changed_touches();
            for i in 0..list.length() {
                let Some(p) = list.item(i) else { continue };
                let x = ((p.client_x() as f64 - r.left()) / w) as f32;
                let y = ((p.client_y() as f64 - r.top()) / h) as f32;
                s.push(Event::Finger { id: p.identifier() as u32 as u64, phase, x, y, timestamp_ns });
            }
            // browsers only raise the keyboard for focus during a gesture:
            // a tap while text input is on (re)focuses the field
            if phase == FingerPhase::Up && s.text_input.get() {
                focus_field(&s);
            }
        })?);
    }
    Ok(out)
}

/// Shows the on-screen keyboard (touch devices only) or hides it.
pub(super) fn set_keyboard(s: &Rc<Shared>, on: bool) {
    if on && s.touch_used.get() {
        if s.text_field.borrow().is_none() {
            match create_field(s) {
                Ok(f) => *s.text_field.borrow_mut() = Some(f),
                Err(_) => return,
            }
        }
        focus_field(s);
    } else if let Some((f, _)) = s.text_field.borrow().as_ref() {
        let _ = f.blur();
    }
}

fn focus_field(s: &Shared) {
    if let Some((f, _)) = s.text_field.borrow().as_ref() {
        f.set_value(SENTINEL);
        let _ = f.focus();
    }
}

/// The hidden field and its `input` listener.
fn create_field(s: &Rc<Shared>) -> Result<(HtmlInputElement, Listener)> {
    let doc = web_sys::window().and_then(|w| w.document()).ok_or_else(|| crate::Error("no document".into()))?;
    let f: HtmlInputElement = doc.create_element("input").map_err(js_err)?.dyn_into().map_err(|_| crate::Error("input element".into()))?;
    f.set_type("text");
    f.set_id("quake-text");
    for (k, v) in [("autocomplete", "off"), ("autocorrect", "off"), ("autocapitalize", "off"), ("spellcheck", "false"), ("aria-hidden", "true")] {
        let _ = f.set_attribute(k, v);
    }
    // invisible but focusable; 16px keeps iOS from zooming the page
    let style = f.style();
    for (k, v) in [("position", "fixed"), ("left", "0"), ("bottom", "0"), ("width", "1px"), ("height", "1px"), ("opacity", "0"), ("font-size", "16px"), ("border", "0"), ("padding", "0")] {
        let _ = style.set_property(k, v);
    }
    doc.body().ok_or_else(|| crate::Error("no body".into()))?.append_child(&f).map_err(js_err)?;
    let (sh, field) = (s.clone(), f.clone());
    let l = Listener::new(f.as_ref(), "input", move |e| {
        let kind = e.dyn_ref::<InputEvent>().map(|i| i.input_type()).unwrap_or_default();
        let key = |name: &str| {
            let sc = keymap::scancode(name);
            let k = keymap::keycode(name, sc);
            for down in [true, false] {
                sh.push(Event::Key { scancode: sc, key: k, modifiers: 0, down, repeat: false });
            }
        };
        if kind.starts_with("delete") {
            key("Backspace");
        } else if kind == "insertLineBreak" || kind == "insertParagraph" {
            key("Enter");
        } else {
            let v = field.value();
            let typed = v.strip_prefix(SENTINEL).unwrap_or(&v);
            if !typed.is_empty() && sh.text_input.get() {
                sh.push(Event::Text(typed.to_string()));
            }
        }
        field.set_value(SENTINEL);
    })?;
    Ok((f, l))
}
