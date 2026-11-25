//! The next new game's add-ons and modifiers, kept apart from the running
//! game's. Both live in the same cvars (`qserver::addons::settings::per_game`),
//! which belong to the running game and its saves, never to the config: while
//! the launch pop-up and its Customize pages are up, the cvars hold the new
//! game's choices and the running game's values wait here.
use super::*;

type Values = Vec<(&'static str, String)>;

#[derive(Default)]
pub(super) struct Setup {
    /// The running game's values, while the pop-up is up.
    running: Option<Values>,
    /// The pop-up's choices when it was last up (this session); None: defaults.
    next: Option<Values>,
    /// Play was chosen: the new game is about to start.
    playing: bool,
    spawns: u64,
}

fn values(cv: &Cvars) -> Values {
    qserver::addons::settings::per_game().filter_map(|n| Some((n, cv.by_name(n)?.string.clone()))).collect()
}

fn defaults(cv: &Cvars) -> Values {
    qserver::addons::settings::per_game().filter_map(|n| Some((n, cv.by_name(n)?.default.clone()))).collect()
}

fn apply(cv: &mut Cvars, v: &Values) {
    for (n, s) in v {
        cv.set(n, s);
    }
}

impl Menu {
    /// The cvars hold the next new game's choices, not the running game's.
    pub fn setup_active(&self) -> bool {
        self.setup.running.is_some()
    }

    /// Play was chosen and the new game hasn't started yet.
    pub fn setup_playing(&self) -> bool {
        self.setup_active() && self.setup.playing
    }

    pub(super) fn begin_setup(&mut self, cv: &mut Cvars) {
        self.setup.playing = false;
        if self.setup.running.is_none() {
            self.setup.running = Some(values(cv));
            let next = self.setup.next.take().unwrap_or_else(|| defaults(cv));
            apply(cv, &next);
        }
    }

    pub(super) fn play_setup(&mut self) {
        self.setup.playing = true;
    }

    /// Gives the running game its values back (the pop-up was left, or the
    /// game is about to be saved or replaced by a load).
    pub fn end_setup(&mut self, cv: &mut Cvars) {
        if let Some(run) = self.setup.running.take() {
            self.setup.next = Some(values(cv));
            self.setup.playing = false;
            apply(cv, &run);
            self.launch = None;
            self.new_game_setup = false;
            if matches!(self.prompt, Some(Prompt::Launch { .. })) {
                self.prompt = None;
            }
        }
    }

    /// Once per frame, after commands ran. `spawns` counts level spawns;
    /// `idle`: no command is still queued.
    pub fn sync_setup(&mut self, cv: &mut Cvars, spawns: u64, idle: bool) {
        if !self.setup_active() {
            return;
        }
        if self.setup.playing && spawns != self.setup.spawns {
            // the new game runs with them now
            self.setup.next = Some(values(cv));
            self.setup.running = None;
            self.setup.playing = false;
        } else if idle && self.launch.is_none() && (self.prompt.is_none() || !self.setup.playing) {
            self.end_setup(cv);
        }
        self.setup.spawns = spawns;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cvars() -> Cvars {
        let mut cv = Cvars::default();
        for &(n, d, f) in qserver::addons::CVARS.iter().chain(qserver::modifiers::CVARS) {
            cv.register(n, d, f);
        }
        cv
    }

    #[test]
    fn the_running_game_keeps_its_choices() {
        let mut cv = cvars();
        cv.set("addon_portalgun", "1"); // a loaded game's
        cv.set("modifier_horde", "2");
        let mut m = Menu::new();
        m.open_launch(launch::NEW_GAME_CMD.into(), false, None, &mut cv);
        assert_eq!((cv.value_of("addon_portalgun"), cv.value_of("modifier_horde")), (0.0, 0.0), "a new game starts from defaults");
        cv.set("addon_railgun", "1");
        m.sync_setup(&mut cv, 1, true);
        assert!(m.setup_active());
        // cancelled: as it was, and the choice comes back with the pop-up
        m.key(K_ESCAPE, &mut cv, true);
        m.sync_setup(&mut cv, 1, true);
        assert!(!m.setup_active());
        assert_eq!((cv.value_of("addon_portalgun"), cv.value_of("addon_railgun"), cv.value_of("modifier_horde")), (1.0, 0.0, 2.0));
        m.open_launch(launch::NEW_GAME_CMD.into(), false, None, &mut cv);
        assert_eq!((cv.value_of("addon_portalgun"), cv.value_of("addon_railgun")), (0.0, 1.0));
        // confirmed over a running game: cancelling that keeps it too
        m.game_running = true;
        m.key(K_ENTER, &mut cv, true);
        assert!(m.prompt.is_some() && m.setup_playing());
        m.sync_setup(&mut cv, 1, true);
        assert!(m.setup_active(), "the question is still open");
        m.key(K_ESCAPE, &mut cv, true);
        m.sync_setup(&mut cv, 1, true);
        assert_eq!((cv.value_of("addon_portalgun"), cv.value_of("addon_railgun")), (1.0, 0.0));
    }

    #[test]
    fn a_new_game_starts_with_the_pop_ups_choices() {
        let mut cv = cvars();
        cv.set("addon_portalgun", "1");
        let mut m = Menu::new();
        m.open_launch(launch::NEW_GAME_CMD.into(), false, None, &mut cv);
        cv.set("addon_railgun", "1");
        m.sync_setup(&mut cv, 1, true);
        m.key(K_ENTER, &mut cv, false);
        // the map command is still queued (a `wait`): not yet
        m.sync_setup(&mut cv, 1, false);
        assert!(m.setup_playing());
        m.sync_setup(&mut cv, 2, true);
        assert!(!m.setup_active());
        assert_eq!((cv.value_of("addon_portalgun"), cv.value_of("addon_railgun")), (0.0, 1.0));
        assert_eq!(cv.value_of("custom_enable"), 1.0);
        // a command that started nothing gives the old game back
        cv.set("addon_portalgun", "1");
        m.open_launch(launch::NEW_GAME_CMD.into(), false, None, &mut cv);
        m.sync_setup(&mut cv, 2, true);
        m.key(K_ENTER, &mut cv, false);
        m.sync_setup(&mut cv, 2, true);
        assert_eq!(cv.value_of("addon_portalgun"), 1.0);
    }

    #[test]
    fn choices_are_not_in_the_config() {
        let cv = cvars();
        let lines = cv.archive_lines();
        for n in qserver::addons::settings::per_game().chain(["custom_enable"]) {
            assert!(!lines.iter().any(|l| l.starts_with(&format!("{n} "))), "{n}");
        }
        assert!(lines.iter().any(|l| l.starts_with("addon_portalgun_slot ")), "preferences are");
    }
}
