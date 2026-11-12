use super::*;

pub(super) struct Built {
    pub(super) geo: Geo,
    pub(super) nav: Nav,
    pub(super) lifts: HashMap<(u32, u32), Lift, FxBuild>,
    pub(super) exit_nodes: Vec<Option<u32>>,
    pub(super) sig: u64,
    /// Hint lines that couldn't be applied.
    pub(super) bad_hints: Vec<String>,
}

/// Hash of where every mover is.
pub(super) fn mover_sig(vm: &Vm, o: &Ofs) -> u64 {
    use std::hash::Hasher;
    let mut h = nav::Fx::default();
    for e in 1..vm.num_edicts() as u32 {
        if !vm.is_free(e) && vm.ent_f(e, o.solid) as i32 == SOLID_BSP {
            for x in vm.ent_v(e, o.origin) {
                h.write_u32((x * 8.0).round() as i32 as u32);
            }
        }
    }
    h.finish()
}

/// Nav graph from the world, static brush entities (no `blocked` function,
/// which every mover has) and movers as floors where they are now; then
/// lifts, teleporters and exit entries.
#[allow(clippy::too_many_arguments)]
pub(super) fn build_nav(vm: &Vm, world: &qworld::world::World, o: &Ofs, cm: &Arc<CollisionModel>, start: Vec3, forbid: &[Forbid], exits: &[u32], hints: &[hints::Line]) -> Built {
    let mut geo = Geo::new(cm.clone());
    let blocked_fn = vm.field_ofs("blocked");
    let modelindex = vm.field_ofs("modelindex");
    let velocity = vm.field_ofs("velocity");
    for e in 1..vm.num_edicts() as u32 {
        if vm.is_free(e) || vm.ent_f(e, o.solid) as i32 != SOLID_BSP {
            continue;
        }
        let mover = blocked_fn.is_some_and(|b| vm.ent_i(e, b) != 0);
        // Movers are floors only while at rest, and never trains (always moving).
        let resting = velocity.is_some_and(|f| v(vm.ent_v(e, f)).length_sq() == 0.0) && !vm.classname(e).contains("train");
        if mover && !resting {
            continue;
        }
        // A mover thinner than a player is a door panel, not a floor: landing
        // on top of a closed door in a corridor would cut the corridor off.
        let size = v(vm.ent_v(e, o.absmax)) - v(vm.ent_v(e, o.absmin));
        if mover && size.x.min(size.y) < 32.0 {
            continue;
        }
        let Some(mi) = modelindex else { continue };
        if let Some((m, sub)) = world.model(vm.ent_f(e, mi) as usize) {
            geo.add_solid(m.clone(), *sub, v(vm.ent_v(e, o.origin)), v(vm.ent_v(e, o.absmin)), v(vm.ent_v(e, o.absmax)), mover);
        }
    }
    let mut nav = Nav::build(&geo, start, forbid.to_vec());
    let mut lifts = HashMap::default();
    add_movers(&mut nav, &geo, vm, o, &mut lifts);
    let bad_hints = hints::apply(&mut nav, &geo, hints);
    let exit_nodes = exits
        .iter()
        .map(|&e| add_entry(&mut nav, &geo, v(vm.ent_v(e, o.absmin)), v(vm.ent_v(e, o.absmax))))
        .collect();
    Built { geo, nav, lifts, exit_nodes, sig: mover_sig(vm, o), bad_hints }
}

