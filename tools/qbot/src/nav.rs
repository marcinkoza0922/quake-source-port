//! Navigation graph: flood fill over a 32-unit XY grid from the spawn point,
//! using world-only hull 1 traces (brush entities such as doors and lifts are
//! ignored, so closed doors count as open and lift shafts as drops).

use qcore::math::Vec3;
use qworld::hull::CollisionModel;
use qworld::{CONTENTS_LAVA, CONTENTS_SKY, CONTENTS_SLIME, CONTENTS_SOLID};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

pub const GRID: f32 = 32.0;
const STEP: f32 = 18.0;
const JUMP: f32 = 40.0;
const MAX_DROP: f32 = 640.0;
const MAX_NODES: usize = 400_000;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EdgeKind {
    Walk,
    Jump,
}

#[derive(Clone, Copy)]
pub struct Edge {
    pub to: u32,
    pub cost: f32,
    pub kind: EdgeKind,
}

pub struct Nav {
    /// Player origins standing on the floor.
    pub pos: Vec<Vec3>,
    pub edges: Vec<Vec<Edge>>,
    cells: HashMap<(i32, i32), Vec<u32>>,
    origin: Vec3,
    forbid: Vec<Forbid>,
}

/// A box the bot must not enter (level exits).
#[derive(Clone, Copy)]
pub struct Forbid {
    pub mins: Vec3,
    pub maxs: Vec3,
}

impl Forbid {
    fn contains(&self, p: Vec3) -> bool {
        // Expand by the player hull (16 wide, -24..32 tall).
        p.x > self.mins.x - 16.0
            && p.x < self.maxs.x + 16.0
            && p.y > self.mins.y - 16.0
            && p.y < self.maxs.y + 16.0
            && p.z > self.mins.z - 32.0
            && p.z < self.maxs.z + 24.0
    }
}

fn hazard(cm: &CollisionModel, p: Vec3) -> bool {
    let feet = cm.point_contents(p - Vec3::new(0.0, 0.0, 23.0));
    let waist = cm.point_contents(p);
    [feet, waist].iter().any(|&c| c == CONTENTS_LAVA || c == CONTENTS_SLIME || c == CONTENTS_SKY || c == CONTENTS_SOLID)
}

/// Drop a player hull from `p` to the floor below.
fn ground(cm: &CollisionModel, p: Vec3, max: f32) -> Option<Vec3> {
    let h = cm.hull(0, 1);
    let tr = h.trace(p, p - Vec3::new(0.0, 0.0, max));
    if tr.startsolid || tr.allsolid || tr.fraction >= 1.0 || tr.plane_normal.z < 0.7 {
        return None;
    }
    Some(tr.endpos)
}

fn clear(cm: &CollisionModel, a: Vec3, b: Vec3) -> bool {
    let tr = cm.hull(0, 1).trace(a, b);
    !tr.startsolid && !tr.allsolid && tr.fraction >= 1.0
}

impl Nav {
    pub fn build(cm: &CollisionModel, start: Vec3, forbid: Vec<Forbid>) -> Nav {
        let mut nav = Nav { pos: Vec::new(), edges: Vec::new(), cells: HashMap::new(), origin: start, forbid };
        nav.extend(cm, start);
        nav
    }

    /// Flood fill from a new seed at `start` (snapped to the grid). Returns
    /// whether any node was added.
    pub fn extend(&mut self, cm: &CollisionModel, start: Vec3) -> bool {
        let nav = self;
        let (cx, cy) = nav.cell(start);
        let snapped = Vec3::new(nav.origin.x + cx as f32 * GRID, nav.origin.y + cy as f32 * GRID, start.z + 1.0);
        let seed = [snapped, start + Vec3::new(0.0, 0.0, 1.0)]
            .into_iter()
            .filter(|&p| clear(cm, start, p) || p == start + Vec3::new(0.0, 0.0, 1.0))
            .find_map(|p| ground(cm, p, 2048.0).filter(|&g| !hazard(cm, g)));
        let Some(s) = seed else { return false };
        if nav.find_exact(s).is_some() {
            return false;
        }
        let first = nav.pos.len();
        nav.add(s);
        let dirs: [(i32, i32); 8] = [(1, 0), (-1, 0), (0, 1), (0, -1), (1, 1), (1, -1), (-1, 1), (-1, -1)];
        let mut i = first;
        while i < nav.pos.len() && nav.pos.len() < MAX_NODES {
            let p = nav.pos[i];
            let (cx, cy) = nav.cell(p);
            for (dx, dy) in dirs {
                let tx = nav.origin.x + (cx + dx) as f32 * GRID;
                let ty = nav.origin.y + (cy + dy) as f32 * GRID;
                for (kind, rise) in [(EdgeKind::Walk, STEP), (EdgeKind::Jump, JUMP)] {
                    let up = p + Vec3::new(0.0, 0.0, rise);
                    if kind == EdgeKind::Jump && !clear(cm, p, up) {
                        break;
                    }
                    let t = Vec3::new(tx, ty, up.z);
                    if !clear(cm, up, t) {
                        continue;
                    }
                    let Some(g) = ground(cm, t, MAX_DROP) else { break };
                    if g.z - p.z > rise + 0.5 || hazard(cm, g) || nav.forbid.iter().any(|f| f.contains(g)) {
                        break;
                    }
                    if kind == EdgeKind::Walk && g.z - p.z > STEP + 0.5 {
                        continue;
                    }
                    let to = match nav.find_exact(g) {
                        Some(n) => n,
                        None => nav.add(g),
                    };
                    let mut cost = (g - p).length();
                    if kind == EdgeKind::Jump {
                        cost += 32.0;
                    }
                    if p.z - g.z > 64.0 {
                        cost += 16.0;
                    }
                    nav.edges[i].push(Edge { to, cost, kind });
                    break;
                }
            }
            i += 1;
        }
        true
    }

