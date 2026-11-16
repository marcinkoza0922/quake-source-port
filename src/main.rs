//! quake-source-port: host binary wiring the crates together (see docs/DESIGN.md).
//!
//! Options: -basedir <dir>, -game <dir>, -hipnotic / -rogue / -quoth,
//! -width <w>, -height <h>, -fullscreen, -window, -headless (offscreen
//! renderer, simulated time, no audio), -autoquit <seconds>, and `+cmd args`
//! console commands.
//! `-fixedstep <Hz>` runs uncapped with a fixed simulation step for profiling,
//! including the windowed path; it cannot be combined with real-time pacing.

mod addons;
mod assets;
mod autosave;
mod captions;
mod console;
mod debris;
mod blood;
mod hitfx;
mod impacts;
mod tracers;
mod bboxes;
mod demo;
mod draw;
mod font;
mod fte;
mod glyphs;
mod host;
mod menu;
mod reticle;
mod modmenu;
mod net;
mod platform;
mod png;
mod quaddicted;
mod qw;
mod sbar;
mod saveinfo;
mod splitscreen;
mod save_inventory;
mod view;
mod wheel;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use qcore::fs::Fs;
use qsound::SoundSystem;

struct Opts {
    args: Vec<String>,
}

impl Opts {
    fn fixed_step(&self) -> Result<Option<f64>, &'static str> {
        if !self.has("-fixedstep") { return Ok(None); }
        if self.has("-realtime") || self.has("-dedicated") {
            return Err("-fixedstep cannot be combined with -realtime or -dedicated");
        }
        let hz = self.val("-fixedstep").and_then(|v| v.parse::<f64>().ok())
            .filter(|v| v.is_finite() && (1.0..=1000.0).contains(v))
            .ok_or("-fixedstep needs a frequency from 1 to 1000 Hz")?;
        Ok(Some(1.0 / hz))
    }

    fn has(&self, f: &str) -> bool {
        self.args.iter().any(|a| a.eq_ignore_ascii_case(f))
    }
    fn val(&self, f: &str) -> Option<&str> {
        let i = self.args.iter().position(|a| a.eq_ignore_ascii_case(f))?;
        self.args.get(i + 1).map(String::as_str)
    }
}

/// `$XDG_DATA_HOME/quake-source-port/<game>` (default `~/.local/share/...`).
pub fn user_dir(game: &str) -> PathBuf {
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("quake-source-port").join(game.to_ascii_lowercase())
}

