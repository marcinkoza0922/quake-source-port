//! Tests on real maps; skipped when QUAKE_BASEDIR is absent.
mod common;
use common::{Ents, Rng, fs};
use qformats::{Bsp, BspVersion, parse_entities};
use qworld::hull::CollisionModel;
use qworld::world::MoveType;
use qworld::*;
use std::sync::Arc;

fn load(game: Option<&str>, name: &str) -> Option<(Bsp, Arc<CollisionModel>)> {
    let d = fs(game)?.open(name)?;
    let b = Bsp::parse(&d).unwrap();
    let cm = Arc::new(CollisionModel::from_bsp(&b));
    Some((b, cm))
}

fn player_start(b: &Bsp) -> Vec3 {
    let ents = parse_entities(&b.entities).unwrap();
    let e = ents.iter().find(|e| e.classname() == Some("info_player_start")).unwrap();
    let o: Vec<f32> = e.get("origin").unwrap().split_whitespace().map(|s| s.parse().unwrap()).collect();
    Vec3::new(o[0], o[1], o[2])
}

/// Random rays through the world bounds; compare with sampled point contents.
fn check_traces(cm: &CollisionModel, hull: usize, seed: u64) {
    let m = &cm.submodels[0];
    let h = cm.hull(0, hull);
    let mut rng = Rng(seed);
    let (mut bad, mut n) = (0, 0);
    for _ in 0..3000 {
        let p = |r: &mut Rng| Vec3::new(r.f(m.mins.x, m.maxs.x), r.f(m.mins.y, m.maxs.y), r.f(m.mins.z, m.maxs.z));
        let a = p(&mut rng);
        let b = a + (p(&mut rng) - a).normalize() * rng.f(1.0, 600.0);
        let t = h.trace(a, b);
        if h.point_contents(h.first, a) == CONTENTS_SOLID {
            assert!(t.startsolid);
            continue;
        }
        n += 1;
        assert!(!t.allsolid);
        assert!(h.point_contents(h.first, t.endpos) != CONTENTS_SOLID, "{a:?}->{b:?} {t:?}");
        let len = (b - a).length();
        let steps = (len * 4.0) as usize;
        let mut ok = true;
        for i in 0..steps {
            let f = i as f32 / steps as f32;
            if f * len < t.fraction * len - 0.1 && h.point_contents(h.first, a.lerp(b, f)) == CONTENTS_SOLID {
                ok = false;
                break;
            }
        }
        if t.fraction == 1.0 {
            // brute force must agree no solid was crossed (within sampling)
        } else if h.point_contents(h.first, t.endpos - t.plane_normal * 0.1) != CONTENTS_SOLID {
            bad += 1;
        }
        assert!(ok, "trace passed through solid {a:?}->{b:?} {t:?}");
    }
    assert!(bad * 20 < n, "{bad}/{n} hits without solid behind the plane");
}

#[test]
fn start_point_contents() {
    let Some((b, cm)) = load(None, "maps/start.bsp") else { return };
    let ps = player_start(&b);
    assert_eq!(cm.point_contents(ps), CONTENTS_EMPTY);
    assert_eq!(cm.point_contents(Vec3::new(0.0, 0.0, -20000.0)), CONTENTS_SOLID);
    let h1 = cm.hull(0, 1);
    assert_ne!(h1.point_contents(h1.first, ps), CONTENTS_SOLID);
    // start.bsp has water and lava.
    let m = &cm.submodels[0];
    let mut rng = Rng(7);
    let mut seen = [false; 3];
    for _ in 0..400_000 {
        let p = Vec3::new(rng.f(m.mins.x, m.maxs.x), rng.f(m.mins.y, m.maxs.y), rng.f(m.mins.z, m.maxs.z));
        match cm.point_contents(p) {
            CONTENTS_WATER => seen[0] = true,
            CONTENTS_SLIME => seen[1] = true,
            CONTENTS_LAVA => seen[2] = true,
            _ => {}
        }
    }
    // Water and lava are big enough to hit by sampling; slime only if present.
    assert!(seen[0] && seen[2], "{seen:?}");
    assert!(cm.leaf_contents.contains(&CONTENTS_WATER));
}

