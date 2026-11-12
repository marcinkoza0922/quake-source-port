use super::*;

/// A world that is solid below z = 0 (one clip node on the z plane).
fn floor() -> CollisionModel {
    use crate::hull::{CNode, SubModel};
    CollisionModel {
        planes: vec![HPlane { normal: Vec3::new(0.0, 0.0, 1.0), dist: 0.0, kind: 2 }],
        hull0: vec![CNode { plane: 0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] }],
        // Hull 1 is the floor raised by the player's mins (24 units).
        clipnodes: vec![CNode { plane: 1, children: [CONTENTS_EMPTY, CONTENTS_SOLID] }],
        submodels: vec![SubModel { mins: Vec3::splat(-4096.0), maxs: Vec3::splat(4096.0), origin: Vec3::ZERO, headnode: [0, 0, 0, 0] }],
        ..Default::default()
    }
    .with_plane(HPlane { normal: Vec3::new(0.0, 0.0, 1.0), dist: 24.0, kind: 2 })
}

trait WithPlane {
    fn with_plane(self, p: HPlane) -> Self;
}
impl WithPlane for CollisionModel {
    fn with_plane(mut self, p: HPlane) -> Self {
        self.planes.push(p);
        self
    }
}

#[test]
fn falls_lands_and_runs() {
    let w = floor();
    let ents = [PhysEnt { origin: Vec3::ZERO, shape: PhysShape::Model(&w, 0) }];
    let mv = MoveVars::default();
    let mut s = PlayerState { origin: Vec3::new(0.0, 0.0, 100.0), ..Default::default() };
    let idle = MoveCmd { msec: 13, ..Default::default() };
    for _ in 0..200 {
        s = predict(&ents, &mv, &s, &idle, false, false);
    }
    assert!(s.onground && (s.origin.z - 24.0).abs() < 0.1, "{s:?}");
    let run = MoveCmd { msec: 13, forwardmove: 400.0, ..Default::default() };
    for _ in 0..100 {
        s = predict(&ents, &mv, &s, &run, false, false);
    }
    // Capped at maxspeed along +x (yaw 0).
    assert!((s.velocity.x - 320.0).abs() < 1.0 && s.velocity.y.abs() < 0.01, "{s:?}");
    let jump = MoveCmd { buttons: BUTTON_JUMP, ..run };
    let j = predict(&ents, &mv, &s, &jump, false, false);
    assert!(!j.onground && j.velocity.z > 200.0, "{j:?}");
    // Holding jump doesn't pogo.
    let mut s = j;
    for _ in 0..120 {
        s = predict(&ents, &mv, &s, &jump, false, false);
    }
    assert!(s.onground, "{s:?}");
    let again = predict(&ents, &mv, &s, &jump, false, false);
    assert!(again.onground);
}
