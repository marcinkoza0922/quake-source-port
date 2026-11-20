use qcore::{cvar::Cvars, fs::Fs};
use qserver::Server;
use std::{
    fs,
    path::{Path, PathBuf},
};

struct MapDir(PathBuf);

impl Drop for MapDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn setup() -> Option<(Server, MapDir, Vec<u8>)> {
    let base = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
        });
    let mut game = Fs::new(base);
    if !game.add_game_dir("id1") {
        eprintln!("skip: id1 data unavailable");
        return None;
    }
    let bsp = game.open("maps/e1m1.bsp")?;
    let temp = std::env::temp_dir().join(format!("qserver-map-reload-{}", std::process::id()));
    fs::create_dir_all(temp.join("maps")).unwrap();
    let temp = MapDir(temp);
    fs::write(temp.0.join("maps/e1m1.bsp"), &bsp).unwrap();
    game.add_user_dir(&temp.0);
    let mut server = Server::new(game, Cvars::new(), 1);
    server.spawn_server("e1m1").unwrap();
    server.connect_local().unwrap();
    Some((server, temp, bsp))
}

#[test]
fn map_reload_preserves_live_player_and_rejects_broken_bsp() {
    let Some((mut server, temp, bsp)) = setup() else {
        return;
    };
    server.command("noclip 1");
    server.command("setpos 128 -224 80 12 90 0");
    let f = server.core.sv.fields.clone();
    let inventory = [
        (f.items, 131107.0),
        (f.ammo_shells, 61.0),
        (f.armorvalue, 45.0),
        (f.health, 79.0),
    ];
    for (field, value) in inventory {
        server.level.as_mut().unwrap().vm.set_ent_f(1, field, value);
    }
    let spawns = server.core.spawns;
    let path = temp.0.join("maps/e1m1.bsp");
    fs::write(&path, b"truncated BSP").unwrap();
    server.command("map_reload");
    assert!(server.active() && server.error().is_none());
    assert_eq!(server.core.spawns, spawns);

    // Replace the BSP from disk and override its entities through the normal search path.
    fs::write(&path, bsp).unwrap();
    let ent = b"{\"classname\" \"worldspawn\" \"message\" \"Reloaded\"}\n{\"classname\" \"info_player_start\" \"origin\" \"480 -352 88\"}\n";
    fs::write(temp.0.join("maps/e1m1.ent"), ent).unwrap();
    server.command("map_reload");
    assert!(server.active() && server.error().is_none());
    assert_eq!(server.core.spawns, spawns + 1);
    let vm = &server.level.as_ref().unwrap().vm;
    assert_eq!(
        vm.ent_str(0, vm.field_ofs("message").unwrap()).unwrap(),
        b"Reloaded"
    );
    assert_eq!(vm.ent_v(1, f.origin), [128.0, -224.0, 80.0]);
    assert_eq!(vm.ent_v(1, f.v_angle), [12.0, 90.0, 0.0]);
    assert_eq!(vm.ent_f(1, f.movetype) as i32, qworld::MOVETYPE_NOCLIP);
    for (field, value) in inventory {
        assert_eq!(vm.ent_f(1, field), value);
    }
}

#[test]
fn map_reload_watch_debounces_and_recovers_after_a_failed_edit() {
    let Some((mut server, temp, bsp)) = setup() else {
        return;
    };
    server.command("map_reload watch");
    server.poll_map_reload(0.0);
    let spawns = server.core.spawns;
    let ent = temp.0.join("maps/e1m1.ent");
    fs::write(&ent, b"{").unwrap();
    server.poll_map_reload(0.25);
    server.poll_map_reload(0.50);
    assert!(
        server
            .core
            .log
            .iter()
            .all(|s| !s.contains("couldn't reload"))
    );
    server.poll_map_reload(0.75);
    assert!(
        server
            .core
            .log
            .iter()
            .any(|s| s.contains("couldn't reload"))
    );
    let logs = server.core.log.len();
    server.poll_map_reload(1.0);
    assert_eq!(
        server.core.log.len(),
        logs,
        "the bad edit should only be tried once"
    );
    assert!(server.active() && server.error().is_none());
    assert_eq!(server.core.spawns, spawns);

    fs::remove_file(ent).unwrap();
    let new_bsp = temp.0.join("maps/compile.bsp");
    fs::write(&new_bsp, bsp).unwrap();
    fs::rename(new_bsp, temp.0.join("maps/e1m1.bsp")).unwrap();
    server.poll_map_reload(1.25);
    server.poll_map_reload(1.75);
    assert_eq!(server.core.spawns, spawns + 1);
    server.command("map_reload off");
    fs::write(temp.0.join("maps/e1m1.ent"), b"{").unwrap();
    server.poll_map_reload(2.0);
    server.poll_map_reload(3.0);
    assert_eq!(server.core.spawns, spawns + 1);
}
