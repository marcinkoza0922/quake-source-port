//! Where the HUD editor's layout comes from and goes: `hud_layout`, or its
//! changes for a combination of add-on elements (`hud_addon_layouts`).

use qcore::cvar::Cvars;

use super::addons::{self, Combo, Patches};
use super::editor::{CVAR, Editor, PRESET};
use super::layout::Layout;
use super::presets::{self, PRESET_NAME, SavedPreset};

/// `hud_layout` itself.
pub(super) fn base_layout(cv: &Cvars) -> Layout {
    Layout::parse(cv.by_name(CVAR).map_or("", |c| c.string.as_str()))
}

impl Editor {
    /// The layout being edited.
    pub(super) fn layout(&self, cv: &Cvars) -> Layout {
        addons::compose(&base_layout(cv), &Patches::of(cv), self.combo)
    }

    /// Stores the layout being edited.
    pub fn write(&self, cv: &mut Cvars, l: &Layout) {
        if self.combo.is_empty() {
            cv.set(CVAR, &l.to_text());
        } else {
            let mut p = Patches::of(cv);
            addons::store(&base_layout(cv), &mut p, self.combo, l);
            cv.set(addons::CVAR, &p.to_text());
        }
    }

    /// Writes the edited layout; it is now an unsaved custom layout.
    pub(super) fn save(&self, cv: &mut Cvars, l: &Layout) {
        self.write(cv, l);
        cv.set(PRESET, "2");
        cv.set(PRESET_NAME, "");
    }

    /// Starts editing the HUD as it is while `combo`'s elements are in use.
    pub fn open_combo(&mut self, cv: &Cvars, combo: Combo) {
        self.open(cv);
        self.combo = combo;
    }

    /// Saves the HUD, with every combination of add-ons, as the preset `name`.
    pub(super) fn store_preset(&mut self, name: &str, cv: &mut Cvars) -> Option<SavedPreset> {
        let text = base_layout(cv).to_text();
        let idx = presets::upsert(&mut self.presets, name, &text, &addons::Patches::of(cv).to_text())?;
        let saved = self.presets[idx].clone();
        self.persist_saved_preset(&saved);
        saved.load(cv);
        Some(saved)
    }
}
