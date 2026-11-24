//! Stub: filled in by its feature branch.
use crate::server::{Core, Level};

/// Per-level state.
#[derive(Clone, Debug, Default)]
pub struct Rifle {}

impl Rifle {
    /// `.qsa` header lines.
    pub fn save_text(&self) -> String {
        String::new()
    }
    /// Reads this toy's lines of a save header (others are skipped).
    pub fn load_text(&mut self, _text: &str) {}
    /// Add-ons went off.
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }
}

/// One server frame (after physics), also while the toy is off so it can
/// drop its state.
pub(crate) fn frame(_st: &mut Rifle, _l: &mut Level, _c: &mut Core) {}
