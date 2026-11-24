use super::*;

fn tmpdir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("qcore-fs-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write_pak(path: &Path, files: &[(&str, &[u8])]) {
    let mut data = Vec::new();
    let mut dir = Vec::new();
    let mut ofs = 12;
    for (n, b) in files {
        data.extend_from_slice(b);
        let mut name = [0u8; 56];
        name[..n.len()].copy_from_slice(n.as_bytes());
        dir.extend_from_slice(&name);
        dir.extend_from_slice(&(ofs as i32).to_le_bytes());
        dir.extend_from_slice(&(b.len() as i32).to_le_bytes());
        ofs += b.len();
    }
    let mut out = b"PACK".to_vec();
    out.extend_from_slice(&(ofs as i32).to_le_bytes());
    out.extend_from_slice(&(dir.len() as i32).to_le_bytes());
    out.extend(data);
    out.extend(dir);
    std::fs::write(path, out).unwrap();
}

fn write_zip(path: &Path, files: &[(&str, &[u8], bool)]) {
    let (mut out, mut cd) = (Vec::new(), Vec::new());
    for (n, b, deflate) in files {
        let comp = if *deflate { miniz_oxide::deflate::compress_to_vec(b, 6) } else { b.to_vec() };
        let method: u16 = if *deflate { 8 } else { 0 };
        let local = out.len() as u32;
        let common = |v: &mut Vec<u8>| {
            v.extend_from_slice(&[20, 0, 0, 0]);
            v.extend_from_slice(&method.to_le_bytes());
            v.extend_from_slice(&[0; 8]); // time, date, crc (unchecked)
            v.extend_from_slice(&(comp.len() as u32).to_le_bytes());
            v.extend_from_slice(&(b.len() as u32).to_le_bytes());
            v.extend_from_slice(&(n.len() as u16).to_le_bytes());
            v.extend_from_slice(&[0, 0]);
        };
        out.extend_from_slice(b"PK\x03\x04");
        common(&mut out);
        out.extend_from_slice(n.as_bytes());
        out.extend_from_slice(&comp);
        cd.extend_from_slice(b"PK\x01\x02\x14\x00");
        common(&mut cd);
        cd.extend_from_slice(&[0; 10]);
        cd.extend_from_slice(&local.to_le_bytes());
        cd.extend_from_slice(n.as_bytes());
    }
    let cd_ofs = out.len() as u32;
    out.extend_from_slice(&cd);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd_ofs.to_le_bytes());
    out.extend_from_slice(&[0, 0]);
    std::fs::write(path, out).unwrap();
}

