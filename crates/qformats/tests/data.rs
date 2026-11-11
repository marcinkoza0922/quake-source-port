//! Tests against real game data; skipped when QUAKE_BASEDIR is absent.

use qformats::{Bsp, Mdl, Spr, Wad, Wav, bsp, entities, image};
use std::path::{Path, PathBuf};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
        });
    d.join("id1").is_dir().then_some(d)
}

/// Tiny PAK reader: (name, data) for every entry.
fn read_pak(path: &Path) -> Vec<(String, Vec<u8>)> {
    let d = std::fs::read(path).unwrap();
    assert_eq!(&d[0..4], b"PACK", "{}", path.display());
    let le = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap()) as usize;
    let (ofs, len) = (le(4), le(8));
    (0..len / 64)
        .map(|i| {
            let e = ofs + i * 64;
            let name = qformats::cstr(&d[e..e + 56]).to_ascii_lowercase();
            let (p, l) = (le(e + 56), le(e + 60));
            (name, d[p..p + l].to_vec())
        })
        .collect()
}

/// All files from paks + loose files of a gamedir.
fn game_files(game: &str) -> Vec<(String, Vec<u8>)> {
    let Some(base) = basedir() else {
        return Vec::new();
    };
    let dir = base.join(game);
    let mut out = Vec::new();
    for i in 0..10 {
        let p = dir.join(format!("pak{i}.pak"));
        if p.exists() {
            out.extend(read_pak(&p));
        }
    }
    if let Ok(rd) = std::fs::read_dir(dir.join("maps")) {
        for e in rd.flatten() {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            out.push((format!("maps/{n}"), std::fs::read(e.path()).unwrap()));
        }
    }
    out
}

fn check_bsp(name: &str, d: &[u8]) {
    let b = Bsp::parse(d).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert!(!b.models.is_empty(), "{name}");
    let ents = entities::parse_entities(&b.entities).unwrap_or_else(|e| panic!("{name}: {e}"));
    assert_eq!(ents[0].classname(), Some("worldspawn"), "{name}");
    for f in &b.faces {
        assert!((f.texinfo as usize) < b.texinfo.len(), "{name}");
    }
    let row = b.leaf_pvs(1);
    assert_eq!(row.len(), (b.models[0].visleafs as usize).div_ceil(8));
}

fn check_game_maps(game: &str) {
    let mut n = 0;
    for (name, d) in game_files(game) {
        if name.ends_with(".bsp") && name.starts_with("maps/") {
            check_bsp(&name, &d);
            n += 1;
        }
    }
    if basedir().is_some_and(|b| b.join(game).is_dir()) {
        assert!(n > 0, "{game}: no maps found");
    }
}

#[test]
fn maps_id1() {
    check_game_maps("id1");
}
#[test]
fn maps_ad() {
    check_game_maps("ad");
}
#[test]
fn maps_mg1() {
    check_game_maps("mg1");
}
#[test]
fn maps_mg3() {
    check_game_maps("mg3");
}
#[test]
fn maps_hipnotic() {
    check_game_maps("hipnotic");
}
#[test]
fn maps_rogue() {
    check_game_maps("rogue");
}

#[test]
fn id1_assets() {
    let files = game_files("id1");
    if files.is_empty() {
        return;
    }
    let (mut mdl, mut spr, mut wav) = (0, 0, 0);
    for (name, d) in &files {
        if name.ends_with(".mdl") {
            let m = Mdl::parse(d).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!m.frames.is_empty() && !m.skins.is_empty(), "{name}");
            mdl += 1;
        } else if name.ends_with(".spr") {
            Spr::parse(d).unwrap_or_else(|e| panic!("{name}: {e}"));
            spr += 1;
        } else if name.ends_with(".wav") {
            let w = Wav::parse(d).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(w.rate > 0);
            wav += 1;
        }
    }
    assert!(mdl > 30 && spr > 0 && wav > 50, "{mdl} {spr} {wav}");
    let get = |n: &str| &files.iter().find(|(f, _)| f == n).unwrap().1;
    let player = Mdl::parse(get("progs/player.mdl")).unwrap();
    assert!(player.frames.len() > 100);
    let flame = Mdl::parse(get("progs/flame2.mdl")).unwrap();
    assert!(
        flame.frames.iter().any(|f| f.poses.len() > 1),
        "frame groups"
    );
    let rocket = Mdl::parse(get("progs/missile.mdl")).unwrap();
    assert_eq!(
        rocket.flags & qformats::mdl::EF_ROCKET,
        qformats::mdl::EF_ROCKET
    );
    let ambience = Wav::parse(get("sound/ambience/fire1.wav")).unwrap();
    assert!(ambience.loop_start.is_some());
    let wad = Wad::parse(get("gfx.wad")).unwrap();
    let conchars = wad.get("CONCHARS").unwrap();
    assert_eq!(conchars.data.len(), 128 * 128);
    let sbar = image::parse_pic(&wad.get("sbar").unwrap().data).unwrap();
    assert_eq!((sbar.width, sbar.height), (320, 24));
    let pal = image::parse_palette(get("gfx/palette.lmp")).unwrap();
    assert_eq!(pal[0], [0, 0, 0]);
    image::parse_colormap(get("gfx/colormap.lmp")).unwrap();
    let conback = image::parse_pic(get("gfx/conback.lmp")).unwrap();
    assert_eq!((conback.width, conback.height), (320, 200));
    let start = Bsp::parse(get("maps/start.bsp")).unwrap();
    assert!(
        start
            .textures
            .iter()
            .flatten()
            .any(|t| t.kind == bsp::TextureKind::Sky)
    );
    assert!(
        start
            .textures
            .iter()
            .flatten()
            .any(|t| t.kind == bsp::TextureKind::Liquid)
    );
}

#[test]
fn bsp2_present() {
    // At least one BSP2/2PSB map among the mods, if installed.
    let mut found = false;
    for g in ["ad", "mg1", "mg3"] {
        for (name, d) in game_files(g) {
            if name.ends_with(".bsp") && (d.starts_with(b"BSP2") || d.starts_with(b"2PSB")) {
                found = true;
            }
        }
    }
    if basedir().is_some_and(|b| b.join("ad").is_dir()) {
        assert!(found);
    }
}
