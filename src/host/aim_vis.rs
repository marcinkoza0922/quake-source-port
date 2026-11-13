//! Aim visibility includes the map and moving inline brush models (doors/lifts).
use qcore::math::{Vec3, angle_vectors};
use qinput::aim::Visibility;
use qworld::hull::CollisionModel;

pub(super) struct WorldVis<'a> {
    cm: &'a CollisionModel,
    brushes: Vec<(usize, Vec3, [Vec3; 3])>,
}

impl<'a> WorldVis<'a> {
    pub(super) fn new(cm: &'a CollisionModel, cl: &qclient::ClientState) -> Self {
        let brushes = cl.entities.iter().filter_map(|e| {
            let sub = cl.model_name(e.model)?.strip_prefix('*')?.parse::<usize>().ok()?;
            if sub == 0 || sub >= cm.submodels.len() { return None; }
            let (forward, right, up) = angle_vectors(Vec3::from_array(e.angles));
            Some((sub, Vec3::from_array(e.origin), [forward, -right, up]))
        }).collect();
        Self { cm, brushes }
    }
}

impl Visibility for WorldVis<'_> {
    fn visible(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        let (from, to) = (Vec3::from_array(from), Vec3::from_array(to));
        let clear = |t: qworld::hull::Trace| t.fraction >= 1.0 && !t.startsolid && !t.allsolid;
        if !clear(self.cm.hull(0, 0).trace(from, to)) { return false; }
        self.brushes.iter().all(|(sub, origin, basis)| {
            let local = |p: Vec3| {
                let d = p - *origin;
                Vec3::new(d.dot(basis[0]), d.dot(basis[1]), d.dot(basis[2]))
            };
            clear(self.cm.hull(*sub, 0).trace(local(from), local(to)))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qworld::hull::{CNode, HPlane, SubModel};
    use qworld::{CONTENTS_EMPTY, CONTENTS_SOLID};

    #[test]
    fn rotating_and_opening_door_changes_visibility() {
        let empty = SubModel { mins: Vec3::splat(-100.0), maxs: Vec3::splat(100.0), origin: Vec3::ZERO, headnode: [CONTENTS_EMPTY; 4] };
        let cm = CollisionModel {
            planes: vec![HPlane { normal: Vec3::new(1.0, 0.0, 0.0), dist: 0.0, kind: 0 }],
            hull0: vec![CNode { plane: 0, children: [CONTENTS_EMPTY, CONTENTS_SOLID] }],
            submodels: vec![empty, SubModel { headnode: [0; 4], ..empty }],
            ..Default::default()
        };
        let mut cl = qclient::ClientState::new();
        cl.model_precache.push("*1".into());
        cl.entities[0].model = 1;
        cl.entities[0].angles = [0.0, 90.0, 0.0];
        let ray = ([0.0, 10.0, 0.0], [0.0, -10.0, 0.0]);
        assert!(!WorldVis::new(&cm, &cl).visible(ray.0, ray.1));
        cl.entities[0].origin = [0.0, -20.0, 0.0];
        assert!(WorldVis::new(&cm, &cl).visible(ray.0, ray.1));
        cl.entities[0].model = 0;
        assert!(WorldVis::new(&cm, &cl).visible(ray.0, ray.1));
    }
}
