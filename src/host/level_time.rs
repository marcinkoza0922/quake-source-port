//! Level exits and personal bests: the per-mod `qsp_stats.json` record,
//! personal bests per map, skill and category, the HUD's Time & Stats elements
//! (`crate::hud`), and saving a new PB's auto-demo (`cl_autodemo_pb`).
//!
//! The level time is the client clock, as on the intermission screen: it
//! stops while the game is paused and at the exit, and a loaded save goes
//! on from the time it was saved at. Runs that load a save still count.

use super::*;
use crate::sbar::time;
use qui::stats::{MapStats, PbCategory, Stats};

#[derive(Default)]
pub(super) struct LevelPb {
    /// The level's skill when it is a local game (not a demo or a remote
    /// server): only then are personal bests shown and recorded.
    skill: Option<usize>,
    /// This map's record, loaded on first use each level.
    record: Option<Option<MapStats>>,
    /// Whether damage history is known for the whole attempt.
    pub(super) damage_history_known: bool,
    /// Restored once at serverinfo after a successful load command.
    pub(super) saved_damage: Option<bool>,
}

impl Host {
    /// A new level (serverinfo): forget the last map's record.
    pub(super) fn pb_new_map(&mut self) {
        let local = self.local.is_some() && self.demo.is_none();
        self.pb.skill = local.then(|| self.cv("skill").clamp(0.0, 3.0) as usize);
        self.pb.record = None;
        let saved_damage = self.pb.saved_damage.take();
        self.pb.damage_history_known = !local || !self.server.core.sv.loadgame || saved_damage.is_some();
        self.cl.took_damage |= saved_damage == Some(true);
    }

    /// Personal bests count only in a local, uncustomized, non-deathmatch game.
    pub(super) fn pb_allowed(&self) -> bool {
        self.pb.skill.is_some() && !self.server.core.custom && self.cv("deathmatch") == 0.0 && !self.level.mapname.is_empty()
    }

    pub(super) fn pb_skill(&self) -> Option<usize> {
        self.pb.skill
    }

    fn map_record(&mut self) -> Option<MapStats> {
        if self.pb.record.is_none() {
            let rec = Stats::load(&self.user_dir()).get(&self.level.mapname).copied();
            self.pb.record = Some(rec);
        }
        self.pb.record.flatten()
    }

