//! Stateful traversal actions. Deadlines belong to stages, not whole routes.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiftStage {
    ApproachButton,
    ActivateButton,
    ReturnToLift,
    Wait,
    Board,
    Ride,
    Leave,
}

pub struct ButtonTrip {
    pub ent: u32,
    pub shoot: bool,
    pub path: Vec<u32>,
    pub step: usize,
    pub returning: bool,
}

pub struct LiftAction {
    pub edge: (u32, u32),
    pub stage: LiftStage,
    pub since: f64,
    started: f64,
    pub trip: Option<ButtonTrip>,
    /// Do not repeatedly call a button whose activation was acknowledged.
    pub called: bool,
}

impl LiftAction {
    pub fn new(edge: (u32, u32), now: f64) -> Self {
        Self { edge, stage: LiftStage::Wait, since: now, started: now, trip: None, called: false }
    }

    pub fn enter(&mut self, stage: LiftStage, now: f64) {
        if self.stage != stage {
            self.stage = stage;
            self.since = now;
        }
    }

    pub fn expired(&self, now: f64, patience: f64) -> bool {
        now - self.started > 120.0 || now - self.since > match self.stage {
            LiftStage::ApproachButton | LiftStage::ReturnToLift => 30.0,
            LiftStage::ActivateButton => 5.0,
            _ => patience,
        }
    }
}

/// A caller must be reachable without a mover and have a route back. This
/// excludes circular plans such as riding the lift to reach its own button.
pub fn button_trip(nav: &crate::nav::Nav, src: u32, candidates: &[(u32, u32, bool)], blocked: &crate::nav::Blocked, seed: u64) -> Option<ButtonTrip> {
    use crate::nav::{EdgeKind, path_to};
    let mut blocked = blocked.clone();
    for (n, edges) in nav.edges.iter().enumerate() {
        for e in edges {
            if matches!(e.kind, EdgeKind::Ride | EdgeKind::Teleport) { blocked.insert((n as u32, e.to)); }
        }
    }
    let (dist, prev) = nav.dijkstra(src, &blocked, seed);
    let mut best = None;
    for &(ent, n, shoot) in candidates {
        if !dist[n as usize].is_finite() || dist[n as usize] > 1536.0 { continue; }
        let path = path_to(&prev, n);
        if path.windows(2).any(|w| blocked.contains(&(w[1], w[0])) || nav.edge(w[1], w[0]).is_none()) { continue; }
        if best.as_ref().is_none_or(|(cost, _)| dist[n as usize] < *cost) {
            best = Some((dist[n as usize], ButtonTrip { ent, shoot, path, step: 0, returning: false }));
        }
    }
    best.map(|(_, trip)| trip)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caller_detour_avoids_lift_and_one_way_drop() {
        use crate::nav::{Nav, Blocked, EdgeKind};
        use qcore::math::Vec3;
        let mut nav = Nav::empty(Vec3::ZERO, Vec::new());
        let start = nav.add(Vec3::ZERO);
        let corner = nav.add(Vec3::new(64.0, 0.0, 0.0));
        let caller = nav.add(Vec3::new(64.0, 64.0, 0.0));
        let upstairs = nav.add(Vec3::new(0.0, 0.0, 128.0));
        let pit = nav.add(Vec3::new(32.0, 0.0, -128.0));
        for (a, b) in [(start, corner), (corner, caller)] {
            nav.link(a, b, EdgeKind::Walk, 64.0);
            nav.link(b, a, EdgeKind::Walk, 64.0);
        }
        nav.link(start, upstairs, EdgeKind::Ride, 1.0);
        nav.link(upstairs, start, EdgeKind::Ride, 1.0);
        nav.link(start, pit, EdgeKind::Walk, 1.0);
        let candidates = [(10, upstairs, false), (11, pit, false), (12, caller, true)];
        let trip = button_trip(&nav, start, &candidates, &Blocked::default(), 0).unwrap();
        assert_eq!(trip.ent, 12);
        assert_eq!(trip.path, vec![start, corner, caller]);
        assert!(trip.shoot);
        let blocked = Blocked::from_iter([(corner, caller)]);
        assert!(button_trip(&nav, start, &candidates, &blocked, 0).is_none());
    }

    #[test]
    fn lift_stages_have_separate_bounded_deadlines() {
        let mut a = LiftAction::new((1, 2), 0.0);
        a.enter(LiftStage::ApproachButton, 2.0);
        assert!(!a.expired(31.0, 15.0));
        assert!(a.expired(33.0, 15.0));
        a.enter(LiftStage::ActivateButton, 31.0);
        a.enter(LiftStage::ActivateButton, 34.0);
        assert!(a.expired(37.0, 15.0)); // observations must not reset a deadline
        a.enter(LiftStage::ReturnToLift, 37.0);
        assert!(!a.expired(60.0, 15.0));
        a.enter(LiftStage::Wait, 60.0);
        assert!(!a.expired(74.0, 15.0));
        assert!(a.expired(76.0, 15.0));
        a.enter(LiftStage::Board, 76.0);
        a.enter(LiftStage::Ride, 80.0);
        assert!(!a.expired(94.0, 15.0));
        a.enter(LiftStage::Leave, 94.0);
        assert!(!a.expired(95.0, 15.0));
    }
}
