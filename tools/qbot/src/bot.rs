//! One map run: spawn a headless server, build the nav graph, then chase goals
//! (secrets, monsters, buttons, keys, finally the exit) with usercmds until
//! none are left.
//!
//! The bot moves by game means only: walking, jumping, riding lifts and
//! teleporters. When an edge turns out impassable (a locked door, a lift that
//! needs a trigger) it is blocked and other goals come first; blocks are lifted
//! once something changed. Noclip (an instant `setpos`) is the last resort and
//! is reported as a soft failure.

use crate::hints;
use crate::movement::{Jump, Probe};
use crate::actions::{ButtonTrip, LiftAction, LiftStage};
use crate::nav::{self, Blocked, EdgeKind, Forbid, FxBuild, Geo, Nav, path_to};
use crate::route::{self, Route};
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::Vec3;
use qserver::{ClientMove, Server};
use qvm::Vm;
use qworld::hull::CollisionModel;
use qworld::{CONTENTS_WATER, FL_MONSTER, MOVETYPE_NOCLIP, SOLID_BSP, SOLID_NOT, SOLID_TRIGGER};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Server tick: 20 Hz like a dedicated server (sys_ticrate 0.05), so a third
/// of the frames a 72 Hz listen server would run.
const DT: f64 = 1.0 / 20.0;
/// Frames per second of sim time.
const FPS: u32 = 20;
const GOAL_TIMEOUT: f64 = 30.0;
const STUCK_TIME: f64 = 4.0;
/// A node counts as reached this close (XY); a tick moves the bot 16 units.
const REACH: f32 = 20.0;
/// How long to wait for a lift before blocking it.
const RIDE_TIME: f64 = 15.0;

pub struct Options {
    pub skill: i32,
    /// Sim time limit; `None` scales with the number of goals.
    pub max_time: Option<f64>,
    pub verbose: bool,
    /// Where cached routes live (`<dir>/<map>.qbr`); `None` disables caching.
    pub routes: Option<PathBuf>,
    /// Ignore cached routes (still record new ones).
    pub fresh: bool,
    /// Variant seed: 0 is the plain bot; others vary goal order, routes and
    /// lift patience, deterministically per seed.
    pub seed: u64,
    /// Where per-map hint files live (`<dir>/<map>.txt`); `None`: no hints.
    pub hints: Option<PathBuf>,
}

#[derive(Default)]
pub struct Report {
    pub kills: i32,
    pub total_monsters: i32,
    pub secrets: i32,
    pub total_secrets: i32,
    /// Soft failures: places the bot could not reach by game means.
    pub noclip_uses: u32,
    pub goals_failed: u32,
    /// Triggers the bot never got into (often monster-only or decorative).
    pub triggers_missed: u32,
    /// Noclips to monsters (included in `noclip_uses`): coverage, not
    /// navigation; the rest went to goals a player has to reach.
    pub monster_noclips: u32,
    /// Noclips directly to exit goals (excludes returns to the graph).
    pub exit_noclips: u32,
    pub nav_nodes: usize,
    pub jump_links: u32,
    pub jumps_completed: u32,
    pub jump_failures: u32,
    pub lift_detours: u32,
    pub sim_time: f64,
    pub wall_time: f64,
    /// The level has a trigger_changelevel.
    pub has_exit: bool,
    /// Left the level through its exit.
    pub exited: bool,
    /// The bot couldn't get back onto the nav graph and gave up early.
    pub stranded: bool,
    /// Cached route: "replayed", "diverged t=…", "recorded" or empty.
    pub route: String,
    /// Finished by game means only: exit reached, no noclip, no error.
    pub clean: bool,
    /// A clean run's recording, for the caller to cache.
    pub recording: Option<Route>,
    pub seed: u64,
    /// Runs made for this map, and how many finished cleanly.
    pub variants: u32,
    pub clean_variants: u32,
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Secret,
    Monster,
    Button,
    Key,
    /// Runes that fire progression targets.
    Pickup,
    /// A brush entity that opens when shot (secret doors).
    Shoot,
    /// trigger_once / trigger_multiple: walk through it (fires the level's
    /// scripting, e.g. monsters teleporting in from closets).
    Trigger,
    Exit,
}

struct Goal {
    ent: u32,
    kind: Kind,
    done: bool,
    failed: bool,
    /// Put off until nothing else is reachable (timed out, out of sight).
    deferred: bool,
    defers: u32,
    initial_state: f32,
    noclips: u32,
    /// Fixed entry node (exits).
    node: Option<u32>,
    /// Exits: the touch function, restored when the exit becomes the goal.
    touch: i32,
    /// Shootable goals: nodes with a view of the target, and where the
    /// target was when they were found.
    views: Vec<u32>,
    views_at: Option<Vec3>,
    /// Keys: placed in the world (became a trigger) at least once.
    armed: bool,
}

struct Ofs {
    origin: usize,
    absmin: usize,
    absmax: usize,
    health: usize,
    flags: usize,
    solid: usize,
    takedamage: usize,
    movetype: usize,
    touch: usize,
    state: Option<usize>,
    waterlevel: usize,
    view_ofs: usize,
    target: Option<usize>,
    targetname: Option<usize>,
    pos1: Option<usize>,
    pos2: Option<usize>,
    maxs: usize,
    owner: Option<usize>,
    items: Option<usize>,
}

impl Ofs {
    fn new(vm: &Vm) -> Option<Ofs> {
        let f = |n| vm.field_ofs(n);
        Some(Ofs {
            origin: f("origin")?,
            absmin: f("absmin")?,
            absmax: f("absmax")?,
            health: f("health")?,
            flags: f("flags")?,
            solid: f("solid")?,
            takedamage: f("takedamage")?,
            movetype: f("movetype")?,
            touch: f("touch")?,
            state: f("state"),
            waterlevel: f("waterlevel")?,
            view_ofs: f("view_ofs")?,
            target: f("target"),
            targetname: f("targetname"),
            pos1: f("pos1"),
            pos2: f("pos2"),
            maxs: f("maxs")?,
            owner: f("owner"),
            items: f("items"),
        })
    }
}

/// `QBOT_DEBUG=1`: lift, entry and node dumps while building the graph.
fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("QBOT_DEBUG").is_some())
}

/// `QBOT_TRACE=1`: the bot's position and move twice a second.
fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("QBOT_TRACE").is_some())
}

fn v(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// Yaw/pitch (Quake view convention: positive pitch looks down) from `d`.
fn look_angles(d: Vec3) -> [f32; 3] {
    let yaw = d.y.atan2(d.x).to_degrees();
    let pitch = -(d.z.atan2((d.x * d.x + d.y * d.y).sqrt())).to_degrees();
    [pitch, yaw, 0.0]
}

/// A lift between two stop nodes.
#[derive(Clone, Copy)]
struct Lift {
    ent: u32,
    /// Top surface heights at the low and high stops.
    lo: f32,
    hi: f32,
    mins: Vec3,
    maxs: Vec3,
    plat: bool,
}

struct JumpAction {
    edge: (u32, u32),
    since: f64,
    tick: Option<usize>,
    released: bool,
}

struct Bot<'a> {
    sv: Server,
    c: usize,
    me: u32,
    o: Ofs,
    cm: Arc<CollisionModel>,
    geo: Geo,
    nav: Nav,
    goals: Vec<Goal>,
    opt: &'a Options,
    rep: Report,
    /// Current goal, its path and progress.
    cur: Option<usize>,
    path: Vec<u32>,
    step: usize,
    goal_start: f64,
    best_dist: f32,
    best_time: f64,
    last_replan: f64,
    yaw: f32,
    /// Impulses still to send, popped one per frame.
    impulses: Vec<u8>,
    /// Consecutive flights back to the graph without reaching a goal.
    returns: u32,
    stranded: bool,
    /// Weapon impulse last selected.
    weapon: u8,
    max_time: f64,
    blocked: Blocked,
    /// Nodes the bot couldn't walk onto from where it stood; not used as the
    /// start of a path until blocks are lifted.
    bad_src: std::collections::HashSet<u32, FxBuild>,
    /// A goal was finished since blocks and deferrals were last lifted.
    progress: bool,
    lifts: HashMap<(u32, u32), Lift, FxBuild>,
    /// Waiting on a lift edge (path step) since.
    ride_since: Option<(usize, f64)>,
    lift_action: Option<LiftAction>,
    action_jump: bool,
    jump_links: HashMap<(u32, u32), Jump, FxBuild>,
    jump_tried: std::collections::HashSet<(u32, u32), FxBuild>,
    jump_action: Option<JumpAction>,
    local_recovery_goals: std::collections::HashSet<u32, FxBuild>,
    jump_failed_goals: std::collections::HashSet<u32, FxBuild>,
    /// QC's jump impulse, measured from an ordinary successful jump.
    jump_speed: Option<f32>,
    /// A shootable button being shot to call the lift the bot waits on.
    call: Option<u32>,
    /// Time of the last noclip, for goals reached that way.
    noclip_at: f64,
    /// Whether the last noclip went to the current goal (not back to the graph).
    noclip_to_goal: bool,
    intermission: Option<usize>,
    /// For rebuilding the nav graph.
    forbid: Vec<Forbid>,
    start: Vec3,
    /// Mover positions the graph was built with.
    mover_sig: u64,
    /// Frames run so far.
    frame: u32,
    /// Target picked by `target()`, refreshed every few frames.
    target_cache: Option<(Vec3, u32)>,
    /// What this run did, for the route cache.
    rec: Route,
    /// The cached route being replayed.
    replay: Option<Route>,
    /// Edges gated by locked doors at the last replan (for the log).
    gated: usize,
    /// This map's navigation hints, applied on every graph build.
    hints: Vec<hints::Line>,
}

struct Built {
    geo: Geo,
    nav: Nav,
    lifts: HashMap<(u32, u32), Lift, FxBuild>,
    exit_nodes: Vec<Option<u32>>,
    sig: u64,
    /// Hint lines that couldn't be applied.
    bad_hints: Vec<String>,
}

