//! Live scenes of the lean plan (`docs/perf/PLAN.md`, Feature scenes): a
//! map started with `-fixedstep`, the player in god mode and notarget, a
//! fixed `freecam` camera (optionally dipping to a second camera now and
//! then), and the engine's own frame profiler (`prof_start` / `prof_stop`,
//! `src/host/prof.rs`) around a fixed measurement window.
//!
//! Times are `delay` seconds, which count host realtime; with `-fixedstep`
//! that advances one step per frame, so every run does the same game work
//! whatever its frame rate.

/// Simulation rate of live runs (`-fixedstep`).
pub const STEP_HZ: u32 = 250;
/// When the camera is placed (seconds after the map command).
pub const CAMERA_AT: f32 = 2.5;
/// Measurement start: load, spawn and the first pipeline compiles are over.
pub const MEASURE_AT: f32 = 5.0;
/// Measurement length (PLAN.md: 10–20 s).
pub const MEASURE_SECS: f32 = 15.0;
/// Console line that ends a run.
pub const DONE: &str = "perfkit-live-done";

/// A `freecam x y z pitch yaw` camera.
pub type Cam = [f32; 5];

/// A console command at `at` seconds after the map command, repeated every
/// `every` seconds until the measurement ends when `every` > 0.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Cmd {
    pub at: f32,
    pub cmd: &'static str,
    pub every: f32,
}

pub const fn at(at: f32, cmd: &'static str) -> Cmd {
    Cmd { at, cmd, every: 0.0 }
}

pub const fn every(at: f32, every: f32, cmd: &'static str) -> Cmd {
    Cmd { at, cmd, every }
}

/// Where a live scene plays. The off and on runs share it; the pins decide
/// whether the feature works (a toy or `goggles` command does nothing while
/// its add-on is off, and portals need a customized game).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Live {
    pub game: &'static str,
    pub map: &'static str,
    pub cam: Cam,
    /// Place the player there with `setpos` (so toys aim where it looks)
    /// instead of a `freecam` camera.
    pub player: bool,
    /// `(camera, period, hold)`: switch to `camera` for `hold` seconds every
    /// `period` seconds (dipping under water and surfacing).
    pub alt: Option<(Cam, f32, f32)>,
    /// Commands after the camera: toys, fire, portals, goggles.
    pub setup: &'static [Cmd],
    /// Local players: more than 1 starts split-screen co-op.
    pub players: u8,
    /// Skill 3 and no notarget: monsters fight the (god mode) players.
    pub fight: bool,
    /// Real time instead of `-fixedstep`: audio scenes, whose cost depends
    /// on wall-clock mixing. Game work then varies with the frame rate.
    pub realtime: bool,
}

impl Live {
    /// A camera-only scene.
    pub const fn cam(game: &'static str, map: &'static str, cam: Cam) -> Live {
        Live { game, map, cam, player: false, alt: None, setup: &[], players: 1, fight: false, realtime: false }
    }

    /// The player placed at `cam`, running `setup`.
    pub const fn player(game: &'static str, map: &'static str, cam: Cam, setup: &'static [Cmd]) -> Live {
        Live { game, map, cam, player: true, alt: None, setup, players: 1, fight: false, realtime: false }
    }

    fn place(&self, c: &Cam) -> String {
        if self.player {
            format!("setpos {} {} {} {} {} 0", c[0], c[1], c[2], c[3], c[4])
        } else {
            format!("freecam {} {} {} {} {}", c[0], c[1], c[2], c[3], c[4])
        }
    }
}

/// The engine argument that ends the profile and writes it to `json`
/// (absolute: the engine runs in the sandbox). Not in [`script`]: a config
/// may not name output files.
pub fn profile_arg(json: &str) -> String {
    // one argument: the engine quotes a separate one that holds spaces
    format!("+delay {} prof_stop {json}", MEASURE_AT + MEASURE_SECS)
}

/// The console lines after the pins: map, setup, camera, profiler, marker.
pub fn script(l: &Live) -> Vec<String> {
    let end = MEASURE_AT + MEASURE_SECS;
    let mut v = vec!["host_maxfps 10000".to_string(), "deathmatch 0".into()];
    if l.players > 1 {
        v.extend(["coop 1".to_string(), format!("maxplayers {}", l.players), format!("cl_splitscreen {}", l.players)]);
    } else {
        v.push("coop 0".into());
    }
    if l.fight {
        v.push("skill 3".into());
    }
    v.push(format!("map {}", l.map));
    v.push("delay 2 god".into());
    if !l.fight {
        v.push("delay 2 notarget".into());
    }
    v.push(format!("delay {CAMERA_AT} {}", l.place(&l.cam)));
    if let Some((alt, period, hold)) = l.alt {
        let mut t = CAMERA_AT + 0.5;
        while t < end {
            v.push(format!("delay {t:.2} {}", l.place(&alt)));
            v.push(format!("delay {:.2} {}", t + hold, l.place(&l.cam)));
            t += period;
        }
    }
    for c in l.setup {
        let mut t = c.at;
        while t < end {
            v.push(format!("delay {t:.2} {}", c.cmd));
            if c.every <= 0.0 {
                break;
            }
            t += c.every;
        }
    }
    v.push(format!("delay {MEASURE_AT} prof_start"));
    v.push(format!("delay {} echo {DONE}", end + 0.2));
    v
}