#[test]
fn start_traces() {
    let Some((_, cm)) = load(None, "maps/start.bsp") else { return };
    for h in 0..3 {
        check_traces(&cm, h, 0x1234 + h as u64);
    }
}

#[test]
fn start_physics() {
    let Some((b, cm)) = load(None, "maps/start.bsp") else { return };
    let mut w = World::new(cm);
    let mut ents = Ents::default();
    let world = ents.spawn();
    ents.set_f(world, F::Solid, SOLID_BSP as f32);
    ents.set_f(world, F::MoveType, MOVETYPE_PUSH as f32);
    ents.set_f(world, F::ModelIndex, 1.0);
    let ps = player_start(&b);
    // Player in hull 1 lands on the floor via walk physics.
    let e = ents.spawn();
    ents.set_v(e, V::Origin, ps + Vec3::new(0.0, 0.0, 20.0));
    ents.set_v(e, V::Mins, Vec3::new(-16.0, -16.0, -24.0));
    ents.set_v(e, V::Maxs, Vec3::new(16.0, 16.0, 32.0));
    ents.set_v(e, V::ViewOfs, Vec3::new(0.0, 0.0, 22.0));
    ents.set_f(e, F::Solid, SOLID_SLIDEBOX as f32);
    ents.set_f(e, F::MoveType, MOVETYPE_WALK as f32);
    w.link_edict(&mut ents, e, false);
    let mut p = PhysParams { frametime: 0.01, ..Default::default() };
    for _ in 0..200 {
        phys::physics_client_move(&mut w, &mut ents, &p, e);
        p.time += p.frametime;
    }
    let o = ents.v(e, V::Origin);
    let down = w.trace(&ents, o, ents.v(e, V::Mins), ents.v(e, V::Maxs), o - Vec3::new(0.0, 0.0, 1.0), MoveType::Normal, Some(e));
    assert!(down.fraction < 1.0 && down.plane_normal.z > 0.7, "{o:?} {down:?}");
    assert!(w.test_entity_position(&ents, e).is_none());
    // Toss item comes to rest with FL_ONGROUND.
    let t = ents.spawn();
    ents.set_v(t, V::Origin, ps + Vec3::new(0.0, 0.0, 10.0));
    ents.set_v(t, V::Mins, Vec3::splat(-4.0));
    ents.set_v(t, V::Maxs, Vec3::splat(4.0));
    ents.set_v(t, V::Velocity, Vec3::new(50.0, 30.0, 200.0));
    ents.set_f(t, F::Solid, SOLID_BBOX as f32);
    ents.set_f(t, F::MoveType, MOVETYPE_BOUNCE as f32);
    w.link_edict(&mut ents, t, false);
    for _ in 0..1000 {
        phys::physics_entity(&mut w, &mut ents, &p, t);
        p.time += p.frametime;
    }
    assert!(ents.flags(t) & FL_ONGROUND != 0);
}

#[test]
fn ad_bsp2() {
    let Some(f) = fs(Some("ad")) else { return };
    let mut found = 0;
    for name in f.list("maps/", "bsp") {
        let Some(d) = f.open(&name) else { continue };
        let Ok(b) = Bsp::parse(&d) else { continue };
        if b.version != Some(BspVersion::Bsp2) {
            continue;
        }
        let cm = CollisionModel::from_bsp(&b);
        let ents = parse_entities(&b.entities).unwrap();
        if let Some(_e) = ents.iter().find(|e| e.classname() == Some("info_player_start")) {
            assert_eq!(cm.point_contents(player_start(&b)), CONTENTS_EMPTY, "{name}");
        }
        check_traces(&cm, 1, 99);
        found += 1;
        if found == 2 {
            break;
        }
    }
    assert!(found > 0, "no BSP2 map in ad");
}
