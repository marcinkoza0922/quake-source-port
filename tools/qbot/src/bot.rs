//! One map run: spawn a headless server, give the player god mode and noclip,
//! then work through every goal the edicts reveal (monsters, secrets,
//! triggers, buttons, shootable brushes, keys, runes) and finally touch the
//! exit. The bot does not try to play like a person: it knows where
//! everything is and flies straight through walls. Its job is to set off
//! every scripted event on the level and see whether the level (or the
//! engine) breaks.

use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::Vec3;
use qserver::{ClientMove, Server};
use qvm::Vm;
use qworld::hull::CollisionModel;
use qworld::{FL_MONSTER, MOVETYPE_NOCLIP, SOLID_BBOX, SOLID_BSP, SOLID_NOT, SOLID_TRIGGER};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Instant;
mod attempt;
pub mod devlog;
mod fly;
mod goals;
pub mod trace;

/// Server tick: 20 Hz like a dedicated server (sys_ticrate 0.05).
const DT: f64 = 1.0 / 20.0;
/// Noclip flight speed (sv_maxspeed is raised to this for the bot).
const SPEED: f32 = 1000.0;
/// Sim seconds a single goal may take before it counts as failed.
const GOAL_TIMEOUT: f64 = 20.0;
const IT_KEY1: i32 = 131072;
const IT_KEY2: i32 = 262144;

pub struct Options {
    pub skill: i32,
    /// Sim time limit; `None` scales with the number of goals.
    pub max_time: Option<f64>,
    pub verbose: bool,
}

#[derive(Default)]
pub struct Report {
    pub kills: i32,
    pub total_monsters: i32,
    pub secrets: i32,
    pub total_secrets: i32,
    /// Trigger volumes entered (trigger_once/multiple/teleport/push/...).
    pub triggers: u32,
    pub total_triggers: u32,
    /// Buttons pressed or shot, shootable brushes broken.
    pub buttons: u32,
    pub total_buttons: u32,
    /// Keys and runes picked up.
    pub pickups: u32,
    pub total_pickups: u32,
    /// Goals given up on, as "kind classname #ent at (x y z): why".
    pub failed: Vec<String>,
    /// The level has a trigger_changelevel.
    pub has_exit: bool,
    /// Left the level through its exit (or reached its intermission).
    pub exited: bool,
    pub sim_time: f64,
    pub wall_time: f64,
    pub errors: Vec<String>,
    /// Developer-log lines a level doesn't normally print (see devlog.rs).
    pub unusual: Vec<String>,
    /// The whole run, console and bot events (see trace.rs).
    pub trace: Vec<String>,
}

impl Report {
    /// Scripted events set off: what the baseline compares besides kills,
    /// secrets and the exit.
    pub fn events(&self) -> u32 {
        self.triggers + self.buttons + self.pickups
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Monster,
    Secret,
    /// Any other touchable trigger volume.
    Trigger,
    /// func_button pressed by touch (or shot when it has health).
    Button,
    /// A brush or box that opens or breaks when shot.
    Shoot,
    /// Keys and runes.
    Pickup,
    Exit,
}

struct Goal {
    ent: u32,
    kind: Kind,
    class: String,
    done: bool,
    /// Why the last attempt failed; failed goals get one more try once
    /// everything else is done (the level may have opened up by then).
    failed: Option<String>,
    tries: u32,
    /// Pickups: placed in the world (became a trigger) at least once.
    armed: bool,
    /// Buttons: `state` at spawn; pressed once it changes.
    initial_state: f32,
    /// Exits: the touch function, cleared until the exit becomes the goal.
    touch: i32,
    /// Buttons: pressed before and re-armed to press again (see
    /// `repress`); counts as done in the report whatever happens next.
    again: bool,
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
    view_ofs: usize,
    state: Option<usize>,
    items: Option<usize>,
    modelindex: Option<usize>,
    spawnfunction: Option<usize>,
    target: Option<usize>,
    targetname: Option<usize>,
    attack_finished: Option<usize>,
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
            view_ofs: f("view_ofs")?,
            state: f("state"),
            items: f("items"),
            modelindex: f("modelindex"),
            spawnfunction: f("spawnfunction"),
            target: f("target"),
            targetname: f("targetname"),
            attack_finished: f("attack_finished"),
        })
    }
}

