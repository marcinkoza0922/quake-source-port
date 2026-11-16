use super::*;

fn release(json: &str) -> Release {
    parse_release(&qui::json::parse(json).unwrap()).unwrap()
}

const COTD: &str = r#"{
 "bytes": 1558519,
 "description": "Medium-sized cathedral.<br /><br />The map source is <i>included</i> &amp; more.",
 "install": {"extract": "{base}/id1/maps/"},
 "sha256": "0057e131",
 "tags": ["author=Rastrelly", "filename=cotdq1sp.zip", "game_mode=singleplayer",
          "release_date=2021-07-12", "startmap=cotdq1sp", "title=Cathedral of the Doubt", "type=map"],
 "urls": ["https://www.quaddicted.com/filebase/cotdq1sp.zip"]
}"#;

fn names(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

#[test]
fn parses_release_tags() {
    let r = release(COTD);
    assert_eq!(r.title, "Cathedral of the Doubt");
    assert_eq!(r.author, "Rastrelly");
    assert_eq!(r.date, "2021-07-12");
    assert_eq!(r.stem, "cotdq1sp");
    assert_eq!(r.startmaps, ["cotdq1sp"]);
    assert!(r.singleplayer);
    assert_eq!(r.description, "Medium-sized cathedral. The map source is included & more.");
    assert_eq!(r.install.extract.as_deref(), Some("id1/maps"));
}

#[test]
fn id1_maps_go_to_their_own_dir() {
    let r = release(COTD);
    let (p, base) = plan(&r.install, &names(&["cotdq1sp.bsp", "cotdq1.txt"]), "cotdq1sp");
    assert_eq!(p[0].0, "cotdq1sp/maps/cotdq1sp.bsp");
    assert_eq!(p[1].0, "cotdq1sp/maps/cotdq1.txt");
    assert_eq!(base, None);
}

#[test]
fn mapping_beats_extract_and_can_skip() {
    let r = release(
        r#"{"sha256":"x","urls":["u/a.zip"],"tags":[],
        "install":{"extract":"{base}/id1/maps/","extractmapping":{"/":"/quoth/maps/","/music/":"/quoth/music/","/junk.pts":null}}}"#,
    );
    let (p, base) = plan(&r.install, &names(&["e.bsp", "music/track2.ogg", "junk.pts"]), "a");
    let dests: Vec<&str> = p.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(dests, ["a/maps/e.bsp", "a/music/track2.ogg"]);
    assert_eq!(base, Some("quoth"));
}

#[test]
fn mod_content_stays_in_the_mod_and_needs_it() {
    let r = release(r#"{"sha256":"x","urls":["u/b.zip"],"tags":[],"install":{"extract":"{base}/ad/maps/"}}"#);
    let (p, _) = plan(&r.install, &names(&["b.bsp"]), "b");
    assert_eq!(p[0].0, "ad/maps/b.bsp");
    let none = std::path::Path::new("/nonexistent-basedir");
    assert_eq!(missing_mods(&p, none, "b"), ["ad"]);
    // a whole mod in its own folder installs as itself
    let r = release(r#"{"sha256":"x","urls":["u/c.zip"],"tags":[],"install":{"extract":"{base}/"}}"#);
    let (p, _) = plan(&r.install, &names(&["copper/progs.dat", "copper/maps/start.bsp", "readme.txt"]), "c");
    let dests: Vec<&str> = p.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(dests, ["copper/progs.dat", "copper/maps/start.bsp", "c/readme.txt"]);
    assert!(missing_mods(&p, none, "c").is_empty());
}

#[test]
fn rejects_escaping_paths() {
    let i = Install { extract: Some("id1".into()), mapping: vec![] };
    assert_eq!(map_entry(&i, "../../etc/passwd"), None);
    assert_eq!(map_entry(&i, "maps/./x.bsp").as_deref(), Some("id1/maps/x.bsp"));
}

#[test]
fn installed_list_round_trips() {
    let dir = std::env::temp_dir().join(format!("qsp-qd-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("installed.txt");
    let m: HashMap<String, String> = [("abc".to_string(), "cotdq1sp".to_string())].into();
    save_installed(&f, &m);
    assert_eq!(load_installed(&f), m);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Parses a real index zip when `QSP_QUADDICTED_INDEX` points at one.
#[test]
fn real_index_parses() {
    let Some(p) = std::env::var_os("QSP_QUADDICTED_INDEX") else { return };
    let v = load_index(std::path::Path::new(&p)).expect("index");
    assert!(v.len() > 1000, "{}", v.len());
    assert!(v.iter().all(|r| !r.url.is_empty() && !r.stem.is_empty()));
}

/// Live download + install of a small release into a temp basedir, when
/// `QSP_QUADDICTED_INDEX` names an index zip and `QSP_QUADDICTED_LIVE` is set.
#[test]
fn live_install() {
    let (Some(p), Some(_)) = (std::env::var_os("QSP_QUADDICTED_INDEX"), std::env::var_os("QSP_QUADDICTED_LIVE")) else { return };
    let v = load_index(std::path::Path::new(&p)).unwrap();
    let base = std::env::temp_dir().join(format!("qsp-qd-live-{}", std::process::id()));
    for stem in ["002blue", "dm7sp"] {
        let r = v.iter().find(|r| r.stem == stem).expect(stem);
        let zip = base.join(format!("{stem}.zip"));
        std::fs::create_dir_all(&base).unwrap();
        let ok = std::process::Command::new("curl").args(["-fsSL", "-o"]).arg(&zip).arg(&r.url).status().unwrap();
        assert!(ok.success());
        let res = install(r, &zip, &base);
        eprintln!("{stem}: {res:?}");
    }
    assert!(qui::mods::is_game_dir(&base.join("002blue")));
    let m = qui::mods::scan(&base);
    assert!(m.iter().any(|m| m.dir == "002blue" && m.title == "Blue Is The Darkest Shade"), "{m:?}");
    let _ = std::fs::remove_dir_all(&base);
}