/// Hash of where every mover is.
fn mover_sig(vm: &Vm, o: &Ofs) -> u64 {
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
fn build_nav(vm: &Vm, world: &qworld::world::World, o: &Ofs, cm: &Arc<CollisionModel>, start: Vec3, forbid: &[Forbid], exits: &[u32], hints: &[hints::Line]) -> Built {
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
fn add_entry(nav: &mut Nav, cm: &Geo, mins: Vec3, maxs: Vec3) -> Option<u32> {
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
struct Gates {
    edges: Blocked,
    door: HashMap<(u32, u32), u32, FxBuild>,
    openers: HashMap<u32, Vec<usize>, FxBuild>,
}

const IT_KEY1: i32 = 131072;
const IT_KEY2: i32 = 262144;

/// Whether segment a-b crosses the box in XY (slab test).
fn seg_hits_box_xy(a: Vec3, b: Vec3, lo: Vec3, hi: Vec3) -> bool {
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

fn ent_str(vm: &Vm, e: u32, ofs: Option<usize>) -> Vec<u8> {
    ofs.and_then(|o| vm.ent_str(e, o).ok()).map(|s| s.to_vec()).unwrap_or_default()
}

/// A brush entity that moves straight up and down between two stops: plats,
/// vertical doors, and trains cycling between two path_corners.
fn lift_of(vm: &Vm, o: &Ofs, e: u32, ents: &[u32]) -> Option<Lift> {
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
fn lift_stop(nav: &mut Nav, cm: &Geo, mins: Vec3, maxs: Vec3, p: Vec3) -> Vec<u32> {
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
fn add_movers(nav: &mut Nav, cm: &Geo, vm: &Vm, o: &Ofs, lifts: &mut HashMap<(u32, u32), Lift, FxBuild>) {
    let mut done = std::collections::HashSet::new();
    for _ in 0..8 {
        let before = nav.pos.len();
        add_movers_pass(nav, cm, vm, o, lifts, &mut done);
        if nav.pos.len() == before {
            break;
        }
    }
}

fn add_movers_pass(nav: &mut Nav, cm: &Geo, vm: &Vm, o: &Ofs, lifts: &mut HashMap<(u32, u32), Lift, FxBuild>, done: &mut std::collections::HashSet<u32>) {
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

/// Where a map's cached route lives.
pub fn route_file(dir: &std::path::Path, map: &str) -> PathBuf {
    dir.join(format!("{map}.qbr"))
}

pub fn run(fs: Fs, map: &str, opt: &Options) -> Report {
    let wall = Instant::now();
    let mut rep = Report::default();
    let route_path = opt.routes.as_ref().map(|d| route_file(d, map));
    let key = match (fs.open(&format!("maps/{map}.bsp")), fs.open("progs.dat")) {
        (Some(b), Some(p)) => route::key(&b, &p, opt.skill),
        _ => 0,
    };
    let replay = route_path.as_ref().filter(|_| !opt.fresh && key != 0 && opt.seed == 0).and_then(|p| Route::load(p, key));
    let hints = match opt.hints.as_ref().map(|d| hints::load(d, map)) {
        Some(Ok(h)) => h,
        Some(Err(e)) => {
            rep.errors.push(e);
            Vec::new()
        }
        None => Vec::new(),
    };
    let mut sv = Server::new(fs, Cvars::new(), 1);
    // Nobody reads the client's datagrams.
    sv.core.skip_entity_updates = true;
    sv.command(&format!("skill {}", opt.skill));
    if let Err(e) = sv.spawn_server(map) {
        rep.errors.push(format!("spawn: {e}"));
        return rep;
    }
    let Some(c) = sv.connect_local() else {
        rep.errors.push("connect failed".into());
        return rep;
    };
    sv.command("god");
    let l = sv.level.as_mut().unwrap();
    let Some(o) = Ofs::new(&l.vm) else {
        rep.errors.push("progs lack standard fields".into());
        return rep;
    };
    let cm = l.world.worldmodel.clone();
    let me = c as u32 + 1;
    let start = v(l.vm.ent_v(me, o.origin));
    let mut forbid = Vec::new();
    let mut goals = Vec::new();
    let mut exits = Vec::new();
    // Exits stay inert until everything else is done.
    for e in 1..l.vm.num_edicts() as u32 {
        if !l.vm.is_free(e) && l.vm.classname(e) == "trigger_changelevel" {
            exits.push((e, l.vm.ent_i(e, o.touch)));
            l.vm.set_ent_i(e, o.touch, 0);
        }
    }
    let l = sv.level.as_ref().unwrap();
    for e in 1..l.vm.num_edicts() as u32 {
        if l.vm.is_free(e) {
            continue;
        }
        let cls = l.vm.classname(e);
        let fb = || Forbid { mins: v(l.vm.ent_v(e, o.absmin)), maxs: v(l.vm.ent_v(e, o.absmax)) };
        let kind = match cls.as_str() {
            // The flood fill keeps out of exits and teleporters (which would
            // bounce the bot away from a node inside them); both get entry
            // nodes afterwards.
            "trigger_changelevel" | "trigger_teleport" => {
                forbid.push(fb());
                continue;
            }
            "trigger_secret" => Kind::Secret,
            "trigger_once" | "trigger_multiple" if l.vm.ent_f(e, o.solid) as i32 == SOLID_TRIGGER && l.vm.ent_i(e, o.touch) != 0 => Kind::Trigger,
            "func_button" => Kind::Button,
            c if c.starts_with("item_") && c.contains("key") => Kind::Key,
            "item_sigil" => Kind::Pickup,
            _ if l.vm.ent_f(e, o.solid) as i32 == SOLID_BSP && l.vm.ent_f(e, o.takedamage) != 0.0 && l.vm.ent_f(e, o.health) > 0.0 => Kind::Shoot,
            _ if l.vm.ent_f(e, o.flags) as i32 & FL_MONSTER != 0 => Kind::Monster,
            _ => continue,
        };
        let initial_state = o.state.map_or(0.0, |s| l.vm.ent_f(e, s));
        goals.push(Goal { ent: e, kind, done: false, failed: false, deferred: false, defers: 0, initial_state, noclips: 0, node: None, touch: 0, views: Vec::new(), views_at: None, armed: false });
    }
    rep.has_exit = !exits.is_empty();
    let exit_ents: Vec<u32> = exits.iter().map(|x| x.0).collect();
    // A replayed route needs no graph; one is built if the replay diverges.
    let built = if replay.is_some() {
        Built { geo: Geo::new(cm.clone()), nav: Nav::empty(start, forbid.clone()), lifts: HashMap::default(), exit_nodes: vec![None; exit_ents.len()], sig: 0, bad_hints: Vec::new() }
    } else {
        build_nav(&l.vm, &l.world, &o, &cm, start, &forbid, &exit_ents, &hints)
    };
    // A hint that doesn't fit the map is an authoring error: fail loudly.
    rep.errors.extend(built.bad_hints);
    if opt.verbose && !hints.is_empty() {
        eprintln!("{map}: {} navigation hints", hints.len());
    }
    let (geo, nav, lifts, sig) = (built.geo, built.nav, built.lifts, built.sig);
    for ((e, touch), node) in exits.into_iter().zip(built.exit_nodes) {
        goals.push(Goal { ent: e, kind: Kind::Exit, done: false, failed: false, deferred: false, defers: 0, initial_state: 0.0, noclips: 0, node, touch, views: Vec::new(), views_at: None, armed: false });
    }
    rep.nav_nodes = nav.pos.len();
    if debug() {
        for p in &nav.pos {
            eprintln!("node {} {} {}", p.x, p.y, p.z);
        }
    }
    if opt.verbose {
        eprintln!("{map}: {} nav nodes, {} goals, {} lift edges", nav.pos.len(), goals.len(), lifts.len());
    }
    let intermission = l.vm.global_ofs("intermission_running");
    let mut bot = Bot {
        sv,
        c,
        me,
        o,
        cm,
        geo,
        nav,
        goals,
        opt,
        rep,
        cur: None,
        path: Vec::new(),
        step: 0,
        goal_start: 0.0,
        best_dist: f32::INFINITY,
        best_time: 0.0,
        last_replan: -10.0,
        yaw: 0.0,
        impulses: Vec::new(),
        returns: 0,
        stranded: false,
        weapon: 5,
        max_time: 0.0,
        blocked: Blocked::default(),
        bad_src: Default::default(),
        progress: false,
        lifts,
        ride_since: None,
        lift_action: None,
        action_jump: false,
        jump_links: HashMap::default(),
        jump_tried: Default::default(),
        jump_action: None,
        local_recovery_goals: Default::default(),
        jump_failed_goals: Default::default(),
        jump_speed: None,
        call: None,
        noclip_at: -100.0,
        noclip_to_goal: false,
        intermission,
        forbid,
        start,
        mover_sig: sig,
        frame: 0,
        target_cache: None,
        rec: Route { key, ..Default::default() },
        replay,
        gated: 0,
        hints,
    };
    if bot.replay.is_some() {
        bot.rep.route = "replayed".into();
    }
    if std::env::var_os("QBOT_PROBE").is_some() {
        let v: Vec<f32> = std::env::var("QBOT_PROBE").unwrap().split(',').map(|x| x.parse().unwrap()).collect();
        let p = Vec3::new(v[0], v[1], v[2]);
        let tr = bot.geo.floor_trace(1, p, p - Vec3::new(0.0, 0.0, 4096.0));
        let mut z = p.z;
        let mut layers = String::new();
        while z > tr.endpos.z - 30.0 {
            layers += &format!(" {z:.0}:{}", bot.cm.point_contents(Vec3::new(p.x, p.y, z)));
            z -= 32.0;
        }
        eprintln!("probe {p:?}: floor {:?} normal {:?} start_solid {}; contents{layers}", tr.endpos, tr.plane_normal, tr.startsolid);
    }
    // `QBOT_PATH=x,y,z;x,y,z`: print the graph's path between two spots
    // (for writing hints: where does the plan go, and on which edges).
    if let Ok(s) = std::env::var("QBOT_PATH") {
        let pts: Vec<Vec3> = s.split(';').map(|p| { let v: Vec<f32> = p.split(',').map(|x| x.trim().parse().unwrap()).collect(); Vec3::new(v[0], v[1], v[2]) }).collect();
        let (a, b) = (bot.nav.nearest(pts[0], 128.0), bot.nav.nearest(pts[1], 128.0));
        eprintln!("path {:?} -> {:?}: nodes {:?} -> {:?}", pts[0], pts[1], a.map(|n| bot.nav.pos[n as usize]), b.map(|n| bot.nav.pos[n as usize]));
        if let (Some(a), Some(b)) = (a, b) {
            let (dist, prev) = bot.nav.dijkstra(a, &Blocked::default(), 0);
            if dist[b as usize].is_finite() {
                let p = path_to(&prev, b);
                for w in p.windows(2) {
                    let k = bot.nav.edge(w[0], w[1]).unwrap().kind;
                    if k != EdgeKind::Walk || (bot.nav.pos[w[1] as usize].z - bot.nav.pos[w[0] as usize].z).abs() > 20.0 || std::env::var_os("QBOT_PATH_ALL").is_some() {
                        eprintln!("  {k:?} {:?} -> {:?}", bot.nav.pos[w[0] as usize], bot.nav.pos[w[1] as usize]);
                    }
                }
                eprintln!("  cost {:.0}, {} nodes", dist[b as usize], p.len());
            } else {
                eprintln!("  unreachable");
            }
        }
    }
    if let Ok(s) = std::env::var("QBOT_SEG") {
        let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect();
        let (a, b) = (Vec3::new(v[0], v[1], v[2]), Vec3::new(v[3], v[4], v[5]));
        let t = bot.geo.trace(1, a, b);
        let w = bot.cm.hull(0, 1).trace(a, b);
        eprintln!("seg geo frac {} end {:?} start_solid {}; world frac {} start_solid {}; ground at b {:?}", t.fraction, t.endpos, t.startsolid, w.fraction, w.startsolid, nav::ground(&bot.geo, b, 512.0));
    }
    // `QBOT_INTO=x,y,z`: the nodes that can reach that spot (its basin), as
    // extremes per axis: where the area leading there ends.
    if let Ok(s) = std::env::var("QBOT_INTO") {
        let v: Vec<f32> = s.split(',').map(|x| x.trim().parse().unwrap()).collect();
        if let Some(t) = bot.nav.nearest(Vec3::new(v[0], v[1], v[2]), 128.0) {
            let n = bot.nav.pos.len();
            let mut rev: Vec<Vec<u32>> = vec![Vec::new(); n];
            for (a, es) in bot.nav.edges.iter().enumerate() {
                for e in es {
                    rev[e.to as usize].push(a as u32);
                }
            }
            let mut seen = vec![false; n];
            let mut q = vec![t];
            seen[t as usize] = true;
            while let Some(a) = q.pop() {
                for &b in &rev[a as usize] {
                    if !seen[b as usize] {
                        seen[b as usize] = true;
                        q.push(b);
                    }
                }
            }
            let basin: Vec<Vec3> = (0..n).filter(|&i| seen[i]).map(|i| bot.nav.pos[i]).collect();
            eprintln!("into {:?}: {} nodes", bot.nav.pos[t as usize], basin.len());
            for (name, k) in [("x", 0), ("y", 1), ("z", 2)] {
                let key = |p: &Vec3| p.to_array()[k];
                let lo = basin.iter().min_by(|a, b| key(a).total_cmp(&key(b))).unwrap();
                let hi = basin.iter().max_by(|a, b| key(a).total_cmp(&key(b))).unwrap();
                eprintln!("  min {name} {lo:?}  max {name} {hi:?}");
            }
            // Where the start's reach comes closest to the basin: the gap to bridge.
            if let Some(src) = bot.nav.nearest(start, 96.0) {
                let (dist, _) = bot.nav.dijkstra(src, &Blocked::default(), 0);
                if dist[t as usize].is_finite() {
                    eprintln!("  reachable from the start");
                } else {
                    let fwd: Vec<Vec3> = (0..n).filter(|&i| dist[i].is_finite()).map(|i| bot.nav.pos[i]).collect();
                    let mut pairs: Vec<(f32, Vec3, Vec3)> = basin.iter().filter_map(|&b| fwd.iter().map(|&f| ((f - b).length(), f, b)).min_by(|x, y| x.0.total_cmp(&y.0))).collect();
                    pairs.sort_by(|x, y| x.0.total_cmp(&y.0));
                    let mut shown: Vec<Vec3> = Vec::new();
                    for (d, f, b) in pairs {
                        if shown.iter().all(|s| (*s - b).length() > 192.0) {
                            eprintln!("  gap {d:.0}: {f:?} -> {b:?}");
                            shown.push(b);
                            if shown.len() >= 8 {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    if debug() {
        let p = bot.me_origin();
        if let Some(src) = bot.nav.nearest(p, 96.0) {
            let (dist, _) = bot.nav.dijkstra(src, &Blocked::default(), 0);
            let reach: Vec<usize> = (0..dist.len()).filter(|&i| dist[i].is_finite()).collect();
            let (mut lo, mut hi) = (Vec3::splat(f32::INFINITY), Vec3::splat(f32::NEG_INFINITY));
            for &i in &reach {
                lo = lo.min(bot.nav.pos[i]);
                hi = hi.max(bot.nav.pos[i]);
            }
            eprintln!("reachable from start: {} of {} nodes, box {lo:?}-{hi:?}", reach.len(), bot.nav.pos.len());
            for g in bot.goals.iter().filter(|g| g.kind == Kind::Exit) {
                eprintln!("  exit #{} node {:?} reachable {}", g.ent, g.node.map(|n| bot.nav.pos[n as usize]), g.node.is_some_and(|n| dist[n as usize].is_finite()));
            }
            let vm = bot.vm();
            for e in 1..vm.num_edicts() as u32 {
                if vm.is_free(e) || vm.ent_f(e, bot.o.solid) as i32 == SOLID_NOT {
                    continue;
                }
                let (a, b) = (v(vm.ent_v(e, bot.o.absmin)), v(vm.ent_v(e, bot.o.absmax)));
                let cls = vm.classname(e);
                if (cls.starts_with("func") || cls.starts_with("trigger") || cls == "door" || cls == "plat") && a.x < hi.x + 64.0 && b.x > lo.x - 64.0 && a.y < hi.y + 64.0 && b.y > lo.y - 64.0 && a.z < hi.z + 64.0 && b.z > lo.z - 64.0 {
                    eprintln!("  in reach: #{e} {cls} {a:?}-{b:?}");
                }
            }
        }
    }
    if opt.verbose {
        let p = bot.me_origin();
        eprintln!("  start at {p:?}{}", bot.ents_near(p));
    }
    // Big maps get more time: 900 s, or 15 s per goal.
    bot.max_time = opt.max_time.unwrap_or_else(|| (bot.goals.len() as f64 * 15.0).max(900.0));
    bot.main_loop(map);
    // Finished by game means only: hand back the recording for the route
    // cache (a replay that held is already in it).
    bot.rep.clean = bot.rep.exited && bot.rep.noclip_uses == 0 && bot.rep.errors.is_empty() && !bot.rep.stranded;
    bot.rep.seed = opt.seed;
    if bot.rep.clean && key != 0 && bot.rep.route != "replayed" {
        bot.rep.recording = Some(std::mem::take(&mut bot.rec));
    }
    let st = bot.sv.stats();
    let mut rep = bot.rep;
    rep.kills = st.killed_monsters;
    rep.total_monsters = st.total_monsters;
    rep.secrets = st.found_secrets;
    rep.total_secrets = st.total_secrets;
    rep.sim_time = bot.sv.time();
    rep.wall_time = wall.elapsed().as_secs_f64();
    rep
}

impl Bot<'_> {
    fn vm(&self) -> &Vm {
        &self.sv.level.as_ref().unwrap().vm
    }

    fn me_origin(&self) -> Vec3 {
        v(self.vm().ent_v(self.me, self.o.origin))
    }

    fn center(&self, e: u32) -> Vec3 {
        let vm = self.vm();
        (v(vm.ent_v(e, self.o.absmin)) + v(vm.ent_v(e, self.o.absmax))) * 0.5
    }

    fn goal_done(&self, g: &Goal) -> bool {
        let vm = self.vm();
        let e = g.ent;
        if vm.is_free(e) {
            return g.kind != Kind::Exit;
        }
        match g.kind {
            Kind::Monster => vm.ent_f(e, self.o.health) <= 0.0 || vm.ent_f(e, self.o.flags) as i32 & FL_MONSTER == 0,
            Kind::Secret => vm.ent_i(e, self.o.touch) == 0 || vm.ent_f(e, self.o.solid) as i32 != SOLID_TRIGGER,
            // Keys spawn SOLID_NOT until PlaceItem runs: taken once placed
            // and not solid again.
            Kind::Key | Kind::Pickup => g.armed && vm.ent_f(e, self.o.solid) as i32 == SOLID_NOT,
            Kind::Button => self.o.state.is_some_and(|s| vm.ent_f(e, s) != g.initial_state),
            Kind::Shoot => vm.ent_f(e, self.o.takedamage) == 0.0 || vm.ent_f(e, self.o.health) <= 0.0,
            Kind::Trigger => {
                vm.ent_i(e, self.o.touch) == 0 || vm.ent_f(e, self.o.solid) as i32 != SOLID_TRIGGER || {
                    // trigger_multiple stays: done once we stand in it.
                    let (a, b) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
                    let p = self.me_origin();
                    p.x + 16.0 > a.x && p.x - 16.0 < b.x && p.y + 16.0 > a.y && p.y - 16.0 < b.y && p.z + 32.0 > a.z && p.z - 24.0 < b.z
                }
            }
            Kind::Exit => false,
        }
    }

    fn visible(&self, from: Vec3, to: Vec3) -> bool {
        self.cm.hull(0, 0).trace(from, to).fraction >= 1.0
    }

    fn eye(&self) -> Vec3 {
        self.me_origin() + v(self.vm().ent_v(self.me, self.o.view_ofs))
    }

    /// Whether a shot from `a` reaches `b`: world, static solids and every
    /// other brush entity where it is now (bars, glass, closed doors).
    fn clear_shot(&self, a: Vec3, b: Vec3, target: u32) -> bool {
        if self.geo.trace(0, a, b).fraction < 1.0 {
            return false;
        }
        let l = self.sv.level.as_ref().unwrap();
        let vm = &l.vm;
        let Some(mi) = vm.field_ofs("modelindex") else { return true };
        let (lo, hi) = (a.min(b), a.max(b));
        for e in 1..vm.num_edicts() as u32 {
            if e == target || vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP {
                continue;
            }
            let (emin, emax) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            if emin.x > hi.x || emax.x < lo.x || emin.y > hi.y || emax.y < lo.y || emin.z > hi.z || emax.z < lo.z {
                continue;
            }
            if let Some((m, sub)) = l.world.model(vm.ent_f(e, mi) as usize) {
                let o = v(vm.ent_v(e, self.o.origin));
                if m.hull(*sub, 0).trace(a - o, b - o).fraction < 1.0 {
                    return false;
                }
            }
        }
        true
    }

    /// Nearest live monster or shootable target with a clear shot, within range.
    fn target(&self) -> Option<(Vec3, u32)> {
        let eye = self.eye();
        let vm = self.vm();
        let mut cands: Vec<(f32, Vec3, u32)> = Vec::new();
        for g in &self.goals {
            if g.done {
                continue;
            }
            let shoot = match g.kind {
                Kind::Monster => !self.dormant(g),
                Kind::Button | Kind::Shoot => vm.ent_f(g.ent, self.o.health) > 0.0 && vm.ent_f(g.ent, self.o.takedamage) != 0.0,
                _ => false,
            };
            if !shoot {
                continue;
            }
            let c = self.center(g.ent);
            let d = (c - eye).length_sq();
            if d < 2000.0 * 2000.0 {
                cands.push((d, c, g.ent));
            }
        }
        // Nearest first; traces only until a clear shot turns up.
        cands.sort_by(|a, b| a.0.total_cmp(&b.0));
        cands.iter().take(12).filter(|c| self.visible(eye, c.1)).take(4).find(|c| self.clear_shot(eye, c.1, c.2)).map(|c| (c.1, c.2))
    }

    /// Node to reach for a goal.
    fn goal_node(&self, g: &Goal) -> Option<u32> {
        let vm = self.vm();
        let (mins, maxs) = (v(vm.ent_v(g.ent, self.o.absmin)), v(vm.ent_v(g.ent, self.o.absmax)));
        match g.kind {
            Kind::Secret | Kind::Key | Kind::Pickup | Kind::Trigger => self.nav.inside(mins, maxs),
            Kind::Monster => self.nav.nearest(v(vm.ent_v(g.ent, self.o.origin)), 256.0),
            Kind::Button => self.nav.inside(mins - Vec3::splat(24.0), maxs + Vec3::splat(24.0)),
            // Anywhere in the open within 128 units, shooting it from there.
            Kind::Shoot => self.nav.nearest_clear(&self.geo, (mins + maxs) * 0.5, 160.0),
            Kind::Exit => g.node,
        }
    }

    fn shootable(&self, g: &Goal) -> bool {
        let vm = self.vm();
        match g.kind {
            Kind::Monster | Kind::Shoot => true,
            Kind::Button => vm.ent_f(g.ent, self.o.health) > 0.0 && vm.ent_f(g.ent, self.o.takedamage) != 0.0,
            _ => false,
        }
    }

    /// Refresh the view nodes of shootable goals whose target moved.
    fn update_views(&mut self) {
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if !Self::pending(g) || !self.shootable(g) {
                continue;
            }
            let c = self.center(g.ent);
            if g.views_at.is_some_and(|p| (p - c).length() < 64.0) {
                continue;
            }
            let r = Vec3::splat(768.0);
            let mut ns = self.nav.around(c - r, c + r, 0.0, c.z, 768.0);
            ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - c).length_sq().total_cmp(&(self.nav.pos[b as usize] - c).length_sq()));
            let mut views = Vec::new();
            for &n in ns.iter().take(256) {
                if self.geo.trace(0, self.nav.pos[n as usize] + Vec3::new(0.0, 0.0, 22.0), c).fraction >= 1.0 {
                    views.push(n);
                    if views.len() >= 8 {
                        break;
                    }
                }
            }
            let g = &mut self.goals[i];
            g.views = views;
            g.views_at = Some(c);
        }
    }

    /// Monsters waiting to be spawned by a trigger (hipnotic, AD): not solid
    /// and not damageable yet.
    fn dormant(&self, g: &Goal) -> bool {
        let vm = self.vm();
        g.kind == Kind::Monster && vm.ent_f(g.ent, self.o.solid) as i32 == SOLID_NOT && vm.ent_f(g.ent, self.o.takedamage) == 0.0
    }

    fn pending(g: &Goal) -> bool {
        !g.done && !g.failed
    }

    fn fail(&mut self, i: usize, why: &str) {
        if !self.goals[i].failed {
            self.goals[i].failed = true;
            self.rep.goals_failed += 1;
            if self.opt.verbose {
                eprintln!("  t={:.1} goal #{} failed: {why}", self.sv.time(), self.goals[i].ent);
            }
        }
        if self.cur == Some(i) {
            self.cur = None;
        }
    }

    fn defer(&mut self, i: usize, why: &str) {
        let g = &mut self.goals[i];
        g.deferred = true;
        g.defers += 1;
        if self.opt.verbose {
            eprintln!("  t={:.1} goal #{} deferred: {why}", self.sv.time(), g.ent);
        }
        // A failed walking/firing approach is not an exhausted monster.
        // Keep its bounded assist budget available after other goals finish.
        if g.defers > 3 && (g.kind != Kind::Monster || g.noclips >= 3) {
            self.fail(i, "deferred too often");
        }
        if self.cur == Some(i) {
            self.cur = None;
        }
    }

    /// Rebuild the graph when movers have moved since it was built (a bridge
    /// came out, a lift is somewhere else).
    fn rebuild_nav(&mut self) {
        let l = self.sv.level.as_ref().unwrap();
        if mover_sig(&l.vm, &self.o) == self.mover_sig && !self.nav.pos.is_empty() {
            return;
        }
        let exits: Vec<u32> = self.goals.iter().filter(|g| g.kind == Kind::Exit).map(|g| g.ent).collect();
        let b = build_nav(&l.vm, &l.world, &self.o, &self.cm, self.start, &self.forbid, &exits, &self.hints);
        for e in b.bad_hints {
            if !self.rep.errors.contains(&e) {
                self.rep.errors.push(e);
            }
        }
        // Seed from where the bot is too, in case the start is cut off now.
        let mut nav = b.nav;
        nav.extend(&b.geo, self.me_origin());
        self.geo = b.geo;
        self.nav = nav;
        self.lifts = b.lifts;
        self.mover_sig = b.sig;
        let mut nodes = b.exit_nodes.into_iter();
        for g in &mut self.goals {
            g.views.clear();
            g.views_at = None;
            if g.kind == Kind::Exit {
                g.node = nodes.next().flatten();
            }
        }
        self.path.clear();
        self.step = 0;
        self.ride_since = None;
        self.lift_action = None;
        self.bad_src.clear();
        self.jump_links.clear();
        self.jump_tried.clear();
        self.jump_action = None;
        self.rep.nav_nodes = self.rep.nav_nodes.max(self.nav.pos.len());
        if self.opt.verbose {
            eprintln!("  t={:.1} nav rebuilt: {} nodes", self.sv.time(), self.nav.pos.len());
        }
    }

    /// Lift every block and deferral when something changed since last time.
    fn reset_blocks(&mut self) -> bool {
        let any = !self.blocked.is_empty() || self.goals.iter().any(|g| Self::pending(g) && g.deferred);
        if !any || !self.progress {
            return false;
        }
        if self.opt.verbose {
            eprintln!("  t={:.1} lifting {} blocked edges and deferrals", self.sv.time(), self.blocked.len());
        }
        self.blocked.clear();
        self.bad_src.clear();
        for g in &mut self.goals {
            g.deferred = false;
        }
        self.progress = false;
        self.rebuild_nav();
        true
    }

    /// Pick the cheapest reachable goal; when nothing is reachable, lift
    /// blocks, and only then noclip. Returns false when there is nothing left to do.
    fn replan(&mut self) -> bool {
        self.last_replan = self.sv.time();
        let here = self.me_origin();
        if self.nav.nearest(here, 96.0).is_none() && self.nav.extend(&self.geo, here) && self.opt.verbose {
            eprintln!("  t={:.1} nav extended from {here:?}: {} nodes", self.sv.time(), self.nav.pos.len());
        }
        // Standing on a brush entity (lift, platform) above a known floor:
        // path from the floor node below.
        let below = || {
            let tr = self.geo.trace(1, here, here - Vec3::new(0.0, 0.0, 1024.0));
            (!tr.startsolid && tr.fraction < 1.0).then_some(tr.endpos).and_then(|g| self.nav.nearest(g, 48.0))
        };
        let Some(src) = self.nav.nearest_clear_except(&self.geo, here, 96.0, &self.bad_src).or_else(below) else {
            // Off the graph (after a noclip or a fall): back to the nearest
            // node, and don't walk the edge it fell from again.
            if self.sv.time() - self.noclip_at > 1.0 {
                self.block_here();
            }
            self.path.clear();
            self.returns += 1;
            if self.returns > 8 {
                if self.opt.verbose {
                    eprintln!("  stranded at {here:?}");
                }
                self.rep.stranded = true;
                self.stranded = true;
                return false;
            }
            if let Some(n) = self.landing(here).or_else(|| self.nav.nearest(here, 2048.0)).or_else(|| self.nav.nearest_any(here)) {
                let to = self.nav.pos[n as usize];
                self.noclip(to, "off the nav graph");
            } else if let Some(i) = self.cur {
                self.fail(i, "no nav graph");
            }
            return true;
        };
        let others_left = self.goals.iter().any(|g| g.kind != Kind::Exit && Self::pending(g) && !self.dormant(g));
        let prev_goal = self.cur;
        let mut choice = None;
        let mut opener = None;
        self.update_views();
        let gates = self.gates();
        if gates.edges.len() != self.gated && self.opt.verbose {
            let mut doors: Vec<u32> = gates.door.values().copied().collect();
            doors.sort_unstable();
            doors.dedup();
            let ds: Vec<String> = doors.iter().map(|&d| format!("#{d} {} opened by {:?}", self.vm().classname(d), gates.openers.get(&d).map(|o| o.iter().map(|&i| self.goals[i].ent).collect::<Vec<_>>()))).collect();
            eprintln!("  t={:.1} {} edges gated by locked doors: {}", self.sv.time(), gates.edges.len(), ds.join(", "));
        }
        self.gated = gates.edges.len();
        // Gates are a preference: with them first, then without (the opener
        // may have fired already), then with blocks lifted.
        let gated = (!gates.edges.is_empty()).then(|| self.blocked.union(&gates.edges).copied().collect::<Blocked>());
        for pass in 0..3 {
            let blocked = match (&gated, pass) {
                (Some(g), 0) => g,
                (None, 2) => break,
                _ => &self.blocked,
            };
            let (dist, prev) = self.nav.dijkstra(src, blocked, self.opt.seed);
            let mut best: Option<(f32, usize, u32)> = None;
            for (i, g) in self.goals.iter().enumerate() {
                if !Self::pending(g) || g.deferred || (g.kind == Kind::Exit && others_left) || self.dormant(g) {
                    continue;
                }
                let n = self.goal_node(g).into_iter().chain(g.views.iter().copied()).filter(|&n| dist[n as usize].is_finite()).min_by(|&a, &b| dist[a as usize].total_cmp(&dist[b as usize]));
                if let Some(n) = n {
                    // Prefer finishing the current goal unless something is much closer.
                    let mut bias = if Some(i) == self.cur { 0.7 } else { 1.0 };
                    if self.opt.seed != 0 {
                        // A fixed preference per goal and variant: another goal order.
                        bias *= 1.0 + 0.6 * nav::unit_hash(self.opt.seed, 1, g.ent as u64);
                    }
                    let d = dist[n as usize] * bias;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, i, n));
                    }
                }
            }
            if let Some((_, i, n)) = best {
                let path = path_to(&prev, n);
                // Ungated plan through a locked door: go for its opener
                // instead (walking there if possible, else by noclip).
                if pass > 0 && gated.is_some()
                    && let Some(m) = path.windows(2).find_map(|w| gates.door.get(&(w[0], w[1])))
                    && let Some(&o) = gates.openers.get(m).into_iter().flatten().find(|&&o| Self::pending(&self.goals[o]) && self.goals[o].noclips < 3)
                {
                    let og = &self.goals[o];
                    // Shootable openers can be activated from a visible
                    // approach node even when their touch node is unreachable.
                    let views: Vec<u32> = if og.kind == Kind::Button && self.shootable(og) && !og.deferred {
                        og.views.iter().copied().filter(|&n| self.clear_shot(
                            self.nav.pos[n as usize] + Vec3::new(0.0, 0.0, 22.0), self.center(og.ent), og.ent,
                        )).collect()
                    } else { Vec::new() };
                    let reach = opener_node(self.goal_node(og), &views, &dist);
                    if self.opt.verbose && self.cur != Some(o) {
                        eprintln!("  t={:.1} door #{m} locked on the way to #{}: opener #{} first", self.sv.time(), self.goals[i].ent, og.ent);
                    }
                    match reach {
                        Some(n) => choice = Some((o, path_to(&prev, n))),
                        None => opener = Some(o),
                    }
                    break;
                }
                choice = Some((i, path));
                break;
            }
            if pass == 0 && gated.is_some() {
                continue;
            }
            let sig = self.mover_sig;
            if !self.reset_blocks() {
                break;
            }
            if self.mover_sig != sig {
                // New graph: node numbers changed, plan again next frame.
                self.cur = None;
                return true;
            }
        }
        if let Some(o) = opener {
            if self.cur != Some(o) {
                self.returns = 0;
            }
            self.cur = Some(o);
            self.noclip_goal(o);
        } else if let Some((i, path)) = choice {
            if self.cur != Some(i) {
                self.returns = 0;
            }
            self.cur = Some(i);
            self.path = path;
            self.step = 0;
            self.lift_action = None;
        } else {
            // Nothing reachable by game means. Triggers are not worth a noclip
            // (many are meant for monsters); noclip to anything else,
            // non-deferred goals first.
            for i in 0..self.goals.len() {
                if self.goals[i].kind == Kind::Trigger && Self::pending(&self.goals[i]) {
                    self.goals[i].failed = true;
                    self.rep.triggers_missed += 1;
                }
            }
            // In entity order, which tends to follow the level's layout;
            // goals not deferred first.
            let pick = |deferred: bool| {
                self.goals
                    .iter()
                    .position(|g| Self::pending(g) && !self.dormant(g) && g.deferred == deferred && (g.kind != Kind::Exit || !others_left))
            };
            // Keys first: a key door can cut off much of the level.
            let key = self.goals.iter().position(|g| g.kind == Kind::Key && Self::pending(g));
            let pick = key.or_else(|| pick(false)).or_else(|| pick(true));
            match pick {
                Some(i) => {
                    if self.goals[i].noclips >= 3 {
                        self.fail(i, "given up after 3 noclips");
                        return true;
                    }
                    self.cur = Some(i);
                    self.noclip_goal(i);
                }
                None => self.cur = None,
            }
        }
        let active = self.cur.is_some() || self.goals.iter().any(|g| Self::pending(g) && !self.dormant(g));
        if self.cur != prev_goal {
            self.goal_start = self.sv.time();
            self.best_dist = f32::INFINITY;
            self.best_time = self.sv.time();
            if let Some(i) = self.cur {
                let g = &self.goals[i];
                if g.kind == Kind::Exit {
                    let (e, touch) = (g.ent, g.touch);
                    self.sv.level.as_mut().unwrap().vm.set_ent_i(e, self.o.touch, touch);
                    self.rec.exits.push((self.frame, e));
                }
                if self.opt.verbose {
                    let g = &self.goals[i];
                    eprintln!("  t={:.1} goal {:?} #{} ({})", self.sv.time(), g.kind, g.ent, self.vm().classname(g.ent));
                }
            }
        }
        active
    }

    /// Edges (walk, jump, lift rides) through closed doors the bot can't open by walking into them:
    /// key doors while it lacks the key, and doors opened only by a button or
    /// trigger (the master door has a targetname and no health) that a
    /// pending goal still fires, directly or through relays. The key, button
    /// or trigger is an ordinary goal, so the bot fetches it first; the edges
    /// open again once the key is held or the door has moved. Doors nothing
    /// pending opens (delayed, opened by a monster's death) stay walkable.
    fn gates(&self) -> Gates {
        let mut out = Gates::default();
        let vm = self.vm();
        let (Some(owner), Some(items), Some(pos1)) = (self.o.owner, self.o.items, self.o.pos1) else { return out };
        let have = vm.ent_f(self.me, items) as i32;
        let velocity = vm.field_ofs("velocity");
        // Names each pending goal fires, through any chain of relays.
        let relays: Vec<(Vec<u8>, Vec<u8>)> = (1..vm.num_edicts() as u32)
            .filter(|&e| !vm.is_free(e))
            .map(|e| (ent_str(vm, e, self.o.targetname), ent_str(vm, e, self.o.target)))
            .filter(|(n, t)| !n.is_empty() && !t.is_empty())
            .collect();
        let mut fires: Vec<(usize, std::collections::HashSet<Vec<u8>>)> = Vec::new();
        for (i, g) in self.goals.iter().enumerate() {
            if !Self::pending(g) || !matches!(g.kind, Kind::Button | Kind::Trigger | Kind::Shoot | Kind::Secret | Kind::Pickup) {
                continue;
            }
            let t = ent_str(vm, g.ent, self.o.target);
            if t.is_empty() {
                continue;
            }
            let mut names = std::collections::HashSet::from([t]);
            let mut grew = true;
            while grew {
                grew = false;
                for (n, t) in &relays {
                    if names.contains(n) && !names.contains(t) {
                        names.insert(t.clone());
                        grew = true;
                    }
                }
            }
            fires.push((i, names));
        }
        let oldorigin = vm.field_ofs("oldorigin");
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP {
                continue;
            }
            let secret = match vm.classname(e).as_str() {
                "door" | "func_door" => false,
                "func_door_secret" => true,
                _ => continue,
            };
            if velocity.is_some_and(|f| v(vm.ent_v(e, f)).length_sq() > 0.0) {
                continue;
            }
            let m = if secret {
                // Closed: where it spawned (oldorigin); opened by its
                // trigger, or by shooting it (then it is its own goal).
                if oldorigin.is_none_or(|f| (v(vm.ent_v(e, self.o.origin)) - v(vm.ent_v(e, f))).length() > 1.0) {
                    continue;
                }
                let name = ent_str(vm, e, self.o.targetname);
                let shoot = self.goals.iter().position(|g| g.ent == e && g.kind == Kind::Shoot && Self::pending(g));
                let remote: Vec<usize> = if name.is_empty() { Vec::new() } else { fires.iter().filter(|f| f.1.contains(&name)).map(|f| f.0).collect() };
                if shoot.is_none() && remote.is_empty() {
                    continue;
                }
                out.openers.entry(e).or_insert_with(|| shoot.into_iter().chain(remote).collect());
                e
            } else {
                // Closed: resting where it spawned.
                if (v(vm.ent_v(e, self.o.origin)) - v(vm.ent_v(e, pos1))).length() > 1.0 {
                    continue;
                }
                let m = match vm.ent_i(e, owner) as u32 {
                    0 => e,
                    m if vm.is_free(m) => e,
                    m => m,
                };
                let need = vm.ent_f(m, items) as i32 & (IT_KEY1 | IT_KEY2) & !have;
                let name = ent_str(vm, m, self.o.targetname);
                let remote = vm.ent_f(m, self.o.health) <= 0.0 && fires.iter().any(|f| f.1.contains(&name));
                if need == 0 && !remote {
                    continue;
                }
                out.openers.entry(m).or_insert_with(|| {
                    let mut o: Vec<usize> = fires.iter().filter(|f| remote && f.1.contains(&name)).map(|f| f.0).collect();
                    o.extend(self.goals.iter().enumerate().filter(|(_, g)| {
                        g.kind == Kind::Key && Self::pending(g) && vm.ent_f(g.ent, items) as i32 & need != 0
                    }).map(|(i, _)| i));
                    o
                });
                m
            };
            let (lo, hi) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            let (zc, dz) = ((lo.z + hi.z) * 0.5, (hi.z - lo.z) * 0.5 + 64.0);
            // Slightly under the player's half width: edges grazing the frame stay.
            let (bl, bh) = (lo - Vec3::new(14.0, 14.0, 0.0), hi + Vec3::new(14.0, 14.0, 0.0));
            for n in self.nav.around(lo, hi, 64.0, zc, dz) {
                let pa = self.nav.pos[n as usize];
                for ed in &self.nav.edges[n as usize] {
                    if ed.kind == EdgeKind::Teleport {
                        continue;
                    }
                    let pb = self.nav.pos[ed.to as usize];
                    // The body spans origin -24..+32; standing on top of the door is fine.
                    if pa.z.min(pb.z) - 23.0 >= hi.z || pa.z.max(pb.z) + 32.0 <= lo.z {
                        continue;
                    }
                    if seg_hits_box_xy(pa, pb, bl, bh) {
                        out.edges.insert((n, ed.to));
                        out.edges.insert((ed.to, n));
                        out.door.insert((n, ed.to), m);
                        out.door.insert((ed.to, n), m);
                    }
                }
            }
        }
        out
    }

    /// The nearest node within 2048 units with room to stand and a floor
    /// under it now (nodes built on a plat float once it has moved away), or
    /// in water.
    fn landing(&self, here: Vec3) -> Option<u32> {
        let ok = |n: u32| {
            let p = self.nav.pos[n as usize];
            self.fits(p) && (self.geo.point_contents(p) == CONTENTS_WATER || !self.fits(p - Vec3::new(0.0, 0.0, 4.0)))
        };
        if let Some(n) = self.nav.nearest(here, 2048.0).filter(|&n| ok(n)) {
            return Some(n);
        }
        let r = Vec3::splat(2048.0);
        let mut ns = self.nav.around(here - r, here + r, 0.0, here.z, 2048.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - here).length_sq().total_cmp(&(self.nav.pos[b as usize] - here).length_sq()));
        ns.into_iter().take(64).find(|&n| ok(n))
    }

    /// Noclip to goal `i` (the current goal).
    fn noclip_goal(&mut self, i: usize) {
        if let Some(path) = self.jump_route(i) {
            self.path = path;
            self.step = 0;
            self.best_time = self.sv.time();
            self.best_dist = f32::INFINITY;
            self.last_replan = self.sv.time();
            return;
        }
        self.path.clear();
        let g = &self.goals[i];
        let to = match (g.kind, self.goal_node(g)) {
            (Kind::Monster, _) => self.monster_spot(g.ent),
            (Kind::Secret | Kind::Key | Kind::Pickup | Kind::Trigger | Kind::Exit, node) => self.trigger_spot(g.ent, node.map(|n| self.nav.pos[n as usize])).unwrap_or_else(|| self.center(g.ent)),
            (Kind::Button, None) => self.touch_spot(g.ent).unwrap_or_else(|| self.center(g.ent)),
            (_, Some(n)) => self.nav.pos[n as usize],
            (_, None) => self.center(g.ent),
        };
        self.goals[i].deferred = false;
        if self.opt.verbose {
            eprintln!("    near goal:{}", self.ents_near(to));
        }
        self.noclip(to, "unreachable");
    }

    /// A touch goal needs actual hull overlap after fitting around current
    /// doors and bodies. A generic nearest free node can land outside it.
    fn trigger_spot(&self, e: u32, preferred: Option<Vec3>) -> Option<Vec3> {
        let vm = self.vm();
        let lo = v(vm.ent_v(e, self.o.absmin));
        let hi = v(vm.ent_v(e, self.o.absmax));
        let c = (lo + hi) * 0.5;
        let inside = |p: Vec3| p.x + 16.0 > lo.x && p.x - 16.0 < hi.x
            && p.y + 16.0 > lo.y && p.y - 16.0 < hi.y
            && p.z + 32.0 > lo.z && p.z - 24.0 < hi.z;
        if let Some(p) = preferred.filter(|&p| inside(p) && self.fits(p)) { return Some(p); }
        let mut candidates = self.nav.around(lo, hi, 16.0, c.z, (hi.z - lo.z) * 0.5 + 32.0)
            .into_iter().map(|n| self.nav.pos[n as usize]).filter(|&p| inside(p)).collect::<Vec<_>>();
        for x in [c.x, lo.x + 1.0, hi.x - 1.0, lo.x - 15.0, hi.x + 15.0] {
            for y in [c.y, lo.y + 1.0, hi.y - 1.0, lo.y - 15.0, hi.y + 15.0] {
                for z in [c.z, lo.z - 31.0, hi.z + 23.0] {
                    let p = Vec3::new(x, y, z);
                    if let Some(g) = nav::ground(&self.geo, p + Vec3::new(0.0, 0.0, 24.0), 128.0)
                        && inside(g) { candidates.push(g); }
                    if inside(p) { candidates.push(p); }
                }
            }
        }
        candidates.sort_by(|a, b| (*a - c).length_sq().total_cmp(&(*b - c).length_sq()));
        candidates.into_iter().find(|&p| self.fits(p))
    }

    /// Where a player stands pressing a brush entity off the graph (a button
    /// on a high wall): just off one of its faces, body overlapping it in
    /// height, standing on a floor if any face has one.
    fn touch_spot(&self, e: u32) -> Option<Vec3> {
        let vm = self.vm();
        let (lo, hi) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
        let c = (lo + hi) * 0.5;
        let sides = [
            Vec3::new(lo.x - 17.0, c.y, c.z),
            Vec3::new(hi.x + 17.0, c.y, c.z),
            Vec3::new(c.x, lo.y - 17.0, c.z),
            Vec3::new(c.x, hi.y + 17.0, c.z),
        ];
        let mut floating = None;
        for p in sides {
            if !self.fits(p) {
                continue;
            }
            // Lowest origin whose body still reaches the button (body -24..+32).
            let tr = self.geo.trace(1, p, Vec3::new(p.x, p.y, lo.z - 31.0));
            let ground = tr.fraction < 1.0 && !tr.startsolid;
            if ground && self.fits(tr.endpos) {
                return Some(tr.endpos);
            }
            floating.get_or_insert(p);
        }
        floating
    }

    /// A nav node with a view of the monster, else a spot 72 units from it.
    fn monster_spot(&self, e: u32) -> Vec3 {
        let c = self.center(e);
        let r = Vec3::splat(384.0);
        let mut ns = self.nav.around(c - r, c + r, 0.0, c.z, 384.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - c).length_sq().total_cmp(&(self.nav.pos[b as usize] - c).length_sq()));
        for &n in &ns {
            let p = self.nav.pos[n as usize];
            let eye = p + Vec3::new(0.0, 0.0, 22.0);
            if self.fits(p) && self.clear_shot(eye, c, e) {
                return p;
            }
        }
        // A moving monster may have entered a region absent from the graph.
        // Find a supported, unobstructed firing position near its live origin
        // instead of repeatedly assisting to a distant node behind a door.
        for radius in [72.0, 128.0, 192.0, 256.0] {
            for i in 0..16 {
                let angle = i as f32 * std::f32::consts::TAU / 16.0;
                let point = c + Vec3::new(angle.cos() * radius, angle.sin() * radius, 32.0);
                if let Some(p) = nav::ground(&self.geo, point, 256.0)
                    && self.fits(p) && self.clear_shot(p + Vec3::new(0.0, 0.0, 22.0), c, e) {
                    return p;
                }
            }
        }
        let here = self.me_origin();
        let back = Vec3::new(here.x - c.x, here.y - c.y, 0.0);
        let p = if back.length() > 1.0 { c + back.normalize() * 72.0 } else { c };
        if nav::clear(&self.geo, p, p + Vec3::new(0.0, 0.0, 1.0)) {
            return p;
        }
        self.nav.nearest_any(c).map_or(p, |n| self.nav.pos[n as usize])
    }

    /// Whether a player fits at `p` right now: world, brush entities where
    /// they are now (closed doors, cages) and other bodies.
    fn fits(&self, p: Vec3) -> bool {
        if self.geo.trace(1, p, p).startsolid {
            return false;
        }
        let l = self.sv.level.as_ref().unwrap();
        let vm = &l.vm;
        let modelindex = vm.field_ofs("modelindex");
        let (pmin, pmax) = (p + Vec3::new(-16.0, -16.0, -24.0), p + Vec3::new(16.0, 16.0, 32.0));
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || e == self.me {
                continue;
            }
            let solid = vm.ent_f(e, self.o.solid) as i32;
            if solid == SOLID_NOT || solid == SOLID_TRIGGER {
                continue;
            }
            let (a, b) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            if a.x > pmax.x || b.x < pmin.x || a.y > pmax.y || b.y < pmin.y || a.z > pmax.z || b.z < pmin.z {
                continue;
            }
            if solid != SOLID_BSP {
                return false;
            }
            if let Some(mi) = modelindex
                && let Some((m, sub)) = l.world.model(vm.ent_f(e, mi) as usize)
            {
                let o = v(vm.ent_v(e, self.o.origin));
                if m.hull(*sub, 1).trace(p - o, p - o).startsolid {
                    return false;
                }
            }
        }
        true
    }

    /// Brush entities within 64 units (for the verbose log).
    fn ents_near(&self, p: Vec3) -> String {
        let vm = self.vm();
        let mut out = String::new();
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP {
                continue;
            }
            let (a, b) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            if a.x < p.x + 128.0 && b.x > p.x - 128.0 && a.y < p.y + 128.0 && b.y > p.y - 128.0 && a.z < p.z + 128.0 && b.z > p.z - 128.0 {
                out += &format!(" [#{e} {} {a:?}-{b:?}]", vm.classname(e));
            }
        }
        out
    }

    /// `to`, or the nearest node around it where a player fits.
    fn free_spot(&self, to: Vec3) -> Vec3 {
        if self.fits(to) {
            return to;
        }
        let r = Vec3::splat(256.0);
        let mut ns = self.nav.around(to - r, to + r, 0.0, to.z, 256.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - to).length_sq().total_cmp(&(self.nav.pos[b as usize] - to).length_sq()));
        ns.iter().take(48).map(|&n| self.nav.pos[n as usize]).find(|&p| self.fits(p)).unwrap_or(to)
    }

    /// Last resort: jump straight to `to` (a soft failure in the report).
    fn noclip(&mut self, to: Vec3, why: &str) {
        let t = self.sv.time();
        if why != "off the nav graph" && let Some(i) = self.cur {
            self.goals[i].noclips += 1;
            if self.goals[i].noclips > 3 {
                self.fail(i, "given up after 3 noclips");
                return;
            }
        }
        self.rep.noclip_uses += 1;
        if self.cur.is_some_and(|i| self.goals[i].kind == Kind::Monster) {
            self.rep.monster_noclips += 1;
        }
        if why != "off the nav graph" && self.cur.is_some_and(|i| self.goals[i].kind == Kind::Exit) {
            self.rep.exit_noclips += 1;
        }
        if self.opt.verbose {
            let g = self.cur.map(|i| (self.goals[i].kind, self.goals[i].ent, self.vm().classname(self.goals[i].ent)));
            eprintln!("  t={t:.1} NOCLIP ({why}) {:?} -> {to:?} (goal {g:?})", self.me_origin());
        }
        let to = self.free_spot(to);
        self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
        self.noclip_at = t;
        self.noclip_to_goal = why != "off the nav graph";
        self.best_dist = f32::INFINITY;
        self.best_time = t;
        self.ride_since = None;
        self.lift_action = None;
        self.jump_action = None;
    }

    /// Block the edge being walked and its parallel lanes (a doorway is
    /// several grid edges wide).
    fn block_here(&mut self) {
        if self.step == 0 || self.step >= self.path.len() {
            return;
        }
        let (a, b) = (self.path[self.step - 1], self.path[self.step]);
        let (pa, pb) = (self.nav.pos[a as usize], self.nav.pos[b as usize]);
        let dir = pb - pa;
        let r = Vec3::splat(64.0);
        for n in self.nav.around(pa - r, pa + r, 0.0, pa.z, 40.0) {
            for e in &self.nav.edges[n as usize] {
                let q = self.nav.pos[e.to as usize];
                if (q - pb).length() < 96.0 && (q - self.nav.pos[n as usize]).dot(dir) > 0.0 {
                    self.blocked.insert((n, e.to));
                }
            }
        }
        self.blocked.insert((a, b));
        if self.opt.verbose {
            eprintln!("  t={:.1} blocked {pa:?} -> {pb:?} ({} edges blocked)", self.sv.time(), self.blocked.len());
        }
    }

    fn main_loop(&mut self, map: &str) {
        let mut last_refill = -100.0;
        let mut intermission_since: Option<f64> = None;
        loop {
            let t = self.sv.time();
            if t > self.max_time {
                break;
            }
            if self.sv.core.changelevel_issued || !self.sv.core.sv.name.eq_ignore_ascii_case(map) {
                self.rep.exited = true;
                break;
            }
            if let Some(e) = self.sv.error() {
                self.rep.errors.push(format!("t={t:.2}: {}", e.to_string().replace('\n', " | ")));
                break;
            }
            for i in 0..self.goals.len() {
                if matches!(self.goals[i].kind, Kind::Key | Kind::Pickup) && !self.goals[i].armed && self.vm().ent_f(self.goals[i].ent, self.o.solid) as i32 == SOLID_TRIGGER {
                    self.goals[i].armed = true;
                }
                if !self.goals[i].done && self.goal_done(&self.goals[i]) {
                    self.goals[i].done = true;
                    self.progress = true;
                    if self.opt.verbose && self.goals[i].kind == Kind::Key {
                        eprintln!("  t={t:.1} got key #{}", self.goals[i].ent);
                    }
                    if self.cur == Some(i) {
                        self.cur = None;
                    }
                }
            }
            let in_intermission = self.intermission.is_some_and(|g| self.vm().g_f(g) != 0.0);
            // The level is finished once its intermission runs (finales have
            // no changelevel to wait for).
            if in_intermission {
                let since = *intermission_since.get_or_insert(t);
                if t - since > 8.0 {
                    self.rep.exited = true;
                    break;
                }
            }
            let mv = match self.replay_move() {
                Some(mv) => mv,
                None => match self.live_move(t, in_intermission, &mut last_refill) {
                    Some(mv) => mv,
                    None => break,
                },
            };
            if self.frame.is_multiple_of(FPS) {
                let p = self.me_origin();
                self.rec.checkpoints.push((self.frame, p));
            }
            self.rec.moves.push(mv);
            self.frame += 1;
            // impulse 9 (ammo) also hands out every key in id1: strip the
            // keys it added, so key doors stay locked until the key is taken.
            let keys_before = (mv.impulse == 9).then(|| self.o.items.map(|it| (it, self.vm().ent_f(self.me, it) as i32))).flatten();
            let jump_from = (mv.buttons & 2 != 0 && self.vm().ent_f(self.me, self.o.flags) as i32 & qworld::FL_ONGROUND != 0
                && self.vm().ent_f(self.me, self.o.waterlevel) == 0.0).then(|| self.me_origin());
            self.sv.set_move(self.c, mv);
            self.sv.frame(DT);
            if let Some(from) = jump_from {
                let fields = &self.sv.core.sv.fields;
                let velocity = v(self.vm().ent_v(self.me, fields.velocity));
                let delta = self.me_origin() - from;
                if delta.z > 1.0 && delta.length() < 48.0 && velocity.z > 100.0
                    && self.vm().ent_f(self.me, self.o.waterlevel) == 0.0 {
                    let scale = self.vm().field_ofs("gravity").map_or(0.0, |f| self.vm().ent_f(self.me, f));
                    let gravity = self.sv.core.sv.phys.gravity * if scale == 0.0 { 1.0 } else { scale };
                    let impulse = velocity.z + gravity * DT as f32;
                    if (100.0..=600.0).contains(&impulse) { self.jump_speed = Some(impulse); }
                }
            }
            if let Some((it, before)) = keys_before {
                let vm = &mut self.sv.level.as_mut().unwrap().vm;
                let now = vm.ent_f(self.me, it) as i32;
                let added = now & (IT_KEY1 | IT_KEY2) & !before;
                if added != 0 {
                    vm.set_ent_f(self.me, it, (now & !added) as f32);
                }
            }
            let _ = self.sv.take_reliable(self.c);
            let _ = self.sv.take_datagrams(self.c);
        }
        if self.opt.verbose {
            for g in self.goals.iter().filter(|g| !g.done) {
                eprintln!("  left: {:?} #{} ({}) at {:?}", g.kind, g.ent, self.vm().classname(g.ent), self.center(g.ent));
            }
        }
    }

    /// The next move of the cached route, or `None` (no route, or it no
    /// longer matches what the engine does: then the bot takes over).
    fn replay_move(&mut self) -> Option<ClientMove> {
        let r = self.replay.as_ref()?;
        let f = self.frame;
        let here = self.me_origin();
        let off = r.checkpoints.binary_search_by_key(&f, |c| c.0).ok().map(|i| (r.checkpoints[i].1 - here).length());
        if off.is_some_and(|d| d > 16.0) || f as usize >= r.moves.len() {
            let t = self.sv.time();
            if self.opt.verbose {
                eprintln!("  t={t:.1} route diverged ({} units off), taking over", off.unwrap_or(0.0));
            }
            self.rep.route = format!("diverged t={t:.0}");
            self.replay = None;
            self.rebuild_nav();
            return None;
        }
        let mv = r.moves[f as usize];
        let exits: Vec<u32> = r.exits.iter().filter(|x| x.0 == f).map(|x| x.1).collect();
        for e in exits {
            if let Some(g) = self.goals.iter().find(|g| g.ent == e && g.kind == Kind::Exit) {
                let touch = g.touch;
                self.sv.level.as_mut().unwrap().vm.set_ent_i(e, self.o.touch, touch);
                self.rec.exits.push((f, e));
            }
        }
        Some(mv)
    }

    /// The bot's own move; `None` when there is nothing left to do.
    fn live_move(&mut self, t: f64, in_intermission: bool, last_refill: &mut f64) -> Option<ClientMove> {
        if t - *last_refill > 5.0 {
            // All weapons and ammo, then the super nailgun (no splash).
            self.impulses = vec![self.weapon, 9];
            *last_refill = t;
        }
        // Just noclipped onto an unreachable goal: give it time.
        let settling = self.path.is_empty() && self.cur.is_some() && t - self.noclip_at < 5.0;
        if let Some(i) = self.cur
            && !in_intermission
        {
            let g = &self.goals[i];
            if t - self.goal_start > GOAL_TIMEOUT && self.ride_since.is_none() && self.jump_action.is_none() {
                self.goal_start = t;
                self.defer(i, "timed out");
            } else if self.path.is_empty() && g.kind != Kind::Monster && g.kind != Kind::Exit && self.noclip_to_goal && t - self.noclip_at > 4.0 && t - self.noclip_at < 5.0 {
                // Noclipped onto a goal that still isn't done: it won't be.
                self.fail(i, "not done after noclip");
            }
        }
        // QC can complete the goal and teleport the player during a jump.
        // The successful interaction supersedes the remaining input replay.
        if self.cur.is_none() && self.jump_action.is_some() {
            self.jump_action = None;
            self.path.clear();
            self.last_replan = -100.0;
        }
        let monster_goal = self.cur.is_some_and(|i| self.goals[i].kind == Kind::Monster);
        if !in_intermission
            && !settling
            && (self.cur.is_none() || t - self.last_replan > if monster_goal { 1.0 } else { 3.0 })
            && (self.ride_since.is_none() || self.cur.is_none())
            && self.jump_action.is_none()
            && !self.replan()
        {
            return None;
        }
        let mut mv = self.think();
        if let Some(i) = self.impulses.pop() {
            mv.impulse = i;
        }
        if trace() && self.frame.is_multiple_of(FPS / 2) {
            let next = self.path.get(self.step).map(|&n| self.nav.pos[n as usize]);
            eprintln!(
                "    t={t:.1} at {:?} wl {} step {}/{} next {next:?} mv f{:.0} s{:.0} u{:.0} b{}",
                self.me_origin(),
                self.vm().ent_f(self.me, self.o.waterlevel),
                self.step,
                self.path.len(),
                mv.forward,
                mv.side,
                mv.up,
                mv.buttons
            );
        }
        Some(mv)
    }

    fn aim_approach(&mut self, mv: &mut ClientMove) {
        if self.frame.is_multiple_of(2) { self.target_cache = self.target(); }
        let Some((_, ent)) = self.target_cache else { return };
        if self.vm().is_free(ent) || self.vm().ent_f(ent, self.o.health) <= 0.0 { return; }
        let (sin, cos) = mv.angles[1].to_radians().sin_cos();
        let wish = Vec3::new(mv.forward * cos + mv.side * sin, mv.forward * sin - mv.side * cos, 0.0);
        let target = self.center(ent);
        mv.angles = look_angles(target - self.eye());
        let (sin, cos) = mv.angles[1].to_radians().sin_cos();
        mv.forward = wish.x * cos + wish.y * sin;
        mv.side = wish.x * sin - wish.y * cos;
        mv.buttons |= 1;
        let weapon = if self.vm().classname(ent).contains("zombie") { 7 } else if (target - self.eye()).length() < 560.0 { 8 } else { 5 };
        if weapon != self.weapon { self.weapon = weapon; self.impulses.push(weapon); }
    }

    fn refine_local(&mut self, center: Vec3) -> bool {
        let changed = self.nav.refine(&self.geo, center);
        if changed {
            hints::reapply_blocks(&mut self.nav, &self.hints);
            self.rep.nav_nodes = self.rep.nav_nodes.max(self.nav.pos.len());
        }
        changed
    }

    /// Last walking recovery before an assist: bridge a nearby graph gap only
    /// after the real movement code lands the player on its far side.
    fn jump_route(&mut self, goal: usize) -> Option<Vec<u32>> {
        if self.jump_failed_goals.contains(&self.goals[goal].ent) { return None; }
        let checkpoint = self.nav.clone();
        let route = self.jump_route_inner(goal);
        if route.is_none() {
            let count = checkpoint.pos.len() as u32;
            self.nav.rollback(checkpoint);
            self.jump_tried.retain(|&(a, b)| a < count && b < count);
        }
        route
    }

    fn jump_route_inner(&mut self, goal: usize) -> Option<Vec<u32>> {
        let here = self.me_origin();
        let goal_center = self.center(self.goals[goal].ent);
        // A reachable goal that failed its interaction needs the existing
        // fallback, not more samples or another identical walking route.
        if let (Some(src), Some(dst)) = (self.nav.nearest_clear(&self.geo, here, 96.0), self.goal_node(&self.goals[goal])) {
            let blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
            if self.nav.dijkstra(src, &blocked, self.opt.seed).0[dst as usize].is_finite() { return None; }
        }
        let refined = self.refine_local(here) | self.refine_local(goal_center);
        let vm = self.vm();
        let fields = &self.sv.core.sv.fields;
        if v(vm.ent_v(self.me, fields.mins)) != qworld::hull::HULL_MINS[1]
            || v(vm.ent_v(self.me, fields.maxs)) != qworld::hull::HULL_MAXS[1]
            || vm.ent_f(self.me, self.o.waterlevel) >= 2.0 {
            return None;
        }
        let src = self.nav.nearest_clear(&self.geo, self.me_origin(), 96.0)?;
        if self.goal_node(&self.goals[goal]).is_none() {
            let c = self.center(self.goals[goal].ent);
            if let Some(g) = nav::ground(&self.geo, c + Vec3::new(0.0, 0.0, 32.0), 192.0) {
                self.nav.extend(&self.geo, g);
            }
        }
        let dst = self.goal_node(&self.goals[goal])?;
        let blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
        let (dist, prev) = self.nav.dijkstra(src, &blocked, self.opt.seed);
        if dist[dst as usize].is_finite() {
            return refined.then(|| path_to(&prev, dst));
        }
        let basin = self.nav.reaching(dst, &blocked);
        let mut candidates = Vec::new();
        for (a, &d) in dist.iter().enumerate() {
            if !d.is_finite() { continue; }
            let p = self.nav.pos[a];
            for b in self.nav.around(p, p, 192.0, p.z, 48.0) {
                if !basin[b as usize] || self.jump_tried.contains(&(a as u32, b)) || self.nav.edge(a as u32, b).is_some() { continue; }
                let q = self.nav.pos[b as usize];
                let delta = q - p;
                let xy = delta.x.hypot(delta.y);
                if !(40.0..=192.0).contains(&xy) || delta.z > 40.0 { continue; }
                let cost = d + delta.length() + (self.nav.pos[dst as usize] - q).length();
                candidates.push((cost, a as u32, b));
            }
        }
        candidates.sort_by(|a, b| a.0.total_cmp(&b.0).then_with(|| (a.1, a.2).cmp(&(b.1, b.2))));
        if candidates.is_empty() { return None; }
        // The missing passage may be far from both endpoints of the overall
        // route. Refine the best two reachable/basin boundary pairs as well.
        let mut added = false;
        for &(_, a, b) in candidates.iter().take(2) {
            let center = (self.nav.pos[a as usize] + self.nav.pos[b as usize]) * 0.5;
            added |= self.refine_local(center);
        }
        if added {
            let refined_blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
            let (cost, prev) = self.nav.dijkstra(src, &refined_blocked, self.opt.seed);
            if cost[dst as usize].is_finite() { return Some(path_to(&prev, dst)); }
        }
        let blocked: Blocked = self.blocked.union(&self.gates().edges).copied().collect();
        let mut probe = Probe::snapshot(&self.sv, self.jump_speed?)?;
        for (_, a, b) in candidates.into_iter().take(24) {
            self.jump_tried.insert((a, b));
            let (from, to) = (self.nav.pos[a as usize], self.nav.pos[b as usize]);
            if let Some(jump) = probe.jump(from, to, &self.forbid).or_else(|| probe.recover(from, to, &self.forbid))
                && hints::allows_motion(&jump.positions, &self.hints) {
                let cost = jump.moves.len() as f32 * 16.0 + 64.0;
                self.nav.link(a, b, EdgeKind::Jump, cost);
                self.jump_links.insert((a, b), jump);
                self.rep.jump_links += 1;
                let (dist, prev) = self.nav.dijkstra(src, &blocked, self.opt.seed);
                if dist[dst as usize].is_finite() {
                    if self.opt.verbose {
                        eprintln!("  t={:.1} physics-tested jump {from:?} -> {to:?} for #{}", self.sv.time(), self.goals[goal].ent);
                    }
                    self.rep.nav_nodes = self.rep.nav_nodes.max(self.nav.pos.len());
                    return Some(path_to(&prev, dst));
                }
            }
        }
        None
    }

    /// Try a verified short maneuver to the current waypoint before
    /// blocking it. Use the same alignment, revalidation and replay checks
    /// as gap recovery, and never bypass a gate or special mover edge.
    fn try_local_recovery(&mut self, here: Vec3, t: f64) -> bool {
        let Some(i) = self.cur else { return false; };
        let goal = self.goals[i].ent;
        if self.local_recovery_goals.contains(&goal) { return false; }
        let Some(&dst) = self.path.get(self.step) else { return false; };
        if self.step == 0 || self.jump_failed_goals.contains(&goal) { return false; }
        let edge = (self.path[self.step - 1], dst);
        if self.gates().edges.contains(&edge) || self.jump_tried.contains(&edge)
            || self.nav.edge(edge.0, edge.1).is_none_or(|e| !matches!(e.kind, EdgeKind::Walk | EdgeKind::Jump)) {
            return false;
        }
        let to = self.nav.pos[dst as usize];
        let velocity = v(self.vm().ent_v(self.me, self.sv.core.sv.fields.velocity));
        if velocity.x.hypot(velocity.y) > 16.0 || nav::clear(&self.geo, here, to) { return false; }
        if (to - here).length() > 192.0 || self.vm().ent_f(self.me, self.o.waterlevel) >= 2.0 { return false; }
        let fields = &self.sv.core.sv.fields;
        if v(self.vm().ent_v(self.me, fields.mins)) != qworld::hull::HULL_MINS[1]
            || v(self.vm().ent_v(self.me, fields.maxs)) != qworld::hull::HULL_MAXS[1] { return false; }
        let Some(mut probe) = self.jump_speed.and_then(|speed| Probe::snapshot(&self.sv, speed)) else { return false; };
        self.jump_tried.insert(edge);
        self.local_recovery_goals.insert(goal);
        let Some(plan) = probe.detour(here, to, &self.forbid) else { return false; };
        if !hints::allows_motion(&plan.positions, &self.hints) { return false; }
        let src = self.nav.add(here);
        self.nav.link(src, dst, EdgeKind::Jump, plan.moves.len() as f32 * 16.0);
        self.jump_links.insert((src, dst), plan);
        self.rep.jump_links += 1;
        self.path = std::iter::once(src).chain(self.path[self.step..].iter().copied()).collect();
        self.step = 1;
        self.best_time = t;
        self.jump_action = Some(JumpAction { edge: (src, dst), since: t, tick: None, released: false });
        true
    }

    /// Align and stop before replaying a verified jump. Abort on divergence;
    /// the ordinary route planner then chooses another way.
    fn jump_move(&mut self, here: Vec3, t: f64) -> Option<ClientMove> {
        if self.jump_action.is_none() {
            let edge = if self.step > 0 && self.step < self.path.len()
                && self.jump_links.contains_key(&(self.path[self.step - 1], self.path[self.step])) {
                Some((self.path[self.step - 1], self.path[self.step]))
            } else if self.step + 1 < self.path.len()
                && (self.nav.pos[self.path[self.step] as usize] - here).length() < 32.0
                && self.jump_links.contains_key(&(self.path[self.step], self.path[self.step + 1])) {
                self.step += 1;
                Some((self.path[self.step - 1], self.path[self.step]))
            } else { None };
            self.jump_action = edge.map(|edge| JumpAction { edge, since: t, tick: None, released: false });
        }
        let mut a = self.jump_action.take()?;
        let jump = self.jump_links.get(&a.edge)?;
        let from = self.nav.pos[a.edge.0 as usize];
        let mut failed = t - a.since > 4.0;
        let mut refreshed = None;
        let mv = match a.tick {
            None => {
                let d = from - here;
                let velocity = v(self.vm().ent_v(self.me, self.sv.core.sv.fields.velocity));
                if d.z.abs() > 24.0 { failed = true; }
                if d.x.hypot(d.y) < 2.0 && d.z.abs() < 4.0 && velocity.length() < 8.0 {
                    if a.released {
                        // Doors, monsters and movers may have changed while
                        // approaching. Verify again from the actual launch.
                        refreshed = self.jump_speed.and_then(|speed| Probe::snapshot(&self.sv, speed))
                            .and_then(|mut probe| if jump.recovery {
                                probe.recover(here, self.nav.pos[a.edge.1 as usize], &self.forbid)
                            } else { probe.jump(here, self.nav.pos[a.edge.1 as usize], &self.forbid) })
                            .filter(|plan| hints::allows_motion(&plan.positions, &self.hints));
                        if refreshed.is_some() { a.tick = Some(0); } else { failed = true; }
                    }
                    a.released = true;
                    ClientMove::default()
                } else {
                    ClientMove { angles: [0.0, d.y.atan2(d.x).to_degrees(), 0.0], forward: if d.x.hypot(d.y) < 2.0 { 0.0 } else { (d.x.hypot(d.y) * 4.0).min(80.0) }, ..Default::default() }
                }
            }
            Some(tick) => {
                if (jump.positions[tick] - here).length() > 20.0 { failed = true; }
                if tick == jump.moves.len() {
                    if self.vm().ent_f(self.me, self.o.flags) as i32 & qworld::FL_ONGROUND != 0 && !failed {
                        self.best_time = t;
                        self.last_replan = t;
                        self.step += 1;
                        self.rep.jumps_completed += 1;
                        return None;
                    }
                    failed = true;
                    ClientMove::default()
                } else {
                    a.tick = Some(tick + 1);
                    jump.moves[tick]
                }
            }
        };
        if let Some(jump) = refreshed { self.jump_links.insert(a.edge, jump); }
        self.best_time = t;
        if failed {
            self.rep.jump_failures += 1;
            if let Some(i) = self.cur { self.jump_failed_goals.insert(self.goals[i].ent); }
            self.blocked.insert(a.edge);
            self.nav.unlink(a.edge.0, a.edge.1);
            self.jump_links.remove(&a.edge);
            self.path.clear();
            self.last_replan = -100.0;
            if self.opt.verbose { eprintln!("  t={t:.1} jump {:?} diverged at {here:?} (tick {:?})", a.edge, a.tick); }
            Some(ClientMove::default())
        } else {
            self.jump_action = Some(a);
            Some(mv)
        }
    }

    /// Steering on a lift edge; `None` when not on one.
    fn ride(&mut self, here: Vec3, t: f64) -> Option<Vec3> {
        if self.step == 0 || self.step >= self.path.len() {
            self.ride_since = None;
            self.lift_action = None;
            return None;
        }
        let key = (self.path[self.step - 1], self.path[self.step]);
        let Some(&lift) = self.lifts.get(&key) else {
            self.ride_since = None;
            self.lift_action = None;
            return None;
        };
        if self.lift_action.as_ref().is_none_or(|a| a.edge != key) {
            self.lift_action = Some(LiftAction::new(key, t));
        }
        let since = match self.ride_since {
            Some((s, since)) if s == self.step => since,
            _ => {
                self.ride_since = Some((self.step, t));
                t
            }
        };
        // Progress is measured by the lift arriving, not by distance.
        self.best_time = t;
        let patience = if self.opt.seed == 0 { RIDE_TIME } else { RIDE_TIME * (0.6 + 0.9 * nav::unit_hash(self.opt.seed, 2, 0) as f64) };
        if self.lift_action.as_ref().is_some_and(|a| a.expired(t, patience)
            || (a.trip.is_none() && !a.called && t - since > patience)) {
            if self.opt.verbose {
                eprintln!("  t={t:.1} lift #{} never came", lift.ent);
            }
            self.block_here();
            self.ride_since = None;
            self.last_replan = -100.0;
            self.path.clear();
            return Some(Vec3::ZERO);
        }
        self.call = None;
        if self.lift_action.as_ref().is_some_and(|a| a.trip.is_some()) {
            return Some(self.lift_button_trip(here, t));
        }
        let surf = self.vm().ent_v(lift.ent, self.o.absmax)[2];
        let c = (lift.mins + lift.maxs) * 0.5;
        let still = self.vm().field_ofs("velocity").is_none_or(|f| v(self.vm().ent_v(lift.ent, f)).length_sq() == 0.0);
        let (up0, up1) = (self.nav.pos[key.0 as usize].z, self.nav.pos[key.1 as usize].z);
        let arrived = (surf - if up1 > up0 { lift.hi } else { lift.lo }).abs() < 4.0;
        // A lift that only moves when called: press (or shoot) a button that
        // fires it, once it has kept still for a moment.
        if still && !arrived && t - since > 1.5
            && let Some(w) = self.call_lift(lift.ent, here)
        {
            return Some(w);
        }
        if still && !arrived && t - since > 1.5
            && self.lift_action.as_ref().is_some_and(|a| !a.called)
            && let Some(trip) = self.plan_lift_button(lift.ent, here, key)
        {
            let a = self.lift_action.as_mut().unwrap();
            a.enter(LiftStage::ApproachButton, t);
            a.trip = Some(trip);
            self.rep.lift_detours += 1;
            return Some(self.lift_button_trip(here, t));
        }
        let inside = here.x > lift.mins.x - 16.0 && here.x < lift.maxs.x + 16.0 && here.y > lift.mins.y - 16.0 && here.y < lift.maxs.y + 16.0;
        let on = inside && (here.z - 24.0 - surf).abs() < 6.0;
        let up = self.nav.pos[key.1 as usize].z > self.nav.pos[key.0 as usize].z;
        let to_center = Vec3::new(c.x - here.x, c.y - here.y, 0.0);
        let at = |z: f32| (surf - z).abs() < 4.0;
        // Going up: board it at the bottom. Going down: drop onto it when it
        // is at the bottom; ride it down only if it isn't a plat (a plat stays
        // up while someone stands on it).
        let board = if up { at(lift.lo) } else { at(lift.lo) || (at(lift.hi) && !lift.plat) };
        let stage = if arrived && on { LiftStage::Leave } else if on { LiftStage::Ride } else if board { LiftStage::Board } else { LiftStage::Wait };
        self.lift_action.as_mut().unwrap().enter(stage, t);
        if on || board {
            Some(if to_center.length() > 8.0 { to_center } else { Vec3::ZERO })
        } else if inside && surf > here.z {
            // Under it while it is away: step out so it can come down.
            Some(-to_center)
        } else {
            Some(Vec3::ZERO)
        }
    }

    /// Plan a walk/jump detour to a caller without using the lift being called,
    /// other movers, teleporters or locked doors. Return along that same route.
    fn plan_lift_button(&self, lift: u32, here: Vec3, edge: (u32, u32)) -> Option<ButtonTrip> {
        let src = self.nav.nearest_clear(&self.geo, here, 96.0)?;
        let mut blocked = self.blocked.clone();
        blocked.extend(self.gates().edges);
        blocked.extend(self.jump_links.keys().copied());
        blocked.insert(edge);
        blocked.insert((edge.1, edge.0));
        let mut candidates = Vec::new();
        for b in self.lift_callers(lift) {
            let shoot = self.vm().ent_f(b, self.o.health) > 0.0 && self.vm().ent_f(b, self.o.takedamage) != 0.0;
            let Some(g) = self.goals.iter().find(|g| g.ent == b) else { continue };
            let nodes = self.goal_node(g).into_iter().chain(if shoot { g.views.as_slice() } else { &[] }.iter().copied());
            for n in nodes {
                if shoot && !self.clear_shot(self.nav.pos[n as usize] + Vec3::new(0.0, 0.0, 22.0), self.center(b), b) {
                    continue;
                }
                candidates.push((b, n, shoot));
            }
        }
        let trip = crate::actions::button_trip(&self.nav, src, &candidates, &blocked, self.opt.seed)?;
        let ent = trip.ent;
        if self.opt.verbose {
            eprintln!("  t={:.1} lift #{lift}: detour to caller #{ent}", self.sv.time());
        }
        Some(trip)
    }

    fn lift_button_trip(&mut self, here: Vec3, t: f64) -> Vec3 {
        let mut a = self.lift_action.take().unwrap();
        let trip = a.trip.as_mut().unwrap();
        let activated = self.vm().is_free(trip.ent)
            || self.o.state.is_some_and(|s| self.vm().ent_f(trip.ent, s) != 1.0);
        if activated && !trip.returning {
            trip.returning = true;
            trip.path.reverse();
            trip.step = 0;
            a.called = true;
            a.stage = LiftStage::ReturnToLift;
            a.since = t;
            self.call = None;
        }
        while trip.step + 1 < trip.path.len() {
            let d = self.nav.pos[trip.path[trip.step] as usize] - here;
            if d.x.hypot(d.y) >= REACH || d.z.abs() >= 30.0 { break; }
            trip.step += 1;
        }
        let p = self.nav.pos[trip.path[trip.step] as usize];
        let d = p - here;
        let at_end = trip.step + 1 == trip.path.len() && d.x.hypot(d.y) < REACH && d.z.abs() < 30.0;
        let wish = if at_end && trip.returning {
            a.trip = None;
            a.enter(LiftStage::Wait, t);
            // Restart the lift wait after returning from the button.
            self.ride_since = Some((self.step, t));
            Vec3::ZERO
        } else if at_end {
            if a.stage != LiftStage::ActivateButton {
                a.stage = LiftStage::ActivateButton;
                a.since = t;
            }
            if trip.shoot {
                if self.clear_shot(self.eye(), self.center(trip.ent), trip.ent) {
                    self.call = Some(trip.ent);
                }
                Vec3::ZERO
            } else {
                let c = self.center(trip.ent);
                let bottom = self.vm().ent_v(trip.ent, self.o.absmin)[2];
                self.action_jump = bottom > here.z + 31.0 && bottom <= here.z + 72.0;
                Vec3::new(c.x - here.x, c.y - here.y, 0.0)
            }
        } else {
            self.action_jump = trip.step > 0 && self.nav.edge(trip.path[trip.step - 1], trip.path[trip.step])
                .is_some_and(|e| e.kind == EdgeKind::Jump);
            Vec3::new(d.x, d.y, 0.0)
        };
        self.lift_action = Some(a);
        wish
    }

    /// A shootable brush entity (secret door, button) right in front of the
    /// bot in the direction it wants to go.
    fn shootable_ahead(&self, here: Vec3, wish: Vec3) -> Option<u32> {
        if wish.length_sq() < 1.0 {
            return None;
        }
        let ahead = here + wish.normalize() * 48.0;
        let vm = self.vm();
        (1..vm.num_edicts() as u32).find(|&e| {
            if vm.is_free(e) || vm.ent_f(e, self.o.solid) as i32 != SOLID_BSP || vm.ent_f(e, self.o.takedamage) == 0.0 || vm.ent_f(e, self.o.health) <= 0.0 {
                return false;
            }
            let (lo, hi) = (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)));
            ahead.x + 16.0 > lo.x && ahead.x - 16.0 < hi.x && ahead.y + 16.0 > lo.y && ahead.y - 16.0 < hi.y && here.z + 32.0 > lo.z && here.z - 24.0 < hi.z
        })
    }

    /// Unpressed buttons that fire `lift` (directly or through relays).
    fn lift_callers(&self, lift: u32) -> Vec<u32> {
        let vm = self.vm();
        let owner = self.o.owner.map_or(0, |f| vm.ent_i(lift, f) as u32);
        let m = if owner != 0 && !vm.is_free(owner) { owner } else { lift };
        let mut names = vec![ent_str(vm, m, self.o.targetname)];
        if names[0].is_empty() {
            return Vec::new();
        }
        // Walk relays backwards: anything targeting a name we want, with a name itself.
        let mut i = 0;
        while i < names.len() && names.len() < 16 {
            for e in 1..vm.num_edicts() as u32 {
                if !vm.is_free(e) && ent_str(vm, e, self.o.target) == names[i] && vm.classname(e) != "func_button" {
                    let n = ent_str(vm, e, self.o.targetname);
                    if !n.is_empty() && !names.contains(&n) {
                        names.push(n);
                    }
                }
            }
            i += 1;
        }
        (1..vm.num_edicts() as u32)
            .filter(|&e| !vm.is_free(e) && vm.classname(e) == "func_button" && names.contains(&ent_str(vm, e, self.o.target)))
            // STATE_BOTTOM: out, ready to press.
            .filter(|&e| self.o.state.is_none_or(|s| vm.ent_f(e, s) == 1.0))
            .collect()
    }

    /// Move (or aim) to press a button calling `lift`; `None` when there is none in reach.
    fn call_lift(&mut self, lift: u32, here: Vec3) -> Option<Vec3> {
        let eye = self.eye();
        let mut best: Option<(f32, u32)> = None;
        for b in self.lift_callers(lift) {
            let d = (self.center(b) - here).length();
            if d < 640.0 && best.is_none_or(|x| d < x.0) {
                best = Some((d, b));
            }
        }
        let (_, b) = best?;
        let c = self.center(b);
        let vm = self.vm();
        if vm.ent_f(b, self.o.health) > 0.0 && vm.ent_f(b, self.o.takedamage) != 0.0 && self.clear_shot(eye, c, b) {
            self.call = Some(b);
            return Some(Vec3::ZERO);
        }
        // Walk into it if it is within a jump of the floor and nothing is in between.
        let spot = self.touch_spot(b).unwrap_or(c);
        if (spot.z - here.z).abs() > 48.0 || !nav::clear(&self.geo, here, Vec3::new(spot.x, spot.y, here.z)) {
            return None;
        }
        if self.opt.verbose && self.frame.is_multiple_of(20) {
            eprintln!("  t={:.1} calling lift #{lift} with button #{b}", self.sv.time());
        }
        Some(Vec3::new(c.x - here.x, c.y - here.y, 0.0))
    }

    fn think(&mut self) -> ClientMove {
        let t = self.sv.time();
        let here = self.me_origin();
        let mut mv = ClientMove::default();
        if self.vm().ent_f(self.me, self.o.movetype) as i32 == MOVETYPE_NOCLIP {
            self.sv.command("noclip 0");
        }
        if self.intermission.is_some_and(|g| self.vm().g_f(g) != 0.0) {
            // Exit reached: press fire to leave the intermission.
            mv.buttons = if (t * 4.0) as i64 % 2 == 0 { 1 } else { 0 };
            return mv;
        }

        if let Some(mut mv) = self.jump_move(here, t) {
            // Keep fighting while lining up; flight replays its verified
            // steering without changing view pitch or yaw.
            if self.jump_action.as_ref().is_some_and(|a| a.tick.is_none()) {
                self.aim_approach(&mut mv);
            }
            return mv;
        }

        // Skip ahead when a later path node is already reached (teleporters,
        // lifts and drops move the bot past nodes).
        let close = |nav: &Nav, n: u32| {
            let d = nav.pos[n as usize] - here;
            let reach = if nav.is_fine(n) { 4.0 } else { REACH };
            (d.x * d.x + d.y * d.y) < reach * reach && d.z.abs() < 30.0
        };
        for k in (self.step + 1..self.path.len().min(self.step + 6)).rev() {
            if close(&self.nav, self.path[k]) {
                self.step = k;
                break;
            }
        }

        // Steer along the path.
        let mut wish = Vec3::ZERO;
        self.action_jump = false;
        let riding = self.ride(here, t);
        let mut jump = self.action_jump;
        if let Some(w) = riding {
            wish = w;
            if self.path.is_empty() {
                return mv;
            }
            if close(&self.nav, self.path[self.step]) && self.step + 1 < self.path.len() {
                self.step += 1;
                self.ride_since = None;
            }
        } else {
            while self.step < self.path.len() {
                let p = self.nav.pos[self.path[self.step] as usize];
                let d = p - here;
                if close(&self.nav, self.path[self.step]) && self.step + 1 < self.path.len() {
                    self.step += 1;
                    if self.lifts.contains_key(&(self.path[self.step - 1], self.path[self.step])) {
                        break;
                    }
                    continue;
                }
                wish = Vec3::new(d.x, d.y, 0.0);
                if self.step > 0
                    && let Some(e) = self.nav.edge(self.path[self.step - 1], self.path[self.step])
                {
                    jump = e.kind == EdgeKind::Jump;
                }
                // Jump into teleporters and updrafts (some start above head height).
                if self.step + 1 < self.path.len()
                    && (d.x * d.x + d.y * d.y) < 48.0 * 48.0
                    && self.nav.edge(self.path[self.step], self.path[self.step + 1]).is_some_and(|e| e.kind == EdgeKind::Teleport)
                {
                    jump = true;
                }
                break;
            }
        }
        // Final approach: walk into the goal itself (buttons sit in walls).
        if let Some(i) = self.cur
            && self.step + 1 >= self.path.len()
            && riding.is_none()
        {
            let g = &self.goals[i];
            if matches!(g.kind, Kind::Button | Kind::Secret | Kind::Key | Kind::Pickup | Kind::Exit | Kind::Trigger) {
                let c = self.center(g.ent);
                let d = c - here;
                if (d.x * d.x + d.y * d.y) > 4.0 {
                    wish = Vec3::new(d.x, d.y, 0.0);
                }
                let bottom = self.vm().ent_v(g.ent, self.o.absmin)[2];
                // Trigger centers can be above the player's head even though
                // a jump reaches their lower face.
                if (matches!(g.kind, Kind::Trigger | Kind::Exit) && bottom > here.z + 31.0 && bottom <= here.z + 72.0)
                    || (g.kind == Kind::Secret && d.z > 40.0)
                {
                    jump = true;
                }
            }
        }

        // At the end of the path but the monster is still out of sight (on a
        // ledge the nav graph doesn't reach): try other goals first.
        if let Some(i) = self.cur
            && self.goals[i].kind == Kind::Monster
            && !self.path.is_empty()
            && self.step + 1 >= self.path.len()
            && wish.length() < 24.0
        {
            let (e, c) = (self.goals[i].ent, self.center(self.goals[i].ent));
            if !self.clear_shot(self.eye(), c, e) {
                self.defer(i, "out of sight at the end of its path");
                return mv;
            }
        }

        // Progress towards the goal; stuck → jump and strafe, then block the edge.
        let mut unstick = false;
        if !self.path.is_empty()
            && let Some(gp) = self.goal_point()
        {
            let d = (gp - here).length();
            if d < self.best_dist - 16.0 || d < 32.0 {
                self.best_dist = self.best_dist.min(d);
                self.best_time = t;
                if self.ride_since.is_none() {
                    self.call = None;
                }
            } else if t - self.best_time > STUCK_TIME * 0.5 && t - self.best_time <= STUCK_TIME {
                unstick = true;
                // Up against something that opens when shot (a secret door
                // that closed again): shoot it, as a player would.
                if self.call.is_none() {
                    self.call = self.shootable_ahead(here, wish);
                }
            } else if t - self.best_time > STUCK_TIME {
                if self.opt.verbose {
                    eprintln!("  t={t:.1} stuck at {here:?}{}", self.ents_near(here));
                }
                if self.try_local_recovery(here, t) { return mv; }
                if self.step > 0 && self.step < self.path.len() {
                    self.block_here();
                    self.last_replan = -100.0;
                    self.best_dist = f32::INFINITY;
                    self.best_time = t;
                } else if self.bad_src.len() < 64 && !self.path.is_empty() {
                    // Couldn't get onto the first node: plan from another one.
                    self.bad_src.insert(self.path[0]);
                    self.last_replan = -100.0;
                    self.best_dist = f32::INFINITY;
                    self.best_time = t;
                } else {
                    self.noclip(gp, "stuck before the first node");
                }
                return mv;
            }
        }

        if self.frame.is_multiple_of(2) || self.target_cache.is_some_and(|(_, e)| self.vm().is_free(e) || self.vm().ent_f(e, self.o.health) <= 0.0) {
            self.target_cache = self.target();
        }
        if self.call.is_some_and(|e| self.vm().is_free(e) || self.vm().ent_f(e, self.o.takedamage) == 0.0 || self.vm().ent_f(e, self.o.health) <= 0.0) {
            self.call = None;
        }
        // Aim at where the cached target is now (a lift's button first).
        let target = self.call.map(|e| (self.center(e), e)).or_else(|| self.target_cache.map(|(_, e)| (self.center(e), e)));
        if let Some((tg, e)) = target {
            mv.angles = look_angles(tg - self.eye());
            mv.buttons |= 1;
            // Zombies only die when gibbed: rockets. Otherwise the
            // lightning gun in its range (hitscan: no projectiles for the
            // server to simulate), the super nailgun beyond it.
            let w = if self.vm().classname(e).contains("zombie") {
                7
            } else if (tg - self.eye()).length() < 560.0 {
                8
            } else {
                5
            };
            if w != self.weapon {
                self.weapon = w;
                self.impulses.push(w);
            }
        } else if wish.length_sq() > 1.0 {
            self.yaw = wish.y.atan2(wish.x).to_degrees();
            mv.angles = [0.0, self.yaw, 0.0];
        } else {
            mv.angles = [0.0, self.yaw, 0.0];
        }
        if wish.length_sq() > 1.0 {
            // Wish direction relative to the view yaw; slow down near a lift's middle.
            let fine = self.path.get(self.step).is_some_and(|&n| self.nav.is_fine(n));
            let speed = if riding.is_some() { (wish.length() * 8.0).min(320.0) }
                else if fine { (wish.length() * 8.0).min(160.0) } else { 320.0 };
            let w = wish.normalize() * speed;
            let (s, c) = mv.angles[1].to_radians().sin_cos();
            mv.forward = w.x * c + w.y * s;
            mv.side = w.x * s - w.y * c;
        }
        if unstick && riding.is_none() {
            mv.buttons |= 2;
            mv.side = if (t * 2.0) as i64 % 2 == 0 { 320.0 } else { -320.0 };
        }
        if jump {
            mv.buttons |= 2;
        }
        let waterlevel = self.vm().ent_f(self.me, self.o.waterlevel);
        if waterlevel >= 2.0 || self.cm.point_contents(here) == CONTENTS_WATER {
            // Swim towards the next node's height: up (jump at the surface to
            // climb out), or dive for a node on the floor below.
            let next = self.path.get(self.step).map(|&n| self.nav.pos[n as usize]);
            match next {
                Some(p) if p.z < here.z - 16.0 => mv.up = -300.0,
                _ if next.is_none_or(|p| p.z > here.z - 8.0) => {
                    mv.up = 300.0;
                    mv.buttons |= 2;
                }
                _ => {}
            }
        }
        mv
    }

    fn goal_point(&self) -> Option<Vec3> {
        let g = &self.goals[self.cur?];
        match self.path.last() {
            Some(&n) => Some(self.nav.pos[n as usize]),
            None => Some(self.center(g.ent)),
        }
    }
}

