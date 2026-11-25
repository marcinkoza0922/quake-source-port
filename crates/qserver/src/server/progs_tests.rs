//! id1's progs with a few functions changed; skips without game data.
use super::*;

/// A server on e1m1 whose progs have each named function replaced by builtin `n`.
fn patched(changes: &[(&str, i32)]) -> Option<Server> {
    let base = std::env::var_os("QUAKE_BASEDIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    if !base.join("id1").is_dir() {
        return None;
    }
    let mut fs = qcore::fs::Fs::new(&base);
    fs.add_game_dir("id1");
    let mut d = fs.open("progs.dat")?;
    let int = |d: &[u8], at: usize| i32::from_le_bytes(d[at..at + 4].try_into().unwrap()) as usize;
    let (functions, count, strings) = (int(&d, 32), int(&d, 36), int(&d, 40));
    for &(name, n) in changes {
        let f = (0..count).map(|i| functions + i * 36).find(|&f| {
            let s = strings + int(&d, f + 16);
            d[s..].starts_with(name.as_bytes()) && d[s + name.len()] == 0
        })?;
        d[f..f + 4].copy_from_slice(&(-n).to_le_bytes());
    }
    let dir = std::env::temp_dir().join(format!("qsp-progs-{}-{}", std::process::id(), changes[0].0));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("progs.dat"), d).unwrap();
    fs.add_user_dir(&dir);
    let mut sv = Server::new(fs, qcore::cvar::Cvars::new(), 1);
    let ok = sv.spawn_server("e1m1");
    let _ = std::fs::remove_dir_all(&dir);
    ok.ok()?;
    Some(sv)
}

/// vkQuake's PF_copyentity links the copy.
#[test]
fn a_copied_entity_is_in_the_world() {
    // setspawnparms takes an entity, as copyentity(from) does
    let Some(mut sv) = patched(&[("setspawnparms", 400)]) else { return };
    let f = sv.core.sv.fields.clone();
    let l = sv.level.as_mut().unwrap();
    let copy = l.vm.progs.find_function("setspawnparms").unwrap() as i32;
    let monster = (1..l.vm.num_edicts() as u32)
        .find(|&e| !l.vm.is_free(e) && l.vm.ent_str(e, f.classname).is_ok_and(|c| c.starts_with(b"monster_")))
        .unwrap();
    sv.core.ext(monster).alpha = 77;
    l.vm.set_g_i(qvm::ofs_parm(0), monster as i32);
    exec(&mut l.vm, &mut sv.core, &mut l.world, copy);
    assert!(sv.core.sv.error.is_none(), "{:?}", sv.core.sv.error);
    let new = l.vm.g_i(qvm::OFS_RETURN) as u32;
    assert_ne!(new, monster);
    assert!(l.world.is_linked(new), "solid and visible");
    assert_eq!(l.vm.ent_v(new, f.origin), l.vm.ent_v(monster, f.origin));
    assert!(!sv.core.ext(new).leafs.is_empty(), "sent to clients that see it");
    assert_eq!(sv.core.ext(new).alpha, 77);
}

#[test]
fn an_error_in_set_change_parms_stops_the_level_change() {
    // #10 is error()
    let Some(mut sv) = patched(&[("SetChangeParms", 10)]) else { return };
    sv.connect_local().unwrap();
    assert!(sv.error().is_none());
    sv.command("changelevel e1m2");
    assert!(sv.error().is_some(), "Host_Error");
    assert_eq!(sv.core.sv.name, "e1m1");
}