/// Node standing in the middle of a trigger box (teleporter, exit), linked
/// from the graph nodes around it. The flood fill keeps out of these boxes.
pub(super) fn add_entry(nav: &mut Nav, cm: &Geo, mins: Vec3, maxs: Vec3) -> Option<u32> {
    let c = (mins + maxs) * 0.5;
    // Standing spot in the box: the floor under one of several heights, else
    // any open point (exits in water). Boxes too narrow for a player (in
    // wall recesses) are touched from just outside one side.
    let mut zs: Vec<f32> = vec![mins.z + 40.0, mins.z + 25.0, c.z, maxs.z - 8.0];
    zs.retain(|&z| z <= maxs.z + 1.0);
    let h = (maxs - mins) * 0.5 + Vec3::splat(15.0);
    let offsets = [Vec3::ZERO, Vec3::new(h.x, 0.0, 0.0), Vec3::new(-h.x, 0.0, 0.0), Vec3::new(0.0, h.y, 0.0), Vec3::new(0.0, -h.y, 0.0)];
    let spots: Vec<Vec3> = offsets.iter().flat_map(|&o| zs.iter().map(move |&z| Vec3::new(c.x + o.x, c.y + o.y, z))).collect();
    let g = spots
        .iter()
        .filter(|&&p| nav::clear(cm, p, p))
        .find_map(|&p| nav::ground(cm, p, (p.z - mins.z) + 64.0).filter(|g| g.z > mins.z - 80.0))
        .or_else(|| spots.iter().copied().find(|&p| nav::clear(cm, p, p)));
    if debug() && g.is_none() {
        eprintln!("entry {mins:?}-{maxs:?}: no spot");
    }
    let g = g?;
    let e = nav.add(g);
    let mut ns = nav.around(mins, maxs, 48.0, g.z, 24.0);
    if ns.is_empty() {
        ns = nav.around(mins, maxs, 64.0, g.z, 64.0);
    }
    if debug() {
        eprintln!("entry {mins:?}-{maxs:?} at {g:?}: {} neighbours", ns.len());
    }
    for n in ns {
        if n != e {
            let d = (nav.pos[n as usize] - g).length();
            nav.link(n, e, EdgeKind::Walk, d);
        }
    }
    Some(e)
}

/// Edges through locked doors, the door (master) of each, and the goals
/// that open each door (buttons, triggers, keys).
#[derive(Default)]
pub(super) struct Gates {
    pub(super) edges: Blocked,
    pub(super) door: HashMap<(u32, u32), u32, FxBuild>,
    pub(super) openers: HashMap<u32, Vec<usize>, FxBuild>,
}

pub(super) const IT_KEY1: i32 = 131072;
pub(super) const IT_KEY2: i32 = 262144;

/// Whether segment a-b crosses the box in XY (slab test).
pub(super) fn seg_hits_box_xy(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (s, d, l, h) in [(a.x, b.x - a.x, lo.x, hi.x), (a.y, b.y - a.y, lo.y, hi.y)] {
        if d.abs() < 1e-6 {
            if s <= l || s >= h {
                return false;
            }
            continue;
        }
        let (mut u, mut w) = ((l - s) / d, (h - s) / d);
        if u > w {
            std::mem::swap(&mut u, &mut w);
        }
        t0 = t0.max(u);
        t1 = t1.min(w);
        if t0 >= t1 {
            return false;
        }
    }
    true
}

pub(super) fn ent_str(vm: &Vm, e: u32, ofs: Option<usize>) -> Vec<u8> {
    ofs.and_then(|o| vm.ent_str(e, o).ok()).map(|s| s.to_vec()).unwrap_or_default()
}

/// A brush entity that moves straight up and down between two stops: plats,
/// vertical doors, and trains cycling between two path_corners.
pub(super) fn lift_of(vm: &Vm, o: &Ofs, e: u32, ents: &[u32]) -> Option<Lift> {
    let cls = vm.classname(e);
    let (mins, maxs) = (v(vm.ent_v(e, o.absmin)), v(vm.ent_v(e, o.absmax)));
    let (lo, hi, mins, maxs) = match cls.as_str() {
        // The spawn functions rename themselves (func_plat → "plat").
        "plat" | "door" | "func_plat" | "func_door" | "func_new_plat" => {
            let (p1, p2) = (v(vm.ent_v(e, o.pos1?)), v(vm.ent_v(e, o.pos2?)));
            let d = p2 - p1;
            if d.x.abs() > 1.0 || d.y.abs() > 1.0 {
                return None;
            }
            // Top surface at each stop.
            let top = vm.ent_v(e, o.maxs)[2];
            (p1.z.min(p2.z) + top, p1.z.max(p2.z) + top, mins, maxs)
        }
        "train" | "func_train" => {
            // A train puts its mins on each corner in turn.
            let mut corners: Vec<Vec3> = Vec::new();
            let mut name = ent_str(vm, e, o.target);
            while !name.is_empty() && corners.len() < 8 {
                let c = *ents.iter().find(|&&c| vm.classname(c) == "path_corner" && ent_str(vm, c, o.targetname) == name)?;
                let p = v(vm.ent_v(c, o.origin));
                if corners.contains(&p) {
                    break;
                }
                corners.push(p);
                name = ent_str(vm, c, o.target);
            }
            let first = *corners.first()?;
            if corners.iter().any(|c| (c.x - first.x).abs() > 1.0 || (c.y - first.y).abs() > 1.0) {
                return None;
            }
            let size = maxs - mins;
            let lo = corners.iter().map(|c| c.z).fold(f32::INFINITY, f32::min);
            let hi = corners.iter().map(|c| c.z).fold(f32::NEG_INFINITY, f32::max);
            (lo + size.z, hi + size.z, first, first + size)
        }
        _ => return None,
    };
    let plat = cls.contains("plat");
    (hi - lo >= 30.0).then_some(Lift { ent: e, lo, hi, mins, maxs, plat })
}

