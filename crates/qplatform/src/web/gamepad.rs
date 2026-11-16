//! Gamepad API polling: diffs `navigator.getGamepads()` each frame into
//! added/removed/button/axis events with SDL ids. The pad index is the id.

use std::collections::HashMap;

use wasm_bindgen::JsCast;
use web_sys::{Gamepad, GamepadButton};

use super::{Shared, js_err, window};
use crate::{Error, Event, Result, axis, button, family_from_vendor_name};

/// "standard" mapping button index to SDL button (triggers 6/7 are axes).
const BUTTONS: [(usize, u8); 15] = [
    (0, button::SOUTH),
    (1, button::EAST),
    (2, button::WEST),
    (3, button::NORTH),
    (4, button::LEFT_SHOULDER),
    (5, button::RIGHT_SHOULDER),
    (8, button::BACK),
    (9, button::START),
    (10, button::LEFT_STICK),
    (11, button::RIGHT_STICK),
    (12, button::DPAD_UP),
    (13, button::DPAD_DOWN),
    (14, button::DPAD_LEFT),
    (15, button::DPAD_RIGHT),
    (16, button::GUIDE),
];

#[derive(Default)]
struct PadState {
    buttons: [bool; BUTTONS.len()],
    axes: [i16; 6],
}

#[derive(Default)]
pub(super) struct Pads {
    pads: HashMap<u32, PadState>,
}

fn connected_pads() -> Vec<Gamepad> {
    let Some(arr) = web_sys::window().and_then(|w| w.navigator().get_gamepads().ok()) else { return Vec::new() };
    arr.iter().filter_map(|v| v.dyn_into::<Gamepad>().ok()).filter(|g| g.connected()).collect()
}

fn button_at(g: &Gamepad, i: usize) -> Option<GamepadButton> {
    g.buttons().get(i as u32).dyn_into::<GamepadButton>().ok()
}

fn to_i16(v: f64) -> i16 {
    (v.clamp(-1.0, 1.0) * 32767.0) as i16
}

/// Vendor id from Chrome/Firefox id strings ("... Vendor: 054c ..." or "054c-0ce6-...").
fn vendor_of(id: &str) -> u16 {
    if let Some(i) = id.find("Vendor: ") {
        return u16::from_str_radix(id.get(i + 8..i + 12).unwrap_or(""), 16).unwrap_or(0);
    }
    id.get(..4).filter(|_| id.as_bytes().get(4) == Some(&b'-')).and_then(|v| u16::from_str_radix(v, 16).ok()).unwrap_or(0)
}

impl Pads {
    pub(super) fn ids(&self) -> Vec<u32> {
        self.pads.keys().copied().collect()
    }

    pub(super) fn poll(&mut self, s: &Shared) {
        let live = connected_pads();
        let gone: Vec<u32> = self.pads.keys().copied().filter(|id| !live.iter().any(|g| g.index() == *id)).collect();
        for id in gone {
            self.pads.remove(&id);
            s.push(Event::GamepadRemoved { id });
        }
        for g in &live {
            let id = g.index();
            let st = self.pads.entry(id).or_insert_with(|| {
                let name = g.id();
                let family = family_from_vendor_name(vendor_of(&name), &name);
                s.push(Event::GamepadAdded { id, name, family });
                PadState::default()
            });
            for (slot, &(i, b)) in BUTTONS.iter().enumerate() {
                let down = button_at(g, i).is_some_and(|b| b.pressed());
                if down != st.buttons[slot] {
                    st.buttons[slot] = down;
                    s.push(Event::GamepadButton { id, button: b, down });
                }
            }
            let axes = g.axes();
            let mut now = [0i16; 6];
            for (a, v) in [axis::LEFTX, axis::LEFTY, axis::RIGHTX, axis::RIGHTY].into_iter().zip(0..) {
                now[a as usize] = to_i16(axes.get(v).as_f64().unwrap_or(0.0));
            }
            for (a, i) in [(axis::LEFT_TRIGGER, 6), (axis::RIGHT_TRIGGER, 7)] {
                now[a as usize] = to_i16(button_at(g, i).map_or(0.0, |b| b.value()));
            }
            for (a, (&v, old)) in now.iter().zip(st.axes.iter_mut()).enumerate() {
                if v != *old {
                    *old = v;
                    s.push(Event::GamepadAxis { id, axis: a as u8, value: v });
                }
            }
        }
    }
}

/// `vibrationActuator.playEffect("dual-rumble", …)` where supported.
pub(super) fn rumble(id: u32, low: u16, high: u16, duration_ms: u32) -> Result<()> {
    let _ = window()?;
    let g = connected_pads().into_iter().find(|g| g.index() == id).ok_or_else(|| Error(format!("no gamepad {id}")))?;
    let get = |o: &wasm_bindgen::JsValue, k: &str| js_sys::Reflect::get(o, &k.into()).map_err(js_err);
    let act = get(&g, "vibrationActuator")?;
    if act.is_undefined() || act.is_null() {
        return Err(Error("gamepad has no rumble".into()));
    }
    let play: js_sys::Function = get(&act, "playEffect")?.dyn_into().map_err(js_err)?;
    let p = js_sys::Object::new();
    let set = |k: &str, v: f64| js_sys::Reflect::set(&p, &k.into(), &v.into()).map_err(js_err);
    set("duration", duration_ms as f64)?;
    set("strongMagnitude", low as f64 / 65535.0)?;
    set("weakMagnitude", high as f64 / 65535.0)?;
    play.call2(&act, &"dual-rumble".into(), &p).map_err(js_err)?;
    Ok(())
}
