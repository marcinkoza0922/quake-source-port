use super::*;
use crate::Server;

fn server() -> Option<Server> {
    let base = std::env::var_os("QUAKE_BASEDIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    if !fs.exists("maps/e1m1.bsp") {
        return None;
    }
    let mut sv = Server::new(fs, qcore::cvar::Cvars::new(), 2);
    sv.spawn_server("e1m1").unwrap();
    sv.connect_local().unwrap();
    for _ in 0..20 {
        sv.frame(0.05);
    }
    Some(sv)
}

fn call(sv: &mut Server, name: &str, argc: usize, set: impl FnOnce(&mut Vm)) {
    let l = sv.level.as_mut().unwrap();
    set(&mut l.vm);
    l.vm.argc = argc;
    let mut h = Ctx::new(&mut sv.core, &mut l.world);
    assert!(h.dp_builtin(&mut l.vm, name).unwrap(), "{name}");
}

fn vm(sv: &Server) -> &Vm {
    &sv.level.as_ref().unwrap().vm
}

const RET: usize = 1;

#[test]
fn traces() {
    let Some(mut sv) = server() else { return };
    let org = vm(&sv).ent_v(1, sv.core.sv.fields.origin);
    let g = sv.core.sv.globals.clone();
    call(&mut sv, "tracebox", 6, |vm| {
        vm.set_g_v(p(0), org);
        vm.set_g_v(p(1), [-16.0, -16.0, -24.0]);
        vm.set_g_v(p(2), [16.0, 16.0, 32.0]);
        vm.set_g_v(p(3), [org[0], org[1], org[2] - 500.0]);
        vm.set_g_f(p(4), 0.0);
        vm.set_g_i(p(5), 1);
    });
    // the player stands on the floor, so its box hardly falls
    assert!(vm(&sv).g_f(g.trace_fraction) < 0.01);
    assert_eq!(vm(&sv).g_v(g.trace_plane_normal)[2], 1.0);
    let (fo, fv) = (sv.core.sv.fields.origin, sv.core.sv.fields.velocity);
    call(&mut sv, "tracetoss", 2, |vm| {
        vm.set_ent_v(1, fo, [org[0], org[1], org[2] + 20.0]);
        vm.set_ent_v(1, fv, [0.0; 3]);
        vm.set_g_i(p(0), 1);
        vm.set_g_i(p(1), 1);
    });
    let end = vm(&sv).g_v(g.trace_endpos);
    assert!((end[2] - org[2]).abs() < 1.0, "lands back where it stood: {end:?} {org:?}");
    call(&mut sv, "checkpvs", 2, |vm| {
        vm.set_g_v(p(0), org);
        vm.set_g_i(p(1), 1);
    });
    assert_eq!(vm(&sv).g_f(RET), 1.0);
}

#[test]
fn clients_and_cvars() {
    let Some(mut sv) = server() else { return };
    let str_parm = |vm: &mut Vm, i: usize, s: &str| {
        let h = vm.strings.temp(s.as_bytes());
        vm.set_g_i(p(i), h);
    };
    for (e, ty) in [(1, 1.0), (2, 0.0), (40, 3.0)] {
        call(&mut sv, "clienttype", 1, |vm| vm.set_g_i(p(0), e));
        assert_eq!(vm(&sv).g_f(RET), ty);
    }
    call(&mut sv, "infokey", 2, |vm| {
        vm.set_g_i(p(0), 1);
        str_parm(vm, 1, "protocol");
    });
    assert_eq!(vm(&sv).g_str(RET).unwrap(), b"rmq999");
    call(&mut sv, "cvar_defstring", 1, |vm| str_parm(vm, 0, "sv_gravity"));
    assert_eq!(vm(&sv).g_str(RET).unwrap(), b"800");
    call(&mut sv, "cvar_type", 1, |vm| str_parm(vm, 0, "no_such_cvar"));
    assert_eq!(vm(&sv).g_f(RET), 0.0);
    call(&mut sv, "whichpack", 1, |vm| str_parm(vm, 0, "progs.dat"));
    assert_eq!(vm(&sv).g_str(RET).unwrap(), b"id1");
    call(&mut sv, "digest_hex", 2, |vm| {
        str_parm(vm, 0, "MD4");
        str_parm(vm, 1, "abc");
    });
    assert_eq!(vm(&sv).g_str(RET).unwrap(), b"A448017AAF21D8525FC10AE87AA6729D");
    let soldier = sv.core.sv.model_precache.iter().position(|m| m == "progs/soldier.mdl").unwrap();
    call(&mut sv, "frameforname", 2, |vm| {
        vm.set_g_f(p(0), soldier as f32);
        str_parm(vm, 1, "stand3");
    });
    assert_eq!(vm(&sv).g_f(RET), 2.0);
    assert!(matches("sv_*y", "sv_gravity") && !matches("sv_?", "sv_gravity") && matches("sv_", "sv_gravity"));
}

#[test]
fn multicast_reaches_one_client_and_dropclient_drops() {
    let Some(mut sv) = server() else { return };
    let g = sv.core.sv.globals.clone();
    sv.take_reliable(0);
    sv.core.sv.multicast.data.extend_from_slice(b"\x09echo hi\n\0");
    call(&mut sv, "multicast", 2, |vm| {
        vm.set_g_f(p(1), ONE_R as f32);
        vm.set_g_i(g.msg_entity, 1);
    });
    assert!(sv.core.sv.multicast.data.is_empty());
    sv.frame(0.05);
    assert!(sv.take_reliable(0).concat().windows(7).any(|w| w == b"echo hi"));
    call(&mut sv, "dropclient", 1, |vm| vm.set_g_i(p(0), 1));
    assert!(sv.core.clients[0].active, "not while QC runs");
    sv.frame(0.05);
    assert!(!sv.core.clients[0].active);
}

/// A field offset from a mod: only one inside the edict is sent as a stat.
#[test]
fn clientstat_offsets_are_checked() {
    let Some(mut sv) = server() else { return };
    let n = vm(&sv).entityfields() as i32;
    let add = |sv: &mut Server, ty: i32, ofs: i32| {
        let l = sv.level.as_mut().unwrap();
        l.vm.set_g_f(p(0), 40.0);
        l.vm.set_g_f(p(1), ty as f32);
        l.vm.set_g_i(p(2), ofs);
        l.vm.argc = 3;
        Ctx::new(&mut sv.core, &mut l.world).ext_builtin(&mut l.vm, "clientstat").unwrap();
        sv.core.sv.custom_stats.len()
    };
    for (ty, ofs) in [(2, 8_000_000), (2, -1), (2, i32::MIN), (2, n), (3, n - 2), (1, n), (4, n)] {
        assert_eq!(add(&mut sv, ty, ofs), 0, "type {ty} at {ofs} of {n}");
    }
    for (ty, ofs) in [(2, n - 1), (3, n - 3)] {
        assert_eq!(add(&mut sv, ty, ofs), 1, "type {ty} at {ofs}");
        for _ in 0..3 {
            sv.frame(0.05);
        }
        assert!(sv.error().is_none());
    }
}
