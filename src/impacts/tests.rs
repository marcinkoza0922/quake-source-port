use super::*;
struct Floor;
impl World for Floor {
    fn trace_line(&self, a: Vec3, b: Vec3) -> qpart::Trace {
        let f = if b.z < 0.0 && a.z >= 0.0 {
            a.z / (a.z - b.z)
        } else {
            1.0
        };
        qpart::Trace {
            frac: f,
            endpos: a.lerp(b, f),
            normal: Vec3::new(0.0, 0.0, 1.0),
            entity: 0,
        }
    }
    fn point_contents(&self, _: Vec3) -> u32 {
        0
    }
    fn entity_origin(&self, e: i32) -> Option<Vec3> {
        (e == 7).then_some(Vec3::new(20.0, 0.0, 0.0))
    }
    fn decal_polys(&self, _: i32, _: Vec3, _: f32, _: Vec3, _: bool, _: &mut dyn FnMut(&[Vec3])) {}
}
fn nail() -> Nail {
    Nail {
        born: 0.0,
        pos: Vec3::new(0.0, 0.0, 12.0),
        dir: Vec3::new(1.0, 0.0, 0.0),
        vel: Vec3::new(40.0, 0.0, 30.0),
        anchor: None,
        sleeping: false,
        support: None,
    }
}
#[test]
fn loose_nail_falls_bounces_and_sleeps() {
    let mut effects = Impacts::default();
    effects.nails.push_back(nail());
    let mut bounce = false;
    for _ in 0..600 {
        let vz = effects.nails[0].vel.z;
        effects.step_nails(&Floor, &HashMap::new(), 1.0 / 120.0, 800.0);
        let n = &effects.nails[0];
        bounce |= vz < 0.0 && n.vel.z > 0.0;
        assert!(n.pos.z >= 0.0);
    }
    assert!(bounce);
    assert!(effects.nails[0].sleeping);
    assert!(effects.nails[0].pos.x > 0.0);
}
#[test]
fn mesh_nail_follows_triangle_deformation_and_rejects_replaced_model() {
    let mut effects = Impacts::default();
    let mut n = nail();
    n.anchor = Some(Anchor::Mesh {
        key: 5,
        model: ModelId(2),
        tri: [0, 1, 2],
        bary: Vec3::new(0.5, 0.25, 0.25),
        side: 1.0,
    });
    effects.nails.push_back(n);
    let mut e = RenderEntity {
        key: 5,
        model: ModelId(2),
        ..Default::default()
    };
    let verts = vec![
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::new(8.0, 0.0, 8.0),
        Vec3::new(0.0, 8.0, 4.0),
    ];
    let mut poses = HashMap::new();
    poses.insert(5, (&e, verts.clone()));
    effects.step_nails(&Floor, &poses, 0.016, 800.0);
    assert!((effects.nails[0].pos - Vec3::new(2.0, 2.0, 5.0)).length() < 0.001);
    drop(poses);
    e.model = ModelId(3);
    let poses = HashMap::from([(5, (&e, verts))]);
    effects.step_nails(&Floor, &poses, 0.016, 800.0);
    assert!(effects.nails.is_empty());
}
#[test]
fn stuck_nail_draws_at_brush_origin() {
    let mut effects = Impacts::default();
    let mut n = nail();
    n.anchor = Some(Anchor::Brush(7));
    effects.nails.push_back(n);
    let mut out = (vec![], vec![], vec![]);
    effects.draw(&Floor, &HashMap::new(), PicId(0), 0.0, 120.0, &mut out);
    assert!(!out.0.is_empty());
    assert!(out.0.iter().all(|v| v.pos[0] >= 13.0));
}
struct Corner;
impl World for Corner {
    fn trace_line(&self, a: Vec3, b: Vec3) -> qpart::Trace {
        Floor.trace_line(a, b)
    }
    fn point_contents(&self, _: Vec3) -> u32 {
        0
    }
    fn entity_origin(&self, _: i32) -> Option<Vec3> {
        None
    }
    fn decal_polys(&self, _: i32, _: Vec3, _: f32, _: Vec3, _: bool, out: &mut dyn FnMut(&[Vec3])) {
        out(&[
            Vec3::new(-100.0, -100.0, 0.0),
            Vec3::new(100.0, -100.0, 0.0),
            Vec3::new(100.0, 100.0, 0.0),
            Vec3::new(-100.0, 100.0, 0.0),
        ]);
        // Adjoining wall shares the hit plane at two vertices.
        out(&[
            Vec3::new(0.0, -100.0, 0.0),
            Vec3::new(0.0, 100.0, 0.0),
            Vec3::new(0.0, 100.0, 100.0),
        ]);
    }
}
#[test]
fn marks_clip_to_surface_without_extruding_around_corners() {
    let mut effects = Impacts::default();
    effects.surface(
        &Corner,
        Vec3::new(0.0, 0.0, 4.0),
        Vec3::new(0.0, 0.0, -4.0),
        0,
        0.0,
    );
    assert_eq!(effects.marks.len(), 1);
    for p in &effects.marks[0].points {
        assert_eq!(p.pos.z, 0.0);
        assert!(p.pos.x.abs() <= 2.501 && p.pos.y.abs() <= 2.501);
        assert!(p.uv.iter().all(|v| (-0.001..=1.001).contains(v)));
    }
}

struct Lift {
    origin: Option<Vec3>,
}
impl World for Lift {
    fn trace_line(&self, a: Vec3, b: Vec3) -> qpart::Trace {
        if let Some(origin) = self.origin {
            let mut tr = Floor.trace_line(a - origin, b - origin);
            tr.endpos += origin;
            tr.entity = 7;
            tr
        } else {
            Floor.trace_line(a, b)
        }
    }
    fn point_contents(&self, _: Vec3) -> u32 {
        0
    }
    fn entity_origin(&self, entity: i32) -> Option<Vec3> {
        (entity == 7).then_some(self.origin).flatten()
    }
    fn decal_polys(&self, _: i32, _: Vec3, _: f32, _: Vec3, _: bool, _: &mut dyn FnMut(&[Vec3])) {}
}

#[test]
fn resting_nail_rides_lift_in_both_directions_and_falls_when_removed() {
    let mut effects = Impacts::default();
    effects.nails.push_back(nail());
    let poses = HashMap::new();
    let mut lift = Lift {
        origin: Some(Vec3::ZERO),
    };
    for _ in 0..600 {
        effects.step_nails(&lift, &poses, 1.0 / 120.0, 800.0);
    }
    let n = &effects.nails[0];
    assert!(n.sleeping);
    assert_eq!(n.support.map(|s| s.0), Some(7));
    let resting = n.pos;
    for delta in [Vec3::new(16.0, -8.0, 64.0), Vec3::new(32.0, 4.0, 24.0)] {
        lift.origin = Some(delta);
        effects.step_nails(&lift, &poses, 1.0 / 60.0, 800.0);
        let n = &effects.nails[0];
        assert!(n.sleeping);
        assert!((n.pos - resting - delta).length() < 0.001);
    }
    let before = effects.nails[0].pos;
    lift.origin = None;
    effects.step_nails(&lift, &poses, 1.0 / 60.0, 800.0);
    let n = &effects.nails[0];
    assert!(!n.sleeping);
    assert!(n.support.is_none());
    assert!(n.pos.z < before.z);
}
