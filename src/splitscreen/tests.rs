use super::*;
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qnet::protocol::STAT_HEALTH;

fn server(rules: &str) -> Option<Server> {
    let mut base = Fs::default_basedir();
    if base.is_relative() && !base.exists() {
        base = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(base);
    }
    if !base.join("id1").is_dir() {
        eprintln!("skip: no game data");
        return None;
    }
    let mut fs = Fs::new(&base);
    fs.add_game_dir("id1");
    let mut sv = Server::new(fs, Cvars::new(), 1);
    sv.command("maxplayers 6");
    sv.command(rules);
    sv.command("map e1m1");
    assert!(sv.active());
    Some(sv)
}

fn step(sv: &mut Server, guests: &mut [Guest], n: usize) {
    for _ in 0..n {
        for g in guests.iter_mut() {
            g.send(sv);
        }
        sv.frame(1.0 / 72.0);
        for g in guests.iter_mut() {
            g.feed(sv);
            g.cl.frame(1.0 / 72.0);
        }
    }
}

fn origin(g: &Guest) -> [f32; 3] {
    g.cl.entities[g.cl.viewentity].origin
}

#[test]
fn six_local_players_join_and_leave() {
    let Some(mut sv) = server("deathmatch 1") else { return };
    let mut guests = Vec::new();
    assert!(sync(&mut guests, &mut sv, 6).is_none());
    step(&mut sv, &mut guests, 20);
    assert_eq!(guests.len(), 5);
    let mut ents: Vec<usize> = guests.iter().map(|g| g.cl.viewentity).collect();
    ents.dedup();
    assert_eq!(ents.len(), 5);
    for (k, g) in guests.iter().enumerate() {
        assert_eq!(g.cl.signon, 4);
        assert_eq!(sv.core.clients[g.slot].name, format!("player{}", k + 2));
    }
    // Every guest sees all six-minus-one players on the scoreboard.
    let named = guests[0].cl.scores.iter().filter(|s| !s.name.is_empty()).count();
    assert_eq!(named, 5);

    // A seventh local player has no slot.
    assert!(sync(&mut guests, &mut sv, 7).is_none(), "clamped to 6");
    assert!(sync(&mut guests, &mut sv, 2).is_none());
    assert_eq!(guests.len(), 1);
    assert_eq!(sv.core.clients.iter().filter(|c| c.active).count(), 1);
}

#[test]
fn guests_survive_a_map_change() {
    let Some(mut sv) = server("coop 1") else { return };
    let mut guests = Vec::new();
    sync(&mut guests, &mut sv, 3);
    step(&mut sv, &mut guests, 10);
    sv.command("changelevel e1m2");
    step(&mut sv, &mut guests, 10);
    assert!(sync(&mut guests, &mut sv, 3).is_none());
    assert_eq!(guests.len(), 2);
    for g in &guests {
        assert_eq!(g.cl.signon, 4);
        assert!(g.cl.levelname.contains("Castle") || g.cl.mapname == "e1m2", "{}", g.cl.mapname);
    }
}

#[test]
fn guests_move_and_fight_independently() {
    let Some(mut sv) = server("coop 1") else { return };
    let mut guests = Vec::new();
    sync(&mut guests, &mut sv, 3);
    step(&mut sv, &mut guests, 10);
    let (a, b) = (guests[0].slot, guests[1].slot);
    // Face each other along the start corridor.
    sv.client_command(a, "setpos 480 -352 88 0 90 0");
    sv.client_command(b, "setpos 480 -200 88 0 270 0");
    step(&mut sv, &mut guests, 5);

    // Player 2 walks; player 3 stays put.
    let (a0, b0) = (origin(&guests[0]), origin(&guests[1]));
    guests[0].cl.viewangles = [0.0, 180.0, 0.0];
    for _ in 0..30 {
        guests[0].cmd = UserCmd { viewangles: [0.0, 180.0, 0.0], forwardmove: 200.0, ..Default::default() };
        step(&mut sv, &mut guests, 1);
    }
    let (a1, b1) = (origin(&guests[0]), origin(&guests[1]));
    assert!(a1[0] < a0[0] - 20.0, "{a0:?} -> {a1:?}");
    assert!((b1[0] - b0[0]).abs() < 1.0 && (b1[1] - b0[1]).abs() < 1.0, "{b0:?} -> {b1:?}");

    // Player 3 shoots player 2.
    sv.client_command(a, "setpos 480 -352 88 0 90 0");
    step(&mut sv, &mut guests, 5);
    let hp = guests[0].cl.stats[STAT_HEALTH];
    for _ in 0..80 {
        guests[1].cmd = UserCmd { viewangles: [0.0, 270.0, 0.0], buttons: 1, ..Default::default() };
        step(&mut sv, &mut guests, 1);
    }
    assert!(guests[0].cl.stats[STAT_HEALTH] < hp, "health {hp} -> {}", guests[0].cl.stats[STAT_HEALTH]);
}

#[test]
fn layouts_tile_the_screen() {
    assert_eq!(layout(1, 1920, 1080, false), vec![[0, 0, 1920, 1080]]);
    assert_eq!(layout(2, 1920, 1080, false), vec![[0, 0, 1920, 540], [0, 540, 1920, 540]]);
    assert_eq!(layout(2, 1920, 1080, true), vec![[0, 0, 960, 1080], [960, 0, 960, 1080]]);
    assert_eq!(layout(3, 1920, 1080, false).len(), 3);
    assert_eq!(layout(4, 1920, 1080, false)[3], [960, 540, 960, 540]);
    let six = layout(6, 1921, 1081, false);
    assert_eq!(six[0], [0, 0, 640, 540]);
    assert_eq!(six[5], [1280, 540, 641, 541]);
    let area: u32 = six.iter().map(|r| r[2] * r[3]).sum();
    assert_eq!(area, 1921 * 1081);
}
