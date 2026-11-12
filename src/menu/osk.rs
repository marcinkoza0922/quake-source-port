use super::*;

/// On-screen keyboard cursor: `row` 0..=4 (4 = action row), `col` 0..10.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Osk {
    pub(super) row: usize,
    pub(super) col: usize,
    pub(super) shift: bool,
}

pub(super) const OSK_ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl.", "zxcvbnm-_:"];
pub(super) const OSK_ACTIONS: [&str; 4] = ["Shift", "Space", "Del", "Done"];
pub(super) const OSK_KEY: (f32, f32) = (20.0, 14.0);
pub(super) const OSK_Y: f32 = 240.0;

impl Osk {
    /// Action under the cursor on the bottom row.
    pub(super) fn action(&self) -> usize {
        self.col * OSK_ACTIONS.len() / 10
    }
}

/// Keys bound to exactly `cmd` (whitespace/case-insensitive).
pub fn keys_for(bindings: &[Option<String>], cmd: &str) -> Vec<i32> {
    bindings
        .iter()
        .enumerate()
        .filter(|(_, b)| b.as_deref().is_some_and(|b| b.trim().eq_ignore_ascii_case(cmd)))
        .map(|(k, _)| k as i32)
        .collect()
}

pub(super) fn bind_cmd(key: i32, cmd: &str) -> String {
    let name = keynum_to_string(key);
    format!("bind \"{name}\" \"{cmd}\"\n")
}
