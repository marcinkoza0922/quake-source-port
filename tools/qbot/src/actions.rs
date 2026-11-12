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

/// Search prerequisite chains without pretending that pressing a button has
/// already changed the world. Execute one reachable leaf, then observe/replan.
pub struct Prerequisites<'a> {
    pub nav: &'a crate::nav::Nav,
    pub gates: &'a crate::nav::Blocked,
    pub doors: &'a std::collections::HashMap<(u32, u32), u32, crate::nav::FxBuild>,
    pub openers: &'a std::collections::HashMap<u32, Vec<usize>, crate::nav::FxBuild>,
    /// Only pending, usable approaches; empty for exhausted/dormant goals.
    pub approaches: &'a [Approach],
    pub blocked: &'a crate::nav::Blocked,
}

pub struct Approach {
    pub touch: Option<u32>,
    pub views: Vec<u32>,
}

/// Preserve touch priority; use the cheapest firing node as a fallback.
pub fn opener_node(touch: Option<u32>, views: &[u32], dist: &[f32]) -> Option<u32> {
    touch.filter(|&n| dist[n as usize].is_finite()).or_else(|| {
        views.iter().copied().filter(|&n| dist[n as usize].is_finite())
            .min_by(|&a, &b| dist[a as usize].total_cmp(&dist[b as usize]))
    })
}

pub struct PrerequisiteStep {
    pub goal: usize,
    pub path: Vec<u32>,
    pub doors: Vec<u32>,
}

impl Prerequisites<'_> {
    /// Bounded search across alternative openers. A recursion stack rejects
    /// circular dependencies; the shared budget bounds branching too.
    pub fn first_step(&self, src: u32, door: u32, seed: u64) -> Option<PrerequisiteStep> {
        let combined = self.blocked.union(self.gates).copied().collect();
        let (dist, prev) = self.nav.dijkstra(src, &combined, seed);
        let (raw_dist, raw_prev) = self.nav.dijkstra(src, self.blocked, seed);
        let mut budget = 64usize;
        let mut stack = Vec::new();
        self.search(door, &dist, &prev, &raw_dist, &raw_prev, &mut stack, &mut budget)
    }

    #[allow(clippy::too_many_arguments)]
    fn search(&self, door: u32, dist: &[f32], prev: &[u32], raw_dist: &[f32], raw_prev: &[u32], stack: &mut Vec<u32>, budget: &mut usize) -> Option<PrerequisiteStep> {
        if stack.len() >= 8 || stack.contains(&door) || *budget == 0 { return None; }
        *budget -= 1;
        stack.push(door);
        let mut best: Option<(f32, PrerequisiteStep)> = None;
        for &goal in self.openers.get(&door).into_iter().flatten() {
            let nodes = &self.approaches[goal];
            let nearest = |ds: &[f32]| opener_node(nodes.touch, &nodes.views, ds);
            let candidate = if let Some(n) = nearest(dist) {
                Some(PrerequisiteStep { goal, path: crate::nav::path_to(prev, n), doors: vec![door] })
            } else if let Some(n) = nearest(raw_dist) {
                let path = crate::nav::path_to(raw_prev, n);
                path.windows(2).find_map(|w| self.doors.get(&(w[0], w[1])))
                    .and_then(|&next| self.search(next, dist, prev, raw_dist, raw_prev, stack, budget))
                    .map(|mut step| { step.doors.insert(0, door); step })
            } else { None };
            if let Some(candidate) = candidate {
                let cost = dist[*candidate.path.last().unwrap() as usize];
                if best.as_ref().is_none_or(|(old, _)| cost < *old) { best = Some((cost, candidate)); }
            }
        }
        stack.pop();
        best.map(|(_, step)| step)
    }
}

#[cfg(test)]
mod prerequisite_tests {
    use super::*;
    use crate::nav::{Blocked, EdgeKind, FxBuild, Nav};
    use qcore::math::Vec3;
    use std::collections::HashMap;

