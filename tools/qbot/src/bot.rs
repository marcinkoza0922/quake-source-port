//! One map run: spawn a headless server, build the nav graph, then chase goals
//! (secrets, monsters, buttons, keys, finally the exit) with usercmds until
//! none are left.
//!
//! The bot moves by game means only: walking, jumping, riding lifts and
//! teleporters. When an edge turns out impassable (a locked door, a lift that
//! needs a trigger) it is blocked and other goals come first; blocks are lifted
//! once something changed. Noclip (an instant `setpos`) is the last resort and
//! is reported as a soft failure.

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
    pub nav_nodes: usize,
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
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Secret,
    Monster,
    Button,
    Key,
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
    /// A goal was finished since blocks and deferrals were last lifted.
    progress: bool,
    lifts: HashMap<(u32, u32), Lift, FxBuild>,
    /// Waiting on a lift edge (path step) since.
    ride_since: Option<(usize, f64)>,
    /// Time of the last noclip, for goals reached that way.
    noclip_at: f64,
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
}

struct Built {
    geo: Geo,
    nav: Nav,
    lifts: HashMap<(u32, u32), Lift, FxBuild>,
    exit_nodes: Vec<Option<u32>>,
    sig: u64,
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
fn build_nav(vm: &Vm, world: &qworld::world::World, o: &Ofs, cm: &Arc<CollisionModel>, start: Vec3, forbid: &[Forbid], exits: &[u32]) -> Built {
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
        let Some(mi) = modelindex else { continue };
        if let Some((m, sub)) = world.model(vm.ent_f(e, mi) as usize) {
            geo.add_solid(m.clone(), *sub, v(vm.ent_v(e, o.origin)), v(vm.ent_v(e, o.absmin)), v(vm.ent_v(e, o.absmax)), mover);
        }
    }
    let mut nav = Nav::build(&geo, start, forbid.to_vec());
    let mut lifts = HashMap::default();
    add_movers(&mut nav, &geo, vm, o, &mut lifts);
    let exit_nodes = exits
        .iter()
        .map(|&e| add_entry(&mut nav, &geo, v(vm.ent_v(e, o.absmin)), v(vm.ent_v(e, o.absmax))))
        .collect();
    Built { geo, nav, lifts, exit_nodes, sig: mover_sig(vm, o) }
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
        .find_map(|&p| nav::ground(cm, p, (p.z - mins.z) + 64.0).filter(|g| g.z > mins.z - 40.0))
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
        } else if let Some(lift) = lift_of(vm, o, e, &ents) {
            let Lift { lo, hi, mins, maxs, .. } = lift;
            let c = (mins + maxs) * 0.5;
            let (bp, tp) = (Vec3::new(c.x, c.y, lo + 25.0), Vec3::new(c.x, c.y, hi + 25.0));
            if !nav::clear(cm, bp, bp + Vec3::new(0.0, 0.0, 1.0)) || !nav::clear(cm, tp, tp + Vec3::new(0.0, 0.0, 1.0)) {
                if debug() {
                    eprintln!("lift #{e} {} lo {lo} hi {hi}: stop in solid", vm.classname(e));
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

pub fn run(fs: Fs, map: &str, opt: &Options) -> Report {
    let wall = Instant::now();
    let mut rep = Report::default();
    let route_path = opt.routes.as_ref().map(|d| d.join(format!("{map}.qbr")));
    let key = match (fs.open(&format!("maps/{map}.bsp")), fs.open("progs.dat")) {
        (Some(b), Some(p)) => route::key(&b, &p, opt.skill),
        _ => 0,
    };
    let replay = route_path.as_ref().filter(|_| !opt.fresh && key != 0).and_then(|p| Route::load(p, key));
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
            _ if l.vm.ent_f(e, o.solid) as i32 == SOLID_BSP && l.vm.ent_f(e, o.takedamage) != 0.0 && l.vm.ent_f(e, o.health) > 0.0 => Kind::Shoot,
            _ if l.vm.ent_f(e, o.flags) as i32 & FL_MONSTER != 0 => Kind::Monster,
            _ => continue,
        };
        let initial_state = o.state.map_or(0.0, |s| l.vm.ent_f(e, s));
        goals.push(Goal { ent: e, kind, done: false, failed: false, deferred: false, defers: 0, initial_state, noclips: 0, node: None, touch: 0, views: Vec::new(), views_at: None });
    }
    rep.has_exit = !exits.is_empty();
    let exit_ents: Vec<u32> = exits.iter().map(|x| x.0).collect();
    // A replayed route needs no graph; one is built if the replay diverges.
    let built = if replay.is_some() {
        Built { geo: Geo::new(cm.clone()), nav: Nav::empty(start, forbid.clone()), lifts: HashMap::default(), exit_nodes: vec![None; exit_ents.len()], sig: 0 }
    } else {
        build_nav(&l.vm, &l.world, &o, &cm, start, &forbid, &exit_ents)
    };
    let (geo, nav, lifts, sig) = (built.geo, built.nav, built.lifts, built.sig);
    for ((e, touch), node) in exits.into_iter().zip(built.exit_nodes) {
        goals.push(Goal { ent: e, kind: Kind::Exit, done: false, failed: false, deferred: false, defers: 0, initial_state: 0.0, noclips: 0, node, touch, views: Vec::new(), views_at: None });
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
        progress: false,
        lifts,
        ride_since: None,
        noclip_at: -100.0,
        intermission,
        forbid,
        start,
        mover_sig: sig,
        frame: 0,
        target_cache: None,
        rec: Route { key, ..Default::default() },
        replay,
    };
    if bot.replay.is_some() {
        bot.rep.route = "replayed".into();
    }
    // Big maps get more time: 900 s, or 15 s per goal.
    bot.max_time = opt.max_time.unwrap_or_else(|| (bot.goals.len() as f64 * 15.0).max(900.0));
    bot.main_loop(map);
    // Finished by game means only: cache the route (a replay that held is
    // already in the cache).
    let ok = bot.rep.exited && bot.rep.noclip_uses == 0 && bot.rep.errors.is_empty() && !bot.rep.stranded;
    if ok
        && key != 0
        && bot.rep.route != "replayed"
        && let Some(p) = &route_path
    {
        match bot.rec.save(p) {
            Ok(()) => bot.rep.route = if bot.rep.route.is_empty() { "recorded".into() } else { format!("{}, recorded", bot.rep.route) },
            Err(e) => bot.rep.errors.push(format!("saving route {}: {e}", p.display())),
        }
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
            Kind::Key => vm.ent_f(e, self.o.solid) as i32 == SOLID_NOT,
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
            Kind::Secret | Kind::Key | Kind::Trigger => self.nav.inside(mins, maxs),
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
        if g.defers > 3 {
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
        let b = build_nav(&l.vm, &l.world, &self.o, &self.cm, self.start, &self.forbid, &exits);
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
        let Some(src) = self.nav.nearest_clear(&self.geo, here, 96.0).or_else(below) else {
            // Off the graph (after a noclip or a fall): back to the nearest node.
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
            if let Some(n) = self.nav.nearest(here, 2048.0).or_else(|| self.nav.nearest_any(here)) {
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
        self.update_views();
        for _ in 0..2 {
            let (dist, prev) = self.nav.dijkstra(src, &self.blocked);
            let mut best: Option<(f32, usize, u32)> = None;
            for (i, g) in self.goals.iter().enumerate() {
                if !Self::pending(g) || g.deferred || (g.kind == Kind::Exit && others_left) || self.dormant(g) {
                    continue;
                }
                let n = self.goal_node(g).into_iter().chain(g.views.iter().copied()).filter(|&n| dist[n as usize].is_finite()).min_by(|&a, &b| dist[a as usize].total_cmp(&dist[b as usize]));
                if let Some(n) = n {
                    // Prefer finishing the current goal unless something is much closer.
                    let bias = if Some(i) == self.cur { 0.7 } else { 1.0 };
                    let d = dist[n as usize] * bias;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, i, n));
                    }
                }
            }
            if let Some((_, i, n)) = best {
                choice = Some((i, path_to(&prev, n)));
                break;
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
        if let Some((i, path)) = choice {
            if self.cur != Some(i) {
                self.returns = 0;
            }
            self.cur = Some(i);
            self.path = path;
            self.step = 0;
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
            let pick = |deferred: bool| {
                self.goals
                    .iter()
                    .position(|g| Self::pending(g) && !self.dormant(g) && g.deferred == deferred && (g.kind != Kind::Exit || !others_left))
            };
            match pick(false).or_else(|| pick(true)) {
                Some(i) => {
                    if self.goals[i].noclips >= 3 {
                        self.fail(i, "given up after 3 noclips");
                        return true;
                    }
                    self.cur = Some(i);
                    self.path.clear();
                    let g = &self.goals[i];
                    let to = match (g.kind, self.goal_node(g)) {
                        (Kind::Monster, _) => self.monster_spot(g.ent),
                        (_, Some(n)) => self.nav.pos[n as usize],
                        (_, None) => self.center(g.ent),
                    };
                    self.goals[i].deferred = false;
                    self.noclip(to, "unreachable");
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

    /// A nav node with a view of the monster, else a spot 72 units from it.
    fn monster_spot(&self, e: u32) -> Vec3 {
        let c = self.center(e);
        let r = Vec3::splat(384.0);
        let mut ns = self.nav.around(c - r, c + r, 0.0, c.z, 384.0);
        ns.sort_by(|&a, &b| (self.nav.pos[a as usize] - c).length_sq().total_cmp(&(self.nav.pos[b as usize] - c).length_sq()));
        for &n in ns.iter().take(64) {
            let p = self.nav.pos[n as usize];
            if self.visible(p + Vec3::new(0.0, 0.0, 22.0), c) {
                return p;
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
            if a.x < p.x + 64.0 && b.x > p.x - 64.0 && a.y < p.y + 64.0 && b.y > p.y - 64.0 && a.z < p.z + 64.0 && b.z > p.z - 64.0 {
                out += &format!(" [#{e} {}]", vm.classname(e));
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
        if let Some(i) = self.cur {
            self.goals[i].noclips += 1;
            if self.goals[i].noclips > 3 {
                self.fail(i, "given up after 3 noclips");
                return;
            }
        }
        self.rep.noclip_uses += 1;
        if self.opt.verbose {
            let g = self.cur.map(|i| (self.goals[i].kind, self.goals[i].ent, self.vm().classname(self.goals[i].ent)));
            eprintln!("  t={t:.1} NOCLIP ({why}) {:?} -> {to:?} (goal {g:?})", self.me_origin());
        }
        let to = self.free_spot(to);
        self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
        self.noclip_at = t;
        self.best_dist = f32::INFINITY;
        self.best_time = t;
        self.ride_since = None;
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
                if !self.goals[i].done && self.goal_done(&self.goals[i]) {
                    self.goals[i].done = true;
                    self.progress = true;
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
            self.sv.set_move(self.c, mv);
            self.sv.frame(DT);
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
            if t - self.goal_start > GOAL_TIMEOUT {
                self.goal_start = t;
                self.defer(i, "timed out");
            } else if self.path.is_empty() && g.kind != Kind::Monster && g.kind != Kind::Exit && t - self.noclip_at > 4.0 && t - self.noclip_at < 5.0 {
                // Noclipped onto a goal that still isn't done: it won't be.
                self.fail(i, "not done after noclip");
            }
        }
        let monster_goal = self.cur.is_some_and(|i| self.goals[i].kind == Kind::Monster);
        if !in_intermission
            && !settling
            && (self.cur.is_none() || t - self.last_replan > if monster_goal { 1.0 } else { 3.0 })
            && (self.ride_since.is_none() || self.cur.is_none())
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

    /// Steering on a lift edge; `None` when not on one.
    fn ride(&mut self, here: Vec3, t: f64) -> Option<Vec3> {
        if self.step == 0 || self.step >= self.path.len() {
            self.ride_since = None;
            return None;
        }
        let key = (self.path[self.step - 1], self.path[self.step]);
        let Some(&lift) = self.lifts.get(&key) else {
            self.ride_since = None;
            return None;
        };
        let since = match self.ride_since {
            Some((s, since)) if s == self.step => since,
            _ => {
                self.ride_since = Some((self.step, t));
                t
            }
        };
        // Progress is measured by the lift arriving, not by distance.
        self.best_time = t;
        if t - since > RIDE_TIME {
            if self.opt.verbose {
                eprintln!("  t={t:.1} lift #{} never came", lift.ent);
            }
            self.block_here();
            self.ride_since = None;
            self.last_replan = -100.0;
            self.path.clear();
            return Some(Vec3::ZERO);
        }
        let surf = self.vm().ent_v(lift.ent, self.o.absmax)[2];
        let c = (lift.mins + lift.maxs) * 0.5;
        let inside = here.x > lift.mins.x - 16.0 && here.x < lift.maxs.x + 16.0 && here.y > lift.mins.y - 16.0 && here.y < lift.maxs.y + 16.0;
        let on = inside && (here.z - 24.0 - surf).abs() < 6.0;
        let up = self.nav.pos[key.1 as usize].z > self.nav.pos[key.0 as usize].z;
        let to_center = Vec3::new(c.x - here.x, c.y - here.y, 0.0);
        let at = |z: f32| (surf - z).abs() < 4.0;
        // Going up: board it at the bottom. Going down: drop onto it when it
        // is at the bottom; ride it down only if it isn't a plat (a plat stays
        // up while someone stands on it).
        let board = if up { at(lift.lo) } else { at(lift.lo) || (at(lift.hi) && !lift.plat) };
        if on || board {
            Some(if to_center.length() > 8.0 { to_center } else { Vec3::ZERO })
        } else if inside && surf > here.z {
            // Under it while it is away: step out so it can come down.
            Some(-to_center)
        } else {
            Some(Vec3::ZERO)
        }
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

        // Skip ahead when a later path node is already reached (teleporters,
        // lifts and drops move the bot past nodes).
        let close = |p: Vec3| {
            let d = p - here;
            (d.x * d.x + d.y * d.y) < REACH * REACH && d.z.abs() < 30.0
        };
        for k in (self.step + 1..self.path.len().min(self.step + 6)).rev() {
            if close(self.nav.pos[self.path[k] as usize]) {
                self.step = k;
                break;
            }
        }

        // Steer along the path.
        let mut wish = Vec3::ZERO;
        let mut jump = false;
        let riding = self.ride(here, t);
        if let Some(w) = riding {
            wish = w;
            if self.path.is_empty() {
                return mv;
            }
            if close(self.nav.pos[self.path[self.step] as usize]) && self.step + 1 < self.path.len() {
                self.step += 1;
                self.ride_since = None;
            }
        } else {
            while self.step < self.path.len() {
                let p = self.nav.pos[self.path[self.step] as usize];
                let d = p - here;
                if close(p) && self.step + 1 < self.path.len() {
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
                break;
            }
        }
        // Final approach: walk into the goal itself (buttons sit in walls).
        if let Some(i) = self.cur
            && self.step + 1 >= self.path.len()
            && riding.is_none()
        {
            let g = &self.goals[i];
            if matches!(g.kind, Kind::Button | Kind::Secret | Kind::Key | Kind::Exit | Kind::Trigger) {
                let c = self.center(g.ent);
                let d = c - here;
                if (d.x * d.x + d.y * d.y) > 4.0 {
                    wish = Vec3::new(d.x, d.y, 0.0);
                }
                if g.kind == Kind::Secret && d.z > 40.0 {
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
            } else if t - self.best_time > STUCK_TIME * 0.5 && t - self.best_time <= STUCK_TIME {
                unstick = true;
            } else if t - self.best_time > STUCK_TIME {
                if self.opt.verbose {
                    eprintln!("  t={t:.1} stuck at {here:?}{}", self.ents_near(here));
                }
                if self.step > 0 && self.step < self.path.len() {
                    self.block_here();
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
        // Aim at where the cached target is now.
        let target = self.target_cache.map(|(_, e)| (self.center(e), e));
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
            let speed = if riding.is_some() { (wish.length() * 8.0).min(320.0) } else { 320.0 };
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
