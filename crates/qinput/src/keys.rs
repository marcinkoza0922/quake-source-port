//! Key numbers and names (ironwail keys.h/keys.c compatible).

pub const MAX_KEYS: usize = 256;

pub const K_TAB: i32 = 9;
pub const K_ENTER: i32 = 13;
pub const K_ESCAPE: i32 = 27;
pub const K_SPACE: i32 = 32;
pub const K_BACKSPACE: i32 = 127;
pub const K_UPARROW: i32 = 128;
pub const K_DOWNARROW: i32 = 129;
pub const K_LEFTARROW: i32 = 130;
pub const K_RIGHTARROW: i32 = 131;
pub const K_ALT: i32 = 132;
pub const K_CTRL: i32 = 133;
pub const K_SHIFT: i32 = 134;
pub const K_F1: i32 = 135; // ..K_F12 = 146
pub const K_INS: i32 = 147;
pub const K_DEL: i32 = 148;
pub const K_PGDN: i32 = 149;
pub const K_PGUP: i32 = 150;
pub const K_HOME: i32 = 151;
pub const K_END: i32 = 152;
pub const K_KP_NUMLOCK: i32 = 153;
pub const K_KP_SLASH: i32 = 154;
pub const K_KP_STAR: i32 = 155;
pub const K_KP_MINUS: i32 = 156;
pub const K_KP_HOME: i32 = 157;
pub const K_KP_UPARROW: i32 = 158;
pub const K_KP_PGUP: i32 = 159;
pub const K_KP_PLUS: i32 = 160;
pub const K_KP_LEFTARROW: i32 = 161;
pub const K_KP_5: i32 = 162;
pub const K_KP_RIGHTARROW: i32 = 163;
pub const K_KP_END: i32 = 164;
pub const K_KP_DOWNARROW: i32 = 165;
pub const K_KP_PGDN: i32 = 166;
pub const K_KP_ENTER: i32 = 167;
pub const K_KP_INS: i32 = 168;
pub const K_KP_DEL: i32 = 169;
pub const K_COMMAND: i32 = 170;
pub const K_CAPSLOCK: i32 = 171;
pub const K_SCROLLLOCK: i32 = 172;
pub const K_PRINTSCREEN: i32 = 173;

pub const K_MOUSE1: i32 = 200;
pub const K_MOUSE2: i32 = 201;
pub const K_MOUSE3: i32 = 202;
pub const K_MOUSE4: i32 = 203;
pub const K_MOUSE5: i32 = 204;
pub const K_MWHEELUP: i32 = 205;
pub const K_MWHEELDOWN: i32 = 206;

pub const K_GAMEPAD_BEGIN: i32 = 207;
pub const K_START: i32 = 207;
pub const K_BACK: i32 = 208;
pub const K_LTHUMB: i32 = 209;
pub const K_RTHUMB: i32 = 210;
pub const K_LSHOULDER: i32 = 211;
pub const K_RSHOULDER: i32 = 212;
pub const K_DPAD_UP: i32 = 213;
pub const K_DPAD_DOWN: i32 = 214;
pub const K_DPAD_LEFT: i32 = 215;
pub const K_DPAD_RIGHT: i32 = 216;
pub const K_ABUTTON: i32 = 217;
pub const K_BBUTTON: i32 = 218;
pub const K_XBUTTON: i32 = 219;
pub const K_YBUTTON: i32 = 220;
pub const K_LTRIGGER: i32 = 221;
pub const K_RTRIGGER: i32 = 222;
pub const K_MISC1: i32 = 223;
pub const K_PADDLE1: i32 = 224;
pub const K_PADDLE2: i32 = 225;
pub const K_PADDLE3: i32 = 226;
pub const K_PADDLE4: i32 = 227;
pub const K_TOUCHPAD: i32 = 228;
/// `<button>_ALT` = button + this (LTHUMB_ALT..TOUCHPAD_ALT).
pub const K_ALT_OFFSET: i32 = 20;
pub const K_GAMEPAD_END: i32 = 249;
pub const K_PAUSE: i32 = 249;

const PAD_NAMES: [&str; 20] = [
    "LTHUMB", "RTHUMB", "LSHOULDER", "RSHOULDER", "DPAD_UP", "DPAD_DOWN", "DPAD_LEFT", "DPAD_RIGHT", "ABUTTON",
    "BBUTTON", "XBUTTON", "YBUTTON", "LTRIGGER", "RTRIGGER", "MISC1", "PADDLE1", "PADDLE2", "PADDLE3", "PADDLE4",
    "TOUCHPAD",
];

