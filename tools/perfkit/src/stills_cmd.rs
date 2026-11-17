//! `perfkit stills`: Part 1's visual parity stills (R5). Records one demo
//! per map with our engine ([`perfkit::stills`]), patches a `screenshot`
//! into every view's hold, plays it with `playdemo` in each engine (the
//! view's settings profile, HUD off, a 1920×1080 window), and compares the
//! images with DSSIM (ImageMagick; 0 = identical).
//!
//! The references differ from each other too, so they set the allowed band:
//! ours is off when its distance to a reference exceeds `-ratio` times the
//! ironwail↔vkQuake distance plus `-margin`. The defaults (1.5, 0.005)
//! flag ad_tears' water drawn opaque (0.029 against a 0.0135 band), which
//! a flat +0.02 let through. FTE views compare ours
//! with vkQuake only (ironwail can't show FTE particles) and get no verdict.
//!
//! Output in `target/perf/stills/<commit>/` (`latest` links to it):
//! `<view>/<engine>.png`, thumbnails and difference images, `report.md`,
//! `report.json` and `index.html`, a side-by-side gallery.
//!
//! Usage: perfkit stills [-basedir DIR] [-only view,view] [-engines a,b]
//!                       [-ratio 1.5] [-margin 0.005] [-timeout SECS]

use perfkit::demofile::Demo;
use perfkit::engine::{abs, Engine};
use perfkit::guard::ConfigGuard;
use perfkit::json_str;
use perfkit::machine::Machine;
use perfkit::runner::{self, Display, RunSpec};
use perfkit::sandbox::Sandbox;
use perfkit::settings::{self, Profile};
use perfkit::stills::{self, Group, View};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

const W: u32 = 1920;
const H: u32 = 1080;

struct Opts {
    basedir: PathBuf,
    only: Vec<String>,
    engines: Vec<Engine>,
    ratio: f64,
    margin: f64,
    timeout: u64,
}

fn parse(args: &[String]) -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| "vkQuake directory with maps".into()),
        only: Vec::new(),
        engines: Engine::ALL.to_vec(),
        ratio: 1.5,
        margin: 0.005,
        timeout: 90,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut v = || it.next().cloned().unwrap_or_else(|| crate::die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-only" => o.only = v().split(',').map(String::from).collect(),
            "-engines" => o.engines = Engine::parse_list(&v()).unwrap_or_else(|e| crate::die(&e)),
            "-margin" => o.margin = v().parse().unwrap_or_else(|_| crate::die("bad -margin")),
            "-ratio" => o.ratio = v().parse().unwrap_or_else(|_| crate::die("bad -ratio")),
            "-timeout" => o.timeout = v().parse().unwrap_or_else(|_| crate::die("bad -timeout")),
            _ => crate::die(&format!("unknown option {a}")),
        }
    }
    for n in &o.only {
        if !stills::VIEWS.iter().any(|v| v.name == n) {
            crate::die(&format!("unknown view {n}"));
        }
    }
    o
}

/// What every group's runs share.
struct Ctx<'a> {
    o: &'a Opts,
    refdir: PathBuf,
    out: PathBuf,
}

/// What one view produced.
#[derive(Default)]
struct Shot {
    images: BTreeMap<&'static str, PathBuf>,
    errors: Vec<String>,
}

