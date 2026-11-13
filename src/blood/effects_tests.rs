use super::mesh::positions;
use super::tests::MovingFloor;
use super::*;

fn test_model(name: &str) -> Mdl {
    use qformats::mdl::{Frame, Pose, TriVert, Triangle};
    let verts = [[0, 0, 10], [10, 0, 10], [0, 10, 10]].map(|v| TriVert { v, normal: 0 });
    Mdl {
        scale: [1.0; 3],
        scale_origin: [0.0; 3],
        bounding_radius: 20.0,
        eye_position: [0.0; 3],
        skin_width: 1,
        skin_height: 1,
        num_verts: 3,
        synctype: 0,
        flags: 0,
        size: 1.0,
        skins: vec![],
        stverts: vec![],
        triangles: vec![Triangle {
            faces_front: 1,
            verts: [0, 1, 2],
        }],
        frames: vec![Frame {
            bboxmin: verts[0],
            bboxmax: verts[1],
            intervals: vec![],
            poses: vec![Pose {
                name: name.into(),
                bboxmin: verts[0],
                bboxmax: verts[1],
                verts: verts.to_vec(),
            }],
        }],
    }
}

fn settings() -> Settings {
    Settings {
        enabled: true,
        amount: 5,
        size: 1.0,
        lifetime: 120.0,
        limit: 512,
        wounds: true,
        gibs: true,
    }
}

#[test]
fn shootable_objects_blood_particles_require_a_flesh_mesh() {
    let mut cl = qclient::ClientState::new();
    cl.blood_hits.push([2.0, 2.0, 10.0]);
    cl.shot_impacts.push(([2.0, 2.0, 0.0], false));
    let e = RenderEntity {
        key: 2,
        model: ModelId(1),
        lerp: 1.0,
        ..Default::default()
    };
    let view = Vec3::new(2.0, 2.0, 40.0);
    let mut blood = Blood::default();
    // A bare wall hit or QC's blood emitted by a brush trigger.
    blood.impacts(
        settings(),
        &cl,
        &MovingFloor(Vec3::ZERO),
        &HashMap::new(),
        view,
        &crate::debris::Debris::default(),
    );
    assert!(blood.marks.is_empty());
    // An alias shootable object also must not be treated as flesh.
    let m = test_model("button1");
    let v = positions(&m, &e, &[], 0.0);
    blood.models.insert(e.model, m);
    let poses = HashMap::from([(e.key, (&e, v))]);
    blood.impacts(
        settings(),
        &cl,
        &MovingFloor(Vec3::ZERO),
        &poses,
        view,
        &crate::debris::Debris::default(),
    );
    assert!(blood.marks.is_empty());
    // The same geometry with a monster death sequence must still bleed.
    blood.models.insert(e.model, test_model("death1"));
    blood.impacts(
        settings(),
        &cl,
        &MovingFloor(Vec3::ZERO),
        &poses,
        view,
        &crate::debris::Debris::default(),
    );
    assert!(blood.marks.iter().any(|m| m.owner.is_some()));
    assert!(blood.marks.iter().any(|m| m.owner.is_none()));
}

#[test]
fn decal_atlas_has_distinct_shapes_and_no_bleeding_between_cells() {
    use appearance::*;
    let t = texture();
    let mut masks = Vec::new();
    for tile in 0..GRID * GRID {
        let mut mask = Vec::new();
        for y in 0..TILE {
            for x in 0..TILE {
                let alpha = t[((tile / GRID * TILE + y) * WIDTH + tile % GRID * TILE + x) * 4 + 3];
                if x == 0 || y == 0 || x == TILE - 1 || y == TILE - 1 {
                    assert_eq!(alpha, 0);
                }
                mask.push(alpha);
            }
        }
        assert!(mask.iter().filter(|&&a| a > 0).count() > 100);
        assert!(!masks.contains(&mask));
        masks.push(mask);
        for uv in [[0.0, 0.0], [1.0, 1.0]] {
            let p = appearance::uv(uv, tile as u8);
            assert!(p.iter().all(|&v| v > 0.0 && v < 1.0));
        }
    }
}

#[test]
fn explosive_gib_spray_reaches_distant_surfaces() {
    let mut blood = Blood::default();
    // Ordinary hit rays cannot reach this floor 200 units below.
    let floor = MovingFloor(Vec3::new(0.0, 0.0, -200.0));
    blood.splat(&floor, Vec3::ZERO, Vec3::new(0.0, 0.0, -1.0), 0.0, 5.0);
    assert!(blood.marks.is_empty());
    blood.burst(&floor, Vec3::ZERO, 0.0, settings());
    assert!(
        blood.marks.len() >= 10,
        "expected a dense burst: {}",
        blood.marks.len()
    );
    assert!(
        blood
            .marks
            .iter()
            .flat_map(|m| &m.patches)
            .flat_map(|p| &p.points)
            .any(|p| p.pos.x.hypot(p.pos.y) > 112.0)
    );
}

