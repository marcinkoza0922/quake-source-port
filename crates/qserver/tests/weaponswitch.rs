//! Smart weapon auto-switch tests against real id1 progs on e1m1 (skips without game data):
//! verifies that Never and custom Better weapon priorities prevent accidental mid-fire
//! Rocket Launcher pickups from switching or firing a rocket.

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::weaponswitch::{IT_NAILGUN, IT_ROCKET_LAUNCHER, IT_SUPER_SHOTGUN};
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    d.join("id1").is_dir().then_some(d)
}

fn spawn_e1m1(autoswitch: &str, priority: &str) -> Option<(Server, usize)> {
    let base = basedir()?;
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.core.cvars.set("cl_autoswitch", autoswitch);
    sv.core.cvars.set("cl_weaponpriority", priority);
    sv.spawn_server("e1m1").ok()?;
    let c = sv.connect_local()?;
    // Settle a few frames so the player is spawned and grounded.
    for _ in 0..10 {
        sv.set_move(c, ClientMove::default());
        sv.frame(1.0 / 72.0);
    }
    Some((sv, c))
}

fn find_edict_by_classname(sv: &Server, target: &str) -> Option<u32> {
    let l = sv.level.as_ref()?;
    let f = &sv.core.sv.fields;
    (1..l.vm.num_edicts() as u32).find(|&e| {
        !l.vm.is_free(e) && l.vm.ent_str(e, f.classname).ok() == Some(target.as_bytes())
    })
}

/// Gives the player SSG + shells, selects SSG, then places the player directly on
/// `weapon_classname` while holding attack (`button0 = 1`) and steps several frames.
fn pickup_while_firing(autoswitch: &str, priority: &str, weapon_classname: &str) -> Option<(i32, i32, f32)> {
    let (mut sv, c) = spawn_e1m1(autoswitch, priority)?;
    let target_ent = find_edict_by_classname(&sv, weapon_classname)?;
    let target_pos = {
        let l = sv.level.as_mut().unwrap();
        let f = &sv.core.sv.fields;
        // Equip player with Super Shotgun + shells.
        let items = l.vm.ent_f(1, f.items) as i32 | IT_SUPER_SHOTGUN;
        l.vm.set_ent_f(1, f.items, items as f32);
        l.vm.set_ent_f(1, f.ammo_shells, 50.0);
        l.vm.ent_v(target_ent, f.origin)
    };
    // Select SSG via impulse 3 so QC sets weaponmodel/currentammo cleanly.
    sv.set_move(c, ClientMove { impulse: 3, ..Default::default() });
    sv.frame(1.0 / 72.0);
    sv.set_move(c, ClientMove::default());
    sv.frame(1.0 / 72.0);
    assert_eq!(sv.level.as_ref().unwrap().vm.ent_f(1, sv.core.sv.fields.weapon) as i32, IT_SUPER_SHOTGUN);

    // Move player right onto the weapon item (player mins.z = -24, item mins.z = 0) and hold +attack.
    {
        let l = sv.level.as_mut().unwrap();
        let f = &sv.core.sv.fields;
        l.vm.set_ent_v(1, f.origin, [target_pos[0], target_pos[1], target_pos[2] + 24.0]);
    }
    for _ in 0..8 {
        sv.set_move(c, ClientMove { buttons: 1, forward: 200.0, ..Default::default() });
        sv.frame(1.0 / 72.0);
        assert!(sv.error().is_none(), "{:?}", sv.error());
    }
    let l = sv.level.as_ref().unwrap();
    let f = &sv.core.sv.fields;
    let weapon = l.vm.ent_f(1, f.weapon) as i32;
    let items = l.vm.ent_f(1, f.items) as i32;
    let nails = l.vm.ent_f(1, f.ammo_nails);
    Some((weapon, items, nails))
}

#[test]
fn never_mode_keeps_current_weapon_on_pickup_while_collecting_item() {
    let Some((weapon, items, nails)) = pickup_while_firing("0", "8 7 5 6 3 4 2 1", "weapon_nailgun") else {
        return;
    };
    assert_eq!(weapon, IT_SUPER_SHOTGUN, "Never mode must not switch away from SSG");
    assert_ne!(items & IT_NAILGUN, 0, "player must still collect the picked-up weapon");
    assert!(nails > 0.0, "player must still receive the weapon's ammo");
}

#[test]
fn better_weapon_mode_respects_custom_priority_order() {
    // SSG (3) ranked higher than Nailgun (4): should NOT switch.
    let Some((weapon, items, _)) = pickup_while_firing("1", "8 7 5 6 3 4 2 1", "weapon_nailgun") else {
        return;
    };
    assert_eq!(weapon, IT_SUPER_SHOTGUN);
    assert_ne!(items & IT_NAILGUN, 0);

    // Nailgun (4) ranked higher than SSG (3): SHOULD switch to Nailgun!
    let Some((weapon, items, _)) = pickup_while_firing("1", "4 8 7 5 6 3 2 1", "weapon_nailgun") else {
        return;
    };
    assert_eq!(weapon, IT_NAILGUN);
    assert_ne!(items & IT_NAILGUN, 0);
}

#[test]
fn custom_priority_prevents_accidental_rocket_pickup_switch_and_self_fire() {
    let Some(base) = basedir() else { return };
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.core.cvars.set("cl_autoswitch", "1");
    // Put SSG (3) above Rocket Launcher (7).
    sv.core.cvars.set("cl_weaponpriority", "3 5 4 2 6 7 8 1");
    if sv.spawn_server("e1m3").is_err() {
        return;
    }
    let Some(c) = sv.connect_local() else { return };
    for _ in 0..10 {
        sv.set_move(c, ClientMove::default());
        sv.frame(1.0 / 72.0);
    }
    let Some(rl) = find_edict_by_classname(&sv, "weapon_rocketlauncher") else { return };
    let pos = {
        let l = sv.level.as_mut().unwrap();
        let f = &sv.core.sv.fields;
        let items = l.vm.ent_f(1, f.items) as i32 | IT_SUPER_SHOTGUN;
        l.vm.set_ent_f(1, f.items, items as f32);
        l.vm.set_ent_f(1, f.ammo_shells, 50.0);
        l.vm.ent_v(rl, f.origin)
    };
    sv.set_move(c, ClientMove { impulse: 3, ..Default::default() });
    sv.frame(1.0 / 72.0);
    sv.set_move(c, ClientMove::default());
    sv.frame(1.0 / 72.0);

    {
        let l = sv.level.as_mut().unwrap();
        let f = &sv.core.sv.fields;
        l.vm.set_ent_v(1, f.origin, [pos[0], pos[1], pos[2] + 24.0]);
    }
    for _ in 0..8 {
        sv.set_move(c, ClientMove { buttons: 1, forward: 200.0, ..Default::default() });
        sv.frame(1.0 / 72.0);
    }
    let l = sv.level.as_ref().unwrap();
    let f = &sv.core.sv.fields;
    assert_eq!(l.vm.ent_f(1, f.weapon) as i32, IT_SUPER_SHOTGUN, "must stay on SSG when RL has lower priority");
    assert_ne!(l.vm.ent_f(1, f.items) as i32 & IT_ROCKET_LAUNCHER, 0, "must still pick up RL");
    assert_eq!(l.vm.ent_f(1, f.ammo_rockets), 5.0, "must not fire any rocket on pickup!");
}
