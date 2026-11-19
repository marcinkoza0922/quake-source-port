//! Per-mod completion record, `qsp_stats.json` in the mod's user dir:
//! for each map the best kills/secrets seen at a level exit and the best
//! completion time, plus personal bests per skill (any% and 100%).

use std::path::Path;

use crate::json::{self, Value};

pub const FILE_NAME: &str = "qsp_stats.json";

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MapStats {
    pub kills: u32,
    pub total_kills: u32,
    pub secrets: u32,
    pub total_secrets: u32,
    /// Best completion time in seconds.
    pub best_time: f32,
    pub completions: u32,
    /// Personal best per skill (0..=3): any% (any exit) and 100% (every
    /// kill and secret). 0 means none yet.
    pub pb_any: [f32; SKILLS],
    pub pb_full: [f32; SKILLS],
}

/// Easy, Normal, Hard, Nightmare.
pub const SKILLS: usize = 4;

/// Keeps the smaller of two times, where 0 (or less) means no time.
fn best(old: f32, new: f32) -> f32 {
    if new > 0.0 && (old <= 0.0 || new < old) { new } else { old }
}

impl MapStats {
    pub fn all_kills(&self) -> bool {
        self.kills >= self.total_kills
    }
    pub fn all_secrets(&self) -> bool {
        self.secrets >= self.total_secrets
    }
    /// The personal best for `skill`, any% or 100%.
    pub fn pb(&self, skill: usize, full: bool) -> Option<f32> {
        let t = if full { self.pb_full } else { self.pb_any }.get(skill).copied().unwrap_or(0.0);
        (t > 0.0).then_some(t)
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stats {
    /// Sorted by map name.
    pub maps: Vec<(String, MapStats)>,
}

impl Stats {
    pub fn get(&self, map: &str) -> Option<&MapStats> {
        self.maps.iter().find(|(m, _)| m.eq_ignore_ascii_case(map)).map(|(_, s)| s)
    }

    /// Merges one level exit into the record (best of each field).
    pub fn record(&mut self, map: &str, run: MapStats) {
        let map = map.to_ascii_lowercase();
        let i = match self.maps.binary_search_by(|(m, _)| m.as_str().cmp(&map)) {
            Ok(i) => i,
            Err(i) => {
                self.maps.insert(i, (map, MapStats::default()));
                i
            }
        };
        let s = &mut self.maps[i].1;
        s.kills = s.kills.max(run.kills);
        s.total_kills = s.total_kills.max(run.total_kills);
        s.secrets = s.secrets.max(run.secrets);
        s.total_secrets = s.total_secrets.max(run.total_secrets);
        s.best_time = best(s.best_time, run.best_time);
        for k in 0..SKILLS {
            s.pb_any[k] = best(s.pb_any[k], run.pb_any[k]);
            s.pb_full[k] = best(s.pb_full[k], run.pb_full[k]);
        }
        s.completions += 1;
    }

    pub fn parse(text: &str) -> Stats {
        let mut out = Stats::default();
        let Some(maps) = json::parse(text).and_then(|v| v.get("maps").and_then(|m| m.as_obj().map(<[_]>::to_vec))) else {
            return out;
        };
        for (name, v) in maps {
            let n = |k: &str| v.get(k).and_then(Value::num).unwrap_or(0.0);
            let times = |k: &str| {
                let mut out = [0.0; SKILLS];
                if let Some(Value::Arr(a)) = v.get(k) {
                    for (o, t) in out.iter_mut().zip(a) {
                        *o = t.num().unwrap_or(0.0) as f32;
                    }
                }
                out
            };
            out.maps.push((
                name.to_ascii_lowercase(),
                MapStats {
                    kills: n("kills") as u32,
                    total_kills: n("total_kills") as u32,
                    secrets: n("secrets") as u32,
                    total_secrets: n("total_secrets") as u32,
                    best_time: n("best_time") as f32,
                    completions: n("completions") as u32,
                    pb_any: times("pb_any"),
                    pb_full: times("pb_100"),
                },
            ));
        }
        out.maps.sort_by(|a, b| a.0.cmp(&b.0));
        out.maps.dedup_by(|a, b| a.0 == b.0);
        out
    }

    pub fn to_json(&self) -> String {
        let maps = self
            .maps
            .iter()
            .map(|(m, s)| {
                let n = |k: &str, v: f64| (k.to_string(), Value::Num(v));
                let round = |t: f32| (f64::from(t) * 100.0).round() / 100.0;
                let t = round(s.best_time);
                let times = |k: &str, a: &[f32; SKILLS]| (k.to_string(), Value::Arr(a.iter().map(|&t| Value::Num(round(t))).collect()));
                (
                    m.clone(),
                    Value::Obj(vec![
                        n("kills", s.kills.into()),
                        n("total_kills", s.total_kills.into()),
                        n("secrets", s.secrets.into()),
                        n("total_secrets", s.total_secrets.into()),
                        n("best_time", t),
                        n("completions", s.completions.into()),
                        times("pb_any", &s.pb_any),
                        times("pb_100", &s.pb_full),
                    ]),
                )
            })
            .collect();
        Value::Obj(vec![("version".into(), Value::Num(1.0)), ("maps".into(), Value::Obj(maps))]).to_string_pretty()
    }

    /// Missing or unreadable files give empty stats.
    pub fn load(dir: &Path) -> Stats {
        qcore::vfs::read_to_string(&dir.join(FILE_NAME)).map(|t| Stats::parse(&t)).unwrap_or_default()
    }

    /// Writes via a temp file + rename so a crash never truncates it.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        qcore::vfs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        qcore::vfs::write(&tmp, self.to_json().as_bytes())?;
        qcore::vfs::rename(&tmp, &dir.join(FILE_NAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keeps_best_and_round_trips() {
        let mut s = Stats::default();
        let run = |k, s, t| MapStats { kills: k, total_kills: 10, secrets: s, total_secrets: 3, best_time: t, ..Default::default() };
        s.record("E1M1", run(4, 2, 120.0));
        s.record("e1m1", run(10, 1, 90.5));
        s.record("start", run(0, 0, 30.0));
        let m = s.get("e1m1").unwrap();
        assert_eq!((m.kills, m.secrets, m.best_time, m.completions), (10, 2, 90.5, 2));
        assert!(m.all_kills() && !m.all_secrets());
        let back = Stats::parse(&s.to_json());
        assert_eq!(back, s);
        assert_eq!(back.maps[0].0, "e1m1");
        assert_eq!(Stats::parse("garbage"), Stats::default());
    }

    #[test]
    fn personal_bests_per_skill() {
        let mut s = Stats::default();
        let run = |skill: usize, t: f32, full: bool| {
            let mut r = MapStats { best_time: t, ..Default::default() };
            r.pb_any[skill] = t;
            if full {
                r.pb_full[skill] = t;
            }
            r
        };
        s.record("e1m1", run(1, 80.0, false));
        s.record("e1m1", run(1, 95.0, true));
        s.record("e1m1", run(3, 120.0, true));
        s.record("e1m1", run(1, 90.0, true));
        let m = s.get("e1m1").unwrap();
        assert_eq!((m.pb(1, false), m.pb(1, true)), (Some(80.0), Some(90.0)));
        assert_eq!((m.pb(3, false), m.pb(3, true), m.pb(0, false)), (Some(120.0), Some(120.0), None));
        assert_eq!(m.pb(9, false), None);
        assert_eq!(Stats::parse(&s.to_json()), s);
        // files written before personal bests still load
        let old = r#"{"version":1,"maps":{"e1m2":{"kills":3,"best_time":61.5}}}"#;
        let m = *Stats::parse(old).get("e1m2").unwrap();
        assert_eq!((m.kills, m.best_time, m.pb(0, false)), (3, 61.5, None));
    }

    #[test]
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("qui-stats-{}", std::process::id()));
        let mut s = Stats::default();
        s.record("e1m2", MapStats { kills: 1, total_kills: 2, ..Default::default() });
        s.save(&dir).unwrap();
        assert_eq!(Stats::load(&dir), s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
