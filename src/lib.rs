//! quake-source-port: host library wiring the crates together (see docs/DESIGN.md).
//!
//! Options: -basedir <dir>, -game <dir> (repeatable), -hipnotic / -rogue / -quoth,
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
mod capture;
mod demo;
mod draw;
#[cfg(test)]
mod lang_tests;
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
mod hud;
mod skywind;
mod saveinfo;
mod splitscreen;
mod save_inventory;
mod view;
mod vwep;
#[cfg(target_arch = "wasm32")]
mod web;
mod wheel;

use std::path::PathBuf;

use qcore::fs::Fs;
use qsound::SoundSystem;

struct Opts {
    args: Vec<String>,
}

impl Opts {
    #[cfg(not(target_arch = "wasm32"))]
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
    /// Every value of a repeatable flag, in order.
    fn vals<'a>(&'a self, f: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.args.windows(2).filter(move |w| w[0].eq_ignore_ascii_case(f)).map(|w| w[1].as_str())
    }
}

/// `$XDG_DATA_HOME/quake-source-port/<game>` (default `~/.local/share/...`);
/// `/quake-user/<game>` in the web build.
pub fn user_dir(game: &str) -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from(qcore::fs::WEB_USER_ROOT).join(game.to_ascii_lowercase());
    }
    if cfg!(target_os = "android") && std::env::var_os("XDG_DATA_HOME").is_none() {
        return PathBuf::from(qcore::fs::ANDROID_DATA_DIR).join("userdata").join(game.to_ascii_lowercase());
    }
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("quake-source-port").join(game.to_ascii_lowercase())
}

/// Game search paths from -basedir/-game/-hipnotic/-rogue/-quoth plus the
/// user dir, and the game palette.
fn open_game(o: &Opts) -> Result<(Fs, PathBuf, [[u8; 3]; 256]), String> {
    let basedir = o
        .val("-basedir")
        .map(PathBuf::from)
        .unwrap_or_else(Fs::default_basedir);
    let mut fs = Fs::new(&basedir);
    if !fs.add_game_dir("id1") {
        return Err(format!("Couldn't find Quake game data in {}.\n\nCopy this port next to your Quake folder (the one containing id1/pak0.pak), or start it with -basedir DIR or the QUAKE_BASEDIR environment variable.", basedir.display()));
    }
    for (flag, dir) in [("-hipnotic", "hipnotic"), ("-rogue", "rogue"), ("-quoth", "quoth")] {
        if o.has(flag) {
            fs.add_game_dir(dir);
        }
    }
    // Several -game stack in order and the last is the active game, as in
    // ironwail and vkQuake (`-game ad -game mymap` for an AD map pack).
    for g in o.vals("-game") {
        if !g.eq_ignore_ascii_case("id1") && !fs.game_dirs().iter().any(|d| d.eq_ignore_ascii_case(g)) && !fs.add_game_dir(g) {
            eprintln!("game dir {g} not found");
        }
    }
    // User config/saves live outside the game data and override it.
    fs.add_user_dir(user_dir(fs.game_dir().unwrap_or("id1")));
    let pal = fs
        .open("gfx/palette.lmp")
        .and_then(|d| qformats::image::parse_palette(&d).ok())
        .ok_or("couldn't load gfx/palette.lmp")?;
    Ok((fs, basedir, pal))
}

type Sound = (Option<qplatform::AudioStream>, Option<SoundSystem>, Option<std::sync::Arc<qsound::SharedMixer>>);

/// Opens the audio device with a mixer reading sounds through its own `Fs`;
/// the mixer is shared so video capture can paint it (`SharedMixer`).
fn open_sound(p: &qplatform::Platform, basedir: &std::path::Path, fs: &Fs) -> Sound {
    let mut sfs = Fs::new(basedir);
    for g in fs.game_dirs() {
        sfs.add_game_dir(g);
    }
    let rate = 48000;
    let (sys, mixer) = SoundSystem::new(rate, move |n| sfs.open(n));
    let shared = std::sync::Arc::new(qsound::SharedMixer::new(mixer));
    let dev = shared.clone();
    match p.open_audio(rate, move |buf| dev.paint_device(buf)) {
        Ok(a) => {
            let _ = a.resume();
            (Some(a), Some(sys), Some(shared))
        }
        Err(e) => {
            eprintln!("audio: {e:?}");
            (None, None, None)
        }
    }
}

/// Forwards pending rumble to player 1's pad (split-screen) or every pad.
fn apply_rumble(host: &mut host::Host, p: &qplatform::Platform) {
    if let Some((lo, hi, ms)) = host.take_rumble() {
        match host.rumble_pad() {
            Some(id) => {
                let _ = p.rumble(id, lo, hi, ms);
            }
            None => p.rumble_all(lo, hi, ms),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub fn run() {
    use std::time::{Duration, Instant};

    let o = Opts { args: std::env::args().skip(1).collect() };
    let fixed_step = o.fixed_step().unwrap_or_else(|e| {
        eprintln!("{e}");
        std::process::exit(2)
    });
    let (fs, basedir, pal) = open_game(&o).unwrap_or_else(|e| {
        eprintln!("{e}");
        // launched by double-click there is no terminal: show the error too
        if !o.has("-headless") && !o.has("-dedicated") {
            qplatform::error_box("Quake data not found", &e);
        }
        std::process::exit(1)
    });
    // -dedicated [n]: no window, no local client, real-time pacing.
    let dedicated = o.has("-dedicated");
    let realtime = dedicated || o.has("-realtime");
    let headless = o.has("-headless") || dedicated;
    let w: u32 = o.val("-width").and_then(|s| s.parse().ok()).unwrap_or(1280);
    let h: u32 = o.val("-height").and_then(|s| s.parse().ok()).unwrap_or(720);
    let autoquit: Option<f64> = o.val("-autoquit").and_then(|s| s.parse().ok());

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

    let (audio, snd, mixer) = match &platform {
        Some(p) if !o.has("-nosound") => open_sound(p, &basedir, &fs),
        _ => (None, None, None),
    };

    let mut host = host::Host::new(fs, renderer, snd, o.args.clone());
    host.mixer = mixer;
    host.headless = headless;
    if dedicated {
        host.make_dedicated();
    }
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
        let capture = host.capture_step();
        let dt = if let Some(dt) = fixed_step {
            dt
        } else if let Some((step, paced)) = capture {
            // Video capture: exactly one frame per step; live captures keep
            // real-time pace, exports run as fast as they render.
            if paced && !headless {
                while last.elapsed().as_secs_f64() < step {
                    let rem = step - last.elapsed().as_secs_f64();
                    if rem > 0.002 { std::thread::sleep(Duration::from_secs_f64(rem - 0.001)); } else { std::hint::spin_loop(); }
                }
            }
            last = Instant::now();
            step
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
        if let Some(p) = &platform {
            apply_rumble(&mut host, p);
        }
        if autoquit.is_some_and(|t| host.realtime >= t) {
            host.quit = true;
        }
        if host.quit {
            break;
        }
    }
    host.write_config();
    host.shutdown_recordings();
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

/// Entry point called by SDL's `SDLActivity` on Android (the library is built as a `cdylib`
/// with `cargo rustc --lib --crate-type cdylib`, see `android/`).
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn SDL_main(_argc: std::ffi::c_int, _argv: *mut *mut std::ffi::c_char) -> std::ffi::c_int {
    run();
    0
}
