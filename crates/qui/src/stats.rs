//! Per-mod completion record, `qsp_stats.json` in the mod's user dir:
//! for each map the best kills/secrets seen at a level exit and the best
//! completion time.

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
}

impl MapStats {
    pub fn all_kills(&self) -> bool {
        self.kills >= self.total_kills
    }
    pub fn all_secrets(&self) -> bool {
        self.secrets >= self.total_secrets
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
        if run.best_time > 0.0 && (s.best_time <= 0.0 || run.best_time < s.best_time) {
            s.best_time = run.best_time;
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
            out.maps.push((
                name.to_ascii_lowercase(),
                MapStats {
                    kills: n("kills") as u32,
                    total_kills: n("total_kills") as u32,
                    secrets: n("secrets") as u32,
                    total_secrets: n("total_secrets") as u32,
                    best_time: n("best_time") as f32,
                    completions: n("completions") as u32,
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
                let t = (f64::from(s.best_time) * 100.0).round() / 100.0;
                (
                    m.clone(),
                    Value::Obj(vec![
                        n("kills", s.kills.into()),
                        n("total_kills", s.total_kills.into()),
                        n("secrets", s.secrets.into()),
                        n("total_secrets", s.total_secrets.into()),
                        n("best_time", t),
                        n("completions", s.completions.into()),
                    ]),
                )
            })
            .collect();
        Value::Obj(vec![("version".into(), Value::Num(1.0)), ("maps".into(), Value::Obj(maps))]).to_string_pretty()
    }

    /// Missing or unreadable files give empty stats.
    pub fn load(dir: &Path) -> Stats {
        std::fs::read_to_string(dir.join(FILE_NAME)).map(|t| Stats::parse(&t)).unwrap_or_default()
    }

    /// Writes via a temp file + rename so a crash never truncates it.
    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let tmp = dir.join(format!("{FILE_NAME}.tmp"));
        std::fs::write(&tmp, self.to_json())?;
        std::fs::rename(tmp, dir.join(FILE_NAME))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_keeps_best_and_round_trips() {
        let mut s = Stats::default();
        let run = |k, s, t| MapStats { kills: k, total_kills: 10, secrets: s, total_secrets: 3, best_time: t, completions: 0 };
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
    fn save_and_load() {
        let dir = std::env::temp_dir().join(format!("qui-stats-{}", std::process::id()));
        let mut s = Stats::default();
        s.record("e1m2", MapStats { kills: 1, total_kills: 2, ..Default::default() });
        s.save(&dir).unwrap();
        assert_eq!(Stats::load(&dir), s);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