/// Nodes beside a lift stop at standing height `z`, seeding the flood fill
/// there when it hasn't reached that ledge yet.
pub(super) fn lift_stop(nav: &mut Nav, cm: &Geo, mins: Vec3, maxs: Vec3, p: Vec3) -> Vec<u32> {
    let n = nav.around(mins, maxs, 48.0, p.z, 24.0);
    if !n.is_empty() {
        return n;
    }
    let h = (maxs - mins) * 0.5 + Vec3::splat(24.0);
    for s in [Vec3::new(h.x, 0.0, 0.0), Vec3::new(-h.x, 0.0, 0.0), Vec3::new(0.0, h.y, 0.0), Vec3::new(0.0, -h.y, 0.0)] {
        let q = p + s;
        if nav::clear(cm, p, q) && nav::ground(cm, q, 40.0).is_some() {
            nav.extend(cm, q);
        }
    }
    nav.around(mins, maxs, 48.0, p.z, 24.0)
}

/// Teleporters and lifts as nav edges. Each pass can reach new areas, so
/// repeat until nothing changes.
pub(super) fn add_movers(nav: &mut Nav, cm: &Geo, vm: &Vm, o: &Ofs, lifts: &mut HashMap<(u32, u32), Lift, FxBuild>) {
    let mut done = std::collections::HashSet::new();
    for _ in 0..8 {
        let before = nav.pos.len();
        add_movers_pass(nav, cm, vm, o, lifts, &mut done);
        if nav.pos.len() == before {
            break;
        }
    }
}

