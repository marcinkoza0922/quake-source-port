//! Motion blur of ragdolls; skips without game data or a renderer.
use qformats::{Bsp, parse_entities};
use qrender::*;

/// A ragdoll's pose is only in this frame's buffer, so it has no previous
/// position: moved or not, it must blur like an entity never seen before.
#[test]
fn a_moving_ragdoll_leaves_no_stray_velocity() {
    let base = std::env::var_os("QUAKE_BASEDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
    if !base.join("id1").exists() {
        eprintln!("skip: no game data");
        return;
    }
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    let (Some(pal), Some(map), Some(mdl)) = (fs.open("gfx/palette.lmp"), fs.open("maps/start.bsp"), fs.open("progs/player.mdl")) else {
        eprintln!("skip: no start.bsp or player.mdl");
        return;
    };
    let pal = qformats::image::parse_palette(&pal).unwrap();
    let bsp = Bsp::parse(&map).unwrap();
    let mut r = match Renderer::headless(320, 200, &pal) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skip: no renderer: {e}");
            return;
        }
    };
    r.load_world(&bsp, None).unwrap();
    let ents = parse_entities(&bsp.entities).unwrap();
    let start = ents.iter().find(|e| e.classname() == Some("info_player_start")).unwrap();
    let o: Vec<f32> = start.get("origin").unwrap().split_whitespace().map(|v| v.parse().unwrap()).collect();
    let yaw: f32 = start.get("angle").and_then(|a| a.parse().ok()).unwrap_or(0.0);
    let view = View { origin: [o[0], o[1], o[2] + 22.0], angles: [0.0, yaw, 0.0], fov_x: 90.0, ..Default::default() };

    let mdl = qformats::Mdl::parse(&mdl).unwrap();
    // Enough poses that a ragdoll's offset falls on some of them when it is
    // taken for an offset into the models' own.
    for _ in 0..24 {
        r.load_alias(&mdl).unwrap();
    }
    let id = r.load_alias(&mdl).unwrap();
    let pose = &mdl.frames[0].poses[0];
    let verts: Vec<u32> = pose.verts.iter().map(|v| u32::from_le_bytes([v.v[0], v.v[1], v.v[2], v.normal])).collect();
    let (sin, cos) = yaw.to_radians().sin_cos();
    let at = |side: f32| [view.origin[0] + cos * 96.0 + sin * side, view.origin[1] + sin * 96.0 - cos * side, view.origin[2]];
    let ent = |key: u32, ragdoll: bool, pos: [f32; 3]| RenderEntity {
        key,
        model: id,
        origin: pos,
        dyn_pose: ragdoll.then(|| DynPose {
            ofs: 0,
            origin: std::array::from_fn(|k| pos[k] + mdl.scale_origin[k]),
            scale: mdl.scale,
            radius: 64.0,
        }),
        ..Default::default()
    };
    let settings = Settings { motion_blur: 2.0, motion_blur_quality: 1, ..Default::default() };
    let mut shot = |key: u32, ragdoll: bool| {
        let mut draw = |time: f64, pos: [f32; 3]| {
            let e = [ent(key, ragdoll, pos)];
            r.render(&RenderFrame { time, view: Some(view), entities: &e, dyn_verts: &verts, settings, ..Default::default() }).unwrap();
        };
        draw(1.0, at(0.0));
        draw(1.02, at(14.0));
        r.read_pixels().unwrap()
    };
    // (key 0 is never tracked)
    assert!(shot(42, false) != shot(0, false), "an ordinary model blurs");
    assert!(shot(42, true) == shot(0, true), "the ragdoll was blurred with another model's poses");
}
