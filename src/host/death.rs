//! Single-player death recovery; multiplayer and demos retain their own rules.
use super::*;

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("_death_choice", |h, a| {
        if h.menu.screen == Screen::Death {
            h.death_choice(a.argv(1).parse().unwrap_or(0));
        }
    });
}

impl Host {
    pub(super) fn update_death(&mut self) {
        let eligible = !self.dedicated && self.demo.is_none() && self.local.is_some()
            && self.server.active() && self.server.core.maxclients() == 1
            && self.cv("deathmatch") == 0.0 && self.cv("coop") == 0.0
            && self.cl.signon == 4 && self.cl.intermission == 0;
        if !eligible || self.cl.stats[STAT_HEALTH] > 0 {
            self.death_handled = false;
            return;
        }
        if self.death_handled { return; }
        self.death_handled = true;
        self.pend_attack = false;
        self.pend_attack2 = false;
        self.pend_jump = false;
        self.pend_impulse = 0;
        self.con.open = false;
        self.menu.open(Screen::Death);
        self.death_choice(self.cv("cl_deathaction") as u32);
    }

    fn death_choice(&mut self, choice: u32) {
        match choice {
            1 => {
                if let Some(name) = latest_save(&self.user_dir()) {
                    self.menu.open(Screen::None);
                    self.cbuf.add_text(&format!("load \"{name}\"\n"));
                } else {
                    self.print("No readable save available. Select save or restart the level.\n");
                }
            }
            2 => {
                self.menu.autosaves = false;
                self.menu.open(Screen::Load);
                self.scan_saves();
            }
            3 => {
                self.menu.open(Screen::None);
                self.cbuf.add_text("restart\n");
            }
            4 => {
                self.cbuf.add_text("disconnect\n");
                self.menu.open(Screen::Main);
            }
            _ => {}
        }
    }
}

/// Include named/quicksaves and every autosave in the active game's user dir.
fn latest_save(dir: &std::path::Path) -> Option<String> {
    std::fs::read_dir(dir).ok()?.filter_map(Result::ok).filter_map(|entry| {
        let path = entry.path();
        if path.extension()?.to_str()? != "sav" { return None; }
        let name = path.file_stem()?.to_str()?.to_owned();
        if name.chars().any(|c| c.is_control() || matches!(c, '"' | ';' | '\\')) { return None; }
        let modified = entry.metadata().ok()?.modified().ok()?;
        let bytes = std::fs::read(&path).ok()?;
        let info = crate::saveinfo::SaveInfo::parse(&qcore::text::decode(&bytes), Some(modified))?;
        (info.health > 0).then_some((modified, name))
    }).max().map(|(_, name)| name)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newest_save_includes_named_and_autosaves_and_ignores_invalid() {
        let dir = std::env::temp_dir().join(format!("qsp-death-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let text = format!("5\nsave\n{}1\ne1m1\n12\n{}{{}}\n{{}}\n{{\"health\" \"100\"}}\n", "0\n".repeat(16), "m\n".repeat(64));
        for (name, seconds) in [("autosave0", 10), ("s0", 20), ("quick", 30)] {
            let path = dir.join(format!("{name}.sav"));
            std::fs::write(&path, &text).unwrap();
            std::fs::File::options().write(true).open(path).unwrap().set_times(std::fs::FileTimes::new().set_modified(std::time::UNIX_EPOCH + std::time::Duration::from_secs(seconds))).unwrap();
        }
        std::fs::write(dir.join("broken.sav"), "bad").unwrap();
        std::fs::write(dir.join("dead.sav"), text.replace("100", "0")).unwrap();
        assert_eq!(latest_save(&dir).as_deref(), Some("quick"));
        std::fs::remove_file(dir.join("quick.sav")).unwrap();
        assert_eq!(latest_save(&dir).as_deref(), Some("s0"));
        std::fs::remove_file(dir.join("s0.sav")).unwrap();
        assert_eq!(latest_save(&dir).as_deref(), Some("autosave0"));
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(latest_save(&dir), None);
    }
}
