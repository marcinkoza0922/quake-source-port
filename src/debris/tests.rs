use super::*;

/// A flat floor at z = 0 with water below z = -1000 (never reached).
struct Floor;

impl Collide for Floor {
    fn trace(&self, a: Vec3, b: Vec3) -> (f32, Vec3, Vec3) {
        if b.z >= 0.0 || a.z < 0.0 {
            return (1.0, b, Vec3::new(0.0, 0.0, 1.0));
        }
        let f = a.z / (a.z - b.z);
        (f, a.lerp(b, f), Vec3::new(0.0, 0.0, 1.0))
    }
    fn solid(&self, p: Vec3) -> bool {
        p.z < 0.0
    }
    fn water(&self, _: Vec3) -> bool {
        false
    }
}

fn game_fs() -> Option<qcore::fs::Fs> {
    let base = std::env::var_os("QUAKE_BASEDIR").unwrap_or_else(|| "vkQuake directory with maps".into());
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    fs.open("progs/soldier.mdl").map(|_| fs)
}

#[test]
fn ragdolls_settle_on_the_floor() {
    let Some(fs) = game_fs() else { return };
    for name in ["progs/soldier.mdl", "progs/dog.mdl", "progs/ogre.mdl", "progs/shambler.mdl", "progs/demon.mdl", "progs/wizard.mdl"] {
        let info = ModelInfo::load(&fs, name).unwrap();
        let frame = (0..info.mdl.frames.len() as i32).find(|&f| info.is_last(f)).unwrap_or_else(|| panic!("{name}: no death"));
        for q in [Quality::Low, Quality::Medium, Quality::High] {
            let t = tier_for(&Settings { quality: q, ..Settings::default() });
            let rig = Rc::new(Rig::build(&info.mdl, frame as usize, t.max_bones).unwrap());
            assert!(rig.centers.len() <= t.max_bones);
            // lift the corpse a bit and drop it with a shove
            let mut d = Ragdoll::new(rig.clone(), Vec3::new(0.0, 0.0, 40.0), [0.0, 30.0, 0.0], Vec3::new(100.0, 0.0, 0.0), t.step, &Floor);
            let steps = (10.0 / t.step).round() as usize;
            for _ in 0..steps {
                d.step(800.0, &Floor, t.iters, t.skip_down);
            }
            assert!(d.pos.iter().all(|p| p.x.is_finite() && p.z > -0.5), "{name} {q:?}: {:?}", d.pos);
            assert!(d.asleep, "{name} {q:?} still moving");
            let (bc, br) = d.bounds();
            let n = info.mdl.num_verts;
            let packed = d.pack();
            assert_eq!(packed.verts.len(), n, "{name}");
            // Conservative bounds must enclose every packed vertex.
            for &pv in &packed.verts {
                let p = packed.origin
                    + Vec3::new(
                        (pv & 255) as f32 * packed.scale.x,
                        ((pv >> 8) & 255) as f32 * packed.scale.y,
                        ((pv >> 16) & 255) as f32 * packed.scale.z,
                    );
                assert!((p - bc).length() <= br + 0.5, "{name} {q:?}: vert {p:?} outside {bc:?}+{br}");
            }
            // links held: no bone wandered far from its neighbours
            for l in &rig.links {
                let len = (d.pos[l.a as usize] - d.pos[l.b as usize]).length();
                if l.hard {
                    assert!((len - l.rest).abs() < l.rest * 0.25 + 1.0, "{name} {q:?}: link {len} vs {}", l.rest);
                }
            }
            let before = d.pos.clone();
            assert!(d.blast(d.pos[0] - Vec3::new(20.0, 0.0, 0.0), 200.0, 500.0));
            for _ in 0..(0.5 / t.step).round() as usize {
                d.step(800.0, &Floor, t.iters, t.skip_down);
            }
            let moved = d.pos.iter().zip(&before).map(|(a, b)| (*a - *b).length()).fold(0.0, f32::max);
            assert!(moved > 5.0, "{name} {q:?}: blast moved only {moved}");
        }
    }
}

