//! `bugreport "text"`: a development aid that snapshots everything a
//! developer (or agent) needs to reproduce a problem into a new directory
//! under `bugreport_dir` (default `<user dir>/bugreports`): `report.json`,
//! the savegame, the config as it was, and a screenshot without the console.

use super::*;
use qnet::protocol::{STAT_ARMOR, STAT_CELLS, STAT_MONSTERS, STAT_NAILS, STAT_ROCKETS, STAT_SECRETS, STAT_SHELLS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
use std::fmt::Write as _;

/// Quotes `s` as a JSON string.
fn js(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn vec3(v: [f32; 3]) -> String {
    format!("[{:.3}, {:.3}, {:.3}]", v[0], v[1], v[2])
}

/// (`YYYYMMDD-HHMMSS`, ISO 8601) in UTC for a unix time.
fn utc_stamps(secs: u64) -> (String, String) {
    let (days, rem) = ((secs / 86400) as i64, secs % 86400);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    let (hh, mm, ss) = (rem / 3600, rem % 3600 / 60, rem % 60);
    (format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}"), format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"))
}

impl Host {
    /// Where bug reports go: the `bugreport_dir` cvar, else `<user dir>/bugreports`.
    fn bugreport_root(&self) -> PathBuf {
        match self.server.core.cvars.by_name("bugreport_dir").map(|c| c.string.trim().to_string()) {
            Some(s) if !s.is_empty() => PathBuf::from(s),
            _ => self.user_dir().join("bugreports"),
        }
    }