    /// What the HUD's Time & Stats elements show this frame; the
    /// personal best is looked up only when `want_pb`.
    pub(super) fn level_info(&mut self, want_pb: bool) -> time::LevelInfo {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        let category = PbCategory::hud_mode(self.cv("hud_pb_100"));
        let pb = match (want_pb && self.pb_allowed(), self.pb.skill) {
            (true, Some(skill)) => Some(self.map_record().and_then(|r| r.pb(skill, category))),
            _ => None,
        };
        let secs = qcore::time::SystemTime::now().duration_since(qcore::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let clock = match qplatform::local_time(secs as i64) {
            Some((.., h, m)) => (h as u32, m as u32, (secs % 60) as u32),
            None => ((secs / 3600 % 24) as u32, (secs / 60 % 60) as u32, (secs % 60) as u32),
        };
        let st = &self.cl.stats;
        time::LevelInfo {
            time: if self.cl.intermission != 0 { self.cl.completed_time } else { self.cl.time },
            pb,
            kills: (st[STAT_MONSTERS], st[STAT_TOTALMONSTERS]),
            secrets: (st[STAT_SECRETS], st[STAT_TOTALSECRETS]),
            speed: 0,
            deathmatch: self.cl.gametype == 1,
            clock,
        }
    }

    /// Level exit (intermission or finale): merge kills/secrets/time and
    /// the personal bests into the mod's qsp_stats.json. Not for
    /// deathmatch, other people's servers, customized games (modifiers and add-ons change the run)
    /// or headless tool runs.
    pub(super) fn record_level_stats(&mut self) {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        if self.headless || self.demo.is_some() || self.cv("deathmatch") != 0.0 || self.level.mapname.is_empty()
            || self.server.core.custom || self.local.is_none() || self.cl.gametype == 1 {
            return;
        }
        let st = |i: usize| self.cl.stats[i].max(0) as u32;
        let t = self.cl.completed_time as f32;
        let mut run = MapStats {
            kills: st(STAT_MONSTERS),
            total_kills: st(STAT_TOTALMONSTERS),
            secrets: st(STAT_SECRETS),
            total_secrets: st(STAT_TOTALSECRETS),
            best_time: t,
            ..Default::default()
        };
        let full = run.all_kills() && run.all_secrets();
        let skill = self.pb.skill.filter(|_| self.pb_allowed());
        let no_hit = self.pb.damage_history_known && !self.cl.took_damage;
        if let Some(k) = skill {
            for category in PbCategory::ALL.into_iter().filter(|cat| cat.qualifies(full, no_hit)) {
                run.set_pb(k, category, t);
            }
        }
        let dir = self.user_dir();
        let mut stats = match Stats::load_to_update(&dir) {
            Ok((s, None)) => s,
            Ok((s, Some(moved))) => {
                self.con.print(&format!("{} is damaged; it was kept as {moved}\n", qui::stats::FILE_NAME));
                s
            }
            Err(e) => {
                self.con.print(&format!("couldn't read {}: {e}\n", qui::stats::FILE_NAME));
                return;
            }
        };
        let old = stats.get(&self.level.mapname).copied().unwrap_or_default();
        stats.record(&self.level.mapname, run);
        if let Err(e) = stats.save(&dir) {
            self.con.print(&qcore::ctr!("couldn't write {}: {}\n", qui::stats::FILE_NAME, e));
        }
        self.pb.record = Some(stats.get(&self.level.mapname).copied());
        let Some(k) = skill else { return };
        let shown = time::format_timer(f64::from(t), 2);
        let map = self.level.mapname.clone();
        let mut new_bests = Vec::new();
        for category in PbCategory::ALL {
            if run.pb(k, category).is_some_and(|t| old.pb(k, category).is_none_or(|o| t < o)) {
                new_bests.push(category);
                let label = if qcore::lang::console() { qcore::lang::tr(category.label()) } else { category.label().into() };
                self.con.print(&qcore::ctr!("New personal best ({}): {}\n", label, shown));
                let dest = crate::demo::ghost::ghost_path(&dir, &map, k, category);
                self.autodemo_copy_current(&dest);
            }
        }
        self.keep_pb_demo(k, &new_bests, t);
    }

    fn keep_pb_demo(&mut self, skill: usize, new_bests: &[PbCategory], time: f32) {
        use crate::demo::ghost::{SKILL_NAMES, sanitize_map};
        let mode = self.cv("cl_autodemo_pb");
        let cat = if mode.round() as i32 == 3 {
            // Value 3 was the old "any% and 100%" setting.
            match (new_bests.contains(&PbCategory::Any), new_bests.contains(&PbCategory::Full)) {
                (true, true) => "any-100",
                (true, false) => "any",
                (false, true) => "100",
                (false, false) => return,
            }
        } else {
            let Some(category) = PbCategory::demo_mode(mode).filter(|cat| new_bests.contains(cat)) else { return };
            category.file_tag()
        };
        let map = sanitize_map(&self.level.mapname);
        let name = format!("{map}_pb_{cat}_{}_{}", SKILL_NAMES[skill], file_time(time));
        self.autodemo_keep_as(name);
    }
}

/// A time for file names, without dots or colons: `1m05s53`.
fn file_time(t: f32) -> String {
    let cs = (f64::from(t.max(0.0)) * 100.0 + 0.1) as u64;
    format!("{}m{:02}s{:02}", cs / 6000, cs / 100 % 60, cs % 100)
}

#[cfg(test)]
mod tests {
    #[test]
    fn pb_demo_file_time() {
        assert_eq!(super::file_time(65.53), "1m05s53");
        assert_eq!(super::file_time(3725.0), "62m05s00");
    }
}
