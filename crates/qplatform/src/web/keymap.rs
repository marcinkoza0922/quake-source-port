//! DOM `KeyboardEvent.code` / `.key` to SDL scancodes, keycodes and modifiers,
//! so the host's SDL-based key conversion works unchanged.

use web_sys::KeyboardEvent;

const SCANCODE_MASK: u32 = 1 << 30;

/// SDL scancode (USB HID usage) for a DOM `code`; 0 if unknown.
pub(super) fn scancode(code: &str) -> u32 {
    if let Some(l) = code.strip_prefix("Key")
        && let [c @ b'A'..=b'Z'] = l.as_bytes()
    {
        return 4 + (c - b'A') as u32;
    }
    if let Some(d) = code.strip_prefix("Digit")
        && let [c @ b'0'..=b'9'] = d.as_bytes()
    {
        return if *c == b'0' { 39 } else { 30 + (c - b'1') as u32 };
    }
    if let Some(n) = code.strip_prefix('F').and_then(|n| n.parse::<u32>().ok())
        && (1..=12).contains(&n)
    {
        return 57 + n;
    }
    if let Some(d) = code.strip_prefix("Numpad")
        && let [c @ b'0'..=b'9'] = d.as_bytes()
    {
        return if *c == b'0' { 98 } else { 89 + (c - b'1') as u32 };
    }
    match code {
        "Enter" => 40,
        "Escape" => 41,
        "Backspace" => 42,
        "Tab" => 43,
        "Space" => 44,
        "Minus" => 45,
        "Equal" => 46,
        "BracketLeft" => 47,
        "BracketRight" => 48,
        "Backslash" => 49,
        "Semicolon" => 51,
        "Quote" => 52,
        "Backquote" => 53,
        "Comma" => 54,
        "Period" => 55,
        "Slash" => 56,
        "CapsLock" => 57,
        "PrintScreen" => 70,
        "ScrollLock" => 71,
        "Pause" => 72,
        "Insert" => 73,
        "Home" => 74,
        "PageUp" => 75,
        "Delete" => 76,
        "End" => 77,
        "PageDown" => 78,
        "ArrowRight" => 79,
        "ArrowLeft" => 80,
        "ArrowDown" => 81,
        "ArrowUp" => 82,
        "NumLock" => 83,
        "NumpadDivide" => 84,
        "NumpadMultiply" => 85,
        "NumpadSubtract" => 86,
        "NumpadAdd" => 87,
        "NumpadEnter" => 88,
        "NumpadDecimal" => 99,
        "IntlBackslash" => 100,
        "ControlLeft" => 224,
        "ShiftLeft" => 225,
        "AltLeft" => 226,
        "MetaLeft" => 227,
        "ControlRight" => 228,
        "ShiftRight" => 229,
        "AltRight" => 230,
        "MetaRight" => 231,
        _ => 0,
    }
}

/// SDL keycode: the lowercased character for printable keys, ASCII control
/// codes for Enter/Escape/Backspace/Tab/Delete, else the masked scancode.
pub(super) fn keycode(key: &str, scancode: u32) -> u32 {
    let mut ch = key.chars();
    if let (Some(c), None) = (ch.next(), ch.next()) {
        return c.to_lowercase().next().unwrap_or(c) as u32;
    }
    match key {
        "Enter" => 13,
        "Escape" => 27,
        "Backspace" => 8,
        "Tab" => 9,
        "Delete" => 127,
        _ if scancode != 0 => scancode | SCANCODE_MASK,
        _ => 0,
    }
}

/// SDL_Keymod bits (left-hand variants).
pub(super) fn modifiers(k: &KeyboardEvent) -> u16 {
    let mut m = 0;
    if k.shift_key() {
        m |= 0x0001;
    }
    if k.ctrl_key() {
        m |= 0x0040;
    }
    if k.alt_key() {
        m |= 0x0100;
    }
    if k.meta_key() {
        m |= 0x0400;
    }
    m
}
