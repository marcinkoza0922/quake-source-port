//! `perfkit partinfo`: are the FTE-suite workloads like for like? (Part 1,
//! item V3, docs/perf/VANILLA.md "Particle suite").
//!
//! Usage: perfkit partinfo [-samples N] [-tolerance PCT] [-timeout SECS]
//!                         [-basedir DIR]
//!
//! For each FTE-suite case (`spin-sepulcher-fte` and `combat-grendel-fte`
//! from their `.qsd`, profile `ultra-fte`), it stuffs
//! `r_partinfo` into N evenly spread blocks of the recorded demo (as the
//! stills stuff `screenshot`), timedemos it at 960×540 in ours and vkQuake,
//! and reads every engine's live particle and decal counts back from the
//! console. Under `timedemo` every block is drawn once, so both engines
//! answer at the same demo frame. A case is like for like when the summed
//! particle counts differ by at most the tolerance (default 5%). Report:
//! `target/perf/partinfo/<commit>/report.md`.

use perfkit::demofile::Demo;
use perfkit::engine::{Engine, abs};
use perfkit::guard::ConfigGuard;
use perfkit::machine::Machine;
use perfkit::runner::{self, Display, RunSpec};
use perfkit::sandbox::Sandbox;
use perfkit::scenes;
use perfkit::settings::{self, Profile};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

const ENGINES: [Engine; 2] = [Engine::Ours, Engine::VkQuake];
const CASES: [&str; 2] = ["spin-sepulcher-fte", "combat-grendel-fte"];

struct Opts {
    basedir: PathBuf,
    samples: usize,
    tolerance: f64,
    timeout: u64,
}

/// One `r_partinfo` answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sample {
    pub particles: u64,
    pub decals: u64,
    /// Live particles per `config.name`.
    pub types: BTreeMap<String, u64>,
}

fn parse(args: &[String]) -> Opts {
    let mut o = Opts {
        basedir: std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| "vkQuake directory with maps".into()),
        samples: 12,
        tolerance: 5.0,
        timeout: 300,
    };
    let mut it = args.iter();
    while let Some(a) = it.next() {
        let mut v = || it.next().cloned().unwrap_or_else(|| crate::die(&format!("{a} needs a value")));
        match a.as_str() {
            "-basedir" => o.basedir = v().into(),
            "-samples" => o.samples = v().parse().unwrap_or_else(|_| crate::die("bad -samples")),
            "-tolerance" => o.tolerance = v().parse().unwrap_or_else(|_| crate::die("bad -tolerance")),
            "-timeout" => o.timeout = v().parse().unwrap_or_else(|_| crate::die("bad -timeout")),
            _ => crate::die(&format!("unknown option {a}")),
        }
    }
    o
}

/// The blocks to sample: `n` evenly spread over 10–95% of the demo, past
/// the signon and before the final disconnect.
pub fn sample_blocks(len: usize, n: usize) -> Vec<usize> {
    if len < 4 || n == 0 {
        return Vec::new();
    }
    let (a, b) = (len as f64 * 0.10, len as f64 * 0.95);
    let mut v: Vec<usize> = (0..n)
        .map(|i| (a + (b - a) * i as f64 / (n.max(2) - 1) as f64) as usize)
        .map(|i| i.min(len - 2))
        .collect();
    v.dedup();
    v
}

/// Reads our `r_partinfo` answers (`particles: N, beams: N, decals: N`,
/// then `  config.name: N` per running type).
pub fn parse_ours(console: &str) -> Vec<Sample> {
    let mut out: Vec<Sample> = Vec::new();
    let mut in_types = false;
    for line in console.lines() {
        if let Some(rest) = line.strip_prefix("particles: ") {
            let nums: Vec<u64> = rest.split(", ").filter_map(|f| f.rsplit(' ').next()?.parse().ok()).collect();
            let first: u64 = rest.split(',').next().and_then(|s| s.trim().parse().ok()).unwrap_or(0);
            out.push(Sample { particles: first, decals: nums.last().copied().unwrap_or(0), types: BTreeMap::new() });
            in_types = false;
        } else if line.starts_with("requests: ") || line.starts_with("rain submodels: ") {
            in_types = !out.is_empty();
        } else if in_types && let Some(t) = line.strip_prefix("  ") {
            if let Some((name, n)) = t.rsplit_once(": ")
                && let (Ok(n), Some(s)) = (n.trim().parse::<u64>(), out.last_mut())
            {
                *s.types.entry(name.to_string()).or_default() += n;
            }
        } else {
            in_types = false;
        }
    }
    out
}

