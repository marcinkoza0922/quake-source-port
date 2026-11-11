//! qbench: timedemo fps comparison against ironwail and vkQuake.
//!
//! Each engine gets the same GPU-relevant settings on the command line
//! (MSAA off, 8x anisotropy, render scale 1; see `settings`).
//!
//! Usage: qbench [-basedir <dir>] [-game <dir>] [-width W] [-height H]
//!               [-runs N] [-secs S] [-engines ours,ironwail,vkquake]
//!               [-rerecord] [map ...]
//!
//! For each map (default: ad_sepulcher ad_tears with -game ad):
//! 1. Record a benchmark demo with our engine, headless: god mode, a slow
//!    360° spin from the spawn point for `-secs` seconds. Cached in
//!    `target/qbench/demos/<game>_<map>.dem` (`-rerecord` to redo).
//! 2. Build `target/qbench/base`: symlinks to id1 and the game's files, plus
//!    `<game>/qbench/<map>.dem`, so the game data dir is never written to.
//! 3. Run `timedemo qbench/<map>` windowed in each engine (cwd = that base,
//!    `-basedir .`), read the "N frames S seconds F fps" line from stdout and
//!    kill the process (the references don't quit after a timedemo).
//!
//! The median of `-runs` runs per engine goes to stdout and
//! `target/qbench/report.json`. ironwail is extracted from the basedir's
//! `ironwail-*-linux-x64.7z` into `target/refengines` on first use; vkQuake
//! is `<basedir>/vkquake.AppImage`.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

struct Opts {
    basedir: PathBuf,
    game: String,
    width: u32,
    height: u32,
    runs: usize,
    secs: u32,
    engines: Vec<String>,
    rerecord: bool,
    maps: Vec<String>,
}

fn parse_args() -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("vkQuake directory with maps")),
        game: "ad".into(),
        width: 1280,
        height: 720,
        runs: 3,
        secs: 24,
        engines: vec!["ours".into(), "ironwail".into(), "vkquake".into()],
        rerecord: false,
        maps: Vec::new(),
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut v = || it.next().unwrap_or_else(|| die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-game" => o.game = v(),
            "-width" => o.width = v().parse().unwrap_or(o.width),
            "-height" => o.height = v().parse().unwrap_or(o.height),
            "-runs" => o.runs = v().parse::<usize>().unwrap_or(3).max(1),
            "-secs" => o.secs = v().parse().unwrap_or(o.secs),
            "-engines" => o.engines = v().split(',').map(str::to_string).collect(),
            "-rerecord" => o.rerecord = true,
            m if !m.starts_with('-') => o.maps.push(m.to_string()),
            _ => die(&format!("unknown option {a}")),
        }
    }
    if o.maps.is_empty() {
        o.maps = if o.game == "ad" { vec!["ad_sepulcher".into(), "ad_tears".into()] } else { vec!["start".into()] };
    }
    o
}

fn die(m: &str) -> ! {
    eprintln!("qbench: {m}");
    std::process::exit(2)
}

/// "1500 frames   5.4 seconds 278.2 fps" -> (frames, secs, fps).
fn parse_fps(line: &str) -> Option<(u32, f64, f64)> {
    let w: Vec<&str> = line.split_whitespace().collect();
    let i = w.iter().position(|s| *s == "frames")?;
    if w.get(i + 2) != Some(&"seconds") || w.get(i + 4) != Some(&"fps") {
        return None;
    }
    Some((w[i - 1].parse().ok()?, w[i + 1].parse().ok()?, w[i + 3].parse().ok()?))
}

fn abs(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf())
}

fn user_dir(game: &str) -> PathBuf {
    let root = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    root.join("quake-source-port").join(game.to_ascii_lowercase())
}

/// Record the benchmark demo with our engine (headless, simulated time).
fn record(o: &Opts, ours: &Path, map: &str, out: &Path) -> bool {
    let name = format!("qbench_{map}");
    let st = Command::new(ours)
        .args(["-basedir"]).arg(&o.basedir)
        .args(["-headless", "-game", &o.game, "-autoquit", &(o.secs + 3).to_string()])
        .arg(format!("+record {name} {map}"))
        .args(["+god", "+cl_yawspeed 30", "+\"+right\""])
        .arg(format!("+delay {} stop", o.secs))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let src = user_dir(&o.game).join(format!("{name}.dem"));
    if !st.is_ok_and(|s| s.success()) || !src.is_file() {
        return false;
    }
    std::fs::rename(&src, out).or_else(|_| std::fs::copy(&src, out).map(|_| ())).is_ok()
}