#[test]
fn loose_case_insensitive_fallback() {
    let b = tmpdir("case");
    std::fs::create_dir_all(b.join("id1/progs")).unwrap();
    std::fs::write(b.join("id1/progs/demonplush.mdl"), "m").unwrap();
    let mut fs = Fs::new(&b);
    assert!(fs.add_game_dir("id1"));
    assert_eq!(fs.open("progs/DEMONPLUSH.mdl").as_deref(), Some(&b"m"[..]));
    assert_eq!(fs.open("PROGS/demonplush.MDL").as_deref(), Some(&b"m"[..]));
    assert!(fs.open("progs/missing.mdl").is_none());
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn leading_slash_names_a_loose_file_inside_the_game_dir() {
    let b = tmpdir("slash");
    std::fs::create_dir_all(b.join("id1/progs")).unwrap();
    std::fs::write(b.join("id1/progs/smoke1.spr"), "s").unwrap();
    let mut fs = Fs::new(&b);
    fs.add_game_dir("id1");
    assert_eq!(fs.open("/progs/smoke1.spr").as_deref(), Some(&b"s"[..]));
    // an absolute path never leaves the game dir
    assert!(fs.open(b.join("id1/progs/smoke1.spr").to_str().unwrap()).is_none());
    assert!(fs.open("/../id1/progs/smoke1.spr").is_none());
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn trailing_spaces_are_a_last_resort_for_loose_files() {
    let b = tmpdir("spaces");
    for g in ["id1", "mod"] {
        std::fs::create_dir_all(b.join(g).join("progs")).unwrap();
    }
    std::fs::write(b.join("id1/progs/both.mdl "), "spaced").unwrap();
    std::fs::write(b.join("mod/progs/both.mdl"), "plain").unwrap();
    std::fs::write(b.join("mod/progs/pumpkin.mdl"), "p").unwrap();
    write_pak(&b.join("mod/pak0.pak"), &[("progs/packed.mdl", b"k")]);
    let mut fs = Fs::new(&b);
    fs.add_game_dir("id1");
    fs.add_game_dir("mod");
    assert_eq!(fs.open("progs/pumpkin.mdl ").as_deref(), Some(&b"p"[..]));
    assert_eq!(fs.open("progs/PUMPKIN.mdl  ").as_deref(), Some(&b"p"[..]));
    assert!(fs.exists("progs/pumpkin.mdl "));
    // the name as written wins, even from a lower-priority game dir
    assert_eq!(fs.open("progs/both.mdl ").as_deref(), Some(&b"spaced"[..]));
    assert_eq!(fs.open("progs/both.mdl").as_deref(), Some(&b"plain"[..]));
    // C compares pak names exactly, also on Windows
    assert!(fs.open("progs/packed.mdl ").is_none());
    // nothing but spaces, a directory, and leading or inner spaces stay missing
    for n in [" ", "progs/ ", "progs ", " progs/pumpkin.mdl", "progs/pumpkin .mdl", "progs/../x "] {
        assert!(fs.open(n).is_none(), "{n:?}");
    }
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn companion_needs_same_or_higher_priority() {
    let b = tmpdir("companion");
    std::fs::create_dir_all(b.join("id1/maps")).unwrap();
    std::fs::create_dir_all(b.join("q64/maps")).unwrap();
    std::fs::create_dir_all(b.join("lit/maps")).unwrap();
    std::fs::write(b.join("id1/maps/start.bsp"), "id").unwrap();
    std::fs::write(b.join("id1/maps/start.lit"), "idlit").unwrap();
    std::fs::write(b.join("q64/maps/start.bsp"), "q").unwrap();
    let mut fs = Fs::new(&b);
    fs.add_game_dir("id1");
    assert_eq!(fs.open_companion("maps/start.bsp", "maps/start.lit").as_deref(), Some(&b"idlit"[..]));
    fs.add_game_dir("q64");
    assert!(fs.open_companion("maps/start.bsp", "maps/start.lit").is_none());
    std::fs::write(b.join("lit/maps/start.lit"), "modlit").unwrap();
    fs.add_game_dir("lit");
    assert_eq!(fs.open_companion("maps/start.bsp", "maps/start.lit").as_deref(), Some(&b"modlit"[..]));
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn search_order() {
    let b = tmpdir("order");
    std::fs::create_dir_all(b.join("id1/maps")).unwrap();
    std::fs::create_dir_all(b.join("mod")).unwrap();
    std::fs::write(b.join("id1/a.txt"), "loose").unwrap();
    std::fs::write(b.join("id1/maps/x.bsp"), "x").unwrap();
    write_pak(&b.join("id1/pak0.pak"), &[("a.txt", b"pak0"), ("b.txt", b"pak0"), ("maps/start.bsp", b"s")]);
    write_pak(&b.join("id1/pak1.pak"), &[("b.txt", b"pak1")]);
    write_pak(&b.join("id1/pak3.pak"), &[("b.txt", b"pak3")]); // gap: not loaded
    let big: Vec<u8> = (0..5000u32).map(|i| (i % 7) as u8).collect();
    write_zip(&b.join("mod/z.pk3"), &[("c.txt", b"zip", false), ("Big.dat", &big, true), ("dir/", b"", false)]);
    std::fs::write(b.join("mod/b.txt"), "modloose").unwrap();

    let mut fs = Fs::new(&b);
    assert!(fs.add_game_dir("id1"));
    assert!(!fs.add_game_dir("nope"));
    assert_eq!(fs.open("a.txt").unwrap(), b"pak0"); // pak beats loose in same dir
    assert_eq!(fs.open("b.txt").unwrap(), b"pak1");
    assert!(fs.open("../x").is_none());
    assert!(fs.add_game_dir("mod"));
    assert_eq!(fs.open("b.txt").unwrap(), b"modloose"); // later gamedir wins
    assert_eq!(fs.open("c.txt").unwrap(), b"zip");
    assert_eq!(fs.open("big.dat").unwrap(), big);
    assert_eq!(fs.origin("a.txt").unwrap().game, "id1");
    assert_eq!(fs.list("maps/", ".bsp"), vec!["maps/start.bsp", "maps/x.bsp"]);
    fs.truncate_game_dirs(1);
    assert_eq!(fs.open("b.txt").unwrap(), b"pak1");
    let _ = std::fs::remove_dir_all(&b);
}

#[test]
fn flat_paks_mount_as_id1() {
    let b = tmpdir("flat");
    write_pak(&b.join("pak0.pak"), &[("progs.dat", b"flat")]);
    let mut fs = Fs::new(&b);
    assert!(fs.add_game_dir("id1"));
    assert_eq!(fs.game_dir(), Some("id1"));
    assert_eq!(fs.open("progs.dat").as_deref(), Some(&b"flat"[..]));
    let _ = std::fs::remove_dir_all(b);
}

#[test]
fn mod_roots_and_replace_user_dir() {
    let b = tmpdir("modroots");
    let (base, mods, u1, u2) = (b.join("base"), b.join("mods"), b.join("u1"), b.join("u2"));
    for d in [base.join("id1"), base.join("ad/maps"), mods.join("ad/maps"), mods.join("pack/maps"), u1.clone(), u2.clone()] {
        std::fs::create_dir_all(d).unwrap();
    }
    std::fs::write(base.join("ad/maps/base.bsp"), "b").unwrap();
    std::fs::write(mods.join("ad/maps/extra.bsp"), "e").unwrap();
    std::fs::write(mods.join("pack/maps/p.bsp"), "p").unwrap();
    std::fs::write(u1.join("config.cfg"), "u1").unwrap();
    std::fs::write(u2.join("config.cfg"), "u2").unwrap();
    let mut fs = Fs::new(&base);
    fs.add_mod_root(&mods);
    assert!(fs.add_game_dir("id1") && fs.add_game_dir("ad") && fs.add_game_dir("pack"));
    assert_eq!(fs.open("maps/base.bsp").as_deref(), Some(&b"b"[..]));
    assert_eq!(fs.open("maps/extra.bsp").as_deref(), Some(&b"e"[..]));
    assert_eq!(fs.open("maps/p.bsp").as_deref(), Some(&b"p"[..]));
    fs.add_user_dir(&u1);
    assert_eq!(fs.open("config.cfg").as_deref(), Some(&b"u1"[..]));
    fs.replace_user_dir(&u2);
    assert_eq!(fs.open("config.cfg").as_deref(), Some(&b"u2"[..]));
    let _ = std::fs::remove_dir_all(b);
}

#[test]
fn real_data() {
    let mut base = Fs::default_basedir();
    if base.is_relative() && !base.exists() {
        base = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(base);
    }
    if !base.join("id1").is_dir() {
        eprintln!("skip: no game data");
        return;
    }
    let mut fs = Fs::new(&base);
    assert!(fs.add_game_dir("id1"));
    let progs = fs.open("progs.dat").unwrap();
    assert_eq!(i32::from_le_bytes(progs[..4].try_into().unwrap()), 6);
    let bsp = fs.open("maps/start.bsp").unwrap();
    assert_eq!(i32::from_le_bytes(bsp[..4].try_into().unwrap()), 29);
    assert!(fs.list("maps/", ".bsp").iter().any(|m| m == "maps/e1m1.bsp"));
    if fs.add_game_dir("ad") {
        assert!(fs.list("maps/", ".bsp").len() > 10);
    }
}