/// Reads vkQuake's (`Running effects:`, `Type config.name\tN particles…, K
/// decals` lines, `End of list`, `N particles, F free, T traces.`).
pub fn parse_vkquake(console: &str) -> Vec<Sample> {
    let mut out: Vec<Sample> = Vec::new();
    let mut cur: Option<Sample> = None;
    for line in console.lines() {
        if line == "Running effects:" {
            cur = Some(Sample::default());
        } else if let (Some(s), Some(rest)) = (cur.as_mut(), line.strip_prefix("Type ")) {
            let (name, counts) = rest.split_once('\t').unwrap_or((rest, ""));
            let n = counts.split(' ').next().and_then(|x| x.parse::<u64>().ok()).filter(|_| counts.contains("particles"));
            if let Some(n) = n {
                *s.types.entry(name.to_string()).or_default() += n;
            }
        } else if let Some(s) = cur.as_mut() {
            let num = || line.split(' ').next().and_then(|x| x.parse::<u64>().ok());
            if line.contains(" particles, ") && line.ends_with("traces.") {
                s.particles = num().unwrap_or(0);
            } else if line.contains(" decals, ") && line.ends_with("free.") {
                s.decals = num().unwrap_or(0);
                out.push(cur.take().unwrap());
            }
        }
    }
    out
}

pub fn run(args: &[String]) {
    let o = parse(args);
    let demos = std::fs::canonicalize("target/perf/demos/latest")
        .unwrap_or_else(|_| crate::die("no target/perf/demos/latest: run `perfkit record` first"));
    let machine = Machine::detect();
    let commit = if machine.commit.is_empty() { "unknown".to_string() } else { machine.commit.clone() };
    let out = abs(Path::new("target/perf/partinfo")).join(&commit);
    let refdir = abs(Path::new("target")).join("refengines");
    let root = PathBuf::from("target/perf");
    let mut md = format!(
        "# r_partinfo: ours vs vkQuake\n\n{}, demo set `{}`. `ultra-fte`, 960×540, timedemo; `r_partinfo` stuffed into {} blocks per demo. Like for like when summed particle counts differ by ≤ {}%.\n",
        commit,
        demos.file_name().unwrap_or_default().to_string_lossy(),
        o.samples,
        o.tolerance
    );
    let mut failed = false;
    for name in CASES {
        let scene = scenes::find(name).expect("scene");
        let qsd = demos.join(format!("{name}.qsd"));
        let src = if qsd.is_file() { qsd } else { demos.join(format!("{name}.dem")) };
        let guard = ConfigGuard::for_game(&o.basedir, scene.game);
        let sb = Sandbox::build(&o.basedir, &root, scene.game).unwrap_or_else(|e| crate::die(&format!("sandbox: {e}")));
        let bytes = std::fs::read(&src).unwrap_or_else(|e| crate::die(&format!("{}: {e}", src.display())));
        let mut d = Demo::parse(&bytes).unwrap_or_else(|e| crate::die(&format!("{}: {e}", src.display())));
        let blocks = sample_blocks(d.blocks.len(), o.samples);
        for &i in &blocks {
            d.stuff(i, "r_partinfo\n").unwrap_or_else(|e| crate::die(&e));
        }
        let _ = std::fs::create_dir_all(&out);
        let patched = out.join(format!("{name}.dem"));
        std::fs::write(&patched, d.to_bytes()).unwrap_or_else(|e| crate::die(&e.to_string()));
        let demo = sb.install_demo(scene.game, &format!("{name}-partinfo"), &patched).unwrap_or_else(|e| crate::die(&e.to_string()));
        eprintln!("perfkit: {name} ({} samples, {})", blocks.len(), src.file_name().unwrap_or_default().to_string_lossy());
        let mut got: BTreeMap<&str, Vec<Sample>> = BTreeMap::new();
        for e in ENGINES {
            let Some(bin) = e.locate(&o.basedir, &refdir) else {
                eprintln!("perfkit:   {} not found", e.name());
                failed = true;
                continue;
            };
            let (args, cfg) = settings::split(Profile::UltraFte, e);
            let run_out = out.join("runs").join(format!("{name}-{}", e.name()));
            let spec = RunSpec {
                engine: e, bin: &bin, base: &sb.dir, user_data: Some(&sb.userdata), game: scene.game, width: 960, height: 540,
                display: Display::Window, mangohud: false, cold: false, args, cfg, timedemo: &demo,
                timeout: Duration::from_secs(o.timeout), out: &run_out,
            };
            if let Err(err) = runner::run(&spec) {
                eprintln!("perfkit:   {} FAILED: {err}", e.name());
                failed = true;
                continue;
            }
            let console = std::fs::read_to_string(run_out.join("console.txt")).unwrap_or_default();
            let s = if e == Engine::Ours { parse_ours(&console) } else { parse_vkquake(&console) };
            eprintln!("perfkit:   {:<9} {} answers", e.name(), s.len());
            if s.len() != blocks.len() {
                failed = true;
            }
            got.insert(e.name(), s);
        }
        let (ours, vk) = (got.get("ours").cloned().unwrap_or_default(), got.get("vkquake").cloned().unwrap_or_default());
        md.push_str(&case_report(name, &blocks, &ours, &vk, o.tolerance));
        if !guard.changed().is_empty() {
            eprintln!("perfkit: CONFIGS CHANGED, restoring");
            guard.restore();
            failed = true;
        }
    }
    let _ = std::fs::write(out.join("report.md"), &md);
    print!("{md}");
    eprintln!("partinfo: {}", out.join("report.md").display());
    if failed {
        std::process::exit(1);
    }
}

