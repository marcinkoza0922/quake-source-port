use super::*;

use qworld::hull::{CNode, HPlane, SubModel};

/// World: empty. Submodel 1: solid box x ±16, y ±4, z ±16.
fn box_model() -> CollisionModel {
    let pl = |n: [f32; 3], d: f32| HPlane { normal: Vec3::from(n), dist: d, kind: 3 };
    let planes = vec![
        pl([1.0, 0.0, 0.0], 100_000.0),
        pl([1.0, 0.0, 0.0], 16.0),
        pl([-1.0, 0.0, 0.0], 16.0),
        pl([0.0, 1.0, 0.0], 4.0),
        pl([0.0, -1.0, 0.0], 4.0),
        pl([0.0, 0.0, 1.0], 16.0),
        pl([0.0, 0.0, -1.0], 16.0),
    ];
    // node 0: world (all empty); nodes 1..=6: front empty, back next, last back solid
    let mut hull0 = vec![CNode { plane: 0, children: [-1, -1] }];
    for i in 1..=6 {
        hull0.push(CNode { plane: i, children: [-1, if i == 6 { -2 } else { i as i32 + 1 }] });
    }
    let sub = |h: i32, mins: [f32; 3], maxs: [f32; 3]| SubModel {
        mins: Vec3::from(mins),
        maxs: Vec3::from(maxs),
        origin: Vec3::ZERO,
        headnode: [h, 0, 0, 0],
    };
    CollisionModel {
        planes,
        hull0,
        submodels: vec![sub(0, [-1e5; 3], [1e5; 3]), sub(1, [-16.0, -4.0, -16.0], [16.0, 4.0, 16.0])],
        ..Default::default()
    }
}

fn ent(yaw: f32) -> BrushEnt {
    let origin = Vec3::new(100.0, 0.0, 0.0);
    let axis = (yaw != 0.0).then(|| {
        let (f, r, u) = qcore::math::angle_vectors(Vec3::new(0.0, yaw, 0.0));
        [f, r, u]
    });
    BrushEnt { entity: 7, submodel: 1, origin, axis, mins: origin - Vec3::splat(17.0), maxs: origin + Vec3::splat(17.0) }
}

#[test]
fn traces_hit_brush_entities() {
    let cm = box_model();
    let bsp = Bsp::default();
    for (yaw, hit_x) in [(0.0, 84.0), (90.0, 96.0)] {
        let ents = [ent(yaw)];
        let w = BspWorld { bsp: &bsp, cm: &cm, ents: &ents };
        let t = w.trace_line(Vec3::ZERO, Vec3::new(200.0, 0.0, 0.0));
        assert_eq!(t.entity, 7, "yaw {yaw}");
        assert!((t.endpos.x - hit_x).abs() < 0.1, "yaw {yaw}: {:?}", t.endpos);
        assert!((t.normal - Vec3::new(-1.0, 0.0, 0.0)).length() < 1e-3, "yaw {yaw}: {:?}", t.normal);
        // Missing it entirely.
        let t = w.trace_line(Vec3::new(0.0, 50.0, 0.0), Vec3::new(200.0, 50.0, 0.0));
        assert_eq!((t.frac, t.entity), (1.0, 0));
    }
    let ents = [ent(0.0)];
    let w = BspWorld { bsp: &bsp, cm: &cm, ents: &ents };
    assert_eq!(w.entity_origin(7), Some(Vec3::new(100.0, 0.0, 0.0)));
    assert_eq!(fte_contents(-3), qpart::FTECONTENTS_WATER);
    assert_eq!(fte_contents(-2), qpart::FTECONTENTS_SOLID);
}