/// target/qbench/base: id1 + game as symlinks, demos as real files.
fn build_base(o: &Opts, base: &Path) -> std::io::Result<()> {
    let src = abs(&o.basedir);
    std::fs::create_dir_all(base.join(&o.game).join("qbench"))?;
    let link = |from: PathBuf, to: PathBuf| -> std::io::Result<()> {
        if to.symlink_metadata().is_err() {
            std::os::unix::fs::symlink(from, to)?;
        }
        Ok(())
    };
    link(src.join("id1"), base.join("id1"))?;
    if o.game != "id1" {
        for e in std::fs::read_dir(src.join(&o.game))? {
            let e = e?;
            link(e.path(), base.join(&o.game).join(e.file_name()))?;
        }
    }
    Ok(())
}

fn ironwail(o: &Opts, refdir: &Path) -> Option<PathBuf> {
    let find = || {
        std::fs::read_dir(refdir).ok()?.flatten().map(|e| e.path().join("ironwail")).find(|p| p.is_file())
    };
    if let Some(p) = find() {
        return Some(p);
    }
    let arc = std::fs::read_dir(&o.basedir).ok()?.flatten().map(|e| e.path()).find(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy();
        n.starts_with("ironwail-") && n.ends_with("-linux-x64.7z")
    })?;
    std::fs::create_dir_all(refdir).ok()?;
    let ok = Command::new("7z").arg("x").arg("-y").arg(format!("-o{}", refdir.display())).arg(abs(&arc))
        .stdout(Stdio::null()).status().is_ok_and(|s| s.success());
    if ok { find() } else { None }
}

/// Read lines until a timedemo result appears; keeps draining afterwards so
/// the engine never blocks on a full pipe.
fn scan(out: Box<dyn std::io::Read + Send>, tx: std::sync::mpsc::Sender<(u32, f64, f64)>) {
    let mut r = BufReader::new(out);
    let mut buf = Vec::new();
    while r.read_until(b'\n', &mut buf).is_ok_and(|n| n > 0) {
        if let Some(v) = parse_fps(&String::from_utf8_lossy(&buf)) {
            let _ = tx.send(v);
        }
        buf.clear();
    }
}

/// Pins the settings that cost GPU time to the same values in every engine
/// (otherwise each uses its saved config): no MSAA, 8x anisotropy, full
/// render scale.
fn settings(bin: &Path) -> Vec<&'static str> {
    let name = bin.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
    if name.contains("ironwail") {
        vec!["+vid_fsaa", "0", "+gl_texture_anisotropy", "8", "+r_scale", "1"]
    } else if name.contains("vkquake") {
        vec!["+vid_fsaa", "0", "+r_scale", "1"]
    } else {
        vec!["+r_msaa", "0", "+r_anisotropy", "8", "+r_renderscale", "1"]
    }
}

/// One timedemo run; kills the engine once the fps line appears.
fn run_timedemo(bin: &Path, o: &Opts, base: &Path, map: &str) -> Result<(u32, f64, f64), String> {
    use std::os::unix::process::CommandExt;
    // stdio in the C engines is block-buffered on a pipe, so the fps line
    // would only arrive at exit: force line buffering. Own process group so
    // the AppImage's child dies with it.
    let mut child = Command::new("stdbuf")
        .arg("-oL")
        .arg(bin)
        .process_group(0)
        .current_dir(base)
        .args(["-basedir", ".", "-game", &o.game, "-window"])
        .args(["-width", &o.width.to_string(), "-height", &o.height.to_string()])
        .args(["+vid_vsync", "0"])
        .args(settings(bin))
        .args(["+timedemo", &format!("qbench/{map}")])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn {}: {e}", bin.display()))?;
    // Our engine prints the console to stderr, the references to stdout.
    let (tx, rx) = std::sync::mpsc::channel();
    let out: Box<dyn std::io::Read + Send> = Box::new(child.stdout.take().expect("piped"));
    let err: Box<dyn std::io::Read + Send> = Box::new(child.stderr.take().expect("piped"));
    for (out, tx) in [(out, tx.clone()), (err, tx)] {
        std::thread::spawn(move || scan(out, tx));
    }
    let deadline = Instant::now() + Duration::from_secs(o.secs as u64 * 10 + 60);
    let res = loop {
        if let Ok(v) = rx.recv_timeout(Duration::from_millis(200)) {
            break Ok(v);
        }
        if let Ok(Some(s)) = child.try_wait() {
            break rx.try_recv().map_err(|_| format!("exited ({s}) without a timedemo result"));
        }
        if Instant::now() > deadline {
            break Err("timeout".into());
        }
    };
    let _ = Command::new("kill").args(["-KILL", "--", &format!("-{}", child.id())]).status();
    let _ = child.kill();
    let _ = child.wait();
    res
}

