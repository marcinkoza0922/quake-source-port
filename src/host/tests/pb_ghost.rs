//! End-to-end PB ghost and HUD speedometer tests on a real headless host
//! (skips cleanly without game data or Vulkan).

use super::super::*;

fn host() -> Option<Host> {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") || fs.open("maps/e1m1.bsp").is_none() {
        return None;
    }
    let pal = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok())?;
    let r = Renderer::headless(640, 360, &pal).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    Some(h)
}

fn run(h: &mut Host, secs: f64) {
    for _ in 0..(secs * 72.0) as usize {
        h.frame(1.0 / 72.0, Vec::new());
    }
}

#[test]
fn pb_ghost_replays_translucent_entity_and_speedometer_tracks_speed() {
    let Some(mut h) = host() else { return };
    let tmp = std::env::temp_dir().join(format!("qsp-pbghost-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::userdata::set_parent(&tmp, false).unwrap();

    // Record a short run on e1m1 (skill 1 = normal) into the dedicated any% ghost path.
    let ghost_file = crate::demo::ghost::ghost_path(&h.user_dir(), "e1m1", 1, false);
    h.exec_line("cl_startdemos 0");
    h.exec_line("sv_autosave 0");
    h.exec_line("cl_autodemo 0");
    h.exec_line("skill 1");
    h.pending_record = Some((ghost_file.clone(), -1));
    h.exec_line("map e1m1");
    run(&mut h, 0.25);
    h.exec_line("+forward");
    run(&mut h, 1.5);
    // Speedometer peak-hold should now report forward run speed (> 200 ups).
    let spd = h.update_speed().unwrap_or(0.0);
    assert!(spd > 200.0, "expected run speed > 200 ups, got {spd}");
    h.exec_line("-forward");
    let mut w = h.recording.take().expect("recording");
    w.write(h.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).unwrap();
    w.finish().unwrap();
    h.stop_game();
    assert!(ghost_file.exists());

    // Restart e1m1 standing still at spawn while the ghost runs ahead.
    h.exec_line("skill 1");
    h.exec_line("cl_demo_ghost 0");
    h.exec_line("map e1m1");
    run(&mut h, 1.2);
    let view = qrender::View {
        origin: h.cl.entities.get(h.cl.viewentity).map_or([0.0; 3], |e| e.origin),
        angles: h.cl.viewangles,
        fov_x: 90.0,
        ..Default::default()
    };
    assert!(h.ghost_entity(&view, true).is_none(), "off mode must not emit ghost");

    // Switching to any% (1) mid-level immediately loads and renders the translucent ghost.
    h.exec_line("cl_demo_ghost 1");
    run(&mut h, 0.05);
    let ent = h.ghost_entity(&view, true).expect("any% PB ghost should appear once ahead of spawn");
    assert!(ent.alpha > 0.2 && ent.alpha < 0.6, "expected translucent alpha, got {}", ent.alpha);
    assert!(ent.fullbright && ent.player && ent.key == 0);

    // Overlapping camera fades ghost completely out.
    let close_view = qrender::View { origin: ent.origin, ..view };
    assert!(h.ghost_entity(&close_view, true).is_none(), "ghost should fade out when overlapping camera");

    // 100% mode (2) has no 100% PB demo yet -> no ghost.
    h.exec_line("cl_demo_ghost 2");
    run(&mut h, 0.05);
    assert!(h.ghost_entity(&view, true).is_none(), "100% mode should not play an any%-only demo");

    let _ = std::fs::remove_dir_all(&tmp);
}
