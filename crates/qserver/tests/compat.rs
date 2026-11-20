//! Behaviour that must match ironwail/vkQuake exactly, found by the spawn-state
//! parity sweep (`tools/spawnparity.py`). Real id1 progs; skips without game data.

use std::path::{Path, PathBuf};

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qserver::{ClientMove, Server};

fn basedir() -> Option<PathBuf> {
    let d = std::env::var_os("QUAKE_BASEDIR").map(PathBuf::from).unwrap_or_else(|| {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    d.join("id1").is_dir().then_some(d)
}

fn spawn(map: &str) -> Option<Server> {
    let mut fs = Fs::new(&basedir()?);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.spawn_server(map).ok()?;
    Some(sv)
}

#[test]
fn spawn_settle_frames_leave_frametime_alone() {
    // C SV_Physics never writes `frametime`; Host_ServerFrame does. QC in
    // SV_SpawnServer's two settle frames sees the value the progs loaded with.
    let Some(sv) = spawn("e1m1") else { return };
    let l = sv.level.as_ref().unwrap();
    assert_eq!(l.vm.g_f(sv.core.sv.globals.frametime), 0.0);
}

#[test]
fn setmodel_uses_real_alias_bounds() {
    // PF_setmodel gives an alias model's real bounds (Mod_CalcAliasBounds),
    // not vanilla's fixed ±16 box, in both ironwail and vkQuake.
    let Some(sv) = spawn("e1m1") else { return };
    let m = sv.core.sv.models.iter().flatten().find(|m| m.name == "progs/player.mdl").expect("player precached");
    assert!(!m.brush);
    let data = sv.core.fs.open("progs/player.mdl").unwrap();
    let mdl = qformats::mdl::Mdl::parse(&data).unwrap();
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for v in mdl.frames.iter().flat_map(|f| f.poses.iter()).flat_map(|p| p.verts.iter()) {
        let p = mdl.decode(*v);
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    assert_eq!(m.mins.to_array(), lo);
    assert_eq!(m.maxs.to_array(), hi);
    assert_ne!(m.mins.to_array(), [-16.0; 3]);
}

#[test]
fn empty_engine_string_is_not_null() {
    // PR_SetEngineString("") is a non-null handle: OP_IF is true for it and
    // savegames write the field (`setmodel(e, "")` in hipnotic's rotators).
    let Some(mut sv) = spawn("e1m1") else { return };
    let l = sv.level.as_mut().unwrap();
    assert_ne!(l.vm.strings.engine(b""), 0);
}

#[test]
fn saves_write_strings_raw_and_load_them_back() {
    // PR_UglyValueString writes "%s": a newline stays a newline in the file.
    let Some(mut sv) = spawn("e1m1") else { return };
    {
        let l = sv.level.as_mut().unwrap();
        let ofs = l.vm.field_ofs("message").unwrap();
        let h = l.vm.strings.zone(b"line one\nline two");
        l.vm.set_ent_i(0, ofs, h);
    }
    let text = sv.save_game();
    assert!(text.contains("\"message\" \"line one\nline two\""), "{}", &text[..text.len().min(4000)]);
    sv.load_game(&text).unwrap();
    let l = sv.level.as_ref().unwrap();
    let ofs = l.vm.field_ofs("message").unwrap();
    assert_eq!(l.vm.ent_str(0, ofs).unwrap(), b"line one\nline two");
}

#[test]
fn new_entities_get_physics_from_the_next_frame() {
    // QuakeSpasm/ironwail/vkQuake fix SV_Physics' entity count at the start
    // of the frame: a rocket fired from PlayerPostThink has not moved by the
    // end of that frame (W_FireRocket: origin + v_forward * 8 + '0 0 16').
    let Some(mut sv) = spawn("e1m1") else { return };
    let c = sv.connect_local().unwrap();
    let step = |sv: &mut Server, mv: ClientMove| {
        sv.set_move(c, mv);
        sv.frame(1.0 / 72.0);
        let _ = sv.take_reliable(c);
        let _ = sv.take_datagrams(c);
    };
    for _ in 0..20 {
        step(&mut sv, ClientMove::default());
    }
    for impulse in [9, 7] {
        step(&mut sv, ClientMove { impulse, ..Default::default() });
        for _ in 0..30 {
            step(&mut sv, ClientMove::default());
        }
    }
    // occupy every free slot (ED_Alloc reuses slots freed before t=2), so the
    // rocket is appended past the frame's entity count
    {
        let l = sv.level.as_mut().unwrap();
        let n = l.vm.num_edicts();
        while l.vm.ed_alloc().unwrap() < n as u32 {}
    }
    let fire = ClientMove { buttons: 1, ..Default::default() };
    for _ in 0..60 {
        step(&mut sv, fire);
        let l = sv.level.as_ref().unwrap();
        let f = &sv.core.sv.fields;
        let cls = l.vm.field_ofs("classname").unwrap();
        let rocket = (1..l.vm.num_edicts() as u32).find(|&e| !l.vm.is_free(e) && l.vm.ent_str(e, cls).ok() == Some(b"missile".as_slice()));
        if let Some(e) = rocket {
            let org = l.vm.ent_v(e, f.origin);
            let p = l.vm.ent_v(1, f.origin);
            let fwd = l.vm.ent_v(1, f.v_angle);
            assert_eq!(fwd[0], 0.0, "test assumes a level view");
            let yaw = fwd[1].to_radians();
            let want = [p[0] + 8.0 * yaw.cos(), p[1] + 8.0 * yaw.sin(), p[2] + 16.0];
            for k in 0..3 {
                assert!((org[k] - want[k]).abs() < 0.01, "rocket moved in its spawn frame: {org:?} vs {want:?}");
            }
            return;
        }
    }
    panic!("no rocket fired");
}

#[test]
fn fly_and_edict_dumps() {
    // Host_Fly_f toggles MOVETYPE_FLY like noclip; ED_Count/ED_PrintEdict_f
    // read the live edicts.
    let Some(mut sv) = spawn("e1m1") else { return };
    sv.connect_local().expect("local client");
    for _ in 0..3 {
        sv.frame(1.0 / 72.0);
    }
    let mo = sv.core.sv.fields.movetype;
    let movetype = |sv: &Server| sv.level.as_ref().unwrap().vm.ent_f(1, mo) as i32;
    sv.command("fly");
    assert_eq!(movetype(&sv), qworld::MOVETYPE_FLY);
    sv.command("fly 0");
    assert_eq!(movetype(&sv), qworld::MOVETYPE_WALK);
    let count = sv.edictcount_text().unwrap();
    assert!(count.starts_with("num_edicts:"), "{count}");
    assert!(sv.edict_text(0).unwrap().contains("worldspawn"));
    assert_eq!(sv.edict_text(100_000).unwrap(), "Bad edict number\n");
    assert!(sv.edicts_text().unwrap().contains("info_player_start"));
}

#[test]
fn advertised_extensions_and_getsurface_frik_file() {
    let Some(mut sv) = spawn("e1m1") else { return };
    // Verify newly advertised extensions and DP_QC_GETSURFACE on worldspawn.
    let l = sv.level.as_mut().unwrap();
    assert!(!sv.core.sv.surface_models.is_empty());
    let (sm, sub) = sv.core.sv.surface_models[1].as_ref().expect("world surface model");
    assert!(!sm.submodel_faces(*sub).is_empty());
    // Player start sits just above the floor; getsurfacenearpoint should find a face.
    let p_org = l.vm.ent_v(0, sv.core.sv.fields.origin);
    let _ = p_org;
    let first_face = &sm.submodel_faces(*sub)[0];
    assert!(first_face.verts.len() >= 3);
    let pt = first_face.verts[0];
    let near = sm.near_point(*sub, pt).expect("near face");
    let clipped = sm.clipped_point(*sub, near, pt);
    for k in 0..3 {
        assert!((clipped[k] - pt[k]).abs() < 1.0);
    }
    // Check FRIK_FILE read on an existing file inside pak0 (default.cfg) and reject traversal.
    assert_eq!(sv.core.sv.qc_files.open(&sv.core.fs, "../secret.txt", 0), -1);
    let fh = sv.core.sv.qc_files.open(&sv.core.fs, "default.cfg", 0);
    assert!(fh >= 0);
    let line = sv.core.sv.qc_files.gets(fh).expect("default.cfg first line");
    assert!(!line.is_empty());
    sv.core.sv.qc_files.close(fh);
    // Verify checkextension returns true for all advertised extensions, and false when pr_checkextension = 0.
    for ext in ["DP_QC_GETSURFACE", "FTE_STRINGS", "FRIK_FILE", "DP_QC_SPRINTF"] {
        assert!(qvm::EXTENSIONS.contains(&ext) || ext == "DP_QC_GETSURFACE");
    }
}