    fn cell(&self, p: Vec3) -> (i32, i32) {
        (((p.x - self.origin.x) / GRID).round() as i32, ((p.y - self.origin.y) / GRID).round() as i32)
    }

    fn find_exact(&self, p: Vec3) -> Option<u32> {
        self.cells.get(&self.cell(p))?.iter().copied().find(|&n| (self.pos[n as usize].z - p.z).abs() < 20.0)
    }

    fn add(&mut self, p: Vec3) -> u32 {
        let n = self.pos.len() as u32;
        self.pos.push(p);
        self.edges.push(Vec::new());
        let c = self.cell(p);
        self.cells.entry(c).or_default().push(n);
        n
    }

    /// The node nearest to `p` within `radius`, preferring nodes at a similar height.
    pub fn nearest(&self, p: Vec3, radius: f32) -> Option<u32> {
        let (cx, cy) = self.cell(p);
        let r = (radius / GRID).ceil() as i32;
        let mut best: Option<(f32, u32)> = None;
        for x in cx - r..=cx + r {
            for y in cy - r..=cy + r {
                for &n in self.cells.get(&(x, y)).into_iter().flatten() {
                    let d = self.pos[n as usize] - p;
                    let score = d.x * d.x + d.y * d.y + 4.0 * d.z * d.z;
                    if score <= radius * radius * 4.0 && best.is_none_or(|b| score < b.0) {
                        best = Some((score, n));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The closest node anywhere (linear scan).
    pub fn nearest_any(&self, p: Vec3) -> Option<u32> {
        (0..self.pos.len() as u32).min_by(|&a, &b| {
            let da = (self.pos[a as usize] - p).length_sq();
            let db = (self.pos[b as usize] - p).length_sq();
            da.total_cmp(&db)
        })
    }

    /// Nodes whose player box overlaps the given box.
    pub fn inside(&self, mins: Vec3, maxs: Vec3) -> Option<u32> {
        let c = (mins + maxs) * 0.5;
        let r = ((maxs - mins).length() * 0.5 + GRID).min(1024.0);
        let (cx, cy) = self.cell(c);
        let k = (r / GRID).ceil() as i32;
        for x in cx - k..=cx + k {
            for y in cy - k..=cy + k {
                for &n in self.cells.get(&(x, y)).into_iter().flatten() {
                    let p = self.pos[n as usize];
                    if p.x + 16.0 > mins.x
                        && p.x - 16.0 < maxs.x
                        && p.y + 16.0 > mins.y
                        && p.y - 16.0 < maxs.y
                        && p.z + 32.0 > mins.z
                        && p.z - 24.0 < maxs.z
                    {
                        return Some(n);
                    }
                }
            }
        }
        None
    }

    /// Dijkstra from `src`: (cost, previous node) per node.
    pub fn dijkstra(&self, src: u32) -> (Vec<f32>, Vec<u32>) {
        let n = self.pos.len();
        let mut dist = vec![f32::INFINITY; n];
        let mut prev = vec![u32::MAX; n];
        let mut q = BinaryHeap::new();
        dist[src as usize] = 0.0;
        q.push(Reverse((0u32, src)));
        while let Some(Reverse((d, u))) = q.pop() {
            let d = f32::from_bits(d);
            if d > dist[u as usize] {
                continue;
            }
            for e in &self.edges[u as usize] {
                let nd = d + e.cost;
                if nd < dist[e.to as usize] {
                    dist[e.to as usize] = nd;
                    prev[e.to as usize] = u;
                    // Non-negative floats order like their bit patterns.
                    q.push(Reverse((nd.to_bits(), e.to)));
                }
            }
        }
        (dist, prev)
    }

    pub fn edge(&self, a: u32, b: u32) -> Option<&Edge> {
        self.edges[a as usize].iter().find(|e| e.to == b)
    }
}

pub fn path_to(prev: &[u32], dst: u32) -> Vec<u32> {
    let mut p = vec![dst];
    let mut n = dst;
    while prev[n as usize] != u32::MAX {
        n = prev[n as usize];
        p.push(n);
    }
    p.reverse();
    p
}