pub(super) fn add_movers_pass(nav: &mut Nav, cm: &Geo, vm: &Vm, o: &Ofs, lifts: &mut HashMap<(u32, u32), Lift, FxBuild>, done: &mut std::collections::HashSet<u32>) {
    let ents: Vec<u32> = (1..vm.num_edicts() as u32).filter(|&e| !vm.is_free(e) && !done.contains(&e)).collect();
    for &e in &ents {
        let cls = vm.classname(e);
        let (mins, maxs) = (v(vm.ent_v(e, o.absmin)), v(vm.ent_v(e, o.absmax)));
        if cls == "trigger_teleport" {
            // Teleporters with a targetname only work while triggered.
            let target = ent_str(vm, e, o.target);
            if debug() {
                eprintln!("teleport #{e} {mins:?}-{maxs:?} target {:?} targetname {:?}", String::from_utf8_lossy(&target), String::from_utf8_lossy(&ent_str(vm, e, o.targetname)));
            }
            if target.is_empty() || !ent_str(vm, e, o.targetname).is_empty() {
                continue;
            }
            let Some(d) = ents.iter().copied().find(|&d| ent_str(vm, d, o.targetname) == target && vm.classname(d) != "trigger_teleport") else {
                continue;
            };
            let dest = v(vm.ent_v(d, o.origin));
            nav.extend(cm, dest);
            let (Some(dn), Some(en)) = (nav.nearest(dest, 96.0), add_entry(nav, cm, mins, maxs)) else { continue };
            nav.link(en, dn, EdgeKind::Teleport, 64.0);
            done.insert(e);
        } else if cls == "trigger_push" && vm.ent_i(e, o.touch) != 0 {
            let Some(md) = vm.field_ofs("movedir") else { continue };
            let dir = v(vm.ent_v(e, md));
            if dir.length_sq() < 0.5 {
                continue;
            }
            let c = (mins + maxs) * 0.5;
            let land: Vec<u32> = if dir.z > 0.7 {
                // Updraft: blown out of the top onto a ledge beside it.
                let top = Vec3::new(c.x, c.y, maxs.z + 40.0);
                let h = (maxs - mins) * 0.5 + Vec3::splat(40.0);
                for s in [Vec3::new(h.x, 0.0, 0.0), Vec3::new(-h.x, 0.0, 0.0), Vec3::new(0.0, h.y, 0.0), Vec3::new(0.0, -h.y, 0.0)] {
                    let q = top + s;
                    if nav::clear(cm, q, q) && nav::ground(cm, q, 200.0).is_some() {
                        nav.extend(cm, q);
                    }
                }
                nav.around(mins, maxs, 96.0, maxs.z + 40.0, 120.0)
            } else {
                // Wind tunnel: carried out of the far end.
                let e2 = (maxs - mins) * 0.5;
                let half = e2.x * dir.x.abs() + e2.y * dir.y.abs() + e2.z * dir.z.abs();
                let q = c + dir * (half + 48.0);
                match nav::ground(cm, q, 512.0) {
                    Some(g) => {
                        nav.extend(cm, g + Vec3::new(0.0, 0.0, 1.0));
                        nav.nearest(g, 96.0).into_iter().collect()
                    }
                    None => Vec::new(),
                }
            };
            if debug() {
                eprintln!("push #{e} {mins:?}-{maxs:?} dir {dir:?}: {} landing nodes", land.len());
            }
            if land.is_empty() {
                continue;
            }
            let Some(en) = add_entry(nav, cm, mins, maxs) else { continue };
            for n in land {
                let cost = (nav.pos[n as usize] - nav.pos[en as usize]).length() * 0.5 + 64.0;
                nav.link(en, n, EdgeKind::Teleport, cost);
            }
            done.insert(e);
        } else if let Some(lift) = lift_of(vm, o, e, &ents) {
            let Lift { lo, hi, mins, maxs, .. } = lift;
            let c = (mins + maxs) * 0.5;
            let (bp, tp) = (Vec3::new(c.x, c.y, lo + 25.0), Vec3::new(c.x, c.y, hi + 25.0));
            if !nav::clear(cm, bp, bp + Vec3::new(0.0, 0.0, 1.0)) || !nav::clear(cm, tp, tp + Vec3::new(0.0, 0.0, 1.0)) {
                if debug() {
                    let (a, b) = (cm.trace(1, bp, bp + Vec3::new(0.0, 0.0, 1.0)), cm.trace(1, tp, tp + Vec3::new(0.0, 0.0, 1.0)));
                    eprintln!("lift #{e} {} lo {lo} hi {hi} box {mins:?}-{maxs:?}: stop in solid (bottom {} top {}, world bottom {} top {})", vm.classname(e), a.startsolid, b.startsolid, cm.world.hull(0, 1).trace(bp, bp).startsolid, cm.world.hull(0, 1).trace(tp, tp).startsolid);
                }
                continue;
            }
            let top_n = lift_stop(nav, cm, mins, maxs, tp);
            let bot_n = lift_stop(nav, cm, mins, maxs, bp);
            if debug() {
                eprintln!("lift #{e} {} lo {lo} hi {hi} {mins:?}-{maxs:?}: {} top, {} bottom", vm.classname(e), top_n.len(), bot_n.len());
            }
            if top_n.is_empty() || bot_n.is_empty() {
                continue;
            }
            done.insert(e);
            let (b, t) = (nav.add(bp), nav.add(tp));
            for n in bot_n {
                let d = (nav.pos[n as usize] - bp).length();
                nav.link(n, b, EdgeKind::Walk, d);
                nav.link(b, n, EdgeKind::Walk, d);
            }
            for n in top_n {
                let d = (nav.pos[n as usize] - tp).length();
                nav.link(n, t, EdgeKind::Walk, d);
                nav.link(t, n, EdgeKind::Walk, d);
            }
            let cost = hi - lo + 64.0;
            nav.link(b, t, EdgeKind::Ride, cost);
            nav.link(t, b, EdgeKind::Ride, cost);
            lifts.insert((b, t), lift);
            lifts.insert((t, b), lift);
        }
    }
}
