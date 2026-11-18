use qcore::math::Vec3;
use qformats::Mdl;

use super::rig::Spec;
use super::{classic, enhanced};
use super::table::Held;
use super::VWep;

fn game_fs(dirs: &[&str]) -> Option<qcore::fs::Fs> {
    let base = std::env::var_os("QUAKE_BASEDIR").unwrap_or_else(|| "vkQuake directory with maps".into());
    let mut fs = qcore::fs::Fs::new(base);
    for d in dirs {
        if !fs.add_game_dir(d) {
            return None;
        }
    }
    fs.open("progs/player.mdl").map(|_| fs)
}

fn player(fs: &qcore::fs::Fs) -> Mdl {
    Mdl::parse(&fs.open("progs/player.mdl").unwrap()).unwrap()
}

fn frame(m: &Mdl, name: &str) -> u32 {
    m.frames.iter().position(|f| f.poses[0].name == name).unwrap_or_else(|| panic!("{name}")) as u32
}

#[test]
fn id_player_model_splits_into_body_gun_and_axe() {
    let Some(fs) = game_fs(&["id1"]) else { return };
    let m = player(&fs);
    let (rig, body, body_axe) = classic::rig(&m).expect("id1 player.mdl rig");
    // the gun is rigid in every pose (8-bit vertex precision)
    assert!(rig.residual < 1.0, "residual {}", rig.residual);
    assert_eq!(body.triangles.len(), m.triangles.len() - 72 - 28);
    assert_eq!(body_axe.triangles.len(), m.triangles.len() - 72);
    assert!(rig.axe_frame(frame(&m, "axstnd1")) && rig.axe_frame(frame(&m, "axdeth3")));
    assert!(!rig.axe_frame(frame(&m, "stand1")) && !rig.axe_frame(frame(&m, "deatha1")));
    assert!(rig.death_frame(frame(&m, "axdeth3")) && rig.death_frame(frame(&m, "deathe9")));
    assert!(!rig.death_frame(frame(&m, "pain1")));
}

#[test]
fn other_player_models_keep_their_own_weapons() {
    if let Some(fs) = game_fs(&["id1", "ad"]) {
        assert!(classic::rig(&player(&fs)).is_none(), "Arcane Dimensions' player");
    }
    if let Some(fs) = game_fs(&["id1", "hipnotic"]) {
        let ham = Mdl::parse(&fs.open("progs/playham.mdl").unwrap()).unwrap();
        assert!(classic::rig(&ham).is_none());
    }
}

#[test]
fn baked_weapon_follows_the_hand() {
    let Some(fs) = game_fs(&["id1"]) else { return };
    let m = player(&fs);
    let (rig, ..) = classic::rig(&m).unwrap();
    let src = Mdl::parse(&fs.open("progs/g_rock2.mdl").unwrap()).unwrap();
    let spec = Spec { source: "progs/g_rock2.mdl".into(), scale: 1.0, offset: [0.0; 3], front_only: false };
    let w = classic::bake(&rig, &src, &spec).unwrap();
    assert_eq!(w.frames.len(), m.frames.len());
    assert_eq!(w.num_verts, src.num_verts);
    assert_eq!(w.flags, 0, "no pickup rotation");
    let centre = |f: u32| {
        let p = &w.frames[f as usize].poses[0];
        let used = w.triangles.iter().flat_map(|t| t.verts).map(|v| Vec3::from_array(w.decode(p.verts[v as usize])));
        let (lo, hi) = super::rig::bbox(used);
        ((lo + hi) * 0.5, hi - lo)
    };
    // shotatt1: centred on the gun, still pointing along +x at full length
    let (c, size) = centre(frame(&m, "shotatt1"));
    assert!((c - Vec3::new(1.55, -9.95, 11.3)).length() < 1.0, "{c:?}");
    let (slo, shi) = (src.decode(src.frames[0].poses[0].bboxmin), src.decode(src.frames[0].poses[0].bboxmax));
    assert!((size.x - (shi[0] - slo[0])).abs() < 1.0);
    // axe frames: the gun hangs on the back, behind the body's centre
    let (back, _) = centre(frame(&m, "axstnd1"));
    assert!(back.x < -5.0, "{back:?}");
    // view models drop what sits behind the eye
    let v = Mdl::parse(&fs.open("progs/v_shot.mdl").unwrap()).unwrap();
    let spec = Spec { source: "progs/v_shot.mdl".into(), scale: 1.0, offset: [0.0; 3], front_only: true };
    let w = classic::bake(&rig, &v, &spec).unwrap();
    assert_eq!(w.triangles.len(), 108);
}

