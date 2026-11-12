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
use crate::actions::{Approach, ButtonTrip, LiftAction, LiftStage};
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
mod navbuild;
use navbuild::*;
mod jump;
mod lift;
mod plan;
mod spots;
mod control;
mod think;
mod state;
mod util;
use util::*;
mod run;
pub use run::*;

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
    pub assists: Vec<crate::diagnostics::Assist>,
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
    pub prerequisite_actions: u32,
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

impl Bot<'_> {
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::opener_node;

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