/// What `prof_stop` wrote, as far as the report needs it.
#[derive(Clone, Debug, Default)]
pub struct Profile {
    pub frames: u32,
    pub wall_secs: f64,
    /// Host frame time per frame, µs (all phases, fence wait included).
    pub frame_us: Vec<f32>,
    /// Average GPU time per frame over all stages, µs.
    pub gpu_us: Option<f64>,
}

impl Profile {
    pub fn fps(&self) -> f64 {
        if self.wall_secs > 0.0 { self.frames as f64 / self.wall_secs } else { 0.0 }
    }
}

/// The number after `"key": ` in `s`.
fn num_after(s: &str, key: &str) -> Option<f64> {
    let i = s.find(&format!("\"{key}\": "))? + key.len() + 4;
    let t = &s[i..];
    let end = t.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == 'e')).unwrap_or(t.len());
    t[..end].parse().ok()
}

pub fn parse_profile(json: &str) -> Result<Profile, String> {
    let frames = num_after(json, "frames").ok_or("profile without frames")? as u32;
    let wall_secs = num_after(json, "wall_secs").ok_or("profile without wall_secs")?;
    let raw = json.find("\"frame_us_raw\": [").ok_or("profile without frame_us_raw")?;
    let body = &json[raw + 17..];
    let body = &body[..body.find(']').ok_or("unterminated frame_us_raw")?];
    let frame_us = body.split(',').filter(|x| !x.trim().is_empty()).map(|x| x.trim().parse::<f32>().map_err(|e| e.to_string())).collect::<Result<_, _>>()?;
    let gpu_us = json.find("\"gpu_us\": {").and_then(|i| num_after(&json[i..], "total"));
    Ok(Profile { frames, wall_secs, frame_us, gpu_us })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_places_camera_dips_and_measures() {
        let l = Live { alt: Some(([1.0, 2.0, -3.0, 0.0, 90.0], 3.0, 0.5)), ..Live::cam("id1", "e2m2", [1.0, 2.0, 3.0, 0.0, 90.0]) };
        let s = script(&l);
        assert_eq!(s[3], "map e2m2");
        assert!(s.contains(&"delay 2 notarget".to_string()));
        assert!(s.contains(&"delay 2.5 freecam 1 2 3 0 90".to_string()), "{s:?}");
        assert!(s.contains(&"delay 3.00 freecam 1 2 -3 0 90".to_string()) && s.contains(&"delay 3.50 freecam 1 2 3 0 90".to_string()));
        // Dips keep going through the whole measurement.
        assert!(s.iter().any(|l| l.starts_with("delay 18.00 freecam 1 2 -3")));
        assert_eq!(s[s.len() - 2..], ["delay 5 prof_start", "delay 20.2 echo perfkit-live-done"]);
        assert_eq!(profile_arg("/x/p.json"), "+delay 20 prof_stop /x/p.json");
        let plain = script(&Live { alt: None, ..l });
        assert_eq!(plain.iter().filter(|l| l.contains("freecam")).count(), 1);
        const SETUP: &[Cmd] = &[at(3.0, "toy 5"), every(3.2, 2.0, "+attack")];
        let p = script(&Live::player("id1", "e1m1", [4.0, 5.0, 6.0, 25.0, 90.0], SETUP));
        assert!(p.contains(&"delay 2.5 setpos 4 5 6 25 90 0".to_string()) && !p.iter().any(|l| l.contains("freecam")), "{p:?}");
        assert!(p.contains(&"delay 3.00 toy 5".to_string()));
        assert_eq!(p.iter().filter(|l| l.ends_with("+attack")).count(), 9, "3.2, 5.2, ... 19.2");
        let coop = script(&Live { players: 4, fight: true, ..Live::player("ad", "ad_sepulcher", [0.0; 5], &[]) });
        assert_eq!(coop[..7], ["host_maxfps 10000", "deathmatch 0", "coop 1", "maxplayers 4", "cl_splitscreen 4", "skill 3", "map ad_sepulcher"]);
        assert!(!coop.iter().any(|l| l.contains("notarget")));
    }

    #[test]
    fn reads_the_engines_profile() {
        let j = "{\n  \"frames\": 4,\n  \"mono_start\": 1.0,\n  \"mono_end\": 2.0,\n  \"wall_secs\": 0.008,\n  \"cpu_fps\": 600.0,\n  \
                 \"frame_us\": {\"mean\": 2.00},\n  \"gpu_us\": {\"frames\": 4, \"total\": 812.50, \"world\": 800.00},\n  \
                 \"gpu_views_us\": [],\n  \"frame_us_raw\": [1500,2000,2500,2000]\n}\n";
        let p = parse_profile(j).unwrap();
        assert_eq!((p.frames, p.frame_us.len(), p.gpu_us), (4, 4, Some(812.5)));
        assert_eq!(p.fps(), 500.0);
        let no_gpu = j.replace("\"gpu_us\": {\"frames\": 4, \"total\": 812.50, \"world\": 800.00}", "\"gpu_us\": null");
        assert_eq!(parse_profile(&no_gpu).unwrap().gpu_us, None);
        assert!(parse_profile("{}").is_err());
    }
}
