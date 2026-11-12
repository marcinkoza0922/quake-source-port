//! Evidence at each assist, rather than inferred causes from aggregate totals.
use crate::nav::{Blocked, EdgeKind, Nav, path_to};
use qcore::math::Vec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cause {
    OffGraph,
    MissingApproach,
    Disconnected,
    Prerequisite,
    MoverBlocked,
    ExecutionBlocked,
    Reachable,
}

impl Cause {
    pub const ALL: [Self; 7] = [Self::OffGraph, Self::MissingApproach, Self::Disconnected, Self::Prerequisite, Self::MoverBlocked, Self::ExecutionBlocked, Self::Reachable];
    pub fn name(self) -> &'static str {
        match self {
            Self::OffGraph => "off_graph",
            Self::MissingApproach => "missing_approach",
            Self::Disconnected => "disconnected",
            Self::Prerequisite => "prerequisite",
            Self::MoverBlocked => "mover_blocked",
            Self::ExecutionBlocked => "execution_blocked",
            Self::Reachable => "reachable_unfinished",
        }
    }
}

pub struct Assist {
    pub time: f64,
    pub cause: Cause,
    pub goal: Option<u32>,
    pub kind: String,
    pub classname: String,
    pub reason: String,
    pub from: Vec3,
    pub to: Vec3,
    pub door: Option<u32>,
}

/// Reachability evidence only: a blocked ride does not prove a timing bug.
/// Prefer the graph's cheapest approach in each pass, including firing nodes.
pub fn classify(nav: &Nav, src: Option<u32>, targets: &[u32], gates: &Blocked, blocked: &Blocked) -> (Cause, Option<(u32, u32)>) {
    let Some(src) = src else { return (Cause::OffGraph, None) };
    if targets.is_empty() { return (Cause::MissingApproach, None); }
    let nearest = |dist: &[f32]| targets.iter().copied().filter(|&n| dist[n as usize].is_finite()).min_by(|&a, &b| dist[a as usize].total_cmp(&dist[b as usize]));
    let (dist, prev) = nav.dijkstra(src, &Blocked::default(), 0);
    let Some(n) = nearest(&dist) else { return (Cause::Disconnected, None) };
    let raw = path_to(&prev, n);
    let (dist, prev) = nav.dijkstra(src, gates, 0);
    let Some(n) = nearest(&dist) else {
        return (Cause::Prerequisite, raw.windows(2).map(|w| (w[0], w[1])).find(|e| gates.contains(e)));
    };
    let gated_path = path_to(&prev, n);
    let combined = gates.union(blocked).copied().collect();
    let (dist, _) = nav.dijkstra(src, &combined, 0);
    if nearest(&dist).is_some() { return (Cause::Reachable, None); }
    let edge = gated_path.windows(2).map(|w| (w[0], w[1])).find(|e| blocked.contains(e));
    let mover = edge.is_some_and(|(a, b)| nav.edge(a, b).is_some_and(|e| e.kind == EdgeKind::Ride));
    (if mover { Cause::MoverBlocked } else { Cause::ExecutionBlocked }, edge)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinguishes_geometry_prerequisites_execution_and_movers() {
        let mut nav = Nav::empty(Vec3::ZERO, Vec::new());
        let a = nav.add(Vec3::ZERO);
        let b = nav.add(Vec3::new(64.0, 0.0, 0.0));
        let c = nav.add(Vec3::new(128.0, 0.0, 0.0));
        let empty = Blocked::default();
        let blocked = Blocked::from_iter([(a, b)]);
        assert_eq!(classify(&nav, None, &[b], &empty, &empty).0, Cause::OffGraph);
        assert_eq!(classify(&nav, Some(a), &[], &empty, &empty).0, Cause::MissingApproach);
        assert_eq!(classify(&nav, Some(a), &[b], &empty, &empty).0, Cause::Disconnected);
        nav.link(a, b, EdgeKind::Walk, 64.0);
        assert_eq!(classify(&nav, Some(a), &[b], &blocked, &empty), (Cause::Prerequisite, Some((a, b))));
        assert_eq!(classify(&nav, Some(a), &[b], &empty, &blocked).0, Cause::ExecutionBlocked);
        assert_eq!(classify(&nav, Some(a), &[b, c], &empty, &empty).0, Cause::Reachable);
        nav.link(b, c, EdgeKind::Ride, 64.0);
        assert_eq!(classify(&nav, Some(a), &[c], &empty, &Blocked::from_iter([(b, c)])).0, Cause::MoverBlocked);
        // A reachable alternative approach wins over a blocked touch node.
        assert_eq!(classify(&nav, Some(a), &[a, b], &blocked, &empty).0, Cause::Reachable);
    }
}