const NAMES: &[(&str, i32)] = &[
    ("TAB", K_TAB),
    ("ENTER", K_ENTER),
    ("ESCAPE", K_ESCAPE),
    ("SPACE", K_SPACE),
    ("BACKSPACE", K_BACKSPACE),
    ("UPARROW", K_UPARROW),
    ("DOWNARROW", K_DOWNARROW),
    ("LEFTARROW", K_LEFTARROW),
    ("RIGHTARROW", K_RIGHTARROW),
    ("ALT", K_ALT),
    ("CTRL", K_CTRL),
    ("SHIFT", K_SHIFT),
    ("KP_NUMLOCK", K_KP_NUMLOCK),
    ("KP_SLASH", K_KP_SLASH),
    ("KP_STAR", K_KP_STAR),
    ("KP_MINUS", K_KP_MINUS),
    ("KP_HOME", K_KP_HOME),
    ("KP_UPARROW", K_KP_UPARROW),
    ("KP_PGUP", K_KP_PGUP),
    ("KP_PLUS", K_KP_PLUS),
    ("KP_LEFTARROW", K_KP_LEFTARROW),
    ("KP_5", K_KP_5),
    ("KP_RIGHTARROW", K_KP_RIGHTARROW),
    ("KP_END", K_KP_END),
    ("KP_DOWNARROW", K_KP_DOWNARROW),
    ("KP_PGDN", K_KP_PGDN),
    ("KP_ENTER", K_KP_ENTER),
    ("KP_INS", K_KP_INS),
    ("KP_DEL", K_KP_DEL),
    ("F1", K_F1),
    ("F2", K_F1 + 1),
    ("F3", K_F1 + 2),
    ("F4", K_F1 + 3),
    ("F5", K_F1 + 4),
    ("F6", K_F1 + 5),
    ("F7", K_F1 + 6),
    ("F8", K_F1 + 7),
    ("F9", K_F1 + 8),
    ("F10", K_F1 + 9),
    ("F11", K_F1 + 10),
    ("F12", K_F1 + 11),
    ("INS", K_INS),
    ("DEL", K_DEL),
    ("PGDN", K_PGDN),
    ("PGUP", K_PGUP),
    ("HOME", K_HOME),
    ("END", K_END),
    ("COMMAND", K_COMMAND),
    ("CAPSLOCK", K_CAPSLOCK),
    ("SCROLLLOCK", K_SCROLLLOCK),
    ("NUMLOCK", K_KP_NUMLOCK),
    ("PRINTSCREEN", K_PRINTSCREEN),
    ("MOUSE1", K_MOUSE1),
    ("MOUSE2", K_MOUSE2),
    ("MOUSE3", K_MOUSE3),
    ("MOUSE4", K_MOUSE4),
    ("MOUSE5", K_MOUSE5),
    ("PAUSE", K_PAUSE),
    ("MWHEELUP", K_MWHEELUP),
    ("MWHEELDOWN", K_MWHEELDOWN),
    ("SEMICOLON", b';' as i32),
    ("BACKQUOTE", b'`' as i32),
    ("TILDE", b'~' as i32),
];

/// Key_StringToKeynum; -1 if unknown.
pub fn string_to_keynum(s: &str) -> i32 {
    let b = s.as_bytes();
    match b.len() {
        0 => return -1,
        1 => return b[0].to_ascii_lowercase() as i32,
        _ => {}
    }
    if let Some(&(_, k)) = NAMES.iter().find(|(n, _)| n.eq_ignore_ascii_case(s)) {
        return k;
    }
    for (i, n) in PAD_NAMES.iter().enumerate() {
        let k = K_LTHUMB + i as i32;
        if n.eq_ignore_ascii_case(s) {
            return k;
        }
        if let Some(base) = s.get(..s.len().saturating_sub(4))
            && s.len() > 4
            && s[s.len() - 4..].eq_ignore_ascii_case("_ALT")
            && n.eq_ignore_ascii_case(base)
        {
            return k + K_ALT_OFFSET;
        }
    }
    -1
}

/// Key_KeynumToString.
pub fn keynum_to_string(k: i32) -> String {
    if k == -1 {
        return "<KEY NOT FOUND>".into();
    }
    if k > 32 && k < 127 && k != b';' as i32 && k != b'`' as i32 && k != b'~' as i32 && k != b'"' as i32 {
        return (k as u8 as char).to_string();
    }
    if let Some(&(n, _)) = NAMES.iter().find(|(_, v)| *v == k) {
        return n.into();
    }
    let p = k - K_LTHUMB;
    if (0..20).contains(&p) {
        return PAD_NAMES[p as usize].into();
    }
    if (20..40).contains(&p) {
        return format!("{}_ALT", PAD_NAMES[(p - 20) as usize]);
    }
    "<UNKNOWN KEYNUM>".into()
}

/// Where key events go.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeyDest {
    #[default]
    Game,
    Console,
    Menu,
    Message,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_roundtrip() {
        for k in 0..MAX_KEYS as i32 {
            let s = keynum_to_string(k);
            if !s.starts_with('<') && !(k as u8).is_ascii_uppercase() {
                assert_eq!(string_to_keynum(&s), k, "{s}");
            }
        }
        assert_eq!(string_to_keynum("W"), b'w' as i32);
        assert_eq!(string_to_keynum("mouse1"), K_MOUSE1);
        assert_eq!(string_to_keynum("ABUTTON"), K_ABUTTON);
        assert_eq!(string_to_keynum("rtrigger_alt"), K_RTRIGGER + K_ALT_OFFSET);
        assert_eq!(K_TOUCHPAD + K_ALT_OFFSET + 1, K_GAMEPAD_END);
        assert_eq!(string_to_keynum("NOPE"), -1);
        assert_eq!(keynum_to_string(b';' as i32), "SEMICOLON");
    }
}