pub fn run(args: &[String]) {
    let o = parse(args);
    let root = PathBuf::from("target/perf");
    let ours = Engine::Ours.locate(&o.basedir, &root).unwrap_or_else(|| crate::die("target/release/quake-source-port missing"));
    let machine = Machine::detect();
    let commit = if machine.commit.is_empty() { "unknown".to_string() } else { machine.commit.clone() };
    let out = abs(&root.join("stills")).join(&commit);
    let ctx = Ctx { o: &o, refdir: abs(Path::new("target")).join("refengines"), out: out.clone() };
    let mut shots: BTreeMap<&'static str, Shot> = BTreeMap::new();
    let mut changed = Vec::new();
    for g in stills::groups(&o.only) {
        let guard = ConfigGuard::for_game(&o.basedir, g.game);
        let sb = Sandbox::build(&o.basedir, &root, g.game).unwrap_or_else(|e| crate::die(&format!("sandbox: {e}")));
        eprintln!("perfkit: {} ({} views)", g.name(), g.views.len());
        match record(&g, &ours, &sb, &out.join("demos")) {
            Ok(demos) => {
                for &e in &o.engines {
                    play(&ctx, &g, e, &demos, &sb, &mut shots);
                }
            }
            Err(e) => {
                eprintln!("perfkit:   recording failed: {e}");
                for v in &g.views {
                    shots.entry(v.name).or_default().errors.push(format!("recording: {e}"));
                }
            }
        }
        let c = guard.changed();
        if !c.is_empty() {
            eprintln!("perfkit: CONFIGS CHANGED, restoring: {c:?}");
            guard.restore();
            changed.extend(c.iter().map(|p| p.display().to_string()));
        }
    }
    let rows = compare(&o, &out, &shots);
    write_reports(&o, &out, &machine, &rows, &changed);
    let latest = root.join("stills").join("latest");
    let _ = std::fs::remove_file(&latest);
    let _ = std::os::unix::fs::symlink(&commit, &latest);
    println!("stills: {} (index.html)", out.display());
    if rows.iter().any(|r| !r.errors.is_empty()) || !changed.is_empty() {
        std::process::exit(1);
    }
}

/// The patched demos: `.dem` and, when the recording kept FTE messages, `.qsd`.
struct Demos {
    dem: PathBuf,
    qsd: Option<PathBuf>,
}

fn record(g: &Group, ours: &Path, sb: &Sandbox, out: &Path) -> Result<Demos, String> {
    let user = sb.userdata.join("quake-source-port").join(g.game.to_ascii_lowercase());
    let stem = format!("perfkit_{}", g.name());
    for ext in ["dem", "qsd"] {
        let _ = std::fs::remove_file(user.join(format!("{stem}.{ext}")));
    }
    let cfg = sb.dir.join(g.game).join("perfkit").join("still.cfg");
    std::fs::write(&cfg, stills::record_cfg(g).join("\n") + "\n").map_err(|e| format!("{}: {e}", cfg.display()))?;
    let log = Command::new(ours)
        .args(stills::record_args(g, &sb.dir))
        .env("XDG_DATA_HOME", &sb.userdata)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("spawn {}: {e}", ours.display()))?;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let _ = std::fs::write(out.join(format!("{}.record.txt", g.name())), [log.stdout, log.stderr].concat());
    let angles: Vec<([f32; 3], [f32; 3])> = g.views.iter().map(|v| stills::shot_angles(v)).collect();
    let patch = |ext: &str| -> Result<Option<PathBuf>, String> {
        let src = user.join(format!("{stem}.{ext}"));
        let Ok(bytes) = std::fs::read(&src) else { return Ok(None) };
        let mut d = Demo::parse(&bytes).map_err(|e| format!("{}: {e}", src.display()))?;
        for i in d.shot_blocks(&angles, stills::MIN_RUN).map_err(|e| format!("{}: {e}", src.display()))? {
            d.stuff(i, "screenshot\n")?;
        }
        // Not the last block: that is the svc_disconnect `stop` wrote, and
        // parsing ends there.
        let last = d.blocks.len().checked_sub(2).ok_or("demo too short")?;
        d.stuff(last, &format!("echo {}\n", stills::DONE))?;
        let dst = out.join(format!("{}.{ext}", g.name()));
        std::fs::write(&dst, d.to_bytes()).map_err(|e| e.to_string())?;
        Ok(Some(dst))
    };
    let dem = patch("dem")?.ok_or_else(|| format!("no demo written ({}; see {}.record.txt)", log.status, g.name()))?;
    Ok(Demos { dem, qsd: patch("qsd")? })
}

