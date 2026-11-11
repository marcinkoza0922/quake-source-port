//! Data-free tests: box hulls, area tree, SV_Move filtering.
mod common;
use common::Ents;
use qworld::hull::{CollisionModel, Hull, SubModel, box_planes};
use qworld::world::MoveType;
use qworld::*;
use std::sync::Arc;

fn empty_world() -> World {
    let cm = CollisionModel {
        submodels: vec![SubModel {
            mins: Vec3::splat(-4096.0),
            maxs: Vec3::splat(4096.0),
            origin: Vec3::ZERO,
            headnode: [-1; 4],
        }],
        ..Default::default()
    };
    World::new(Arc::new(cm))
}

fn boxent(ents: &mut Ents, w: &mut World, org: Vec3, half: f32, solid: i32) -> Ent {
    let e = ents.spawn();
    ents.set_v(e, V::Origin, org);
    ents.set_v(e, V::Mins, Vec3::splat(-half));
    ents.set_v(e, V::Maxs, Vec3::splat(half));
    ents.set_f(e, F::Solid, solid as f32);
    w.link_edict(ents, e, false);
    e
}

#[test]
fn box_hull_trace() {
    let p = box_planes(Vec3::splat(-10.0), Vec3::splat(10.0));
    let h = Hull::for_box(&p);
    assert_eq!(h.point_contents(0, Vec3::ZERO), CONTENTS_SOLID);
    assert_eq!(h.point_contents(0, Vec3::new(11.0, 0.0, 0.0)), CONTENTS_EMPTY);
    let t = h.trace(Vec3::new(-50.0, 0.0, 0.0), Vec3::new(50.0, 0.0, 0.0));
    assert!(!t.startsolid && !t.allsolid);
    assert_eq!(t.plane_normal, Vec3::new(-1.0, 0.0, 0.0));
    assert!((t.endpos.x - (-10.0 - DIST_EPSILON)).abs() < 1e-4, "{:?}", t.endpos);
    let t = h.trace(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));
    assert!(t.allsolid && t.startsolid);
}

#[test]
fn area_move_filters() {
    let mut w = empty_world();
    let mut ents = Ents::default();
    ents.spawn(); // world
    let a = boxent(&mut ents, &mut w, Vec3::new(100.0, 0.0, 0.0), 16.0, SOLID_BBOX);
    let me = boxent(&mut ents, &mut w, Vec3::ZERO, 8.0, SOLID_SLIDEBOX);
    let (s, e) = (Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0));
    let (mn, mx) = (Vec3::splat(-8.0), Vec3::splat(8.0));
    let t = w.trace(&ents, s, mn, mx, e, MoveType::Normal, Some(me));
    assert_eq!(t.ent, Some(a));
    assert!((t.endpos.x - (100.0 - 24.0 - DIST_EPSILON)).abs() < 1e-3);
    assert_eq!(w.trace(&ents, s, mn, mx, e, MoveType::NoMonsters, Some(me)).fraction, 1.0);
    ents.set_e(a, E::Owner, me);
    assert_eq!(w.trace(&ents, s, mn, mx, e, MoveType::Normal, Some(me)).fraction, 1.0);
    ents.set_e(a, E::Owner, 0);
    w.unlink(a);
    assert_eq!(w.trace(&ents, s, mn, mx, e, MoveType::Normal, Some(me)).fraction, 1.0);
    // Missile vs monster gets the 15-unit box.
    w.link_edict(&mut ents, a, false);
    ents.set_flags(a, FL_MONSTER);
    let t = w.trace(&ents, s, Vec3::ZERO, Vec3::ZERO, e, MoveType::Missile, Some(me));
    assert!((t.endpos.x - (100.0 - 31.0 - DIST_EPSILON)).abs() < 1e-3, "{:?}", t.endpos);
}

#[test]
fn triggers_touch() {
    let mut w = empty_world();
    let mut ents = Ents::default();
    ents.spawn();
    let trig = boxent(&mut ents, &mut w, Vec3::new(1000.0, 0.0, 0.0), 32.0, SOLID_TRIGGER);
    ents.ed[trig as usize].touch = true;
    let p = boxent(&mut ents, &mut w, Vec3::ZERO, 16.0, SOLID_SLIDEBOX);
    assert!(ents.touches.is_empty());
    ents.set_v(p, V::Origin, Vec3::new(960.0, 0.0, 0.0));
    w.link_edict(&mut ents, p, true);
    assert_eq!(ents.touches, vec![(trig, p)]);
    // Triggers never block movement.
    let t = w.trace(&ents, Vec3::ZERO, Vec3::ZERO, Vec3::ZERO, Vec3::new(2000.0, 0.0, 0.0), MoveType::Normal, Some(p));
    assert_eq!(t.fraction, 1.0);
}

#[test]
fn clip_velocity_floor() {
    let (v, b) = phys::clip_velocity(Vec3::new(100.0, 0.0, -100.0), Vec3::new(0.0, 0.0, 1.0), 1.0);
    assert_eq!((v, b), (Vec3::new(100.0, 0.0, 0.0), 1));
}