fn main() {
    let o = parse_args();
    if !o.basedir.join("id1").is_dir() {
        die(&format!("no id1 in {}", o.basedir.display()));
    }
    let root = PathBuf::from("target/qbench");
    let base = root.join("base");
    let demos = root.join("demos");
    std::fs::create_dir_all(&demos).unwrap_or_else(|e| die(&e.to_string()));
    build_base(&o, &base).unwrap_or_else(|e| die(&format!("base dir: {e}")));
    let base = abs(&base);
    let ours = abs(Path::new("target/release/quake-source-port"));
    if !ours.is_file() {
        die("target/release/quake-source-port missing (cargo build --release)");
    }

    let mut engines: Vec<(String, PathBuf)> = Vec::new();
    for e in &o.engines {
        let bin = match e.as_str() {
            "ours" => Some(ours.clone()),
            "ironwail" => ironwail(&o, &root.parent().unwrap_or(&root).join("refengines")).map(|p| abs(&p)),
            "vkquake" => Some(abs(&o.basedir.join("vkquake.AppImage"))).filter(|p| p.is_file()),
            _ => die(&format!("unknown engine {e}")),
        };
        match bin {
            Some(b) => engines.push((e.clone(), b)),
            None => eprintln!("qbench: {e} not found, skipped"),
        }
    }

    let mut json = String::from("[\n");
    println!("{:<16} {:>10} {:>10} {:>10}", "map", "engine", "fps", "frames");
    for map in &o.maps {
        let dem = demos.join(format!("{}_{map}.dem", o.game));
        if (o.rerecord || !dem.is_file()) && !record(&o, &ours, map, &dem) {
            eprintln!("qbench: recording {map} failed");
            continue;
        }
        let _ = std::fs::copy(&dem, base.join(&o.game).join("qbench").join(format!("{map}.dem")));
        for (name, bin) in &engines {
            let mut fps = Vec::new();
            let mut frames = 0;
            let mut err = None;
            for _ in 0..o.runs {
                match run_timedemo(bin, &o, &base, map) {
                    Ok((f, _, v)) => {
                        frames = f;
                        fps.push(v);
                    }
                    Err(e) => err = Some(e),
                }
            }
            fps.sort_by(f64::total_cmp);
            let med = fps.get(fps.len() / 2).copied();
            match med {
                Some(m) => println!("{map:<16} {name:>10} {m:>10.1} {frames:>10}"),
                None => println!("{map:<16} {name:>10} {:>10}", err.as_deref().unwrap_or("failed")),
            }
            if json.len() > 2 {
                json.push_str(",\n");
            }
            json.push_str(&format!(
                "  {{\"game\": \"{}\", \"map\": \"{map}\", \"engine\": \"{name}\", \"width\": {}, \"height\": {}, \
                 \"frames\": {frames}, \"fps_median\": {}, \"runs\": {:?}}}",
                o.game, o.width, o.height, med.map_or("null".into(), |m| format!("{m:.1}")), fps
            ));
        }
    }
    json.push_str("\n]\n");
    let _ = std::fs::write(root.join("report.json"), json);
}

#[cfg(test)]
mod tests {
    #[test]
    fn fps_line() {
        assert_eq!(super::parse_fps("1500 frames   5.4 seconds 278.2 fps\n"), Some((1500, 5.4, 278.2)));
        assert_eq!(super::parse_fps("Playing demo from qbench/s.dem."), None);
    }
}
