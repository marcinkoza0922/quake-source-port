use super::*;

fn base() -> PathBuf {
    std::env::var_os("QUAKE_BASEDIR")
        .map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"), PathBuf::from)
}

#[test]
fn report_json_escapes_strings() {
    let r = bot::Report { failed: vec!["Button custom\"button\n".into()], ..Default::default() };
    let j = qui::json::parse(&report_json(&r)).unwrap();
    let Some(qui::json::Value::Arr(f)) = j.get("failed") else { panic!("expected failed array") };
    assert_eq!(f[0], qui::json::Value::Str("Button custom\"button\n".into()));
}

#[test]
fn baselines_keep_known_errors_and_drop_long_logs() {
    let e = |log: Option<&[String]>| qui::json::parse(&baseline_entry(1.0, 2.0, 3.0, true, &["boom".into()], log)).unwrap();
    let kept = e(Some(&["line".into()]));
    assert_eq!(baseline_errors(&kept), ["boom"]);
    assert_eq!(baseline_log(&kept), Some(vec!["line".to_string()]));
    assert_eq!(baseline_log(&e(None)), None);
    // entries from before logs were kept compare against none
    assert_eq!(baseline_log(&qui::json::parse("{\"kills\": 1}").unwrap()), Some(Vec::new()));
    assert!(flagged().count() > 50 && flagged().any(|g| g == "mjolnir"));
}

/// e1m1 and e1m3 at skill 3: everything killed, found and triggered, and
/// the exit taken.
#[test]
fn breaks_episode_one_maps() {
    let base = base();
    let fs = mount(&base, "id1");
    if fs.open("progs.dat").is_none() {
        eprintln!("no game data; skipping");
        return;
    }
    let opt = bot::Options { skill: 3, max_time: None, max_wall: None, verbose: false };
    for map in ["e1m1", "e1m3"] {
        let r = bot::run(mount(&base, "id1"), map, &opt);
        assert!(r.errors.is_empty(), "{map}: {:?}", r.errors);
        assert_eq!(r.kills, r.total_monsters, "{map}: kills; failed {:?}", r.failed);
        assert_eq!(r.secrets, r.total_secrets, "{map}: secrets; failed {:?}", r.failed);
        assert!(r.exited, "{map}: never reached the exit");
    }
}

/// A baseline that can't be read, or maps that weren't run, are not a pass.
#[test]
fn missing_maps_and_broken_baselines_are_noticed() {
    let dir = std::env::temp_dir().join(format!("qbot-baseline-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("b.json");
    assert!(load_baseline(&path).is_err());
    std::fs::write(&path, "{ not json").unwrap();
    assert!(load_baseline(&path).is_err());
    let entry = baseline_entry(0.0, 0.0, 0.0, false, &[], Some(&[]));
    std::fs::write(&path, format!("{{\"e1m1\": {entry}, \"e1m2\": {entry}}}")).unwrap();
    let o = |maps: &[&str]| Opts {
        basedir: PathBuf::new(),
        game: "id1".into(),
        skill: 3,
        time: None,
        max_wall: None,
        jobs: 1,
        out: dir.clone(),
        baseline: PathBuf::new(),
        save_baseline: false,
        verbose: false,
        maps: maps.iter().map(|m| m.to_string()).collect(),
    };
    let r = bot::Report::default();
    let run = |o: &Opts, maps: &[&str]| {
        let res: Vec<(String, &bot::Report)> = maps.iter().map(|m| (m.to_string(), &r)).collect();
        report_game(o, "id1", false, &dir, &path, &res)
    };
    assert!(!run(&o(&[]), &["e1m1", "e1m2"]));
    assert!(run(&o(&[]), &["e1m1"]), "e1m2 disappeared");
    assert!(!run(&o(&["e1m1"]), &["e1m1"]), "asked for one map");
    let _ = std::fs::remove_dir_all(&dir);
}