fn main() {
    let o = Opts { args: std::env::args().skip(1).collect() };
    let fixed_step = o.fixed_step().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let basedir = o
        .val("-basedir")
        .map(PathBuf::from)
        .unwrap_or_else(Fs::default_basedir);
    let mut fs = Fs::new(&basedir);
    if !fs.add_game_dir("id1") {
        let msg = format!("Couldn't find Quake game data in {}.\n\nCopy this port next to your Quake folder (the one containing id1/pak0.pak), or start it with -basedir DIR or the QUAKE_BASEDIR environment variable.", basedir.display());
        eprintln!("{msg}");
        // launched by double-click there is no terminal: show the error too
        if !o.has("-headless") && !o.has("-dedicated") {
            qplatform::error_box("Quake data not found", &msg);
        }
        std::process::exit(1);
    }
    for (flag, dir) in [("-hipnotic", "hipnotic"), ("-rogue", "rogue"), ("-quoth", "quoth")] {
        if o.has(flag) {
            fs.add_game_dir(dir);
        }
    }
    if let Some(g) = o.val("-game")
        && !g.eq_ignore_ascii_case("id1")
        && !fs.add_game_dir(g)
    {
        eprintln!("game dir {g} not found");
    }
    // User config/saves live outside the game data and override it.
    fs.add_user_dir(user_dir(fs.game_dir().unwrap_or("id1")));
    // -dedicated [n]: no window, no local client, real-time pacing.
    let dedicated = o.has("-dedicated");
    let realtime = dedicated || o.has("-realtime");
    let headless = o.has("-headless") || dedicated;
    let w: u32 = o.val("-width").and_then(|s| s.parse().ok()).unwrap_or(1280);
    let h: u32 = o.val("-height").and_then(|s| s.parse().ok()).unwrap_or(720);
    let autoquit: Option<f64> = o.val("-autoquit").and_then(|s| s.parse().ok());
    let Some(pal) = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok()) else {
        eprintln!("couldn't load gfx/palette.lmp");
        std::process::exit(1);
    };

    let mut platform = None;
    let mut window = None;
    let renderer = if headless {
        qrender::Renderer::headless(w, h, &pal)
    } else {
        let p = qplatform::Platform::init().unwrap_or_else(|e| {
            eprintln!("SDL init failed: {e:?}");
            std::process::exit(1)
        });
        let win = p.create_window("Quake", w, h, true).unwrap_or_else(|e| {
            eprintln!("window creation failed: {e:?}");
            std::process::exit(1)
        });
        let r = qrender::Renderer::new(&platform::Surface(&win), &pal);
        platform = Some(p);
        window = Some(win);
        r
    };
    let renderer = renderer.unwrap_or_else(|e| {
        eprintln!("renderer init failed: {e:?}");
        std::process::exit(1)
    });
    eprintln!("renderer: {}", renderer.device_name());

    // audio
    let mut audio = None;
    let mut snd = None;
    if let Some(p) = &platform
        && !o.has("-nosound")
    {
        let mut sfs = Fs::new(&basedir);
        for g in fs.game_dirs() {
            sfs.add_game_dir(g);
        }
        let rate = 48000;
        let (sys, mut mixer) = SoundSystem::new(rate, move |n| sfs.open(n));
        match p.open_audio(rate, move |buf| mixer.paint(buf)) {
            Ok(a) => {
                let _ = a.resume();
                audio = Some(a);
                snd = Some(sys);
            }
            Err(e) => eprintln!("audio: {e:?}"),
        }
    }

    let mut host = host::Host::new(fs, renderer, snd, o.args.clone());
    host.headless = headless;
    host.dedicated = dedicated;
    if let Some(p) = o.val("-port").and_then(|s| s.parse().ok()) {
        host.net_port = p;
    }
    // -listen [n] / -dedicated [n]: slots for the first map (default 8 / 16).
    for (flag, def) in [("-listen", 8), ("-dedicated", 16)] {
        if o.has(flag) {
            let n = o.val(flag).and_then(|s| s.parse::<usize>().ok()).unwrap_or(def);
            host.exec_line(&format!("maxplayers {n}"));
        }
    }
    if let Some(win) = window {
        let _ = win.set_text_input(true);
        host.window = Some(win);
    }

    let mut last = Instant::now();
    loop {
        // Host_FilterTime: timedemo runs uncapped.
        let maxfps = if dedicated { 72.0 } else if host.timedemo_active() { 0.0 } else { host.server.core.cvars.value_of("host_maxfps") };
        let dt = if let Some(dt) = fixed_step {
            dt
        } else if headless && !realtime {
            1.0 / if maxfps > 0.0 { maxfps.min(1000.0) as f64 } else { 72.0 }
        } else {
            let min = if maxfps > 0.0 { 1.0 / maxfps as f64 } else { 0.0 };
            let mut el = last.elapsed().as_secs_f64();
            if el < min {
                let rem = min - el;
                if rem > 0.002 {
                    std::thread::sleep(Duration::from_secs_f64(rem - 0.001));
                }
                while last.elapsed().as_secs_f64() < min {
                    std::hint::spin_loop();
                }
                el = last.elapsed().as_secs_f64();
            }
            last = Instant::now();
            el.min(0.25)
        };
        let events = match &mut platform {
            Some(p) => p.poll_events().into_iter().flat_map(platform::convert).collect(),
            None => Vec::new(),
        };
        host.frame(dt, events);
        if let (Some(p), Some((lo, hi, ms))) = (&platform, host.take_rumble()) {
            // Split-screen: only player 1's pad feels player 1's hits.
            match host.rumble_pad() {
                Some(id) => {
                    let _ = p.rumble(id, lo, hi, ms);
                }
                None => p.rumble_all(lo, hi, ms),
            }
        }
        if autoquit.is_some_and(|t| host.realtime >= t) {
            host.quit = true;
        }
        if host.quit {
            break;
        }
    }
    host.write_config();
    drop(audio);
}

#[cfg(test)]
mod timing_tests {
    use super::Opts;

    #[test]
    fn fixed_step_is_explicit_and_rejects_invalid_or_realtime_modes() {
        let opts = |s: &str| Opts { args: s.split_whitespace().map(str::to_string).collect() };
        assert_eq!(opts("-window").fixed_step(), Ok(None));
        assert_eq!(opts("-window -fixedstep 250").fixed_step(), Ok(Some(0.004)));
        for flags in ["-fixedstep", "-fixedstep NaN", "-fixedstep inf", "-fixedstep 0",
            "-fixedstep -1", "-fixedstep 1001", "-fixedstep 250 -realtime",
            "-dedicated -fixedstep 250"] {
            assert!(opts(flags).fixed_step().is_err(), "{flags}");
        }
    }
}
