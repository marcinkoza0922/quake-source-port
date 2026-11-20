//! Tests for Quick Reset, Screenshot, Record Demo toggle, and special
//! controller gestures on the stats and menu buttons.

use super::super::*;
use qinput::{GamepadButton, keys::*};

fn host() -> Option<Host> {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") || fs.open("maps/e1m1.bsp").is_none() {
        return None;
    }
    let pal = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok())?;
    let r = Renderer::headless(640, 360, &pal).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    h.exec_line("exec default.cfg");
    h.exec_line("cl_startdemos 0");
    h.exec_line("sv_autosave 0");
    Some(h)
}

fn step(h: &mut Host, secs: f64) {
    let n = (secs * 72.0).round().max(1.0) as usize;
    let dt = secs / n as f64;
    for _ in 0..n {
        h.frame(dt, Vec::new());
    }
}

fn pad(h: &mut Host, button: GamepadButton, down: bool) {
    h.frame(1.0 / 72.0, vec![HostEvent::Pad(1, InputEvent::GamepadButton { button, down })]);
}

#[test]
fn default_bindings_for_new_controls() {
    let Some(mut h) = host() else { return };
    step(&mut h, 0.05);
    // Quick Reset is unbound by default on both keyboard and controller.
    assert!(h.input.bindings.iter().all(|b| b.as_deref() != Some("restart")));
    // Screenshot is F12 on keyboard, unbound on controller.
    assert_eq!(h.input.binding(K_F1 + 11), Some("screenshot"));
    for k in K_GAMEPAD_BEGIN..K_GAMEPAD_END {
        assert_ne!(h.input.binding(k), Some("screenshot"), "controller key {k} should not default to screenshot");
        assert_ne!(h.input.binding(k), Some("record_toggle"), "controller key {k} should not default to record_toggle");
    }
    // Toggle demo recording is F8 on keyboard, unbound on controller.
    assert_eq!(h.input.binding(K_F1 + 7), Some("record_toggle"));
}