fn v(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

struct Bot<'a> {
    sv: Server,
    c: usize,
    me: u32,
    o: Ofs,
    cm: Arc<CollisionModel>,
    opt: &'a Options,
    rep: Report,
    goals: Vec<Goal>,
    /// Edicts already turned into goals (freed ones drop out, so a reused
    /// slot is picked up again).
    known: HashSet<u32>,
    cur: Option<usize>,
    goal_start: f64,
    /// Where to stand for the current goal, and when it was worked out.
    spot: Option<Vec3>,
    spot_at: f64,
    /// Shooting: the part of the target aimed at, from its centre.
    aim_point: Vec3,
    /// Shooting: `firing_spot` found a spot with a clear shot.
    found_spot: bool,
    /// Walking into a button (noclip off) until this time.
    pushing_until: f64,
    pushes: u32,
    /// Frames the bot has not moved although it tried to fly.
    pinned: u32,
    /// Last `setpos` to a goal that flight didn't reach.
    warped_at: f64,
    devlog: devlog::DevLog,
    trace: trace::Trace,
    /// What happened in the current goal's attempt so far.
    att: attempt::Attempt,
    weapon: u8,
    /// Rounds of re-pressing repeatable buttons so far.
    repressed: u32,
    impulses: Vec<u8>,
    intermission: Option<usize>,
}

pub fn run(fs: Fs, map: &str, opt: &Options) -> Report {
    let wall = Instant::now();
    let mut rep = Report::default();
    let mut sv = Server::new(fs, Cvars::new(), 1);
    // Nobody reads the client's datagrams.
    sv.core.skip_entity_updates = true;
    sv.command(&format!("skill {}", opt.skill));
    // Classic noclip: wishvel.z is the up move, so flight is easy to steer
    // independently of where the bot aims.
    sv.command("sv_altnoclip 0");
    sv.command(&format!("sv_maxspeed {SPEED}"));
    sv.command("developer 1");
    if let Err(e) = sv.spawn_server(map) {
        rep.errors.push(format!("spawn: {e}"));
        return rep;
    }
    let Some(c) = sv.connect_local() else {
        rep.errors.push("connect failed".into());
        return rep;
    };
    sv.command("god");
    let l = sv.level.as_ref().unwrap();
    let Some(o) = Ofs::new(&l.vm) else {
        rep.errors.push("progs lack standard fields".into());
        return rep;
    };
    let cm = l.world.worldmodel.clone();
    let intermission = l.vm.global_ofs("intermission_running");
    let mut bot = Bot {
        sv,
        c,
        me: c as u32 + 1,
        o,
        cm,
        opt,
        rep,
        goals: Vec::new(),
        known: HashSet::new(),
        cur: None,
        goal_start: 0.0,
        spot: None,
        spot_at: -100.0,
        aim_point: Vec3::ZERO,
        found_spot: false,
        pushing_until: -1.0,
        pushes: 0,
        pinned: 0,
        warped_at: -100.0,
        devlog: devlog::DevLog::default(),
        trace: trace::Trace::default(),
        att: attempt::Attempt::new(0.0),
        weapon: 5,
        repressed: 0,
        impulses: Vec::new(),
        intermission,
    };
    bot.drain_log();
    bot.set_noclip(true);
    bot.scan();
    bot.rep.has_exit = bot.goals.iter().any(|g| g.kind == Kind::Exit);
    bot.note(&format!("{map}: {} goals", bot.goals.len()));
    let max_time = opt.max_time.unwrap_or_else(|| (bot.goals.len() as f64 * 10.0).max(600.0));
    bot.main_loop(map, max_time);
    for (i, g) in bot.goals.iter().enumerate().filter(|(_, g)| !g.done && !g.again && g.kind != Kind::Exit) {
        let why = g.failed.as_deref().unwrap_or(if bot.dormant(g) { "never activated" } else { "out of time" });
        let line = format!("{}: {why}", bot.describe(i));
        bot.rep.failed.push(line);
    }
    bot.drain_log();
    let st = bot.sv.stats();
    let mut rep = bot.rep;
    rep.unusual = std::mem::take(&mut bot.devlog).finish(bot.sv.time());
    rep.trace = std::mem::take(&mut bot.trace).finish(bot.sv.time());
    for g in &bot.goals {
        let (done, total) = match g.kind {
            Kind::Trigger => (&mut rep.triggers, &mut rep.total_triggers),
            Kind::Button | Kind::Shoot => (&mut rep.buttons, &mut rep.total_buttons),
            Kind::Pickup => (&mut rep.pickups, &mut rep.total_pickups),
            _ => continue,
        };
        *total += 1;
        *done += (g.done || g.again) as u32;
    }
    rep.kills = st.killed_monsters;
    rep.total_monsters = st.total_monsters;
    rep.secrets = st.found_secrets;
    rep.total_secrets = st.total_secrets;
    rep.sim_time = bot.sv.time();
    rep.wall_time = wall.elapsed().as_secs_f64();
    rep
}

