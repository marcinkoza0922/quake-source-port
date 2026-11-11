//! One map run: spawn a headless server, build the nav graph, then chase goals
//! (secrets, monsters, buttons, keys) with usercmds until none are left.

use crate::nav::{EdgeKind, Forbid, Nav, path_to};
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::Vec3;
use qserver::{ClientMove, Server};
use qvm::Vm;
use qworld::hull::CollisionModel;
use qworld::{CONTENTS_WATER, FL_MONSTER, MOVETYPE_NOCLIP, SOLID_NOT, SOLID_TRIGGER};
use std::sync::Arc;
use std::time::Instant;

const DT: f64 = 1.0 / 72.0;
const GOAL_TIMEOUT: f64 = 30.0;
const STUCK_TIME: f64 = 4.0;

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
    pub noclip_uses: u32,
    pub goals_failed: u32,
    pub nav_nodes: usize,
    pub sim_time: f64,
    pub wall_time: f64,
    pub exited: bool,
    /// The bot couldn't get back onto the nav graph and gave up early.
    pub stranded: bool,
    pub errors: Vec<String>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Secret,
    Monster,
    Button,
    Key,
}

struct Goal {
    ent: u32,
    kind: Kind,
    done: bool,
    failed: bool,
    initial_state: f32,
    noclips: u32,
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
        })
    }
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

struct Bot<'a> {
    sv: Server,
    c: usize,
    me: u32,
    o: Ofs,
    cm: Arc<CollisionModel>,
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
    noclip_to: Option<Vec3>,
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
}

