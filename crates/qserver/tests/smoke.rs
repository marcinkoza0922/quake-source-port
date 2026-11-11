//! Smoke test: spawn every map of every installed gamedir, connect a client,
//! run ~10 s at skill 3 and check for QC/host errors and panics.
//! Failures are written to `target/qserver-smoke-<name>.txt`.
//!
//! Fast subset: `cargo nextest run -p qserver smoke_fast`.
//! Full sweep:  `cargo nextest run -p qserver --run-ignored only smoke_all`.

use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    d.join("id1").is_dir().then_some(d)
}

fn make_fs(base: &Path, game: &str) -> Fs {
    let mut fs = Fs::new(base);
    fs.add_game_dir("id1");
    if game != "id1" {
        fs.add_game_dir(game);
    }
    fs
}

/// Playable maps provided by `game` itself (not inherited from id1).
fn maps_of(base: &Path, game: &str) -> Vec<String> {
    let mut only = Fs::new(base);
    only.add_game_dir(game);
    only.list("maps/", ".bsp")
        .into_iter()
        .filter_map(|p| {
            let n = p.strip_prefix("maps/").or_else(|| p.strip_prefix("MAPS/"))?;
            let n = n.strip_suffix(".bsp").or_else(|| n.strip_suffix(".BSP"))?;
            (!n.contains('/')).then(|| n.to_string())
        })
        .collect()
}

fn gamedirs(base: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(base)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|g| !maps_of(base, g).is_empty())
        .collect();
    v.sort();
    v
}

enum Outcome {
    Ok,
    Skipped(String),
    Failed(String),
}

fn run_map(base: &Path, game: &str, map: &str, seconds: f64) -> Outcome {
    let fs = make_fs(base, game);
    if !fs.exists("progs.dat") {
        return Outcome::Skipped("no progs.dat".into());
    }
    let Some(bsp) = fs.open(&format!("maps/{map}.bsp")) else { return Outcome::Skipped("unreadable".into()) };
    let ents = qformats::Bsp::parse(&bsp).map(|b| b.entities).unwrap_or_default();
    if !ents.contains("info_player_start") {
        return Outcome::Skipped("no info_player_start".into());
    }
    let r = catch_unwind(AssertUnwindSafe(|| {
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.command("skill 3");
        if let Err(e) = sv.spawn_server(map) {
            return Outcome::Failed(format!("spawn: {e}"));
        }
        let Some(c) = sv.connect_local() else { return Outcome::Failed("connect failed".into()) };
        sv.command("god");
        let dt = 1.0 / 72.0;
        let frames = (seconds / dt) as usize;
        for i in 0..frames {
            let mv = ClientMove {
                angles: [0.0, (i as f32 * 2.0) % 360.0, 0.0],
                forward: 200.0,
                buttons: if i % 20 < 10 { 1 } else { 2 },
                ..Default::default()
            };
            sv.set_move(c, mv);
            sv.frame(dt);
            let _ = sv.take_reliable(c);
            let _ = sv.take_datagrams(c);
            if let Some(e) = sv.error() {
                return Outcome::Failed(format!("t={:.2}: {}", sv.time(), e.to_string().replace('\n', " | ")));
            }
        }
        Outcome::Ok
    }));
    match r {
        Ok(o) => o,
        Err(p) => {
            let msg = p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()));
            Outcome::Failed(format!("panic: {}", msg.unwrap_or_default()))
        }
    }
}

fn sweep(name: &str, select: impl Fn(&str, &[String]) -> Vec<String>, seconds: f64) {
    let Some(base) = basedir() else {
        eprintln!("QUAKE_BASEDIR not found; skipping");
        return;
    };
    let (mut ok, mut skipped, mut failed) = (0, 0, Vec::new());
    let mut report = String::new();
    for game in gamedirs(&base) {
        let maps = maps_of(&base, &game);
        for map in select(&game, &maps) {
            match run_map(&base, &game, &map, seconds) {
                Outcome::Ok => ok += 1,
                Outcome::Skipped(r) => {
                    skipped += 1;
                    let _ = writeln!(report, "SKIP {game}/{map}: {r}");
                }
                Outcome::Failed(r) => {
                    let _ = writeln!(report, "FAIL {game}/{map}: {r}");
                    failed.push(format!("{game}/{map}"));
                }
            }
        }
    }
    let _ = writeln!(report, "ok {ok}, skipped {skipped}, failed {}", failed.len());
    let out = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/qserver-smoke-{name}.txt"));
    let _ = std::fs::write(&out, &report);
    assert!(failed.is_empty(), "{} maps failed, see {}", failed.len(), out.display());
}

#[test]
fn smoke_fast() {
    // id1 start + e1m1 and the first map of a few mods.
    sweep(
        "fast",
        |game, maps| match game {
            "id1" => maps.iter().filter(|m| *m == "start" || *m == "e1m1").cloned().collect(),
            "hipnotic" | "rogue" | "ad" | "mg1" => maps.iter().filter(|m| !m.starts_with("b_")).take(1).cloned().collect(),
            _ => Vec::new(),
        },
        3.0,
    );
}

#[test]
#[ignore = "slow: every map of every gamedir"]
fn smoke_all() {
    sweep("all", |_, maps| maps.to_vec(), 10.0);
}

#[test]
fn e1m1_state() {
    let Some(base) = basedir() else { return };
    let mut sv = Server::new(make_fs(&base, "id1"), Cvars::new(), 1);
    sv.command("skill 3");
    sv.spawn_server("e1m1").unwrap();
    let c = sv.connect_local().unwrap();
    let rel = sv.take_reliable(c);
    assert!(rel.len() > 1000, "signon stream {}", rel.len());
    let st = sv.stats();
    assert!(st.total_monsters > 0 && st.total_secrets > 0, "{st:?}");
    let l = sv.level.as_ref().unwrap();
    let o = l.vm.field_ofs("origin").unwrap();
    let start = l.vm.ent_v(1, o);
    for _ in 0..144 {
        sv.set_move(c, ClientMove { angles: [0.0, 90.0, 0.0], forward: 320.0, ..Default::default() });
        sv.frame(1.0 / 72.0);
    }
    assert!(sv.error().is_none(), "{:?}", sv.error());
    let d = sv.take_datagrams(c);
    assert!(!d.is_empty() && d.last().unwrap().len() > 20);
    let l = sv.level.as_ref().unwrap();
    let end = l.vm.ent_v(1, o);
    assert!((end[1] - start[1]).abs() > 50.0 || (end[0] - start[0]).abs() > 50.0, "{start:?} -> {end:?}");
    assert!((sv.time() - 3.2).abs() < 0.05, "time {}", sv.time());
}
