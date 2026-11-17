//! One windowed timedemo run of one engine.
//!
//! The command line is, outermost first:
//! `[gamescope -w W -h H -W OW -H OH --] [mangohud] stdbuf -oL <engine>
//! -basedir . -game G -window -width W -height H <base args> <args>
//! +timedemo <demo>`, run from the sandbox dir in its own process group.
//!
//! - gamescope renders the engine at W×H and scales it to OW×OH, so 4K runs
//!   work on a smaller monitor; both resolutions of the plan run through it
//!   so they share one presentation path.
//! - `stdbuf -oL` because the C engines block-buffer stdout on a pipe and
//!   the fps line would only arrive at exit.
//! - `mangohud` (the wrapper, so GL ironwail is hooked as well as the Vulkan
//!   engines) only on the inner command, so gamescope's own Vulkan isn't
//!   logged; see [`crate::mangohud`] for how the log is cut to the demo.
//!
//! The whole process group is SIGKILLed once the result is in, so no engine
//! ever writes its config.

use crate::engine::Engine;
use crate::mangohud;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Display {
    /// A plain window on the desktop.
    Window,
    /// Nested in gamescope, scaled to an output of this size.
    Gamescope { out_w: u32, out_h: u32 },
}

impl Display {
    pub fn name(self) -> &'static str {
        match self {
            Display::Window => "window",
            Display::Gamescope { .. } => "gamescope",
        }
    }
}

pub struct RunSpec<'a> {
    pub engine: Engine,
    pub bin: &'a Path,
    /// The sandbox dir (cwd, `-basedir .`).
    pub base: &'a Path,
    /// `XDG_DATA_HOME` for the engine ([`crate::sandbox::Sandbox::userdata`]),
    /// so ours never touches the user's configs and saves.
    pub user_data: Option<&'a Path>,
    pub game: &'a str,
    pub width: u32,
    pub height: u32,
    pub display: Display,
    /// Record a MangoHud frame log of the demo.
    pub mangohud: bool,
    /// Cold start: the NVIDIA driver's shader disk cache off
    /// (`__GL_SHADER_DISK_CACHE=0`, GL and Vulkan); neither ours nor vkQuake
    /// keeps a pipeline cache of its own.
    pub cold: bool,
    /// Engine settings (`+cvar value` pairs) after [`Engine::base_args`].
    pub args: Vec<String>,
    /// Console lines written to `<game>/perfkit/<run>.cfg` in the sandbox
    /// and `exec`ed before the timedemo (see [`crate::settings::split`]).
    pub cfg: Vec<String>,
    /// Demo name as `timedemo` takes it.
    pub timedemo: &'a str,
    pub timeout: Duration,
    /// This run's artifact dir: `console.txt` and the MangoHud log.
    pub out: &'a Path,
}

#[derive(Clone, Debug)]
pub struct RunResult {
    pub frames: u32,
    pub secs: f64,
    pub fps: f64,
    /// The demo's frame times (ms) from MangoHud, when requested.
    pub frame_ms: Option<Vec<f32>>,
}

static RUN_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

impl RunSpec<'_> {
    /// The run's cfg: base lines, pins, then the timedemo itself. The
    /// references keep only 256 characters of their command line
    /// (`CMDLINE_LENGTH`), which cut the demo name off the end, so only
    /// `vid_*` settings and the `+exec` stay there.
    pub fn cfg_lines(&self) -> Vec<String> {
        let mut v = self.engine.base_cfg();
        v.extend(self.cfg.iter().cloned());
        v.push(format!("timedemo {}", self.timedemo));
        v
    }

    /// The full argv (program first); `cfg` names the run's cfg to `exec`.
    pub fn argv(&self, cfg: &str) -> Vec<String> {
        let (w, h) = (self.width.to_string(), self.height.to_string());
        let mut v: Vec<String> = Vec::new();
        if let Display::Gamescope { out_w, out_h } = self.display {
            v.extend(["gamescope", "-w", &w, "-h", &h, "-W", &out_w.to_string(), "-H", &out_h.to_string(), "--"].map(String::from));
        }
        if self.mangohud {
            v.push("mangohud".into());
        }
        v.extend(["stdbuf".into(), "-oL".into(), self.bin.display().to_string()]);
        v.extend(["-basedir", ".", "-game", self.game, "-window", "-width", &w, "-height", &h].map(String::from));
        v.extend(self.engine.base_args());
        v.extend(self.args.iter().cloned());
        v.extend(["+exec".into(), cfg.into()]);
        v
    }
}

