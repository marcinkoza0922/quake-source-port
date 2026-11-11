//! Navigation graph: flood fill over a 32-unit XY grid from the spawn point,
//! using world-only hull 1 traces (brush entities such as doors and lifts are
//! ignored, so closed doors count as open and lift shafts as drops). Lifts
//! and teleporters are added afterwards as extra nodes with `Ride` and
//! `Teleport` edges (see `bot::add_movers`).

use qcore::math::Vec3;
use qworld::hull::{CollisionModel, Trace};
use std::sync::Arc;
use qworld::{CONTENTS_LAVA, CONTENTS_SKY, CONTENTS_SLIME, CONTENTS_SOLID};
use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

pub const GRID: f32 = 32.0;
const STEP: f32 = 18.0;
const JUMP: f32 = 40.0;
const MAX_DROP: f32 = 640.0;
const MAX_NODES: usize = 400_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EdgeKind {
    Walk,
    Jump,
    /// Stand on a lift (func_plat / vertical func_door) until it arrives.
    Ride,
    /// Walk into a trigger_teleport.
    Teleport,
}

/// FxHash: the grid cell map is hit constantly, SipHash showed up in profiles.
#[derive(Default)]
pub struct Fx(u64);

impl Hasher for Fx {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(b as u64);
        }
    }
    fn write_i32(&mut self, i: i32) {
        self.write_u64(i as u32 as u64);
    }
    fn write_u32(&mut self, i: u32) {
        self.write_u64(i as u64);
    }
    fn write_u64(&mut self, i: u64) {
        self.0 = (self.0.rotate_left(5) ^ i).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

pub type FxBuild = BuildHasherDefault<Fx>;
pub type Blocked = HashSet<(u32, u32), FxBuild>;

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
    cells: HashMap<(i32, i32), Vec<u32>, FxBuild>,
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

/// A static brush entity (func_wall and the like) at a fixed origin.
struct Solid {
    cm: Arc<CollisionModel>,
    sub: usize,
    origin: Vec3,
    mins: Vec3,
    maxs: Vec3,
    /// Movers where they are now: something to stand on, never a wall.
    floor_only: bool,
}

const BUCKET: f32 = 256.0;

/// What the nav graph collides with: the world plus static brush entities
/// (bridges and floors are often func_walls). Movers are left out.
pub struct Geo {
    pub world: Arc<CollisionModel>,
    solids: Vec<Solid>,
    buckets: HashMap<(i32, i32), Vec<u32>, FxBuild>,
}

impl Geo {
    pub fn new(world: Arc<CollisionModel>) -> Geo {
        Geo { world, solids: Vec::new(), buckets: HashMap::default() }
    }

    pub fn add_solid(&mut self, cm: Arc<CollisionModel>, sub: usize, origin: Vec3, mins: Vec3, maxs: Vec3, floor_only: bool) {
        let i = self.solids.len() as u32;
        self.solids.push(Solid { cm, sub, origin, mins, maxs, floor_only });
        let (a, b) = (Self::bucket(mins), Self::bucket(maxs));
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                self.buckets.entry((x, y)).or_default().push(i);
            }
        }
    }

    fn bucket(p: Vec3) -> (i32, i32) {
        ((p.x / BUCKET).floor() as i32, (p.y / BUCKET).floor() as i32)
    }

    /// Solids whose box overlaps `mins..maxs`, each once.
    fn near(&self, mins: Vec3, maxs: Vec3, f: &mut impl FnMut(&Solid)) {
        if self.solids.is_empty() {
            return;
        }
        let (a, b) = (Self::bucket(mins), Self::bucket(maxs));
        let mut seen: Vec<u32> = Vec::new();
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                for &i in self.buckets.get(&(x, y)).into_iter().flatten() {
                    let s = &self.solids[i as usize];
                    if s.mins.x <= maxs.x && s.maxs.x >= mins.x && s.mins.y <= maxs.y && s.maxs.y >= mins.y && s.mins.z <= maxs.z && s.maxs.z >= mins.z && !seen.contains(&i) {
                        seen.push(i);
                        f(s);
                    }
                }
            }
        }
    }

    /// Hull `h` trace through the world and the static solids.
    pub fn trace(&self, h: usize, a: Vec3, b: Vec3) -> Trace {
        self.trace_ex(h, a, b, false)
    }

    /// Like `trace`, but movers count too, unless the trace starts inside one
    /// (a doorway with the door closed is still a doorway).
    pub fn floor_trace(&self, h: usize, a: Vec3, b: Vec3) -> Trace {
        self.trace_ex(h, a, b, true)
    }

    fn trace_ex(&self, h: usize, a: Vec3, b: Vec3, floors: bool) -> Trace {
        let mut tr = self.world.hull(0, h).trace(a, b);
        let hull = self.world.hull(0, h);
        let (lo, hi) = (a.min(b) + hull.clip_mins, a.max(b) + hull.clip_maxs);
        self.near(lo, hi, &mut |s| {
            if s.floor_only && !floors {
                return;
            }
            let t = s.cm.hull(s.sub, h).trace(a - s.origin, b - s.origin);
            if s.floor_only && t.startsolid {
                return;
            }
            if t.allsolid {
                tr.allsolid = true;
            }
            if t.startsolid {
                tr.startsolid = true;
            }
            if t.fraction < tr.fraction {
                tr.fraction = t.fraction;
                tr.endpos = t.endpos + s.origin;
                tr.plane_normal = t.plane_normal;
            }
        });
        tr
    }

    /// World contents, or solid inside a static brush entity.
    pub fn point_contents(&self, p: Vec3) -> i32 {
        let mut c = self.world.point_contents(p);
        if c != CONTENTS_SOLID {
            self.near(p, p, &mut |s| {
                if !s.floor_only && s.cm.hull(s.sub, 0).point_contents(s.cm.hull(s.sub, 0).first, p - s.origin) == CONTENTS_SOLID {
                    c = CONTENTS_SOLID;
                }
            });
        }
        c
    }
}