impl Bot<'_> {
    fn drain_log(&mut self) {
        let t = self.sv.time();
        for s in std::mem::take(&mut self.sv.core.log) {
            self.devlog.feed(&s, t);
            self.trace.con(&s, t);
        }
    }

    fn vm(&self) -> &Vm {
        &self.sv.level.as_ref().unwrap().vm
    }

    fn me_origin(&self) -> Vec3 {
        v(self.vm().ent_v(self.me, self.o.origin))
    }

    fn eye(&self) -> Vec3 {
        self.me_origin() + v(self.vm().ent_v(self.me, self.o.view_ofs))
    }

    fn bounds(&self, e: u32) -> (Vec3, Vec3) {
        let vm = self.vm();
        (v(vm.ent_v(e, self.o.absmin)), v(vm.ent_v(e, self.o.absmax)))
    }

    fn center(&self, e: u32) -> Vec3 {
        let (a, b) = self.bounds(e);
        (a + b) * 0.5
    }

    fn set_noclip(&mut self, on: bool) {
        let mt = if on { MOVETYPE_NOCLIP } else { qworld::MOVETYPE_WALK };
        let (me, ofs) = (self.me, self.o.movetype);
        self.sv.level.as_mut().unwrap().vm.set_ent_f(me, ofs, mt as f32);
    }

    fn main_loop(&mut self, map: &str, max_time: f64) {
        let mut last_refill = -100.0;
        let mut last_scan = 0.0;
        let mut intermission_since: Option<f64> = None;
        loop {
            let t = self.sv.time();
            if t > max_time {
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
            if t - last_scan > 2.0 {
                self.scan();
                last_scan = t;
            }
            self.update_goals();
            let mut mv = ClientMove::default();
            if self.intermission.is_some_and(|g| self.vm().g_f(g) != 0.0) {
                // Finales have no changelevel: the intermission is the end.
                if t - *intermission_since.get_or_insert(t) > 8.0 {
                    self.rep.exited = true;
                    break;
                }
                mv.buttons = ((t * 4.0) as i64 % 2 == 0) as u8;
            } else {
                if t - last_refill > 5.0 {
                    // All weapons and ammo, then back to the current weapon.
                    self.impulses = vec![self.weapon, 9];
                    last_refill = t;
                }
                if self.cur.is_none() {
                    self.cur = self.pick();
                    self.goal_start = t;
                    self.spot = None;
                    self.pushes = 0;
                    if let Some(i) = self.cur {
                        self.att = attempt::Attempt::new(self.vm().ent_f(self.goals[i].ent, self.o.health));
                        self.note(&format!("goal {}", self.describe(i)));
                    }
                }
                let Some(i) = self.cur else {
                    // Nothing left and no exit to take.
                    break;
                };
                if t - self.goal_start > GOAL_TIMEOUT {
                    let why = format!("timed out: {}", self.diagnose(i));
                    self.fail(i, &why);
                    self.set_noclip(true);
                    continue;
                }
                mv = self.act(i, t);
                // QC ignores impulses until the weapon is ready again: hold
                // fire while any are queued and send one once it is.
                if !self.impulses.is_empty() {
                    mv.buttons &= !1;
                    let ready = self.o.attack_finished.is_none_or(|f| t as f32 >= self.vm().ent_f(self.me, f));
                    if ready {
                        mv.impulse = self.impulses.pop().unwrap();
                    }
                }
            }
            self.step(mv);
        }
    }

    /// Escape a spot QC keeps putting the bot back to (see `step`).
    fn unpin(&mut self, at: Vec3) {
        for i in 0..self.goals.len() {
            let g = &self.goals[i];
            if g.kind == Kind::Trigger && !g.done && !self.vm().is_free(g.ent) {
                let (a, b) = self.bounds(g.ent);
                if (at.max(a).min(b) - at).length() < 64.0 {
                    self.goals[i].done = true;
                    if self.cur == Some(i) {
                        self.cur = None;
                    }
                }
            }
        }
        let to = self.cur.map_or(at + Vec3::new(0.0, 0.0, 128.0), |i| self.center(self.goals[i].ent));
        self.note(&format!("pinned at {at:?}, setpos to {to:?}"));
        self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
    }

    fn step(&mut self, mv: ClientMove) {
        // impulse 9 also hands out every key in id1: strip the keys it
        // added, so key doors stay locked until the key is picked up.
        let keys_before = (mv.impulse == 9).then(|| self.o.items.map(|it| (it, self.vm().ent_f(self.me, it) as i32))).flatten();
        let before = self.me_origin();
        self.sv.set_move(self.c, mv);
        self.sv.frame(DT);
        self.drain_log();
        if self.sv.level.is_none() {
            return;
        }
        // A teleporter moves the bot away before it is ever seen inside:
        // a jump further than flight goes counts for triggers it was at.
        let reach = SPEED * DT as f32 + 32.0;
        let moved = (self.me_origin() - before).length();
        // A teleporter whose destination lies inside it puts the bot back
        // every frame: count its triggers and jump out to the goal.
        let flying = self.vm().ent_f(self.me, self.o.movetype) as i32 == MOVETYPE_NOCLIP;
        self.pinned = if flying && moved < 1.0 && (mv.forward != 0.0 || mv.side != 0.0 || mv.up != 0.0) { self.pinned + 1 } else { 0 };
        if self.pinned > 20 {
            self.pinned = 0;
            self.unpin(before);
        }
        if moved > reach {
            for i in 0..self.goals.len() {
                let g = &self.goals[i];
                if g.kind == Kind::Trigger && !g.done && !self.vm().is_free(g.ent) {
                    let (a, b) = self.bounds(g.ent);
                    let p = before.max(a).min(b);
                    if (p - before).length() < reach {
                        self.goals[i].done = true;
                        if self.cur == Some(i) {
                            self.cur = None;
                        }
                    }
                }
            }
        }
        if let Some((it, before)) = keys_before
            && let Some(l) = self.sv.level.as_mut()
        {
            let now = l.vm.ent_f(self.me, it) as i32;
            let added = now & (IT_KEY1 | IT_KEY2) & !before;
            if added != 0 {
                l.vm.set_ent_f(self.me, it, (now & !added) as f32);
            }
        }
        let _ = self.sv.take_reliable(self.c);
        let _ = self.sv.take_datagrams(self.c);
    }
}