fn pct(a: u64, b: u64) -> String {
    if b == 0 {
        return if a == 0 { "0%".into() } else { "–".into() };
    }
    format!("{:+.1}%", (a as f64 / b as f64 - 1.0) * 100.0)
}

/// One case's Markdown: per-sample totals, the verdict, and the types whose
/// summed counts differ most.
pub fn case_report(name: &str, blocks: &[usize], ours: &[Sample], vk: &[Sample], tol: f64) -> String {
    let mut s = format!("\n## {name}\n\n| block | ours particles | vkQuake | diff | ours decals | vkQuake |\n|--:|--:|--:|--:|--:|--:|\n");
    for (i, b) in blocks.iter().enumerate() {
        let (o, v) = (ours.get(i).cloned().unwrap_or_default(), vk.get(i).cloned().unwrap_or_default());
        let _ = writeln!(s, "| {b} | {} | {} | {} | {} | {} |", o.particles, v.particles, pct(o.particles, v.particles), o.decals, v.decals);
    }
    let sum = |x: &[Sample]| x.iter().map(|s| s.particles).sum::<u64>();
    let (so, sv) = (sum(ours), sum(vk));
    let complete = ours.len() == blocks.len() && vk.len() == blocks.len();
    let verdict = if !complete {
        "INCOMPLETE (missing answers)"
    } else if so == 0 && sv == 0 {
        // `r_particledesc classic` loads no scripts: both engines draw these
        // effects with their classic particles, which r_partinfo doesn't count.
        "no FTE particles in either engine (classic effects only), so not an FTE measurement"
    } else if sv > 0 && ((so as f64 / sv as f64 - 1.0) * 100.0).abs() <= tol {
        "like for like"
    } else {
        "NOT like for like"
    };
    let _ = writeln!(s, "\nSummed: ours {so}, vkQuake {sv} ({}): **{verdict}**.", pct(so, sv));
    let dsum = |x: &[Sample]| x.iter().map(|s| s.decals).sum::<u64>();
    let (d_ours, d_vk) = (dsum(ours), dsum(vk));
    let _ = writeln!(s, "Decals summed: ours {d_ours}, vkQuake {d_vk} ({}).", pct(d_ours, d_vk));
    let mut types: BTreeMap<&str, (u64, u64)> = BTreeMap::new();
    for x in ours {
        for (t, n) in &x.types {
            types.entry(t).or_default().0 += n;
        }
    }
    for x in vk {
        for (t, n) in &x.types {
            types.entry(t).or_default().1 += n;
        }
    }
    let mut rows: Vec<_> = types.into_iter().collect();
    rows.sort_by_key(|(_, (o, v))| std::cmp::Reverse(o.abs_diff(*v)));
    if !rows.is_empty() {
        s.push_str("\nTypes by summed difference (top 12):\n\n| type | ours | vkQuake | diff |\n|---|--:|--:|--:|\n");
        for (t, (o, v)) in rows.iter().take(12) {
            let _ = writeln!(s, "| {t} | {o} | {v} | {} |", pct(*o, *v));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_spread_inside_the_demo() {
        assert_eq!(sample_blocks(1000, 3), vec![100, 525, 950]);
        assert!(sample_blocks(3, 5).is_empty());
        assert!(sample_blocks(15000, 12).iter().all(|&b| (1500..=14250).contains(&b)));
    }

    #[test]
    fn reads_both_engines() {
        let ours = "configs: [\"classic\"]\ntypes: 9 (9 loaded), textures: 2\nprecached: 0\nparticles: 120, beams: 3, decals: 7\nfirst precaches: []\nrequests: 5\n  classic.te_explosion: 100\n  classic.rocket: 20\nPlaying demo\n";
        let s = parse_ours(ours);
        assert_eq!(s.len(), 1);
        assert_eq!((s[0].particles, s[0].decals), (120, 7));
        assert_eq!(s[0].types["classic.te_explosion"], 100);
        assert_eq!(s[0].types.len(), 2);
        let vk = "Running effects:\nType classic.te_explosion\t98 particles\nType classic.rocket\t21 particles(+traceline), 2 decals\nType classic.empty\nEnd of list\n119 particles, 900 free, 21 traces.\n7 decals, 10 free.\n";
        let v = parse_vkquake(vk);
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].particles, v[0].decals), (119, 7));
        assert_eq!(v[0].types["classic.rocket"], 21);
        let r = case_report("x", &[10], &s, &v, 5.0);
        assert!(r.contains("**like for like**"), "{r}");
        let none = case_report("x", &[10], &[Sample::default()], &[Sample::default()], 5.0);
        assert!(none.contains("no FTE particles"), "{none}");
        assert!(case_report("x", &[10, 20], &s, &v, 5.0).contains("INCOMPLETE"));
    }
}