fn play(c: &Ctx, g: &Group, e: Engine, d: &Demos, sb: &Sandbox, shots: &mut BTreeMap<&'static str, Shot>) {
    let (o, out) = (c.o, &c.out);
    let mut fail = |msg: String| {
        eprintln!("perfkit:   {:<9} FAILED: {msg}", e.name());
        for v in &g.views {
            shots.entry(v.name).or_default().errors.push(format!("{}: {msg}", e.name()));
        }
    };
    let Some(bin) = e.locate(&o.basedir, &c.refdir) else { return fail("engine not found".into()) };
    let (file, src) = match (&d.qsd, e, g.fte) {
        (Some(q), Engine::Ours | Engine::VkQuake, true) => ("full", q),
        _ => ("plain", &d.dem),
    };
    let demo = match sb.install_demo(g.game, &format!("{}-{file}", g.name()), src) {
        Ok(x) => x,
        Err(err) => return fail(err.to_string()),
    };
    let profile = if g.fte { Profile::UltraFte } else { Profile::Ultra };
    let (mut args, mut cfg) = settings::split(profile, e);
    // vsync on (after the base `+vid_vsync 0`): vkQuake 1.34.1 copies
    // `swapchain_images[current_cb_index]`, the image matching the command
    // buffer index, not the one just drawn. Without vsync one swapchain
    // image can stay untouched from startup and the shot shows the startup
    // console; with FIFO the images rotate and the copy is a few frames old.
    args.extend(["+vid_vsync".to_string(), "1".to_string()]);
    cfg.extend(stills::hud_off(e));
    let dirs = stills::shot_dirs(e, &sb.dir, &sb.userdata, g.game);
    let before: Vec<PathBuf> = stills::images(&dirs).into_iter().map(|(_, p)| p).collect();
    let run_out = out.join("runs").join(format!("{}-{}", g.name(), e.name()));
    let spec = RunSpec {
        engine: e, bin: &bin, base: &sb.dir, user_data: Some(&sb.userdata), game: g.game, width: W, height: H,
        display: Display::Window, mangohud: false, cold: false, args, cfg, timedemo: &demo,
        timeout: Duration::from_secs(o.timeout), out: &run_out,
    };
    let res = runner::play_until(&spec, stills::DONE, Duration::from_secs(2));
    let new: Vec<PathBuf> = stills::images(&dirs).into_iter().map(|(_, p)| p).filter(|p| !before.contains(p)).collect();
    let res = res.and_then(|_| match new.len() == g.views.len() {
        true => Ok(()),
        false => Err(format!("{} screenshots for {} views", new.len(), g.views.len())),
    });
    if let Err(err) = res {
        for p in &new {
            let _ = std::fs::remove_file(p); // ours, in the sandbox
        }
        return fail(err);
    }
    for (v, img) in g.views.iter().zip(&new) {
        let dir = out.join(v.name);
        let _ = std::fs::create_dir_all(&dir);
        let dst = dir.join(format!("{}.png", e.name()));
        let ok = if img.extension().is_some_and(|x| x == "png") {
            std::fs::rename(img, &dst).is_ok()
        } else {
            let ok = Command::new("magick").arg(img).arg(&dst).status().is_ok_and(|s| s.success());
            let _ = std::fs::remove_file(img);
            ok
        };
        let shot = shots.entry(v.name).or_default();
        if ok {
            shot.images.insert(e.name(), dst);
        } else {
            shot.errors.push(format!("{}: couldn't store {}", e.name(), img.display()));
        }
    }
    eprintln!("perfkit:   {:<9} {} shots ({file})", e.name(), new.len());
}

/// One view's comparison.
struct Row {
    view: &'static View,
    images: BTreeMap<&'static str, PathBuf>,
    /// DSSIM for (ours, ironwail), (ours, vkquake), (ironwail, vkquake).
    d: [Option<f64>; 3],
    verdict: &'static str,
    errors: Vec<String>,
}

const PAIRS: [(&str, &str); 3] = [("ours", "ironwail"), ("ours", "vkquake"), ("ironwail", "vkquake")];