#[test]
fn weapon_choice_per_player() {
    let Some(fs) = game_fs(&["id1"]) else { return };
    let m = player(&fs);
    let (rig, ..) = classic::rig(&m).unwrap();
    let mut cl = qclient::ClientState::new();
    cl.maxclients = 4;
    cl.viewentity = 1;
    cl.model_precache = vec![String::new(), "progs/player.mdl".into(), "progs/v_rock2.mdl".into()];
    cl.sound_precache = vec![String::new(), "weapons/sgun1.wav".into(), "weapons/ax1.wav".into()];
    cl.stats[qnet::protocol::STAT_WEAPON] = 2;
    let gun = |h: Option<Held>| match h {
        Some(Held::Gun(s)) => s.source,
        Some(Held::Axe) => "axe".into(),
        None => "stock".into(),
    };
    let (stand, axe, death) = (frame(&m, "stand1"), frame(&m, "axstnd1"), frame(&m, "deatha3"));
    let mut v = VWep::default();
    // own player: the weapon stat; dying keeps the last weapon
    assert_eq!(gun(v.resolve(1, &cl, &rig, stand, &fs)), "progs/g_rock2.mdl");
    cl.stats[qnet::protocol::STAT_WEAPON] = 0;
    assert_eq!(gun(v.resolve(1, &cl, &rig, death, &fs)), "progs/g_rock2.mdl");
    assert_eq!(gun(v.resolve(1, &cl, &rig, stand, &fs)), "stock");
    // another remote player: unknown until a fire sound agrees with the frames
    assert_eq!(gun(v.resolve(2, &cl, &rig, stand, &fs)), "stock");
    v.sound(&cl, 2, 1);
    assert_eq!(gun(v.resolve(2, &cl, &rig, stand, &fs)), "progs/g_rock2.mdl");
    assert_eq!(gun(v.resolve(2, &cl, &rig, axe, &fs)), "stock");
    v.sound(&cl, 2, 2);
    assert_eq!(gun(v.resolve(2, &cl, &rig, axe, &fs)), "axe");
    // a death forgets the guess (respawns start with another weapon)
    assert_eq!(gun(v.resolve(2, &cl, &rig, frame(&m, "axdeth2"), &fs)), "axe");
    assert_eq!(gun(v.resolve(2, &cl, &rig, axe, &fs)), "stock");
    // a local server knows everyone's weapon
    v.server = Some(vec![String::new(), "progs/v_nail2.mdl".into()]);
    assert_eq!(gun(v.resolve(2, &cl, &rig, axe, &fs)), "progs/g_nail2.mdl");
    v.sound(&cl, 9, 1); // not a player: ignored
}

#[test]
fn enhanced_player_model_holds_enhanced_pickups() {
    let Some(fs) = game_fs(&["id1"]) else { return };
    let m = player(&fs);
    let Some(md5) = crate::assets::md3::parse_md5(&fs, "progs/player.mdl") else { return };
    let (rig, body, body_axe) = enhanced::rig(&md5, &m).expect("re-release player.md5mesh rig");
    assert!(rig.residual < 0.5, "residual {}", rig.residual);
    let tris = |m: &qformats::md3::Md3| m.surfaces.iter().map(|s| s.triangles.len()).sum::<usize>();
    assert!(tris(&body) < tris(&body_axe) && tris(&body_axe) < tris(&md5.md3));
    assert!(rig.axe_frame(frame(&m, "axrun1")) && rig.death_frame(frame(&m, "deathc4")));
    let src = crate::assets::md3::parse_md5(&fs, "progs/g_rock2.mdl").expect("enhanced g_rock2");
    let spec = Spec { source: "progs/g_rock2.mdl".into(), scale: 1.0, offset: [0.0; 3], front_only: false };
    let w = enhanced::bake(&rig, &src.md3, &spec).unwrap();
    assert_eq!(w.frames.len(), m.frames.len());
    for s in &w.surfaces {
        assert_eq!(s.verts.len(), s.num_verts * w.frames.len());
    }
    // centred on the enhanced model's gun in shotatt1, pointing along +x
    let f = frame(&m, "shotatt1") as usize;
    let b = w.frames[f].bounds;
    let size = Vec3::from_array(b[1]) - Vec3::from_array(b[0]);
    assert!(size.x > size.y && size.x > size.z, "{size:?}");
}
