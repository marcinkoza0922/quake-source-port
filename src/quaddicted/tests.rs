use super::*;

const SHA: &str = "0057e131aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// `"sha256":"x"` and `u/...` urls are filled in.
fn try_release(json: &str) -> Option<Release> {
    let json = json.replace("\"sha256\":\"x\"", &format!("\"sha256\":\"{SHA}\"")).replace("[\"u/", "[\"https://u/");
    parse_release(&qui::json::parse(&json).unwrap())
}

fn release(json: &str) -> Release {
    try_release(json).unwrap()
}

#[test]
fn rejects_bad_index_fields() {
    let with = |sha: &str, url: &str| try_release(&format!(r#"{{"sha256":"{sha}","urls":["{url}"],"tags":[]}}"#));
    assert!(with(SHA, "https://x/a.zip").is_some() && with(SHA, "http://x/a.zip").is_some());
    for sha in ["", "../../x", "/home/u/x", "0057e131", &"é".repeat(32), &"g".repeat(64)] {
        assert!(with(sha, "https://x/a.zip").is_none(), "{sha}");
    }
    for url in ["-K/tmp/rc", "file:///etc/passwd", "ftp://x/a.zip", ""] {
        assert!(with(SHA, url).is_none(), "{url}");
    }
    let i = release(r#"{"sha256":"x","urls":["u/c.zip"],"tags":[],"install":{"extract":"{base}/"}}"#).install;
    for n in ["../x", "a/../../x", "C:/x", "maps/con.bsp", "maps/nul"] {
        assert_eq!(map_entry(&i, n), None, "{n}");
    }
}

const COTD: &str = r#"{
 "bytes": 1558519,
 "description": "Medium-sized cathedral.<br /><br />The map source is <i>included</i> &amp; more.",
 "install": {"extract": "{base}/id1/maps/"},
 "sha256": "0057e131aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
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
    assert_eq!(missing_mods(&p, &[none], "b"), ["ad"]);
    // a whole mod in its own folder installs as itself
    let r = release(r#"{"sha256":"x","urls":["u/c.zip"],"tags":[],"install":{"extract":"{base}/"}}"#);
    let (p, _) = plan(&r.install, &names(&["copper/progs.dat", "copper/maps/start.bsp", "readme.txt"]), "c");
    let dests: Vec<&str> = p.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(dests, ["copper/progs.dat", "copper/maps/start.bsp", "c/readme.txt"]);
    assert!(missing_mods(&p, &[none], "c").is_empty());
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
        let res = install(r, &zip, &base, &base);
        eprintln!("{stem}: {res:?}");
    }
    assert!(qui::mods::is_game_dir(&base.join("002blue")));
    let m = qui::mods::scan(&base);
    assert!(m.iter().any(|m| m.dir == "002blue" && m.title == "Blue Is The Darkest Shade"), "{m:?}");
    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn a_base_pack_installs_into_its_own_dir() {
    // Quoth itself (pak files under quoth/) is the base others mount, not a map pack
    let r = release(r#"{"sha256":"x","urls":["u/quoth2pt2full_2.zip"],"tags":[],"install":{"extract":"{base}/quoth/"}}"#);
    let (p, base) = plan(&r.install, &names(&["pak0.pak", "pak1.pak", "quoth.cfg"]), "quoth2pt2full_2");
    let dests: Vec<&str> = p.iter().map(|(d, _)| d.as_str()).collect();
    assert_eq!(dests, ["quoth/pak0.pak", "quoth/pak1.pak", "quoth/quoth.cfg"]);
    assert_eq!(base, None);
}

#[test]
fn a_mod_that_needs_quoth_gets_it_as_base() {
    let dir = std::env::temp_dir().join(format!("qsp-quaddicted-base-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // a zip with a mod dir of its own (bluemonday/progs.dat)
    let zip = dir.join("bm.zip");
    std::fs::write(&zip, stored_zip("bluemonday/progs.dat", b"progs")).unwrap();
    let mut r = release(r#"{"sha256":"x","urls":["u/bluemonday_v2.zip"],"tags":["title=Blue Monday Jam","dependency=quoth2pt2full_2"],"install":{"extract":"{base}/"}}"#);
    let base = dir.join("base");
    let mods = dir.join("mods");
    std::fs::create_dir_all(base.join("quoth")).unwrap();
    // not the file the index names: nothing is installed
    assert!(install(&r, &zip, &mods, &base).unwrap_err().contains("checksum"));
    assert!(!mods.exists());
    r.sha = qcore::sha256::hex(&mut std::fs::File::open(&zip).unwrap()).unwrap();
    assert_eq!(install(&r, &zip, &mods, &base).unwrap(), "bluemonday");
    // Installs into the user mods root, leaving basedir untouched.
    assert!(!base.join("bluemonday").exists());
    let info = std::fs::read_to_string(mods.join("bluemonday/modinfo.txt")).unwrap();
    assert!(info.contains("base quoth"), "{info}");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A one-entry zip, stored (no compression).
fn stored_zip(name: &str, data: &[u8]) -> Vec<u8> {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    let crc = !crc;
    let (n, len) = (name.len() as u16, data.len() as u32);
    let mut v = Vec::new();
    let le16 = |v: &mut Vec<u8>, x: u16| v.extend_from_slice(&x.to_le_bytes());
    let le32 = |v: &mut Vec<u8>, x: u32| v.extend_from_slice(&x.to_le_bytes());
    le32(&mut v, 0x0403_4b50);
    for x in [20u16, 0, 0, 0, 0] {
        le16(&mut v, x);
    }
    le32(&mut v, crc);
    le32(&mut v, len);
    le32(&mut v, len);
    le16(&mut v, n);
    le16(&mut v, 0);
    v.extend_from_slice(name.as_bytes());
    v.extend_from_slice(data);
    let cd = v.len() as u32;
    le32(&mut v, 0x0201_4b50);
    for x in [20u16, 20, 0, 0, 0, 0] {
        le16(&mut v, x);
    }
    le32(&mut v, crc);
    le32(&mut v, len);
    le32(&mut v, len);
    for x in [n, 0, 0, 0, 0] {
        le16(&mut v, x);
    }
    le32(&mut v, 0);
    le32(&mut v, 0);
    v.extend_from_slice(name.as_bytes());
    let cd_len = v.len() as u32 - cd;
    le32(&mut v, 0x0605_4b50);
    for x in [0u16, 0, 1, 1] {
        le16(&mut v, x);
    }
    le32(&mut v, cd_len);
    le32(&mut v, cd);
    le16(&mut v, 0);
    v
}

#[test]
fn dependency_names_installed_base() {
    let inst: Vec<String> = ["id1", "ad", "ad_xmas", "quoth", "mymap"].iter().map(|s| s.to_string()).collect();
    let d = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    assert_eq!(dep_base(&d(&["quoth2pt2full_2"]), &inst, "mymap").as_deref(), Some("quoth"));
    assert_eq!(dep_base(&d(&["ad_v1_80p1final"]), &inst, "mymap").as_deref(), Some("ad"));
    assert_eq!(dep_base(&d(&["ad_xmas_v2"]), &inst, "mymap").as_deref(), Some("ad_xmas"));
    assert_eq!(dep_base(&d(&["adamantine"]), &inst, "mymap"), None);
    assert_eq!(dep_base(&d(&["mymap_v2"]), &inst, "mymap"), None);
}