#[test]
fn only_an_explosion_that_gibs_flesh_produces_a_burst_once() {
    let mut cl = qclient::ClientState::new();
    cl.model_precache = vec![String::new(), "progs/gib1.mdl".into()];
    cl.model_flags = vec![0, 4];
    cl.shot_impacts.push(([0.0, 0.0, 10.0], true));
    let mut blood = Blood::default();
    blood
        .actors
        .insert(2, (ModelId(1), Vec3::new(0.0, 0.0, 10.0)));
    let world = MovingFloor(Vec3::ZERO);
    let debris = crate::debris::Debris::default();
    // An explosion alone must not bleed, even next to a tracked actor.
    blood.impacts(
        settings(),
        &cl,
        &world,
        &HashMap::new(),
        Vec3::ZERO,
        &debris,
    );
    assert!(blood.marks.is_empty());
    blood
        .actors
        .insert(2, (ModelId(1), Vec3::new(0.0, 0.0, 10.0)));
    cl.entities.resize_with(3, Default::default);
    cl.entities[2].model = 1;
    cl.entities[2].origin = [0.0, 0.0, 10.0];
    cl.entities[2].msgtime = cl.mtime[0];
    blood.impacts(
        settings(),
        &cl,
        &world,
        &HashMap::new(),
        Vec3::ZERO,
        &debris,
    );
    assert!(blood.marks.len() >= 20);
    let count = blood.marks.len();
    blood.impacts(
        settings(),
        &cl,
        &world,
        &HashMap::new(),
        Vec3::ZERO,
        &debris,
    );
    assert_eq!(
        blood.marks.len(),
        count,
        "persistent gibs must not repeatedly burst"
    );
}

#[test]
fn living_monster_bbox_hit_bleeds_when_shot_misses_animated_triangles() {
    let mut m = test_model("stand1");
    let mut death = m.frames[0].clone();
    death.poses[0].name = "death1".into();
    m.frames.push(death);
    let e = RenderEntity {
        key: 2,
        model: ModelId(1),
        lerp: 1.0,
        ..Default::default()
    };
    let verts = positions(&m, &e, &[], 0.0);
    // Inside the monster's box, outside the triangle x+y <= 10.
    let mut cl = qclient::ClientState::new();
    cl.blood_hits.push([8.0, 8.0, 10.0]);
    let mut blood = Blood::default();
    blood.models.insert(e.model, m);
    let poses = HashMap::from([(e.key, (&e, verts))]);
    blood.impacts(
        settings(),
        &cl,
        &MovingFloor(Vec3::ZERO),
        &poses,
        Vec3::new(8.0, 8.0, 40.0),
        &crate::debris::Debris::default(),
    );
    assert!(
        blood.marks.iter().any(|m| m.owner.is_some()),
        "bbox damage should stain the closest skin surface"
    );
    assert!(
        blood.marks.iter().any(|m| m.owner.is_none()),
        "bbox damage should spray onto surrounding surfaces"
    );
}

#[test]
fn near_misses_and_blood_behind_cover_do_not_use_live_skin_fallback() {
    struct Cover;
    impl World for Cover {
        fn trace_line(&self, a: Vec3, b: Vec3) -> qpart::Trace {
            qpart::Trace {
                frac: 0.0,
                endpos: a,
                normal: (a - b).normalize(),
                entity: 0,
            }
        }
        fn point_contents(&self, _: Vec3) -> u32 {
            qpart::FTECONTENTS_EMPTY
        }
    }
    let mut m = test_model("stand1");
    let mut death = m.frames[0].clone();
    death.poses[0].name = "death1".into();
    m.frames.push(death);
    let e = RenderEntity {
        key: 2,
        model: ModelId(1),
        lerp: 1.0,
        ..Default::default()
    };
    let verts = positions(&m, &e, &[], 0.0);
    let mut blood = Blood::default();
    blood.models.insert(e.model, m);
    let poses = HashMap::from([(e.key, (&e, verts))]);
    let mut cl = qclient::ClientState::new();
    let view = Vec3::new(8.0, 8.0, 40.0);
    cl.shot_impacts.push(([8.0, 8.0, 0.0], false));
    let debris = crate::debris::Debris::default();
    blood.impacts(
        settings(),
        &cl,
        &MovingFloor(Vec3::ZERO),
        &poses,
        view,
        &debris,
    );
    assert!(
        blood.marks.is_empty(),
        "a near miss on a live enemy is still a wall hit"
    );
    cl.shot_impacts.clear();
    cl.blood_hits.push([8.0, 8.0, 10.0]);
    blood.impacts(settings(), &cl, &Cover, &poses, view, &debris);
    assert!(
        blood.marks.is_empty(),
        "a nearby monster behind cover must not claim trigger blood"
    );
    cl.blood_hits[0] = [100.0, 100.0, 10.0];
    blood.impacts(
        settings(),
        &cl,
        &MovingFloor(Vec3::ZERO),
        &poses,
        view,
        &debris,
    );
    assert!(
        blood.marks.is_empty(),
        "unmatched blood must not jump to a distant monster"
    );
}
