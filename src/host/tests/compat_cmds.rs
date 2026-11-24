//! Tests for standard Quake/ironwail console commands: `help`, `sizeup`/`sizedown`,
//! `togglezoom`/`+zoom`/`-zoom`, `messagemode`/`messagemode2`, and `cd`.

use qinput::keys::{K_BACKSPACE, K_ENTER, K_ESCAPE, KeyDest};

use super::super::*;

fn host() -> Option<Host> {
    let fs = qcore::fs::Fs::new(std::path::Path::new("."));
    let pal = [[0u8; 3]; 256];
    let r = Renderer::headless(640, 480, &pal).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    Some(h)
}

#[test]
fn help_opens_help_screen() {
    let Some(mut h) = host() else { return };
    assert_eq!(h.menu.screen, Screen::None);
    h.exec_line("help");
    assert_eq!(h.menu.screen, Screen::Help);
}

#[test]
fn sizeup_and_sizedown_clamp_and_hide_hud_reserve() {
    let Some(mut h) = host() else { return };
    h.exec_line("hud_layout classic");
    h.exec_line("viewsize 100");
    for _ in 0..3 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(h.hud_reserve() > 0.0);

    h.exec_line("sizeup");
    assert_eq!(h.cv("viewsize"), 110.0);
    assert!(h.hud_reserve() > 0.0);

    h.exec_line("sizeup");
    assert_eq!(h.cv("viewsize"), 120.0);
    assert_eq!(h.hud_reserve(), 0.0);

    h.exec_line("sizeup");
    assert_eq!(h.cv("viewsize"), 130.0);
    h.exec_line("sizeup");
    assert_eq!(h.cv("viewsize"), 130.0);

    for _ in 0..12 {
        h.exec_line("sizedown");
    }
    assert_eq!(h.cv("viewsize"), 30.0);
}

#[test]
fn togglezoom_and_hold_zoom_interpolate_fov() {
    let Some(mut h) = host() else { return };
    h.exec_line("fov 90");
    h.exec_line("zoom_fov 30");
    h.exec_line("zoom_speed 8");
    assert_eq!(h.effective_fov(), 90.0);

    h.exec_line("togglezoom");
    h.update_zoom(0.0625); // halfway: smoothstep(0.5) = 0.5 -> 60 deg
    assert!((h.effective_fov() - 60.0).abs() < 0.01);

    h.update_zoom(0.2);
    assert!((h.effective_fov() - 30.0).abs() < 0.01);

    h.exec_line("togglezoom");
    h.update_zoom(0.2);
    assert!((h.effective_fov() - 90.0).abs() < 0.01);

    h.exec_line("+zoom");
    h.update_zoom(0.2);
    assert!((h.effective_fov() - 30.0).abs() < 0.01);
    h.exec_line("-zoom");
    h.update_zoom(0.2);
    assert!((h.effective_fov() - 90.0).abs() < 0.01);
}

#[test]
fn messagemode_routes_input_and_emits_say_commands() {
    let Some(mut h) = host() else { return };
    // Disconnected: messagemode is ignored.
    h.exec_line("messagemode");
    assert!(!h.con.chat_active);

    // Simulate connected in-game state.
    h.cl.signon = 4;
    h.con.forced = false;
    h.con.open = false;

    h.exec_line("messagemode");
    assert!(h.con.chat_active);
    assert!(!h.con.chat_team);
    assert_eq!(h.key_dest(), KeyDest::Message);

    for ch in "hellox".chars() {
        h.con.chat_text(ch);
    }
    assert_eq!(h.con.chat_key(K_BACKSPACE), None);
    assert_eq!(h.con.chat_key(K_ENTER).as_deref(), Some("say \"hello\"\n"));
    assert!(!h.con.chat_active);
    assert_eq!(h.key_dest(), KeyDest::Game);

    h.exec_line("messagemode2");
    assert!(h.con.chat_active && h.con.chat_team);
    h.con.chat_text('g');
    h.con.chat_text('o');
    assert_eq!(h.con.chat_key(K_ENTER).as_deref(), Some("say_team \"go\"\n"));

    h.exec_line("messagemode");
    h.con.chat_text('a');
    assert_eq!(h.con.chat_key(K_ESCAPE), None);
    assert!(!h.con.chat_active);
}

#[test]
fn cd_subcommands_manage_track_state_and_remap() {
    let Some(mut h) = host() else { return };
    assert!(h.cd.enabled);

    h.exec_line("cd remap 5 7");
    assert_eq!(h.cd.map(1), 5);
    assert_eq!(h.cd.map(2), 7);

    h.exec_line("cd loop 2");
    assert_eq!(h.cd.playing, Some((7, true)));
    assert!(!h.cd.paused);

    h.exec_line("cd pause");
    assert!(h.cd.paused);
    h.exec_line("cd resume");
    assert!(!h.cd.paused);

    h.exec_line("cd next");
    assert_eq!(h.cd.playing, Some((8, true)));
    h.exec_line("cd prev");
    assert_eq!(h.cd.playing, Some((7, true)));

    h.exec_line("cd off");
    assert!(!h.cd.enabled);
    assert_eq!(h.cd.playing, None);
    h.exec_line("cd play 3");
    assert_eq!(h.cd.playing, None);

    h.exec_line("cd reset");
    assert!(h.cd.enabled);
    assert_eq!(h.cd.map(1), 1);
    h.exec_line("cd play 4");
    assert_eq!(h.cd.playing, Some((4, false)));

    h.exec_line("cd stop");
    assert_eq!(h.cd.playing, None);
}

#[test]
fn a_dedicated_server_needs_no_renderer() {
    let fs = qcore::fs::Fs::new(std::path::Path::new("."));
    let mut h = Host::new(fs, Renderer::none(), None, Vec::new());
    h.headless = true;
    h.make_dedicated();
    for line in ["sky foo", "r_visibility list", "vid_restart", "menu_main", "r_shader_preset x", "skies"] {
        h.exec_line(line);
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(!h.r.has_world());
}

#[test]
fn host_error_is_reported_once_and_disconnects() {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") || !fs.exists("maps/e1m1.bsp") {
        return;
    }
    let mut h = Host::new(fs, Renderer::none(), None, Vec::new());
    h.headless = true;
    h.exec_line("sv_autosave 0");
    h.exec_line("map e1m1");
    for _ in 0..8 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(h.server.active());
    h.server.core.host_error(qvm::VmError::new("boom"));
    for _ in 0..8 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(!h.server.active() && h.server.level.is_none() && h.local.is_none());
    let shown = h.con.lines.iter().filter(|l| l.windows(4).any(|w| w == b"boom")).count();
    assert_eq!(shown, 1);
    h.exec_line("map e1m1");
    for _ in 0..8 {
        h.frame(1.0 / 72.0, Vec::new());
    }
    assert!(h.server.active(), "the next map starts");
}