fn hazard(cm: &Geo, p: Vec3) -> bool {
    let feet = cm.point_contents(p - Vec3::new(0.0, 0.0, 23.0));
    let waist = cm.point_contents(p);
    [feet, waist].iter().any(|&c| c == CONTENTS_LAVA || c == CONTENTS_SLIME || c == CONTENTS_SKY || c == CONTENTS_SOLID)
}

/// Drop a player hull from `p` to the floor below.
pub fn ground(cm: &Geo, p: Vec3, max: f32) -> Option<Vec3> {
    let tr = cm.floor_trace(1, p, p - Vec3::new(0.0, 0.0, max));
    if tr.startsolid || tr.allsolid || tr.fraction >= 1.0 || tr.plane_normal.z < 0.7 {
        return None;
    }
    Some(tr.endpos)
}

pub fn clear(cm: &Geo, a: Vec3, b: Vec3) -> bool {
    let tr = cm.trace(1, a, b);
    !tr.startsolid && !tr.allsolid && tr.fraction >= 1.0
}

impl Nav {
    /// A graph with no nodes (replaying a cached route needs none).
    pub fn empty(start: Vec3, forbid: Vec<Forbid>) -> Nav {
        Nav { pos: Vec::new(), edges: Vec::new(), cells: HashMap::default(), origin: start, forbid }
    }

    pub fn build(cm: &Geo, start: Vec3, forbid: Vec<Forbid>) -> Nav {
        let mut nav = Nav { pos: Vec::new(), edges: Vec::new(), cells: HashMap::default(), origin: start, forbid };
        nav.extend(cm, start);
        nav
    }

    /// Flood fill from a new seed at `start` (snapped to the grid). Returns
    /// whether any node was added.
    pub fn extend(&mut self, cm: &Geo, start: Vec3) -> bool {
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

    /// Add a node by hand (lift stops, teleporter and exit entries); it is
    /// not flood-filled.
    pub fn add(&mut self, p: Vec3) -> u32 {
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

    /// The nearest node within `radius` that a player can move to in a
    /// straight line from `p` (so not one behind a wall).
    pub fn nearest_clear(&self, cm: &Geo, p: Vec3, radius: f32) -> Option<u32> {
        let (cx, cy) = self.cell(p);
        let r = (radius / GRID).ceil() as i32;
        let mut cands: Vec<(f32, u32)> = Vec::new();
        for x in cx - r..=cx + r {
            for y in cy - r..=cy + r {
                for &n in self.cells.get(&(x, y)).into_iter().flatten() {
                    let d = self.pos[n as usize] - p;
                    let score = d.x * d.x + d.y * d.y + 4.0 * d.z * d.z;
                    if score <= radius * radius * 4.0 {
                        cands.push((score, n));
                    }
                }
            }
        }
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        let up = Vec3::new(0.0, 0.0, 1.0);
        cands.iter().take(12).find(|&&(_, n)| {
            let q = self.pos[n as usize];
            let tr = cm.trace(1, p + up, Vec3::new(q.x, q.y, p.z.max(q.z) + 1.0));
            !tr.startsolid && tr.fraction >= 1.0
        }).or(cands.first()).map(|c| c.1)
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

    /// Add an edge unless one already links `a` to `b`.
    pub fn link(&mut self, a: u32, b: u32, kind: EdgeKind, cost: f32) {
        if a != b && self.edge(a, b).is_none() {
            self.edges[a as usize].push(Edge { to: b, cost, kind });
        }
    }

    /// Nodes within `pad` (XY) of the box and within `dz` of height `z`.
    pub fn around(&self, mins: Vec3, maxs: Vec3, pad: f32, z: f32, dz: f32) -> Vec<u32> {
        let (a, b) = (self.cell(mins - Vec3::splat(pad)), self.cell(maxs + Vec3::splat(pad)));
        let mut out = Vec::new();
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                for &n in self.cells.get(&(x, y)).into_iter().flatten() {
                    let p = self.pos[n as usize];
                    if p.x > mins.x - pad && p.x < maxs.x + pad && p.y > mins.y - pad && p.y < maxs.y + pad && (p.z - z).abs() <= dz {
                        out.push(n);
                    }
                }
            }
        }
        out
    }

    /// Dijkstra from `src` skipping `blocked` edges: (cost, previous node) per
    /// node. A nonzero `noise` seed scales each edge's cost by a fixed factor
    /// in 1..1.5, so seeded variants take different routes.
    pub fn dijkstra(&self, src: u32, blocked: &Blocked, noise: u64) -> (Vec<f32>, Vec<u32>) {
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
                if !blocked.is_empty() && blocked.contains(&(u, e.to)) {
                    continue;
                }
                let cost = if noise == 0 { e.cost } else { e.cost * (1.0 + 0.5 * unit_hash(noise, u as u64, e.to as u64)) };
                let nd = d + cost;
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

/// A fixed pseudo-random number in 0..1 for (seed, a, b).
pub fn unit_hash(seed: u64, a: u64, b: u64) -> f32 {
    let mut h = Fx::default();
    h.write_u64(seed);
    h.write_u64(a);
    h.write_u64(b);
    // Final avalanche (splitmix64) so neighbouring inputs differ.
    let mut x = h.finish();
    x ^= x >> 30;
    x = x.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^= x >> 31;
    (x >> 40) as f32 / (1u64 << 24) as f32
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
