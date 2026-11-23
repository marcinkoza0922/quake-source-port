//! `language` (re-release text and the port's own) and `con_translate`.

use std::sync::OnceLock;

use super::*;

/// The system's preferred languages, read once (SDL / `navigator.languages`).
fn system_languages() -> &'static [String] {
    static L: OnceLock<Vec<String>> = OnceLock::new();
    L.get_or_init(qplatform::preferred_languages)
}

impl Host {
    /// Applies `language` (`auto` follows the system) and `con_translate`,
    /// reloading the re-release strings when the language changes.
    pub(super) fn sync_language(&mut self) {
        let cvar_gen = self.server.core.cvars.generation();
        if self.lang_cvar_gen == Some(cvar_gen) && self.server.core.loc.lang.is_some() {
            return;
        }
        self.lang_cvar_gen = Some(cvar_gen);
        let lang = qcore::lang::resolve(self.cvs("language"), system_languages());
        qcore::lang::set_console(self.cv("con_translate") != 0.0);
        qcore::lang::set(lang);
        if self.server.core.loc.lang != Some(lang) {
            self.server.core.loc = qserver::loc::Localization::load(&self.server.core.fs);
        }
    }
}
