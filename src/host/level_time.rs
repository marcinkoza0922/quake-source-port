//! Level exits and personal bests: the per-mod `qsp_stats.json` record,
//! personal bests per map, skill and category (any%, 100%: every kill and
//! secret), the HUD Time & Stats items (`sbar::time`) and keeping a new
//! PB's auto-demo as a saved demo (`cl_autodemo_pb`).
//!
//! The level time is the client clock, as on the intermission screen: it
//! stops while the game is paused and at the exit, and a loaded save goes
//! on from the time it was saved at. Runs that load a save still count.

use super::*;
use crate::sbar::time;
use qui::stats::{MapStats, SKILLS, Stats};

#[derive(Default)]
pub(super) struct LevelPb {
    /// The level's skill when it is a local game (not a demo or a remote
    /// server): only then are personal bests shown and recorded.
    skill: Option<usize>,
    /// This map's record, loaded on first use each level.
    record: Option<Option<MapStats>>,
}

/// `cl_autodemo_pb` bits: keep any% and/or 100% PB demos.
const PB_DEMO_ANY: u32 = 1;
const PB_DEMO_FULL: u32 = 2;

const SKILL_NAMES: [&str; SKILLS] = ["easy", "normal", "hard", "nightmare"];

impl Host {
    /// A new level (serverinfo): forget the last map's record.
    pub(super) fn pb_new_map(&mut self) {
        let local = self.local.is_some() && self.demo.is_none();
        self.pb.skill = local.then(|| self.cv("skill").clamp(0.0, 3.0) as usize);
        self.pb.record = None;
    }

    /// Personal bests count only in a local, uncustomized, non-deathmatch game.
    fn pb_allowed(&self) -> bool {
        self.pb.skill.is_some() && !self.server.core.custom && self.cv("deathmatch") == 0.0 && !self.level.mapname.is_empty()
    }

    fn map_record(&mut self) -> Option<MapStats> {
        if self.pb.record.is_none() {
            let rec = Stats::load(&self.user_dir()).get(&self.level.mapname).copied();
            self.pb.record = Some(rec);
        }
        self.pb.record.flatten()
    }

    /// The HUD Time & Stats items this frame (empty when all are off).
    pub(super) fn time_hud_items(&mut self) -> Vec<time::Item> {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        let cfg = time::TimeConfig::from_cvars(&self.server.core.cvars);
        if cfg.clock == 0 && cfg.timer == 0 && cfg.kills == 0 && cfg.secrets == 0 {
            return Vec::new();
        }
        let pb = match (cfg.timer > 0 && cfg.pb && self.pb_allowed(), self.pb.skill) {
            (true, Some(skill)) => Some(self.map_record().and_then(|r| r.pb(skill, cfg.pb_full))),
            _ => None,
        };
        let secs = qcore::time::SystemTime::now().duration_since(qcore::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let clock = match qplatform::local_time(secs as i64) {
            Some((.., h, m)) => (h as u32, m as u32, (secs % 60) as u32),
            None => ((secs / 3600 % 24) as u32, (secs / 60 % 60) as u32, (secs % 60) as u32),
        };
        let st = &self.cl.stats;
        let info = time::LevelInfo {
            time: if self.cl.intermission != 0 { self.cl.completed_time } else { self.cl.time },
            pb,
            kills: (st[STAT_MONSTERS], st[STAT_TOTALMONSTERS]),
            secrets: (st[STAT_SECRETS], st[STAT_TOTALSECRETS]),
            deathmatch: self.cl.gametype == 1,
            clock,
        };
        time::items(&cfg, &info)
    }

    /// Level exit (intermission or finale): merge kills/secrets/time and
    /// the personal bests into the mod's qsp_stats.json. Not for
    /// deathmatch, customized games (modifiers and add-ons change the run)
    /// or headless tool runs.
    pub(super) fn record_level_stats(&mut self) {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        if self.headless || self.demo.is_some() || self.cv("deathmatch") != 0.0 || self.level.mapname.is_empty()
            || self.server.core.custom {
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
        if let Some(k) = skill {
            run.pb_any[k] = t;
            if full {
                run.pb_full[k] = t;
            }
        }
        let dir = self.user_dir();
        let mut stats = Stats::load(&dir);
        let old = stats.get(&self.level.mapname).copied().unwrap_or_default();
        stats.record(&self.level.mapname, run);
        if let Err(e) = stats.save(&dir) {
            self.con.print(&qcore::ctr!("couldn't write {}: {}\n", qui::stats::FILE_NAME, e));
        }
        self.pb.record = Some(stats.get(&self.level.mapname).copied());
        let Some(k) = skill else { return };
        let beats = |old: Option<f32>| t > 0.0 && old.is_none_or(|o| t < o);
        let new_any = beats(old.pb(k, false));
        let new_full = full && beats(old.pb(k, true));
        let shown = time::format_timer(f64::from(t), 2);
        for (new, cat) in [(new_any, "any%"), (new_full, "100%")] {
            if new {
                self.con.print(&qcore::ctr!("New personal best ({}): {}\n", cat, shown));
            }
        }
        self.keep_pb_demo(k, new_any, new_full, t);
    }

    /// `cl_autodemo_pb`: the level's auto-demo is kept as
    /// `<map>_pb_<any|100|any-100>_<skill>_<time>` when it ends. Old PB demos
    /// stay; a name already taken gets a number.
    fn keep_pb_demo(&mut self, skill: usize, new_any: bool, new_full: bool, time: f32) {
        let mode = self.cv("cl_autodemo_pb").clamp(0.0, 3.0) as u32;
        let any = new_any && mode & PB_DEMO_ANY != 0;
        let full = new_full && mode & PB_DEMO_FULL != 0;
        let cat = match (any, full) {
            (true, true) => "any-100",
            (true, false) => "any",
            (false, true) => "100",
            (false, false) => return,
        };
        let map: String = self.level.mapname.chars().map(|c| if c.is_alphanumeric() || "-_".contains(c) { c } else { '_' }).collect();
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
