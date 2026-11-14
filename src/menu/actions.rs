use super::*;

impl Menu {
    /// Commands that apply Player Setup (name and colours).
    pub(super) fn setup_commands(cv: &Cvars) -> String {
        let name = cv.by_name("_cl_name").map_or("player", |c| c.string.as_str());
        let c = cv.value_of("_cl_color") as i32;
        format!("name \"{name}\"\ncolor {} {}\n", c >> 4, c & 15)
    }

    /// `_mp_map`, or a sensible default from the list (a dm map if any).
    pub(super) fn host_map(&self, cv: &Cvars) -> Option<String> {
        let cur = cv.by_name("_mp_map").map_or("", |c| c.string.as_str());
        if self.net_maps.iter().any(|m| m.name == cur) {
            return Some(cur.to_string());
        }
        let m = self.net_maps.iter().find(|m| m.name.starts_with("dm")).or(self.net_maps.first())?;
        Some(m.name.clone())
    }

    pub(super) fn run(&mut self, run: Run, cv: &mut Cvars, out: &mut Vec<Action>) {
        match run {
            Run::ResetShotguns | Run::ResetBulletTime => {
                let names: &[&str] = if run == Run::ResetShotguns { &["addon_shotgun_speed"] } else {
                    &["addon_bullet_time_toggle", "addon_bullet_time_meter", "addon_bullet_time_duration", "addon_bullet_time_regen"]
                };
                for &(name, default, _) in qserver::addons::CVARS {
                    if names.contains(&name) { cv.set(name, default); }
                }
                out.push(Action::Sound("misc/menu3.wav"));
                return;
            }
            Run::PreviewHit | Run::PreviewKill => {
                let cue = if run == Run::PreviewHit { "hit" } else { "kill" };
                out.push(Action::Command(format!("hitfeedback_preview {cue}\n")));
                return;
            }
            Run::Connect => {
                let a = cv.by_name("_mp_address").map_or(String::new(), |c| c.string.trim().to_string());
                if a.is_empty() {
                    return;
                }
                self.screen = Screen::None;
                out.push(Action::Close);
                out.push(Action::Command(format!("connect {a}\n")));
            }
            Run::Search => {
                if !self.searching {
                    out.push(Action::Command("slist\n".into()));
                }
            }
            Run::NewGame => self.start_new_game(out),
            Run::Start => {
                let Some(map) = self.host_map(cv) else { return };
                let n = |c: &str| cv.value_of(c) as i32;
                let coop = n("_mp_coop");
                let local = n("_mp_localplayers").clamp(1, 6);
                let cmd = format!(
                    "disconnect\nmaxplayers {}\ncl_splitscreen {local}\ncoop {coop}\ndeathmatch {}\nteamplay {}\nskill {}\nfraglimit {}\ntimelimit {}\nmap {map}\n",
                    n("_mp_maxplayers").max(2).max(local),
                    (coop == 0) as i32,
                    n("_mp_teamplay"),
                    n("_mp_skill"),
                    n("_mp_fraglimit"),
                    n("_mp_timelimit"),
                );
                cv.set("_mp_map", &map);
                self.screen = Screen::None;
                out.push(Action::Close);
                out.push(Action::Command(cmd));
            }
        }
        out.push(Action::Sound("misc/menu2.wav"));
    }
}