pub fn run(fs: Fs, map: &str, opt: &Options) -> Report {
    let wall = Instant::now();
    let mut rep = Report::default();
    let mut sv = Server::new(fs, Cvars::new(), 1);
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
    // Make level exits inert so a noclip flight can't end the run early.
    for e in 1..l.vm.num_edicts() as u32 {
        if !l.vm.is_free(e) && l.vm.classname(e) == "trigger_changelevel" {
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
            // Exits, and teleporters (which would bounce the bot away from a
            // node inside them; their destinations get added by `extend`).
            "trigger_changelevel" | "trigger_teleport" => {
                forbid.push(fb());
                continue;
            }
            "trigger_secret" => Kind::Secret,
            "func_button" => Kind::Button,
            c if c.starts_with("item_") && c.contains("key") => Kind::Key,
            _ if l.vm.ent_f(e, o.flags) as i32 & FL_MONSTER != 0 => Kind::Monster,
            _ => continue,
        };
        let initial_state = o.state.map_or(0.0, |s| l.vm.ent_f(e, s));
        goals.push(Goal { ent: e, kind, done: false, failed: false, initial_state, noclips: 0 });
    }
    let nav = Nav::build(&cm, start, forbid);
    rep.nav_nodes = nav.pos.len();
    if opt.verbose {
        eprintln!("{map}: {} nav nodes, {} goals", nav.pos.len(), goals.len());
    }
    let mut bot = Bot {
        sv,
        c,
        me,
        o,
        cm,
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
        noclip_to: None,
        last_replan: -10.0,
        yaw: 0.0,
        impulses: Vec::new(),
        returns: 0,
        stranded: false,
        weapon: 5,
        max_time: 0.0,
    };
    // Big maps get more time: 900 s, or 15 s per goal.
    bot.max_time = opt.max_time.unwrap_or_else(|| (bot.goals.len() as f64 * 15.0).max(900.0));
    bot.main_loop(map);
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
            return true;
        }
        match g.kind {
            Kind::Monster => vm.ent_f(e, self.o.health) <= 0.0 || vm.ent_f(e, self.o.flags) as i32 & FL_MONSTER == 0,
            Kind::Secret => vm.ent_i(e, self.o.touch) == 0 || vm.ent_f(e, self.o.solid) as i32 != SOLID_TRIGGER,
            Kind::Key => vm.ent_f(e, self.o.solid) as i32 == SOLID_NOT,
            Kind::Button => self.o.state.is_some_and(|s| vm.ent_f(e, s) != g.initial_state),
        }
    }

    fn visible(&self, from: Vec3, to: Vec3) -> bool {
        self.cm.hull(0, 0).trace(from, to).fraction >= 1.0
    }

    fn eye(&self) -> Vec3 {
        self.me_origin() + v(self.vm().ent_v(self.me, self.o.view_ofs))
    }

    /// Nearest visible live monster or shootable button within range.
    fn target(&self) -> Option<(Vec3, u32)> {
        let eye = self.eye();
        let vm = self.vm();
        let mut best: Option<(f32, Vec3, u32)> = None;
        for g in &self.goals {
            if g.done {
                continue;
            }
            let shoot = match g.kind {
                Kind::Monster => true,
                Kind::Button => vm.ent_f(g.ent, self.o.health) > 0.0 && vm.ent_f(g.ent, self.o.takedamage) != 0.0,
                _ => false,
            };
            if !shoot {
                continue;
            }
            let c = self.center(g.ent);
            let d = (c - eye).length();
            if d < 2000.0 && best.is_none_or(|b| d < b.0) && self.visible(eye, c) {
                best = Some((d, c, g.ent));
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// Node to reach for a goal.
    fn goal_node(&self, g: &Goal) -> Option<u32> {
        let vm = self.vm();
        let (mins, maxs) = (v(vm.ent_v(g.ent, self.o.absmin)), v(vm.ent_v(g.ent, self.o.absmax)));
        match g.kind {
            Kind::Secret | Kind::Key => self.nav.inside(mins, maxs),
            Kind::Monster => self.nav.nearest(v(vm.ent_v(g.ent, self.o.origin)), 256.0),
            Kind::Button => self.nav.inside(mins - Vec3::splat(24.0), maxs + Vec3::splat(24.0)),
        }
    }

    fn set_noclip(&mut self, on: bool) {
        self.sv.command(if on { "noclip 1" } else { "noclip 0" });
    }

    /// Pick the cheapest reachable goal; fall back to noclip for unreachable ones.
    fn replan(&mut self) {
        self.last_replan = self.sv.time();
        let here = self.me_origin();
        if self.nav.nearest(here, 96.0).is_none() && self.nav.extend(&self.cm, here) && self.opt.verbose {
            eprintln!("  t={:.1} nav extended from {here:?}: {} nodes", self.sv.time(), self.nav.pos.len());
        }
        // Standing on a brush entity (lift, platform) above a known floor:
        // path from the floor node below.
        let below = || {
            let tr = self.cm.hull(0, 1).trace(here, here - Vec3::new(0.0, 0.0, 1024.0));
            (!tr.startsolid && tr.fraction < 1.0).then_some(tr.endpos).and_then(|g| self.nav.nearest(g, 48.0))
        };
        let Some(src) = self.nav.nearest(here, 96.0).or_else(below) else {
            // Off the graph (after a noclip): fly back to the nearest node.
            self.path.clear();
            self.returns += 1;
            if self.returns > 8 {
                if self.opt.verbose {
                    eprintln!("  stranded at {here:?}");
                }
                self.rep.stranded = true;
                self.stranded = true;
                return;
            }
            if let Some(n) = self.nav.nearest(here, 2048.0).or_else(|| self.nav.nearest_any(here)) {
                let to = self.nav.pos[n as usize];
                self.start_noclip(to);
            } else if let Some(i) = self.cur {
                self.goals[i].failed = true;
                self.rep.goals_failed += 1;
                self.cur = None;
            }
            return;
        };
        let (dist, prev) = self.nav.dijkstra(src);
        let mut best: Option<(f32, usize, u32)> = None;
        let mut unreachable = None;
        for (i, g) in self.goals.iter().enumerate() {
            if g.done || g.failed {
                continue;
            }
            match self.goal_node(g) {
                Some(n) if dist[n as usize].is_finite() => {
                    // Prefer finishing the current goal unless something is much closer.
                    let bias = if Some(i) == self.cur { 0.7 } else { 1.0 };
                    let d = dist[n as usize] * bias;
                    if best.is_none_or(|b| d < b.0) {
                        best = Some((d, i, n));
                    }
                }
                _ => {
                    if unreachable.is_none() {
                        unreachable = Some(i);
                    }
                }
            }
        }
        let prev_goal = self.cur;
        if let Some((_, i, n)) = best {
            if self.cur != Some(i) {
                self.returns = 0;
            }
            self.cur = Some(i);
            self.path = path_to(&prev, n);
            self.step = 0;
        } else if let Some(i) = unreachable {
            self.cur = Some(i);
            self.path.clear();
            let g = &self.goals[i];
            let to = match self.goal_node(g) {
                Some(n) => self.nav.pos[n as usize],
                None => self.center(g.ent),
            };
            self.start_noclip(to);
        } else {
            self.cur = None;
        }
        if self.cur != prev_goal {
            self.goal_start = self.sv.time();
            self.best_dist = f32::INFINITY;
            self.best_time = self.sv.time();
            if self.opt.verbose
                && let Some(i) = self.cur
            {
                let g = &self.goals[i];
                eprintln!("  t={:.1} goal {:?} #{} ({})", self.sv.time(), g.kind, g.ent, self.vm().classname(g.ent));
            }
        }
    }

    fn start_noclip(&mut self, to: Vec3) {
        if self.noclip_to.is_none() {
            if let Some(i) = self.cur {
                self.goals[i].noclips += 1;
                if self.goals[i].noclips > 3 {
                    self.goals[i].failed = true;
                    self.rep.goals_failed += 1;
                    self.cur = None;
                    if self.opt.verbose {
                        eprintln!("  goal #{} given up after 3 noclips", self.goals[i].ent);
                    }
                }
            }
            self.rep.noclip_uses += 1;
            if self.opt.verbose {
                eprintln!("  t={:.1} noclip {:?} -> {to:?} (goal {:?})", self.sv.time(), self.me_origin(), self.cur.map(|i| self.goals[i].ent));
            }
            self.set_noclip(true);
        }
        self.noclip_to = Some(to);
    }

    fn goal_point(&self) -> Option<Vec3> {
        let g = &self.goals[self.cur?];
        match self.path.last() {
            Some(&n) => Some(self.nav.pos[n as usize]),
            None => Some(self.center(g.ent)),
        }
    }

    fn main_loop(&mut self, map: &str) {
        let mut last_refill = -100.0;
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
            if t - last_refill > 20.0 {
                // All weapons and ammo, then the super nailgun (no splash).
                self.impulses = vec![self.weapon, 9];
                last_refill = t;
            }
            for i in 0..self.goals.len() {
                if !self.goals[i].done && self.goal_done(&self.goals[i]) {
                    self.goals[i].done = true;
                    if self.cur == Some(i) {
                        self.cur = None;
                        if self.noclip_to.take().is_some() {
                            self.set_noclip(false);
                        }
                    }
                }
            }
            if let Some(i) = self.cur
                && t - self.goal_start > GOAL_TIMEOUT
            {
                self.goals[i].failed = true;
                self.rep.goals_failed += 1;
                self.cur = None;
                if self.opt.verbose {
                    eprintln!("  t={t:.1} goal #{} timed out", self.goals[i].ent);
                }
            }
            let monster_goal = self.cur.is_some_and(|i| self.goals[i].kind == Kind::Monster);
            if self.noclip_to.is_none() && (self.cur.is_none() || t - self.last_replan > if monster_goal { 1.0 } else { 3.0 }) {
                self.replan();
                if self.stranded || (self.cur.is_none() && self.noclip_to.is_none()) {
                    break;
                }
            }
            let mut mv = self.think();
            if let Some(i) = self.impulses.pop() {
                mv.impulse = i;
            }
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

    fn think(&mut self) -> ClientMove {
        let t = self.sv.time();
        let here = self.me_origin();
        let mut mv = ClientMove::default();

        if let Some(to) = self.noclip_to {
            let d = to - here;
            mv.angles = look_angles(d);
            mv.forward = if d.length() > 64.0 { 400.0 } else { 150.0 };
            if d.length() < 12.0 || t - self.best_time > 20.0 {
                self.noclip_to = None;
                self.set_noclip(false);
                if d.length() < 12.0 {
                    // Land exactly on the target with no leftover velocity.
                    self.sv.command(&format!("setpos {} {} {}", to.x, to.y, to.z));
                }
                self.best_dist = f32::INFINITY;
                self.best_time = t;
                // A noclipped secret or button goal that still isn't done won't be after this.
                if let Some(i) = self.cur
                    && self.path.is_empty()
                    && self.goals[i].kind != Kind::Monster
                {
                    self.goals[i].failed = true;
                    self.rep.goals_failed += 1;
                    self.cur = None;
                }
            }
            if d.length() < self.best_dist - 8.0 {
                self.best_dist = d.length();
                self.best_time = t;
            }
            return mv;
        }
        if self.vm().ent_f(self.me, self.o.movetype) as i32 == MOVETYPE_NOCLIP {
            self.set_noclip(false);
        }

        // Steer along the path.
        let mut wish = Vec3::ZERO;
        let mut jump = false;
        while self.step < self.path.len() {
            let p = self.nav.pos[self.path[self.step] as usize];
            let d = p - here;
            if (d.x * d.x + d.y * d.y) < 14.0 * 14.0 && d.z.abs() < 30.0 && self.step + 1 < self.path.len() {
                self.step += 1;
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
        // Final approach: walk into the goal itself (buttons sit in walls).
        if let Some(i) = self.cur
            && self.step + 1 >= self.path.len()
        {
            let g = &self.goals[i];
            if matches!(g.kind, Kind::Button | Kind::Secret | Kind::Key) {
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
        // ledge the nav graph doesn't reach): noclip to a spot near it.
        if let Some(i) = self.cur
            && self.goals[i].kind == Kind::Monster
            && self.step + 1 >= self.path.len()
            && wish.length() < 24.0
        {
            let c = self.center(self.goals[i].ent);
            if !self.visible(self.eye(), c) {
                let back = Vec3::new(here.x - c.x, here.y - c.y, 0.0);
                let to = if back.length() > 1.0 { c + back.normalize() * 72.0 } else { c };
                if self.opt.verbose {
                    eprintln!("  t={t:.1} monster #{} out of sight, noclip to {to:?}", self.goals[i].ent);
                }
                self.start_noclip(to);
                return mv;
            }
        }

        // Progress towards the goal; stuck → jump and strafe, then noclip.
        let mut unstick = false;
        if let Some(gp) = self.goal_point() {
            let d = (gp - here).length();
            if d < self.best_dist - 16.0 || d < 32.0 {
                self.best_dist = self.best_dist.min(d);
                self.best_time = t;
            } else if t - self.best_time > STUCK_TIME * 0.5 && t - self.best_time <= STUCK_TIME {
                unstick = true;
            } else if t - self.best_time > STUCK_TIME {
                if self.opt.verbose {
                    eprintln!("  t={t:.1} stuck at {here:?}, noclip to {gp:?}");
                }
                self.start_noclip(gp);
                return mv;
            }
        }

        let target = self.target();
        if let Some((tg, e)) = target {
            mv.angles = look_angles(tg - self.eye());
            mv.buttons |= 1;
            // Zombies only die when gibbed: rockets. Otherwise the super
            // nailgun, which has no splash to push the bot around.
            let w = if self.vm().classname(e).contains("zombie") { 7 } else { 5 };
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
            // Wish direction relative to the view yaw.
            let w = wish.normalize() * 320.0;
            let (s, c) = mv.angles[1].to_radians().sin_cos();
            mv.forward = w.x * c + w.y * s;
            mv.side = w.x * s - w.y * c;
        }
        if unstick {
            mv.buttons |= 2;
            mv.side = if (t * 2.0) as i64 % 2 == 0 { 320.0 } else { -320.0 };
        }
        if jump {
            mv.buttons |= 2;
        }
        let waterlevel = self.vm().ent_f(self.me, self.o.waterlevel);
        if waterlevel >= 2.0 || self.cm.point_contents(here) == CONTENTS_WATER {
            let next = self.path.get(self.step).map(|&n| self.nav.pos[n as usize]);
            if next.is_none_or(|p| p.z > here.z - 8.0) {
                mv.up = 300.0;
                mv.buttons |= 2;
            }
        }
        mv
    }
}