#[test]
fn quick_reset_restarts_level_timer_and_fresh_autodemo() {
    let Some(mut h) = host() else { return };
    let tmp = std::env::temp_dir().join(format!("qsp-quickreset-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::userdata::set_parent(&tmp, false).unwrap();

    h.exec_line("cl_autodemo 1");
    h.exec_line("map e1m1");
    step(&mut h, 2.5);
    let first_time = h.cl.time;
    assert!(first_time > 3.0, "expected timer to advance, got {first_time}");
    let first_auto = h.autodemo.as_ref().expect("autodemo active").writer_path().to_path_buf();
    assert!(first_auto.exists());

    // Quick reset (`restart`) restarts level, resets timer, and starts a fresh autodemo.
    h.exec_line("restart");
    step(&mut h, 0.15);
    assert!(h.cl.time < first_time - 1.5, "expected timer reset, was {first_time}, now {}", h.cl.time);
    let second_auto = h.autodemo.as_ref().expect("fresh autodemo active").writer_path().to_path_buf();
    assert_ne!(first_auto, second_auto, "quick reset must start a fresh autodemo file");
    assert!(second_auto.exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn record_toggle_starts_and_stops_manual_demo() {
    let Some(mut h) = host() else { return };
    let tmp = std::env::temp_dir().join(format!("qsp-rectoggle-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::userdata::set_parent(&tmp, false).unwrap();

    // Disconnected: refuses cleanly.
    h.exec_line("cl_autodemo 0");
    h.exec_line("record_toggle");
    step(&mut h, 0.05);
    assert!(h.recording.is_none());

    // In-game: first toggle starts recording a map-named demo, second stops it.
    h.exec_line("map e1m1");
    step(&mut h, 0.25);
    h.exec_line("record_toggle");
    step(&mut h, 0.25);
    let rec_path = h.recording.as_ref().expect("recording should start").path().to_path_buf();
    assert!(rec_path.file_name().unwrap().to_string_lossy().starts_with("e1m1_"));
    h.exec_line("record_toggle");
    step(&mut h, 0.05);
    assert!(h.recording.is_none());
    assert!(rec_path.exists());

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn controller_stats_double_tap_takes_screenshot_without_scoreboard() {
    let Some(mut h) = host() else { return };
    let tmp = std::env::temp_dir().join(format!("qsp-statsshot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::userdata::set_parent(&tmp, false).unwrap();

    let shot_count = |h: &Host| {
        std::fs::read_dir(h.user_dir())
            .ok()
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "png"))
            .count()
    };

    h.exec_line("cl_autodemo 0");
    h.exec_line("map e1m1");
    step(&mut h, 0.25);
    h.handle_event(HostEvent::Input(InputEvent::GamepadConnected(true)));
    assert_eq!(shot_count(&h), 0);

    // Single tap opens and closes scoreboard, no screenshot.
    pad(&mut h, GamepadButton::Back, true);
    assert!(h.show_scores);
    step(&mut h, 0.05);
    pad(&mut h, GamepadButton::Back, false);
    assert!(!h.show_scores);
    step(&mut h, 0.5);
    assert_eq!(shot_count(&h), 0);

    // Long hold (> 0.35s) followed by a quick tap must NOT take a screenshot.
    pad(&mut h, GamepadButton::Back, true);
    step(&mut h, 0.45);
    pad(&mut h, GamepadButton::Back, false);
    step(&mut h, 0.08);
    pad(&mut h, GamepadButton::Back, true);
    assert_eq!(shot_count(&h), 0);
    assert!(h.show_scores);
    pad(&mut h, GamepadButton::Back, false);
    step(&mut h, 0.5);

    // Double tap on Back takes a screenshot and leaves scoreboard hidden.
    pad(&mut h, GamepadButton::Back, true);
    step(&mut h, 0.05);
    pad(&mut h, GamepadButton::Back, false);
    step(&mut h, 0.10);
    pad(&mut h, GamepadButton::Back, true);
    assert!(!h.show_scores, "scoreboard must not cover double-tap screenshot");
    pad(&mut h, GamepadButton::Back, false);
    assert_eq!(shot_count(&h), 1, "double-tapping Select/View must write a screenshot");

    // Rebinding +showscores to another pad button moves the special behavior with it.
    h.exec_line("unbind BACK");
    h.exec_line("bind MISC1 +showscores");
    step(&mut h, 0.05);
    pad(&mut h, GamepadButton::Misc1, true);
    step(&mut h, 0.05);
    pad(&mut h, GamepadButton::Misc1, false);
    step(&mut h, 0.10);
    pad(&mut h, GamepadButton::Misc1, true);
    assert!(!h.show_scores);
    pad(&mut h, GamepadButton::Misc1, false);
    assert_eq!(shot_count(&h), 2, "rebound controller stats button must also take a screenshot");

    // Keyboard TAB double-tap must NOT trigger a screenshot.
    for _ in 0..2 {
        h.frame(0.05, vec![HostEvent::Input(InputEvent::Key { key: K_TAB, down: true })]);
        h.frame(0.05, vec![HostEvent::Input(InputEvent::Key { key: K_TAB, down: false })]);
    }
    assert_eq!(shot_count(&h), 2);

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn controller_menu_button_double_tap_quicksaves_and_long_press_quickloads() {
    let Some(mut h) = host() else { return };
    let tmp = std::env::temp_dir().join(format!("qsp-menugesture-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::userdata::set_parent(&tmp, false).unwrap();

    h.exec_line("cl_autodemo 0");
    h.exec_line("map e1m1");
    step(&mut h, 0.3);
    h.handle_event(HostEvent::Input(InputEvent::GamepadConnected(true)));

    // Single short tap on Start opens Main Menu on release; pressing Start
    // again after the double-tap window closes it normally (no quick save).
    pad(&mut h, GamepadButton::Start, true);
    assert_eq!(h.menu.screen, Screen::None, "menu waits for release so long-press doesn't flash it");
    step(&mut h, 0.06);
    pad(&mut h, GamepadButton::Start, false);
    assert_eq!(h.menu.screen, Screen::Main);
    step(&mut h, 0.5);
    pad(&mut h, GamepadButton::Start, true);
    pad(&mut h, GamepadButton::Start, false);
    assert_eq!(h.menu.screen, Screen::None);
    assert!(!h.user_dir().join("quick.sav").exists(), "delayed second tap must not quick save");

    // Double tap on Start creates a quick save and returns to gameplay.
    pad(&mut h, GamepadButton::Start, true);
    step(&mut h, 0.05);
    pad(&mut h, GamepadButton::Start, false);
    step(&mut h, 0.10);
    pad(&mut h, GamepadButton::Start, true);
    pad(&mut h, GamepadButton::Start, false);
    step(&mut h, 0.10);
    assert_eq!(h.menu.screen, Screen::None);
    assert!(h.user_dir().join("quick.sav").exists(), "double-tapping Start must create quick.sav");

    // Advance level time, then long-press Start -> quick loads without opening menu.
    step(&mut h, 1.5);
    let before_load_spawns = h.server.core.spawns;
    pad(&mut h, GamepadButton::Start, true);
    step(&mut h, 0.65);
    assert_eq!(h.menu.screen, Screen::None, "long-press must not flash menu");
    assert!(h.server.core.spawns > before_load_spawns, "long-pressing Start must trigger quick load");
    pad(&mut h, GamepadButton::Start, false);
    assert_eq!(h.menu.screen, Screen::None, "releasing after long-press must not open menu");

    // Custom pad button bound to togglemenu gets the same double-tap quick save.
    let _ = std::fs::remove_file(h.user_dir().join("quick.sav"));
    h.exec_line("bind PADDLE1 togglemenu");
    step(&mut h, 0.1);
    pad(&mut h, GamepadButton::Paddle1, true);
    step(&mut h, 0.05);
    pad(&mut h, GamepadButton::Paddle1, false);
    step(&mut h, 0.10);
    pad(&mut h, GamepadButton::Paddle1, true);
    pad(&mut h, GamepadButton::Paddle1, false);
    step(&mut h, 0.10);
    assert_eq!(h.menu.screen, Screen::None);
    assert!(h.user_dir().join("quick.sav").exists(), "custom togglemenu pad button must also quick save");

    // Keyboard Escape double-tap must NOT create a quick save.
    let _ = std::fs::remove_file(h.user_dir().join("quick.sav"));
    for _ in 0..2 {
        h.frame(0.05, vec![HostEvent::Input(InputEvent::Key { key: K_ESCAPE, down: true })]);
        h.frame(0.05, vec![HostEvent::Input(InputEvent::Key { key: K_ESCAPE, down: false })]);
    }
    step(&mut h, 0.1);
    assert!(!h.user_dir().join("quick.sav").exists(), "keyboard Escape must not trigger controller quick save");

    let _ = std::fs::remove_dir_all(&tmp);
}
