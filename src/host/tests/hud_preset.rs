//! HUD presets through a real headless host: picking one rebuilds the
//! layout, the HUD editor makes it custom, and an old classic HUD style
//! comes back as the classic preset.
use super::super::*;
use crate::hud::kind::Kind;

fn host(setup: &[&str]) -> Option<Host> {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") {
        return None;
    }
    let pal = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok())?;
    let r = Renderer::headless(640, 480, &pal).ok()?;
    let mut h = Host::new(fs, r, None, Vec::new());
    h.headless = true;
    for c in setup {
        h.exec_line(c);
    }
    frames(&mut h);
    Some(h)
}

fn frames(h: &mut Host) {
    for _ in 0..3 {
        h.frame(1.0 / 72.0, Vec::new());
    }
}

fn kinds(h: &Host) -> Vec<Kind> {
    h.hud.layout.elems.iter().map(|e| e.kind).collect()
}

#[test]
fn presets_rebuild_the_layout_and_editing_makes_it_custom() {
    // configs may already hold a layout: start from the old modern HUD
    let Some(mut h) = host(&["hud_layout \"\"", "hud_preset 0"]) else { return };
    assert_eq!(h.cv("hud_preset"), 0.0);
    assert_eq!(kinds(&h)[0], Kind::Health);
    h.exec_line("hud_preset 1");
    frames(&mut h);
    assert_eq!(kinds(&h), [Kind::StatusBar, Kind::Center, Kind::Notify, Kind::Captions]);
    assert!(h.hud_reserve() > 0.0, "the 3D view ends above the status bar");
    // a change in the editor: custom, and the layout stays (second ENTER confirms the grid position prompt)
    h.exec_line("menu_hudedit");
    h.exec_line("menu_keys INS DOWNARROW DOWNARROW DOWNARROW DOWNARROW RIGHTARROW DOWNARROW ENTER ENTER");
    frames(&mut h);
    assert_eq!(h.cv("hud_preset"), 2.0);
    assert!(kinds(&h).contains(&Kind::Clock) && kinds(&h).contains(&Kind::StatusBar));
    h.exec_line("menu_keys ESCAPE");
    // back to modern
    h.exec_line("hud_preset 0");
    frames(&mut h);
    assert_eq!(kinds(&h)[..3], [Kind::Health, Kind::Armor, Kind::Ammo]);
    assert_eq!(h.hud_reserve(), 0.0);
    // ironwail's hudstyle picks classic
    h.exec_line("hudstyle 0");
    frames(&mut h);
    assert_eq!(kinds(&h)[0], Kind::StatusBar);
    // Saved user presets can be loaded via hud_preset_load and switched from Options > HUD
    h.menu.hud_edit.presets.push(crate::hud::presets::SavedPreset {
        name: "Minimal".into(),
        layout: "health@bl,i fps@tr center@mc notify@tl captions@bc".into(),
    });
    h.exec_line("hud_preset_load Minimal");
    frames(&mut h);
    assert_eq!(h.cv("hud_preset"), 2.0);
    assert_eq!(h.cvs("hud_preset_name"), "Minimal");
    assert!(kinds(&h).contains(&Kind::Fps) && !kinds(&h).contains(&Kind::StatusBar));
    h.exec_line("hud_preset 0");
    frames(&mut h);
    assert_eq!(h.cvs("hud_preset_name"), "");
    assert!(!kinds(&h).contains(&Kind::Fps));
}

#[test]
fn old_classic_style_becomes_the_classic_preset() {
    let Some(h) = host(&["hud_layout \"\"", "scr_hudstyle 0"]) else { return };
    assert_eq!(h.cv("hud_preset"), 1.0);
    assert_eq!(kinds(&h)[0], Kind::StatusBar);
    let Some(h) = host(&["hud_layout \"\"", "scr_hudstyle 1", "hud_clock 1"]) else { return };
    assert_eq!(h.cv("hud_preset"), 2.0);
    // a layout saved before presets existed is custom; a preset's own stays
    let Some(h) = host(&["hud_layout \"health@bl fps@tr,o0.4\"", "hud_preset 0"]) else { return };
    assert_eq!(h.cv("hud_preset"), 2.0);
    assert_eq!(h.hud.layout.elems.len(), 5);
    let Some(mut h) = host(&["hud_layout \"\"", "scr_hudstyle 0"]) else { return };
    let saved = h.cvs("hud_layout").to_string();
    h.hud = Default::default();
    h.exec_line(&format!("hud_layout \"{saved}\""));
    frames(&mut h);
    assert_eq!(h.cv("hud_preset"), 1.0);
}
