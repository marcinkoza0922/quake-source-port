//! BSP wireframe edge extraction, landmark parsing and PVS exploration tracking.

use std::collections::HashMap;

use qcore::math::Vec3;
use qformats::bsp::{Bsp, CONTENTS_SOLID, TextureKind};
use qformats::parse_entities;

/// Visual category of a wireframe edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeKind {
    Solid,
    Water,
    Slime,
    Lava,
    Teleporter,
    DoorSilver,
    DoorGold,
}

/// One deduplicated architectural edge in world space.
#[derive(Clone, Copy, Debug)]
pub struct MapEdge {
    pub a: Vec3,
    pub b: Vec3,
    pub mid_z: f32,
    pub kind: EdgeKind,
    /// Up to two BSP leaf indices touching this edge (0 = unknown/always visible).
    pub leafs: [u32; 2],
}

/// Static landmark extracted from the map's entity lump.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerKind {
    Exit,
    KeySilver,
    KeyGold,
    Teleporter,
}

#[derive(Clone, Copy, Debug)]
pub struct MapMarker {
    pub pos: Vec3,
    pub kind: MarkerKind,
    pub leaf: u32,
}

/// Precomputed wireframe geometry and landmarks for the current map.
#[derive(Clone, Debug, Default)]
pub struct MapMesh {
    pub mapname: String,
    pub edges: Vec<MapEdge>,
    pub markers: Vec<MapMarker>,
    pub leaf_count: usize,
    pub walkable_leafs: usize,
}

/// Minimum dot product between two adjacent face normals to treat their shared
/// edge as a coplanar BSP split seam and hide it (~16 degrees).
const COPLANAR_DOT: f32 = 0.96;
/// Minimum edge length (in Quake units) for solid world edges to trim BSP sliver noise.
const MIN_SOLID_EDGE_LEN: f32 = 8.0;
/// Maximum distance from the player at which PVS-visible leafs are marked explored.
const EXPLORE_PVS_RADIUS: f32 = 720.0;
/// Fallback exploration radius for unvised maps.
const EXPLORE_UNVISED_RADIUS: f32 = 512.0;

fn classify_liquid(name: &str) -> EdgeKind {
    let low = name.to_ascii_lowercase();
    if low.contains("lava") {
        EdgeKind::Lava
    } else if low.contains("slime") {
        EdgeKind::Slime
    } else if low.contains("tele") {
        EdgeKind::Teleporter
    } else {
        EdgeKind::Water
    }
}

