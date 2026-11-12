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
        let rig = Rc::new(Rig::build(&info.mdl, frame as usize).unwrap());
        // lift the corpse a bit and drop it with a shove
        let mut d = Ragdoll::new(rig.clone(), Vec3::new(0.0, 0.0, 40.0), [0.0, 30.0, 0.0], Vec3::new(100.0, 0.0, 0.0), &Floor);
        for _ in 0..600 {
            d.step(800.0, &Floor);
        }
        assert!(d.pos.iter().all(|p| p.x.is_finite() && p.z > -0.5), "{name}: {:?}", d.pos);
        assert!(d.asleep, "{name} still moving");
        let n = info.mdl.num_verts;
        assert_eq!(d.pack().verts.len(), n, "{name}");
        // links held: no bone wandered far from its neighbours
        for l in &rig.links {
            let len = (d.pos[l.a as usize] - d.pos[l.b as usize]).length();
            if l.hard {
                assert!((len - l.rest).abs() < l.rest * 0.2 + 1.0, "{name}: link {len} vs {}", l.rest);
            }
        }
        let before = d.pos.clone();
        assert!(d.blast(d.pos[0] - Vec3::new(20.0, 0.0, 0.0), 200.0, 500.0));
        for _ in 0..30 {
            d.step(800.0, &Floor);
        }
        let moved = d.pos.iter().zip(&before).map(|(a, b)| (*a - *b).length()).fold(0.0, f32::max);
        assert!(moved > 5.0, "{name}: blast moved only {moved}");
    }
}

#[test]
fn bodies_bounce_and_rest() {
    let mut b = Body::new(ModelId(0), Vec3::new(0.0, 0.0, 50.0), Vec3::new(80.0, 0.0, 200.0), Quat::IDENTITY, 3.0);
    b.random_spin(&mut Rng(7), 9.0);
    for _ in 0..600 {
        b.step(800.0, &Floor);
    }
    assert!(b.asleep);
    assert!((b.pos.z - 3.0).abs() < 0.5, "{:?}", b.pos);
    assert!(b.pos.x > 10.0);
}