fn compare(o: &Opts, out: &Path, shots: &BTreeMap<&'static str, Shot>) -> Vec<Row> {
    let mut rows = Vec::new();
    for v in stills::VIEWS.iter().filter(|v| shots.contains_key(v.name)) {
        let s = &shots[v.name];
        let mut errors = s.errors.clone();
        let dir = out.join(v.name);
        let mut d = [None; 3];
        for (i, (a, b)) in PAIRS.iter().enumerate() {
            if let (Some(pa), Some(pb)) = (s.images.get(a), s.images.get(b)) {
                match stills::dssim(pa, pb) {
                    Ok(x) => d[i] = Some(x),
                    Err(e) => errors.push(e),
                }
                if *a == "ours" {
                    // Difference image, brightened so small changes show.
                    let _ = Command::new("magick")
                        .arg(pa).arg(pb)
                        .args(["-compose", "difference", "-composite", "-evaluate", "multiply", "4", "-resize", "640x"])
                        .arg(dir.join(format!("diff-{b}.jpg")))
                        .status();
                }
            }
        }
        for (e, p) in &s.images {
            let _ = Command::new("magick").arg(p).args(["-resize", "640x"]).arg(dir.join(format!("{e}.jpg"))).status();
        }
        let verdict = match (v.fte, d) {
            (true, _) => "fte (no band)",
            (false, [Some(oi), Some(ov), Some(iv)]) if oi.max(ov) <= iv * o.ratio + o.margin => "ok",
            (false, [Some(_), Some(_), Some(_)]) => "OFF",
            _ => "incomplete",
        };
        rows.push(Row { view: v, images: s.images.clone(), d, verdict, errors });
    }
    rows
}

fn write_reports(o: &Opts, out: &Path, m: &Machine, rows: &[Row], changed: &[String]) {
    let f = |x: Option<f64>| x.map_or("–".to_string(), |x| format!("{x:.4}"));
    let mut md = String::new();
    let _ = writeln!(md, "# Parity stills\n\n{}\n", m.summary());
    let _ = writeln!(
        md,
        "{W}×{H} window, vanilla ultra (`ultra-fte` for FTE views), HUD off, vsync on. DSSIM (0 = identical); **OFF** when ours \
         is further from a reference than {} × the ironwail↔vkQuake distance + {:.3}.\n",
        o.ratio, o.margin
    );
    md.push_str("| view | checks | ours↔ironwail | ours↔vkQuake | ironwail↔vkQuake | verdict |\n|---|---|--:|--:|--:|---|\n");
    for r in rows {
        let verdict = if r.verdict == "OFF" { "**OFF**" } else { r.verdict };
        let _ = writeln!(md, "| {} | {} | {} | {} | {} | {verdict} |", r.view.name, r.view.what, f(r.d[0]), f(r.d[1]), f(r.d[2]));
    }
    let errs: Vec<String> = rows.iter().flat_map(|r| r.errors.iter().map(move |e| format!("- {}: {e}", r.view.name))).collect();
    if !errs.is_empty() || !changed.is_empty() {
        let _ = writeln!(md, "\n## Errors\n\n{}", errs.join("\n"));
        for c in changed {
            let _ = writeln!(md, "- config changed (restored): {c}");
        }
    }
    let _ = std::fs::write(out.join("report.md"), &md);

    let views: Vec<String> = rows
        .iter()
        .map(|r| {
            let imgs: Vec<String> = r.images.iter().map(|(e, p)| format!("\"{e}\": {}", json_str(&p.display().to_string()))).collect();
            let errs: Vec<String> = r.errors.iter().map(|e| json_str(e)).collect();
            let num = |x: Option<f64>| x.map_or("null".to_string(), |x| format!("{x:.5}"));
            format!(
                "    {{\"name\": \"{}\", \"game\": \"{}\", \"map\": \"{}\", \"fte\": {}, \"pos\": {:?}, \"images\": {{{}}}, \
                 \"dssim\": {{\"ours_ironwail\": {}, \"ours_vkquake\": {}, \"ironwail_vkquake\": {}}}, \"verdict\": \"{}\", \"errors\": [{}]}}",
                r.view.name, r.view.game, r.view.map, r.view.fte, r.view.pos, imgs.join(", "),
                num(r.d[0]), num(r.d[1]), num(r.d[2]), r.verdict, errs.join(", ")
            )
        })
        .collect();
    let json = format!(
        "{{\n  \"machine\": {},\n  \"width\": {W},\n  \"height\": {H},\n  \"ratio\": {},\n  \"margin\": {},\n  \"configs_changed\": [{}],\n  \"views\": [\n{}\n  ]\n}}\n",
        m.to_json(), o.ratio, o.margin, changed.iter().map(|c| json_str(c)).collect::<Vec<_>>().join(", "), views.join(",\n")
    );
    let _ = std::fs::write(out.join("report.json"), json);
    let _ = std::fs::write(out.join("index.html"), crate::stills_html::gallery(m, rows.iter().map(|r| (r.view, r.d, r.verdict))));
}
