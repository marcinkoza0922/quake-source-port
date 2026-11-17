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
    pub game: &'a str,
    pub width: u32,
    pub height: u32,
    pub display: Display,
    /// Record a MangoHud frame log of the demo.
    pub mangohud: bool,
    /// Engine settings (`+cvar value` pairs) after [`Engine::base_args`].
    pub args: Vec<String>,
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
    /// The full argv (program first).
    pub fn argv(&self) -> Vec<String> {
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
        v.extend(["+timedemo".into(), self.timedemo.into()]);
        v
    }
}

/// Runs the timedemo and returns its result (and frame log).
pub fn run(spec: &RunSpec) -> Result<RunResult, String> {
    use std::os::unix::process::CommandExt;
    std::fs::create_dir_all(spec.out).map_err(|e| format!("{}: {e}", spec.out.display()))?;
    let argv = spec.argv();
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .current_dir(spec.base)
        .process_group(0)
        .env_remove("MANGOHUD")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let control = format!("perfkit-{}-{}", std::process::id(), RUN_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    if spec.mangohud {
        let empty = spec.out.join("mangohud.conf");
        std::fs::write(&empty, "").map_err(|e| e.to_string())?;
        cmd.env("MANGOHUD_CONFIGFILE", &empty).env("MANGOHUD_CONFIG", mangohud::config(spec.out, &control));
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
                console.push(line);
                if playing && spec.mangohud && ctl.is_none() {
                    match mangohud::Control::connect(&control, Duration::from_secs(5)).and_then(|mut c| c.start_logging().map(|_| c)) {
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
    write_console(spec.out, &argv, &console);

    let (frames, secs, fps) = res.map_err(|e| format!("{e} (console: {})", spec.out.join("console.txt").display()))?;
    let frame_ms = if spec.mangohud { Some(frame_log(spec.out, frames, secs)?) } else { None };
    Ok(RunResult { frames, secs, fps, frame_ms })
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

    #[test]
    fn argv_nests_gamescope_then_mangohud() {
        let out = std::env::temp_dir();
        let spec = RunSpec {
            engine: Engine::Ironwail,
            bin: Path::new("/x/ironwail"),
            base: Path::new("."),
            game: "ad",
            width: 960,
            height: 540,
            display: Display::Gamescope { out_w: 2560, out_h: 1440 },
            mangohud: true,
            args: vec!["+vid_fsaa".into(), "8".into()],
            timedemo: "perfkit/x",
            timeout: Duration::from_secs(1),
            out: &out,
        };
        let a = spec.argv().join(" ");
        assert_eq!(
            a,
            "gamescope -w 960 -h 540 -W 2560 -H 1440 -- mangohud stdbuf -oL /x/ironwail -basedir . -game ad \
             -window -width 960 -height 540 +vid_vsync 0 +vid_fsaa 8 +timedemo perfkit/x"
        );
        let plain = RunSpec { display: Display::Window, mangohud: false, engine: Engine::Ours, ..spec };
        assert!(plain.argv().join(" ").starts_with("stdbuf -oL /x/ironwail"));
        assert!(plain.argv().contains(&"+sv_autosave".to_string()));
    }
}
