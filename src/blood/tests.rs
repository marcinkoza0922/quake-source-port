use super::*;
use mesh::*;

#[test]
fn clipping_keeps_barycentric_attachment_and_uv_bounds() {
    let tri = [
        Vec3::new(-3.0, -3.0, 0.0),
        Vec3::new(3.0, -3.0, 0.0),
        Vec3::new(0.0, 3.0, 0.0),
    ];
    let p = project(&tri, Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 1.0);
    assert!(p.len() >= 4);
    for v in p {
        assert!(v.uv.iter().all(|&u| (-1e-5..=1.00001).contains(&u)));
        let reconstructed = tri[0] * v.bary.x + tri[1] * v.bary.y + tri[2] * v.bary.z;
        assert!((reconstructed - v.pos).length() < 1e-5);
        assert!((v.bary.x + v.bary.y + v.bary.z - 1.0).abs() < 1e-5);
    }
    assert!(
        project(
            &tri,
            Vec3::new(20.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            1.0
        )
        .is_empty()
    );
}

#[test]
fn ray_tests_segment_and_both_sides_of_corpse_triangle() {
    let p = [
        Vec3::new(-1.0, -1.0, 0.0),
        Vec3::new(1.0, -1.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    let (a, b) = (Vec3::new(0.0, 0.0, 2.0), Vec3::new(0.0, 0.0, -2.0));
    assert!((ray(a, b, p).unwrap().0 - 0.5).abs() < 1e-6);
    assert!(ray(b, a, p).is_some());
    assert!(ray(a, Vec3::new(0.0, 0.0, 1.0), p).is_none());
    assert!(ray(Vec3::new(3.0, 0.0, 2.0), Vec3::new(3.0, 0.0, -2.0), p).is_none());
}

#[test]
fn blood_mask_has_clear_borders_and_solid_center() {
    let t = appearance::texture();
    assert_eq!(t.len(), appearance::WIDTH * appearance::WIDTH * 4);
    assert_eq!(t[(32 * appearance::WIDTH + 32) * 4 + 3], 255);
    for i in 0..64 {
        for index in [
            i,
            63 * appearance::WIDTH + i,
            i * appearance::WIDTH,
            i * appearance::WIDTH + 63,
        ] {
            assert_eq!(t[index * 4 + 3], 0);
        }
    }
}

#[test]
fn mesh_animation_and_ragdoll_pose_use_identical_vertex_indices() {
    use qformats::mdl::{Frame, Pose, TriVert};
    let v = TriVert {
        v: [10, 20, 30],
        normal: 0,
    };
    let pose = Pose {
        name: "death1".into(),
        bboxmin: v,
        bboxmax: v,
        verts: vec![v],
    };
    let m = Mdl {
        scale: [1.0; 3],
        scale_origin: [0.0; 3],
        bounding_radius: 1.0,
        eye_position: [0.0; 3],
        skin_width: 1,
        skin_height: 1,
        num_verts: 1,
        synctype: 0,
        flags: 0,
        size: 1.0,
        skins: vec![],
        stverts: vec![],
        triangles: vec![],
        frames: vec![Frame {
            bboxmin: v,
            bboxmax: v,
            intervals: vec![],
            poses: vec![pose],
        }],
    };
    let mut e = RenderEntity {
        origin: [5.0, 6.0, 7.0],
        lerp: 1.0,
        ..Default::default()
    };
    assert_eq!(positions(&m, &e, &[], 0.0)[0], Vec3::new(15.0, 26.0, 37.0));
    e.dyn_pose = Some(qrender::DynPose {
        ofs: 1,
        origin: [5.0, 6.0, 7.0],
        scale: [2.0; 3],
        radius: 100.0,
    });
    assert_eq!(
        positions(&m, &e, &[0, 10 | (20 << 8) | (30 << 16)], 0.0)[0],
        Vec3::new(25.0, 46.0, 67.0)
    );
}

pub(super) struct MovingFloor(pub(super) Vec3);
impl World for MovingFloor {
    fn trace_line(&self, a: Vec3, b: Vec3) -> qpart::Trace {
        let f = (a.z - self.0.z) / (a.z - b.z);
        qpart::Trace {
            frac: f,
            endpos: a.lerp(b, f),
            normal: Vec3::new(0.0, 0.0, 1.0),
            entity: 1,
        }
    }
    fn point_contents(&self, _: Vec3) -> u32 {
        qpart::FTECONTENTS_EMPTY
    }
    fn entity_origin(&self, _: i32) -> Option<Vec3> {
        Some(self.0)
    }
    fn decal_polys(&self, _: i32, _: Vec3, _: f32, _: Vec3, _: bool, out: &mut dyn FnMut(&[Vec3])) {
        out(&[
            Vec3::new(-1000.0, -1000.0, 0.0),
            Vec3::new(1000.0, -1000.0, 0.0),
            Vec3::new(1000.0, 1000.0, 0.0),
            Vec3::new(-1000.0, 1000.0, 0.0),
        ]);
        // A parallel surface behind this one must not receive the decal.
        out(&[
            Vec3::new(-1000.0, -1000.0, -5.0),
            Vec3::new(1000.0, -1000.0, -5.0),
            Vec3::new(0.0, 1000.0, -5.0),
        ]);
    }
}

#[test]
fn splat_clips_to_surface_follows_lift_and_fades() {
    let mut blood = Blood::default();
    let floor = MovingFloor(Vec3::ZERO);
    blood.splat(
        &floor,
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(0.0, 0.0, -1.0),
        0.0,
        5.0,
    );
    assert_eq!(blood.marks.len(), 1);
    assert_eq!(blood.marks[0].patches.len(), 1);
    let set = Settings {
        enabled: true,
        amount: 1,
        size: 1.0,
        lifetime: 10.0,
        limit: 32,
        wounds: true,
        gibs: true,
    };
    let mut first = (vec![], vec![], vec![]);
    blood.draw(set, 0.0, &floor, &HashMap::new(), PicId(0), &mut first);
    assert!(!first.0.is_empty());
    assert!(
        first
            .0
            .iter()
            .all(|v| v.pos[0].hypot(v.pos[1]) <= 9.3 && v.pos[2] > 0.0)
    );
    let mut moved = (vec![], vec![], vec![]);
    blood.draw(
        set,
        9.5,
        &MovingFloor(Vec3::new(12.0, 0.0, 8.0)),
        &HashMap::new(),
        PicId(0),
        &mut moved,
    );
    for (a, b) in first.0.iter().zip(&moved.0) {
        assert!((b.pos[0] - a.pos[0] - 12.0).abs() < 1e-5);
        assert!((b.pos[2] - a.pos[2] - 8.0).abs() < 1e-5);
        assert!(b.color[3] < a.color[3]);
    }
}

#[test]
fn soldier_wounds_attach_to_skin_and_remain_on_deformed_mesh() {
    let base =
        std::env::var_os("QUAKE_BASEDIR").unwrap_or_else(|| "vkQuake directory with maps".into());
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    let Some(data) = fs.open("progs/soldier.mdl") else {
        return;
    };
    let m = Mdl::parse(&data).unwrap();
    let e = RenderEntity {
        key: 2,
        model: ModelId(5),
        lerp: 1.0,
        ..Default::default()
    };
    let v = positions(&m, &e, &[], 0.0);
    let tri = m.triangles[0].verts.map(|i| v[i as usize]);
    let p = (tri[0] + tri[1] + tri[2]) / 3.0;
    let n = (tri[1] - tri[0]).cross(tri[2] - tri[0]).normalize();
    let mut blood = Blood::default();
    blood.models.insert(e.model, m);
    blood.wound(&e, &v, p, n, 0.0, 5.0);
    assert_eq!(blood.marks.len(), 1);
    let patch = &blood.marks[0].patches[0];
    let initial = patch.tri.map(|i| v[i]);
    for point in &patch.points {
        let attached =
            initial[0] * point.bary.x + initial[1] * point.bary.y + initial[2] * point.bary.z;
        assert!((attached - point.pos).length() < 0.001);
    }
    let shifted: Vec<_> = v.iter().map(|p| *p + Vec3::new(20.0, 3.0, -1.0)).collect();
    let poses = HashMap::from([(2, (&e, shifted))]);
    let set = Settings {
        enabled: true,
        amount: 1,
        size: 1.0,
        lifetime: 10.0,
        limit: 32,
        wounds: true,
        gibs: true,
    };
    let mut out = (vec![], vec![], vec![]);
    blood.draw(set, 0.0, &qpart::EmptyWorld, &poses, PicId(0), &mut out);
    assert!(!out.0.is_empty());
    let first = Vec3::from(out.0[0].pos);
    assert!((first - patch.points[0].pos - Vec3::new(20.0, 3.0, -1.0)).length() < 0.13);
}

/// BSP face iteration returns adjacent faces as well as the struck face.
/// Sharing a vertex with the impact plane must not let a wall become an
/// infinitely extruded floor decal (or a floor become a wall decal).
struct Corner;
impl World for Corner {
    fn trace_line(&self, a: Vec3, b: Vec3) -> qpart::Trace {
        MovingFloor(Vec3::ZERO).trace_line(a, b)
    }
    fn point_contents(&self, _: Vec3) -> u32 {
        qpart::FTECONTENTS_EMPTY
    }
    fn entity_origin(&self, _: i32) -> Option<Vec3> {
        Some(Vec3::ZERO)
    }
    fn decal_polys(&self, _: i32, _: Vec3, _: f32, _: Vec3, _: bool, out: &mut dyn FnMut(&[Vec3])) {
        out(&[
            Vec3::new(-100.0, -100.0, 0.0),
            Vec3::new(100.0, -100.0, 0.0),
            Vec3::new(100.0, 100.0, 0.0),
            Vec3::new(-100.0, 100.0, 0.0),
        ]);
        // Perpendicular wall: its bottom vertices lie in the floor plane.
        out(&[
            Vec3::new(-2.0, -100.0, 0.0),
            Vec3::new(-2.0, 100.0, 0.0),
            Vec3::new(-2.0, 100.0, 200.0),
            Vec3::new(-2.0, -100.0, 200.0),
        ]);
        // A nearby bevel also crosses the projection without being coplanar.
        out(&[
            Vec3::new(-3.0, -100.0, 0.0),
            Vec3::new(-3.0, 100.0, 0.0),
            Vec3::new(-1.0, 100.0, 2.0),
            Vec3::new(-1.0, -100.0, 2.0),
        ]);
    }
}

#[test]
fn floor_splat_does_not_extrude_up_adjacent_walls_and_bevels() {
    let mut blood = Blood::default();
    blood.splat(
        &Corner,
        Vec3::new(0.0, 0.0, 10.0),
        Vec3::new(0.0, 0.0, -1.0),
        0.0,
        5.0,
    );
    assert_eq!(blood.marks.len(), 1);
    let patches = &blood.marks[0].patches;
    assert_eq!(
        patches.len(),
        1,
        "only the struck plane should receive this projection"
    );
    assert!(
        patches
            .iter()
            .flat_map(|p| &p.points)
            .all(|v| v.pos.z.abs() < 0.001)
    );
}

#[test]
fn closest_skin_point_handles_faces_edges_vertices_and_degenerate_triangles() {
    let tri = [
        Vec3::ZERO,
        Vec3::new(10.0, 0.0, 0.0),
        Vec3::new(0.0, 10.0, 0.0),
    ];
    assert_eq!(
        closest_triangle(Vec3::new(2.0, 3.0, 5.0), tri),
        Vec3::new(2.0, 3.0, 0.0)
    );
    assert_eq!(
        closest_triangle(Vec3::new(8.0, 8.0, 0.0), tri),
        Vec3::new(5.0, 5.0, 0.0)
    );
    assert_eq!(
        closest_triangle(Vec3::new(-2.0, -3.0, 0.0), tri),
        Vec3::ZERO
    );
    assert_eq!(
        closest_triangle(Vec3::new(1.0, 2.0, 3.0), [Vec3::ZERO; 3]),
        Vec3::ZERO
    );
}