    #[test]
    fn alternatives_chains_and_cycles_use_only_reachable_leaves() {
        let mut nav = Nav::empty(Vec3::ZERO, Vec::new());
        let ns: Vec<_> = (0..4).map(|i| nav.add(Vec3::new(i as f32 * 64.0, 0.0, 0.0))).collect();
        for w in ns.windows(2) { nav.link(w[0], w[1], EdgeKind::Walk, 64.0); }
        let gates = Blocked::from_iter([(ns[0], ns[1]), (ns[1], ns[2])]);
        let doors = HashMap::<_, _, FxBuild>::from_iter([((ns[0], ns[1]), 10), ((ns[1], ns[2]), 20)]);
        // Door 20's first opener is behind door 10, whose first opener
        // depends on itself; its alternative is reachable at the start.
        let mut openers = HashMap::<_, _, FxBuild>::from_iter([(20, vec![0]), (10, vec![1, 2])]);
        let approaches: Vec<_> = [ns[1], ns[2], ns[0]].into_iter().map(|n| Approach { touch: Some(n), views: Vec::new() }).collect();
        let blocked = Blocked::default();
        let planner = Prerequisites { nav: &nav, gates: &gates, doors: &doors, openers: &openers, approaches: &approaches, blocked: &blocked };
        let step = planner.first_step(ns[0], 20, 0).unwrap();
        assert_eq!(step.goal, 2);
        assert_eq!(step.path, vec![ns[0]]);
        assert_eq!(step.doors, vec![20, 10]);
        openers.get_mut(&10).unwrap().pop();
        let planner = Prerequisites { nav: &nav, gates: &gates, doors: &doors, openers: &openers, approaches: &approaches, blocked: &blocked };
        assert!(planner.first_step(ns[0], 20, 0).is_none());
    }

    #[test]
    fn chooses_cheapest_usable_alternative_and_respects_execution_blocks() {
        let mut nav = Nav::empty(Vec3::ZERO, Vec::new());
        let a = nav.add(Vec3::ZERO);
        let b = nav.add(Vec3::new(64.0, 0.0, 0.0));
        let c = nav.add(Vec3::new(128.0, 0.0, 0.0));
        nav.link(a, b, EdgeKind::Walk, 64.0);
        nav.link(a, c, EdgeKind::Walk, 128.0);
        let empty = Blocked::default();
        let doors = HashMap::default();
        let openers = HashMap::from_iter([(10, vec![0, 1, 2])]);
        let approaches: Vec<_> = [None, Some(c), Some(b)].into_iter().map(|touch| Approach { touch, views: Vec::new() }).collect();
        let p = Prerequisites { nav: &nav, gates: &empty, doors: &doors, openers: &openers, approaches: &approaches, blocked: &empty };
        assert_eq!(p.first_step(a, 10, 0).unwrap().goal, 2);
        let blocked = Blocked::from_iter([(a, b)]);
        assert_eq!(Prerequisites { blocked: &blocked, ..p }.first_step(a, 10, 0).unwrap().goal, 1);
    }
}

#[cfg(test)]
mod prerequisite_budget_tests {
    use super::*;
    use crate::nav::{Blocked, EdgeKind, FxBuild, Nav};
    use qcore::math::Vec3;
    use std::collections::HashMap;

    #[test]
    fn prerequisite_depth_is_bounded() {
        let mut nav = Nav::empty(Vec3::ZERO, Vec::new());
        let src = nav.add(Vec3::ZERO);
        let ns: Vec<_> = (0..10).map(|i| nav.add(Vec3::new(64.0, i as f32 * 64.0, 0.0))).collect();
        for &n in &ns { nav.link(src, n, EdgeKind::Walk, 64.0); }
        let doors: HashMap<_, _, FxBuild> = ns.iter().enumerate().map(|(i, &n)| ((src, n), i as u32)).collect();
        let gates: Blocked = doors.keys().copied().collect();
        let openers: HashMap<_, _, FxBuild> = (0..10).map(|i| (i as u32, vec![i])).collect();
        let approaches: Vec<_> = (0..10).map(|i| Approach { touch: Some(ns.get(i + 1).copied().unwrap_or(src)), views: Vec::new() }).collect();
        let blocked = Blocked::default();
        let planner = Prerequisites { nav: &nav, gates: &gates, doors: &doors, openers: &openers, approaches: &approaches, blocked: &blocked };
        assert!(planner.first_step(src, 0, 0).is_none());
        assert_eq!(planner.first_step(src, 2, 0).unwrap().goal, 9);
    }
}