#[test]
fn bodies_bounce_and_rest() {
    for (step, skip_down, expected_z) in [(STEP, false, 3.0), (1.0 / 30.0, true, 0.1)] {
        let mut b = Body::new(ModelId(0), Vec3::new(0.0, 0.0, 50.0), Vec3::new(80.0, 0.0, 200.0), Quat::IDENTITY, 3.0);
        b.random_spin(&mut Rng(7), 9.0);
        for _ in 0..600 {
            b.step(800.0, &Floor, step, skip_down);
        }
        assert!(b.asleep);
        assert!((b.pos.z - expected_z).abs() < 0.5, "{:?}", b.pos);
        assert!(b.pos.x > 10.0);
    }
}

#[test]
fn quality_cvar_and_step_rescale() {
    assert_eq!(Quality::from_cvar(-1.0), Quality::Low);
    assert_eq!(Quality::from_cvar(0.0), Quality::Low);
    assert_eq!(Quality::from_cvar(1.0), Quality::Medium);
    assert_eq!(Quality::from_cvar(2.0), Quality::High);
    assert_eq!(Quality::from_cvar(5.0), Quality::High);

    let Some(fs) = game_fs() else { return };
    let info = ModelInfo::load(&fs, "progs/soldier.mdl").unwrap();
    let frame = (0..info.mdl.frames.len() as i32).find(|&f| info.is_last(f)).unwrap();
    let rig = Rc::new(Rig::build(&info.mdl, frame as usize, 8).unwrap());
    let mut a = Ragdoll::new(rig.clone(), Vec3::new(0.0, 0.0, 40.0), [0.0; 3], Vec3::new(90.0, 0.0, 0.0), 1.0 / 60.0, &Floor);
    a.set_step(1.0 / 30.0);
    let b = Ragdoll::new(rig, Vec3::new(0.0, 0.0, 40.0), [0.0; 3], Vec3::new(90.0, 0.0, 0.0), 1.0 / 30.0, &Floor);
    a.step(800.0, &Floor, 4, false);
    let mut c = b;
    c.step(800.0, &Floor, 4, false);
    for (pa, pc) in a.pos.iter().zip(&c.pos) {
        assert!((*pa - *pc).length() < 1e-4, "step rescale mismatch: {pa:?} vs {pc:?}");
    }
}

#[test]
fn offscreen_ragdoll_skips_pack_until_visible() {
    let Some(fs) = game_fs() else { return };
    let info = ModelInfo::load(&fs, "progs/soldier.mdl").unwrap();
    let frame = (0..info.mdl.frames.len() as i32).find(|&f| info.is_last(f)).unwrap();
    let rig = Rc::new(Rig::build(&info.mdl, frame as usize, 6).unwrap());
    let mut debris = Debris { active: true, ..Debris::default() };
    let doll = Ragdoll::new(rig, Vec3::new(-200.0, 0.0, 40.0), [0.0; 3], Vec3::ZERO, 1.0 / 30.0, &Floor);
    debris.corpses.push(Corpse {
        ent: 1,
        model: 1,
        id: ModelId(1),
        skin: 0,
        colormap: None,
        alpha: 1.0,
        life: 12.0,
        doll,
        gib: None,
    });
    let view = qrender::View { origin: [0.0, 0.0, 40.0], angles: [0.0, 0.0, 0.0], fov_x: 90.0, ..Default::default() };
    let frustum = qrender::ViewFrustum::from_view(&view, (1280, 720), 1.0);
    let mut ents = Vec::new();
    let mut dyn_verts = Vec::new();
    debris.entities(&mut ents, &mut dyn_verts, Some(&frustum));
    assert!(ents.is_empty());
    assert!(dyn_verts.is_empty());

    // Passing None (shared views) or looking toward -X packs and emits it.
    let look_back = qrender::View { origin: [0.0, 0.0, 40.0], angles: [0.0, 180.0, 0.0], fov_x: 90.0, ..Default::default() };
    let frustum_back = qrender::ViewFrustum::from_view(&look_back, (1280, 720), 1.0);
    debris.entities(&mut ents, &mut dyn_verts, Some(&frustum_back));
    assert_eq!(ents.len(), 1);
    assert_eq!(dyn_verts.len(), info.mdl.num_verts);
}

#[test]
fn mixed_start_odds() {
    use super::events::animation_chance;
    assert_eq!(animation_chance(0.0), 1.0);
    assert_eq!(animation_chance(6.0), 0.0);
    assert_eq!(animation_chance(14.0), 0.0);
    assert!(animation_chance(1.0) > animation_chance(4.0));
    assert_eq!(Start::from_cvar(2.0), Start::Mixed);
    assert_eq!(Start::from_cvar(0.0), Start::Animation);
}
