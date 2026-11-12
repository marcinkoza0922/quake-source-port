use super::*;

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