/// Keep the ordinary approach when reachable; a shootable opener on an
/// inaccessible ledge can instead be activated from a reachable view node.
fn opener_node(touch: Option<u32>, views: &[u32], dist: &[f32]) -> Option<u32> {
    touch.filter(|&n| dist[n as usize].is_finite()).or_else(|| {
        views.iter().copied().filter(|&n| dist[n as usize].is_finite())
            .min_by(|&a, &b| dist[a as usize].total_cmp(&dist[b as usize]))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shootable_opener_on_disconnected_ledge() {
        let mut nav = Nav::empty(Vec3::ZERO, Vec::new());
        let start = nav.add(Vec3::ZERO);
        let ledge = nav.add(Vec3::new(0.0, 0.0, 128.0));
        let near = nav.add(Vec3::new(32.0, 0.0, 0.0));
        let far = nav.add(Vec3::new(64.0, 0.0, 0.0));
        nav.link(start, near, EdgeKind::Walk, 32.0);
        nav.link(near, far, EdgeKind::Walk, 32.0);
        let (dist, _) = nav.dijkstra(start, &Blocked::default(), 0);
        assert_eq!(opener_node(Some(ledge), &[ledge, far, near], &dist), Some(near));
        assert_eq!(opener_node(None, &[far, near], &dist), Some(near));
        assert_eq!(opener_node(Some(far), &[near], &dist), Some(far));
        assert_eq!(opener_node(Some(ledge), &[], &dist), None);
        // Blocking the corridor must not leave a stale shooting approach.
        let blocked = Blocked::from_iter([(start, near)]);
        let (dist, _) = nav.dijkstra(start, &blocked, 0);
        assert_eq!(opener_node(Some(ledge), &[far, near], &dist), None);
    }
}