    pub(super) fn bugreport(&mut self, text: &str) {
        let now = qcore::time::SystemTime::now().duration_since(qcore::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let (stamp, iso) = utc_stamps(now);
        let map = if self.level.mapname.is_empty() { "nomap" } else { self.level.mapname.as_str() };
        let root = self.bugreport_root();
        let mut dir = root.join(format!("{stamp}_{map}"));
        for i in 2..1000 {
            if !qcore::vfs::exists(&dir) { break; }
            dir = root.join(format!("{stamp}_{map}_{i}"));
        }
        if let Err(e) = qcore::vfs::create_dir_all(&dir) {
            self.print(&qcore::ctr!("bugreport: can't create {}: {}\n", dir.display(), e));
            return;
        }
        
        // Config as it is right now (bindings + archived cvars), even headless.
        let mut cfg = String::from("// config at the time of the bug report\n");
        for l in self.input.archive_bindings().into_iter().chain(self.server.core.cvars.archive_lines()) {
            cfg += &l;
            cfg.push('\n');
        }
        let _ = qcore::vfs::write(&dir.join("config.cfg"), cfg.as_bytes());
        let in_game = self.server.active() && self.cl.signon == 4;
        let save = if in_game {
            let ext = crate::autosave::ext(self.server.qsa_saves());
            let name = format!("save.{ext}");
            let ok = qcore::vfs::write(&dir.join(&name), &qcore::text::encode(&self.server.save_game())).is_ok();
            ok.then_some(name)
        } else {
            None
        };
        let shot = !self.dedicated;
        let demo = self.autodemo_copy(&dir.join("demo.dem"));
        let json = self.bugreport_json(text, &iso, save.as_deref(), shot, demo);
        let _ = qcore::vfs::write(&dir.join("report.json"), json.as_bytes());
        // Screenshot of the next rendered frame, with no console on top.
        self.screenshot = Some(dir.join("screenshot.png").to_string_lossy().into_owned());
        self.clean_shot = true;
        self.print(&qcore::ctr!("Bug report written to {}\n", dir.display()));
    }

    fn bugreport_json(&self, text: &str, iso: &str, save: Option<&str>, shot: bool, demo: bool) -> String {
        let cl = &self.cl;
        let pe = cl.entities.get(cl.viewentity);
        let origin = pe.map_or([0.0; 3], |e| e.origin);
        let eye = [origin[0], origin[1], origin[2] + cl.viewheight];
        let cv = &self.server.core.cvars;
        let (w, h) = self.r.size();
        let mut o = String::from("{\n");
        let mut f = |k: &str, v: String| { let _ = writeln!(o, "  {}: {},", js(k), v); };
        f("text", js(text));
        f("timestamp_utc", js(iso));
        f("version", js(env!("CARGO_PKG_VERSION")));
        f("map", js(&self.level.mapname));
        f("level_name", js(&qcore::text::plain(&cl.levelname)));
        f("game", js(self.game_name()));
        f("gamedirs", format!("[{}]", self.fs_snapshot.1.iter().map(|g| js(g)).collect::<Vec<_>>().join(", ")));
        f("cmdline", format!("[{}]", self.cmdline.iter().map(|g| js(g)).collect::<Vec<_>>().join(", ")));
        f("viewpos", vec3(eye));
        f("origin", vec3(origin));
        f("viewangles", vec3(cl.viewangles));
        f("velocity", vec3(cl.velocity));
        f("viewheight", format!("{:.2}", cl.viewheight));
        f("setpos_command", js(&format!("setpos {:.2} {:.2} {:.2} {:.2} {:.2} {:.2}", origin[0], origin[1], origin[2], cl.viewangles[0], cl.viewangles[1], cl.viewangles[2])));
        f("on_ground", cl.onground.to_string());
        f("in_water", cl.inwater.to_string());
        f("paused", cl.paused.to_string());
        f("intermission", cl.intermission.to_string());
        f("level_time", format!("{:.3}", cl.time));
        f("server_time", format!("{:.3}", self.server.time()));
        f("skill", format!("{}", cv.value_of("skill")));
        f("deathmatch", format!("{}", cv.value_of("deathmatch")));
        f("coop", format!("{}", cv.value_of("coop")));
        f("maxclients", cl.maxclients.to_string());
        f("custom", self.server.core.custom.to_string());
        f("demo_playback", cl.demoplayback.to_string());
        f("stats", format!("{{\"health\": {}, \"armor\": {}, \"weapon\": {}, \"ammo\": {}, \"shells\": {}, \"nails\": {}, \"rockets\": {}, \"cells\": {}, \"items\": {}, \"monsters\": [{}, {}], \"secrets\": [{}, {}]}}",
            cl.stats[STAT_HEALTH], cl.stats[STAT_ARMOR], cl.stats[STAT_ACTIVEWEAPON], cl.stats[STAT_AMMO], cl.stats[STAT_SHELLS], cl.stats[STAT_NAILS],
            cl.stats[STAT_ROCKETS], cl.stats[STAT_CELLS], cl.items, cl.stats[STAT_MONSTERS], cl.stats[STAT_TOTALMONSTERS], cl.stats[STAT_SECRETS], cl.stats[STAT_TOTALSECRETS]));
        f("visible_entities", cl.visedicts.len().to_string());
        f("fog", format!("{:?}", cl.fog).replace('"', "'").pipe_js());
        f("sky", js(&cl.skybox));
        f("resolution", format!("[{w}, {h}]"));
        f("gpu", js(self.r.device_name()));
        f("headless", self.headless.to_string());
        f("os", js(std::env::consts::OS));
        f("arch", js(std::env::consts::ARCH));
        f("recent_console", format!("[{}]", self.con.lines.iter().rev().take(30).collect::<Vec<_>>().into_iter().rev()
            .map(|l| js(String::from_utf8_lossy(l).trim_end())).collect::<Vec<_>>().join(", ")));
        let changed: Vec<String> = ["fov", "r_enhancedwater", "r_debris", "hud_layout"].iter()
            .filter_map(|n| cv.by_name(n).map(|c| format!("{}: {}", js(n), js(&c.string)))).collect();
        f("key_cvars", format!("{{{}}}", changed.join(", ")));
        f("files", format!("{{\"save\": {}, \"config\": \"config.cfg\", \"screenshot\": {}, \"demo\": {}}}", save.map_or("null".into(), js), if shot { "\"screenshot.png\"" } else { "null" }, if demo { "\"demo.dem\"" } else { "null" }));
        o.truncate(o.trim_end_matches([',', '\n']).len());
        o.push_str("\n}\n");
        o
    }
}

trait PipeJs { fn pipe_js(self) -> String; }
impl PipeJs for String { fn pipe_js(self) -> String { js(&self) } }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_formatting() {
        assert_eq!(utc_stamps(0).1, "1970-01-01T00:00:00Z");
        assert_eq!(utc_stamps(1_700_000_000).0, "20231114-221320");
    }

    #[test]
    fn json_escapes() {
        assert_eq!(js("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
    }
}
