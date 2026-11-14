use super::*;
use qcore::{cvar::Cvars, fs::Fs};
use qworld::MoveType;
use crate::{Server, host::Ctx};

fn server() -> Option<Server> {
    let base = Fs::default_basedir();
    if !base.join("id1").is_dir() { return None; }
    let mut fs = Fs::new(base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.spawn_server("e1m1").unwrap();
    sv.connect_local().unwrap();
    sv.core.set_addons(true);
    Some(sv)
}

fn fire(sv: &mut Server, owner: u32, count: f32, dir: Vec3) -> Vec<u32> {
    let l = sv.level.as_mut().unwrap();
    l.vm.set_self(owner);
    l.vm.set_g_f(ofs_parm(0), count);
    l.vm.set_g_v(ofs_parm(1), dir.to_array());
    l.vm.set_g_v(ofs_parm(2), [0.0; 3]);
    l.vm.execute_by_name(&mut Ctx::new(&mut sv.core, &mut l.world), "FireBullets").unwrap();
    (1..l.vm.num_edicts() as u32).filter(|&e| !l.vm.is_free(e)
        && l.vm.ent_str(e, sv.core.sv.fields.classname).ok() == Some(CLASS)).collect()
}

#[test]
fn qc_pellets_defer_damage_sweep_at_configured_speed_and_save() {
    let Some(mut sv) = server() else { return };
    let l = sv.level.as_mut().unwrap();
    let f = sv.core.sv.fields.clone();
    let spawn = (1..l.vm.num_edicts() as u32).find(|&e| l.vm.classname(e) == "info_player_start").unwrap();
    let origin = Vec3::from_array(l.vm.ent_v(spawn, f.origin));
    let source = origin + Vec3::new(10.0, 0.0, 15.2);
    let dirs = [Vec3::new(1.0,0.0,0.0), Vec3::new(-1.0,0.0,0.0), Vec3::new(0.0,1.0,0.0), Vec3::new(0.0,-1.0,0.0)];
    let dir = dirs.into_iter().find(|&d| {
        let tr = l.world.trace(&Ents { vm: &mut l.vm, c: &mut sv.core }, source, Vec3::ZERO, Vec3::ZERO,
            source + d * 180.0, MoveType::Normal, Some(1));
        tr.fraction == 1.0 && !tr.startsolid
    }).expect("spawn room has an open firing line");
    l.vm.set_ent_v(1, f.origin, origin.to_array());
    l.vm.set_ent_v(1, f.absmin, (origin - Vec3::new(16.0,16.0,24.0)).to_array());
    l.vm.set_ent_v(1, f.size, [32.0,32.0,56.0]);
    l.vm.set_ent_v(1, f.v_angle, [0.0; 3]);
    let target = l.vm.ed_alloc().unwrap();
    let null = l.vm.progs.find_function("SUB_Null").unwrap() as i32;
    for name in ["th_pain", "th_die"] {
        let field = l.vm.field_ofs(name).unwrap();
        l.vm.set_ent_i(target, field, null);
    }
    l.vm.set_ent_f(target, f.health, 100.0);
    l.vm.set_ent_f(target, f.takedamage, 2.0);
    l.vm.set_ent_f(target, f.flags, 32.0);
    l.vm.set_ent_f(target, f.solid, 2.0);
    l.vm.set_ent_v(target, f.origin, (source + dir * 128.0).to_array());
    l.vm.set_ent_v(target, f.mins, [-12.0; 3]);
    l.vm.set_ent_v(target, f.maxs, [12.0; 3]);
    l.world.link_edict(&mut Ents { vm: &mut l.vm, c: &mut sv.core }, target, false);
    let pellets = fire(&mut sv, 1, 1.0, dir);
    assert_eq!(pellets.len(), 1);
    let pellet = pellets[0];
    let l = sv.level.as_ref().unwrap();
    assert_eq!(l.vm.ent_f(target, f.health), 100.0, "no hitscan damage");
    assert!((Vec3::from_array(l.vm.ent_v(pellet, f.velocity)).length() - 10000.0).abs() < 0.1);
    sv.core.cvars.set("addon_shotgun_speed", "30000");
    sv.core.cvars.set("addon_bullet_time_duration", "12");
    let save = sv.save_game();
    sv.core.cvars.set("addon_projectile_shotguns", "0");
    sv.core.cvars.set("addon_bullet_time_duration", "2");
    sv.load_game(&save).unwrap();
    assert_eq!(sv.core.cvars.value_of("addon_projectile_shotguns"), 1.0);
    assert_eq!(sv.core.cvars.value_of("addon_shotgun_speed"), 30000.0);
    assert_eq!(sv.core.cvars.value_of("addon_bullet_time_duration"), 12.0);
    assert_eq!(sv.level.as_ref().unwrap().vm.classname(pellet), "qsp_shotgun_pellet");
    // Run the actual server physics: 1000 units/tick must hit the thin target
    // 128 units away, without sv_maxvelocity clamping the pellet to 2000.
    sv.physics(0.005);
    let l = sv.level.as_ref().unwrap();
    assert!(!l.vm.is_free(pellet));
    assert!((Vec3::from_array(l.vm.ent_v(pellet, f.velocity)).length() - 10000.0).abs() < 0.1);
    sv.physics(0.04);
    let l = sv.level.as_ref().unwrap();
    assert!(l.vm.is_free(pellet));
    assert_eq!(l.vm.ent_f(target, f.health), 96.0);
    assert!(sv.error().is_none());
}

#[test]
fn player_and_grunt_counts_speed_and_disabled_fallback() {
    let Some(mut sv) = server() else { return };
    let f = sv.core.sv.fields.clone();
    let dir = Vec3::new(1.0,0.0,0.0);
    sv.core.cvars.set("addon_projectile_shotguns", "0");
    assert!(fire(&mut sv, 1, 6.0, dir).is_empty());
    sv.core.cvars.set("addon_projectile_shotguns", "1");
    sv.core.cvars.set("addon_shotgun_speed", "700");
    let pellets = fire(&mut sv, 1, 6.0, dir);
    assert_eq!(pellets.len(), 6);
    let l = sv.level.as_ref().unwrap();
    for &e in &pellets {
        assert_eq!(l.vm.ent_i(e, f.owner), 1);
        assert!((Vec3::from_array(l.vm.ent_v(e, f.velocity)).length() - 700.0).abs() < 0.1);
    }
    assert_eq!(fire(&mut sv, 1, 14.0, dir).len(), 20);
    let l = sv.level.as_mut().unwrap();
    let grunt = (1..l.vm.num_edicts() as u32).find(|&e| l.vm.classname(e) == "monster_army").unwrap();
    let pellets = fire(&mut sv, grunt, 4.0, dir);
    assert_eq!(pellets.len(), 24);
    assert_eq!(pellets.iter().filter(|&&e| sv.level.as_ref().unwrap().vm.ent_i(e, f.owner) == grunt as i32).count(), 4);
    sv.core.set_addons(false);
    assert_eq!(fire(&mut sv, 1, 6.0, dir).len(), 24);
}