/// Runs the timedemo and returns its result (and frame log).
pub fn run(spec: &RunSpec) -> Result<RunResult, String> {
    std::fs::create_dir_all(spec.out).map_err(|e| format!("{}: {e}", spec.out.display()))?;
    let id = RUN_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let control = format!("perfkit-{}-{id}", std::process::id());
    // Short: the references count every command-line character (256 max).
    let cfg_name = format!("perfkit/r{id}.cfg");
    let cfg_path = spec.base.join(spec.game).join(&cfg_name);
    let text = spec.cfg_lines().join("\n") + "\n";
    std::fs::write(&cfg_path, &text).map_err(|e| format!("{}: {e}", cfg_path.display()))?;
    let _ = std::fs::write(spec.out.join("run.cfg"), text);
    let argv = spec.argv(&cfg_name);
    let res = run_argv(spec, &argv, &control);
    let _ = std::fs::remove_file(&cfg_path);
    res
}

fn run_argv(spec: &RunSpec, argv: &[String], control: &str) -> Result<RunResult, String> {
    use std::os::unix::process::CommandExt;
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(spec.base)
        .process_group(0)
        .env_remove("MANGOHUD")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if spec.cold {
        cmd.env("__GL_SHADER_DISK_CACHE", "0");
    }
    if let Some(u) = spec.user_data {
        cmd.env("XDG_DATA_HOME", u);
    }
    if spec.mangohud {
        let empty = spec.out.join("mangohud.conf");
        std::fs::write(&empty, "").map_err(|e| e.to_string())?;
        cmd.env("MANGOHUD_CONFIGFILE", &empty).env("MANGOHUD_CONFIG", mangohud::config(spec.out, control));
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn {}: {e}", argv[0]))?;
    let (tx, rx) = mpsc::channel::<String>();
    let out: Box<dyn std::io::Read + Send> = Box::new(child.stdout.take().expect("piped"));
    let err: Box<dyn std::io::Read + Send> = Box::new(child.stderr.take().expect("piped"));
    for (stream, tx) in [(out, tx.clone()), (err, tx)] {
        std::thread::spawn(move || {
            let mut r = BufReader::new(stream);
            let mut buf = Vec::new();
            while r.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
                let _ = tx.send(String::from_utf8_lossy(&buf).trim_end().to_string());
                buf.clear();
            }
        });
    }

    let mut console = Vec::new();
    let mut ctl: Option<mangohud::Control> = None;
    let deadline = Instant::now() + spec.timeout;
    let res = loop {
        match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(line) => {
                let result = crate::engine::parse_fps(&line);
                let playing = line.contains("Playing demo from");
                let fatal = is_fatal(&line);
                console.push(line);
                if fatal {
                    // The engines drop to the menu and never quit: fail now.
                    break Err(format!("engine error: {}", console.last().map_or("", |l| l.trim())));
                }
                if playing && spec.mangohud && ctl.is_none() {
                    match mangohud::Control::connect(control, Duration::from_secs(5)).and_then(|mut c| c.start_logging().map(|_| c)) {
                        Ok(c) => ctl = Some(c),
                        Err(e) => break Err(format!("mangohud control socket {control}: {e}")),
                    }
                }
                if let Some((frames, secs, fps)) = result {
                    let stopped = ctl.as_mut().map(|c| c.stop_logging(Duration::from_secs(10)));
                    break match stopped {
                        Some(Err(e)) => Err(e),
                        _ if spec.mangohud && ctl.is_none() => Err("the demo started before MangoHud could log it".into()),
                        _ => Ok((frames, secs, fps)),
                    };
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break Err("engine closed its output without a timedemo result".into()),
        }
        if let Ok(Some(s)) = child.try_wait() {
            break Err(format!("exited ({s}) without a timedemo result"));
        }
        if Instant::now() > deadline {
            break Err("timeout".into());
        }
    };
    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status();
    let _ = child.kill();
    let _ = child.wait();
    console.extend(rx.try_iter());
    write_console(spec.out, argv, &console);

    let res = res.and_then(|r| match rejected_pins(spec, &console) {
        bad if bad.is_empty() => Ok(r),
        bad => Err(format!("the engine doesn't know pinned cvars {bad:?}")),
    });
    let (frames, secs, fps) = res.map_err(|e| format!("{e} (console: {})", spec.out.join("console.txt").display()))?;
    let frame_ms = if spec.mangohud { Some(frame_log(spec.out, frames, secs)?) } else { None };
    Ok(RunResult { frames, secs, fps, frame_ms })
}

/// Console lines meaning the demo won't play: a missing file, a host error
/// or a protocol the engine can't parse.
fn is_fatal(line: &str) -> bool {
    const PATTERNS: [&str; 6] =
        ["couldn't open", "Host_Error", "ERROR:", "Illegible server message", "Bad server message", "CL_ParseServerMessage"];
    PATTERNS.iter().any(|p| line.contains(p))
}

/// Pinned cvars the engine answered with `Unknown command "<name>"`: the
/// binary differs from the source the settings table was checked against.
fn rejected_pins(spec: &RunSpec, console: &[String]) -> Vec<String> {
    let names = spec
        .args
        .iter()
        .filter_map(|a| a.strip_prefix('+'))
        .chain(spec.cfg.iter().filter_map(|l| l.split_whitespace().next()));
    names.filter(|n| console.iter().any(|l| l.contains(&format!("Unknown command \"{n}\"")))).map(String::from).collect()
}

fn write_console(dir: &Path, argv: &[String], lines: &[String]) {
    if let Ok(mut f) = std::fs::File::create(dir.join("console.txt")) {
        let _ = writeln!(f, "$ {}", argv.join(" "));
        for l in lines {
            let _ = writeln!(f, "{l}");
        }
    }
}

/// The demo's frame times from the MangoHud log in `dir`.
fn frame_log(dir: &Path, frames: u32, secs: f64) -> Result<Vec<f32>, String> {
    let path: PathBuf = mangohud::find_log(dir).ok_or_else(|| format!("no MangoHud log in {}", dir.display()))?;
    let text = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let ms = mangohud::parse_log(&text)?;
    let seg = mangohud::demo_segment(&ms, frames as usize)?;
    if !mangohud::aligned(seg, secs) {
        let total: f32 = seg.iter().sum();
        return Err(format!("MangoHud log misaligned: {:.3} s of frames vs timedemo {secs} s ({})", total / 1000.0, path.display()));
    }
    Ok(seg.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The references keep 256 characters of their command line; the
    /// longest one perfkit builds (vkQuake, `ultra-fte`, 4K, its real path)
    /// must fit with room to spare.
    #[test]
    fn command_line_fits_the_references() {
        let out = std::env::temp_dir();
        for e in Engine::ALL {
            let (args, cfg) = crate::settings::split(crate::settings::Profile::UltraFte, e);
            let spec = RunSpec {
                engine: e, bin: Path::new("/home/someone/Games/vkQuake/vkquake.AppImage"), base: Path::new("."),
                user_data: None, game: "func_mapjam5", width: 3840, height: 2160,
                display: Display::Gamescope { out_w: 2560, out_h: 1440 }, mangohud: true, cold: false, args, cfg,
                timedemo: "perfkit/spin-sepulcher-fte-full", timeout: Duration::from_secs(1), out: &out,
            };
            let argv = spec.argv(&format!("perfkit/r{}.cfg", u32::MAX));
            let engine_part = argv.iter().skip_while(|a| *a != "-basedir").cloned().collect::<Vec<_>>().join(" ");
            let len = spec.bin.display().to_string().len() + 1 + engine_part.len();
            assert!(len < 240, "{} command line is {len} characters: {engine_part}", e.name());
        }
    }

    #[test]
    fn argv_nests_gamescope_then_mangohud() {
        let out = std::env::temp_dir();
        let spec = RunSpec {
            engine: Engine::Ironwail,
            bin: Path::new("/x/ironwail"),
            base: Path::new("."),
            user_data: None,
            game: "ad",
            width: 960,
            height: 540,
            display: Display::Gamescope { out_w: 2560, out_h: 1440 },
            mangohud: true,
            cold: false,
            args: vec!["+vid_fsaa".into(), "8".into()],
            cfg: vec!["r_dynamic \"1\"".into()],
            timedemo: "perfkit/x",
            timeout: Duration::from_secs(1),
            out: &out,
        };
        let a = spec.argv("perfkit/r1-0.cfg").join(" ");
        assert_eq!(
            a,
            "gamescope -w 960 -h 540 -W 2560 -H 1440 -- mangohud stdbuf -oL /x/ironwail -basedir . -game ad \
             -window -width 960 -height 540 +vid_vsync 0 +vid_fsaa 8 +exec perfkit/r1-0.cfg"
        );
        assert_eq!(spec.cfg_lines(), ["r_dynamic \"1\"", "timedemo perfkit/x"]);
        let plain = RunSpec { display: Display::Window, mangohud: false, engine: Engine::Ours, ..spec };
        assert!(plain.argv("c").join(" ").starts_with("stdbuf -oL /x/ironwail"));
        assert_eq!(plain.cfg_lines()[0], "sv_autosave \"0\"");
        let console = ["Unknown command \"r_dynamic\"".to_string(), "Unknown command \"v_cshift\"".to_string()];
        assert_eq!(rejected_pins(&plain, &console), ["r_dynamic"]);
        assert!(rejected_pins(&plain, &console[1..]).is_empty());
        assert!(is_fatal("ERROR: couldn't open perfkit/x.dem"));
        assert!(is_fatal("Host_Error: CL_ParseServerMessage: Illegible server message"));
        assert!(!is_fatal("Playing demo from perfkit/x.dem."));
        assert!(!is_fatal("couldn't exec autoexec.cfg"));
    }
}