fn v3(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

fn face_normal(bsp: &Bsp, face_idx: usize) -> Vec3 {
    let f = &bsp.faces[face_idx];
    let Some(pl) = bsp.planes.get(f.plane as usize) else {
        return Vec3::new(0.0, 0.0, 1.0);
    };
    let n = v3(pl.normal);
    if f.side != 0 { -n } else { n }
}

fn face_kind(bsp: &Bsp, face_idx: usize) -> Option<EdgeKind> {
    let f = &bsp.faces[face_idx];
    let ti = bsp.texinfo.get(f.texinfo as usize)?;
    let tex = bsp.textures.get(ti.miptex as usize).and_then(|t| t.as_ref());
    match tex {
        Some(t) => match t.kind {
            TextureKind::Sky => None,
            TextureKind::Liquid => Some(classify_liquid(&t.name)),
            TextureKind::Normal | TextureKind::Fence => Some(EdgeKind::Solid),
        },
        None => Some(EdgeKind::Solid),
    }
}

/// Walks the BSP hull 0 tree to find the leaf index containing `pos`.
pub fn point_in_leaf(bsp: &Bsp, pos: Vec3) -> usize {
    let Some(world) = bsp.models.first() else { return 0 };
    let mut num = world.headnode[0];
    let mut steps = 0usize;
    while num >= 0 && steps <= bsp.nodes.len() {
        let Some(node) = bsp.nodes.get(num as usize) else { return 0 };
        let Some(plane) = bsp.planes.get(node.plane as usize) else { return 0 };
        let d = v3(plane.normal).dot(pos) - plane.dist;
        num = node.children[usize::from(d < 0.0)];
        steps += 1;
    }
    if num < 0 {
        let leaf = (-num - 1) as usize;
        if leaf < bsp.leafs.len() { return leaf; }
    }
    0
}

fn aabb_dist_sq(p: Vec3, mins: Vec3, maxs: Vec3) -> f32 {
    let dx = (mins.x - p.x).max(0.0).max(p.x - maxs.x);
    let dy = (mins.y - p.y).max(0.0).max(p.y - maxs.y);
    let dz = (mins.z - p.z).max(0.0).max(p.z - maxs.z);
    dx * dx + dy * dy + dz * dz
}

/// Marks the player's current leaf and nearby visible leafs as explored.
pub fn mark_visible_leafs(bsp: &Bsp, pos: Vec3, explored: &mut [bool]) {
    if bsp.leafs.is_empty() || explored.len() != bsp.leafs.len() {
        return;
    }
    let leaf_idx = point_in_leaf(bsp, pos);
    explored[leaf_idx] = true;
    let visofs = bsp.leafs[leaf_idx].visofs;
    let num_vis = bsp.models.first().map_or(0, |m| m.visleafs.max(0) as usize);
    if visofs >= 0 && !bsp.visdata.is_empty() && num_vis > 0 {
        let r2 = EXPLORE_PVS_RADIUS * EXPLORE_PVS_RADIUS;
        let mut i = visofs as usize;
        let mut bit = 0usize;
        while bit < num_vis && i < bsp.visdata.len() {
            let b = bsp.visdata[i];
            i += 1;
            if b == 0 {
                let run = bsp.visdata.get(i).copied().unwrap_or(0) as usize;
                i += 1;
                bit += run * 8;
                continue;
            }
            for k in 0..8 {
                if b & (1 << k) != 0 {
                    let li = bit + k + 1; // leaf 0 is the shared solid leaf; vis bit 0 is leaf 1
                    if let Some(lf) = bsp.leafs.get(li)
                        && !explored[li]
                        && aabb_dist_sq(pos, v3(lf.mins), v3(lf.maxs)) <= r2
                    {
                        explored[li] = true;
                    }
                }
            }
            bit += 8;
        }
    } else {
        let r2 = EXPLORE_UNVISED_RADIUS * EXPLORE_UNVISED_RADIUS;
        for (li, lf) in bsp.leafs.iter().enumerate().skip(1) {
            if lf.contents != CONTENTS_SOLID && !explored[li] && aabb_dist_sq(pos, v3(lf.mins), v3(lf.maxs)) <= r2 {
                explored[li] = true;
            }
        }
    }
}

/// Builds a clean architectural wireframe and landmark set for `bsp`.
pub fn build(mapname: &str, bsp: &Bsp) -> MapMesh {
    let Some(world) = bsp.models.first() else {
        return MapMesh { mapname: mapname.to_string(), ..Default::default() };
    };
    let face_start = world.first_face.max(0) as usize;
    let face_end = (face_start + world.num_faces.max(0) as usize).min(bsp.faces.len());

    // Map each world face to up to two leafs via marksurfaces.
    let mut face_leafs: Vec<[u32; 2]> = vec![[0, 0]; bsp.faces.len()];
    let mut walkable_leafs = 0usize;
    for (li, lf) in bsp.leafs.iter().enumerate().skip(1) {
        if lf.contents != CONTENTS_SOLID {
            walkable_leafs += 1;
        }
        let s = lf.first_marksurface as usize;
        let e = (s + lf.num_marksurfaces as usize).min(bsp.marksurfaces.len());
        for &fi in &bsp.marksurfaces[s..e] {
            if let Some(slot) = face_leafs.get_mut(fi as usize) {
                if slot[0] == 0 {
                    slot[0] = li as u32;
                } else if slot[0] != li as u32 && slot[1] == 0 {
                    slot[1] = li as u32;
                }
            }
        }
    }

    // Deduplicate world edges by BSP edge index and filter coplanar interior seams.
    struct Acc {
        v0: u32,
        v1: u32,
        n0: Vec3,
        kind0: EdgeKind,
        leafs: [u32; 2],
        count: u8,
        keep: bool,
    }
    let mut by_edge: HashMap<u32, Acc> = HashMap::with_capacity((face_end - face_start) * 2);
    for (fi, &fl) in face_leafs.iter().enumerate().take(face_end).skip(face_start) {
        let Some(kind) = face_kind(bsp, fi) else { continue };
        let normal = face_normal(bsp, fi);
        let f = &bsp.faces[fi];
        let es = f.first_edge as usize;
        let ee = (es + f.num_edges as usize).min(bsp.surfedges.len());
        for &se in &bsp.surfedges[es..ee] {
            let e_idx = se.unsigned_abs();
            let Some(&[v0, v1]) = bsp.edges.get(e_idx as usize) else { continue };
            by_edge
                .entry(e_idx)
                .and_modify(|acc| {
                    acc.count = acc.count.saturating_add(1);
                    if acc.leafs[0] == 0 {
                        acc.leafs[0] = fl[0];
                    } else if acc.leafs[1] == 0 && fl[0] != 0 && fl[0] != acc.leafs[0] {
                        acc.leafs[1] = fl[0];
                    }
                    // Two faces with matching kind and nearly parallel normals -> interior BSP split.
                    if acc.count == 2 && acc.kind0 == kind && acc.n0.dot(normal) > COPLANAR_DOT {
                        acc.keep = false;
                    } else {
                        acc.keep = true;
                        if acc.kind0 == EdgeKind::Solid && kind != EdgeKind::Solid {
                            acc.kind0 = kind;
                        }
                    }
                })
                .or_insert(Acc { v0, v1, n0: normal, kind0: kind, leafs: fl, count: 1, keep: true });
        }
    }

    let min_len_sq = MIN_SOLID_EDGE_LEN * MIN_SOLID_EDGE_LEN;
    let mut edges = Vec::with_capacity(by_edge.len());
    for acc in by_edge.into_values() {
        if !acc.keep {
            continue;
        }
        let (Some(&a), Some(&b)) = (bsp.vertexes.get(acc.v0 as usize), bsp.vertexes.get(acc.v1 as usize)) else {
            continue;
        };
        let (a, b) = (v3(a), v3(b));
        let d = b - a;
        if acc.kind0 == EdgeKind::Solid && d.dot(d) < min_len_sq {
            continue;
        }
        edges.push(MapEdge { a, b, mid_z: (a.z + b.z) * 0.5, kind: acc.kind0, leafs: acc.leafs });
    }

    let mut markers = Vec::new();
    if let Ok(ents) = parse_entities(&bsp.entities) {
        for ent in &ents {
            let Some(cn) = ent.get("classname") else { continue };
            let origin = ent.get("origin").and_then(parse_vec3).unwrap_or(Vec3::ZERO);
            let submodel = ent
                .get("model")
                .and_then(|m| m.strip_prefix('*'))
                .and_then(|n| n.parse::<usize>().ok())
                .and_then(|n| bsp.models.get(n));
            let center = submodel.map_or(origin, |m| (v3(m.mins) + v3(m.maxs)) * 0.5 + origin);
            let leaf = point_in_leaf(bsp, center) as u32;
            match cn {
                "trigger_changelevel" => markers.push(MapMarker { pos: center, kind: MarkerKind::Exit, leaf }),
                "trigger_teleport" => markers.push(MapMarker { pos: center, kind: MarkerKind::Teleporter, leaf }),
                "item_key1" => markers.push(MapMarker { pos: center, kind: MarkerKind::KeySilver, leaf }),
                "item_key2" => markers.push(MapMarker { pos: center, kind: MarkerKind::KeyGold, leaf }),
                "func_door" | "func_door_secret" => {
                    let sf: i32 = ent.get("spawnflags").and_then(|s| s.parse().ok()).unwrap_or(0);
                    let door_kind = if sf & 8 != 0 {
                        Some(EdgeKind::DoorGold)
                    } else if sf & 16 != 0 {
                        Some(EdgeKind::DoorSilver)
                    } else {
                        None
                    };
                    if let (Some(kind), Some(m)) = (door_kind, submodel) {
                        push_box_edges(&mut edges, v3(m.mins) + origin, v3(m.maxs) + origin, kind, leaf);
                    }
                }
                _ if cn.starts_with("item_key") => {
                    let kind = if cn.contains("gold") || cn.ends_with('2') {
                        MarkerKind::KeyGold
                    } else {
                        MarkerKind::KeySilver
                    };
                    markers.push(MapMarker { pos: center, kind, leaf });
                }
                _ => {}
            }
        }
    }

    MapMesh {
        mapname: mapname.to_string(),
        edges,
        markers,
        leaf_count: bsp.leafs.len(),
        walkable_leafs,
    }
}

fn push_box_edges(edges: &mut Vec<MapEdge>, mins: Vec3, maxs: Vec3, kind: EdgeKind, leaf: u32) {
    let corners = [
        Vec3::new(mins.x, mins.y, mins.z),
        Vec3::new(maxs.x, mins.y, mins.z),
        Vec3::new(maxs.x, maxs.y, mins.z),
        Vec3::new(mins.x, maxs.y, mins.z),
        Vec3::new(mins.x, mins.y, maxs.z),
        Vec3::new(maxs.x, mins.y, maxs.z),
        Vec3::new(maxs.x, maxs.y, maxs.z),
        Vec3::new(mins.x, maxs.y, maxs.z),
    ];
    const BOX: [(usize, usize); 12] = [
        (0, 1), (1, 2), (2, 3), (3, 0),
        (4, 5), (5, 6), (6, 7), (7, 4),
        (0, 4), (1, 5), (2, 6), (3, 7),
    ];
    for (i, j) in BOX {
        let (a, b) = (corners[i], corners[j]);
        edges.push(MapEdge { a, b, mid_z: (a.z + b.z) * 0.5, kind, leafs: [leaf, 0] });
    }
}

fn parse_vec3(s: &str) -> Option<Vec3> {
    let mut it = s.split_whitespace().filter_map(|t| t.parse::<f32>().ok());
    Some(Vec3::new(it.next()?, it.next()?, it.next()?))
}

impl MapMesh {
    /// True if at least one of an edge's touching leafs has been explored.
    pub fn edge_explored(&self, edge: &MapEdge, explored: &[bool]) -> bool {
        if edge.leafs[0] == 0 && edge.leafs[1] == 0 {
            return true;
        }
        edge.leafs.iter().any(|&l| l != 0 && explored.get(l as usize).copied().unwrap_or(false))
    }

    pub fn marker_explored(&self, m: &MapMarker, explored: &[bool]) -> bool {
        m.leaf == 0 || explored.get(m.leaf as usize).copied().unwrap_or(false)
    }

    /// Percentage (0..=100) of non-solid leafs explored so far.
    pub fn explored_percent(&self, explored: &[bool]) -> u32 {
        if self.walkable_leafs == 0 {
            return 100;
        }
        let n = explored.iter().skip(1).filter(|&&v| v).count();
        ((n * 100) / self.walkable_leafs).min(100) as u32
    }
}
