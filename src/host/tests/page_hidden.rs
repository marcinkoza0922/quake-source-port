//! The web page is hidden or closed.

use super::super::*;

#[test]
fn a_hidden_page_saves_now_and_then() {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::default_basedir());
    if !fs.add_game_dir("id1") || fs.open("maps/e1m1.bsp").is_none() {
        return;
    }
    let pal = fs.open("gfx/palette.lmp").and_then(|d| qformats::image::parse_palette(&d).ok()).unwrap();
    let Ok(r) = Renderer::headless(64, 64, &pal) else { return };
    let mut h = Host::new(fs, r, None, Vec::new());
    let tmp = std::env::temp_dir().join(format!("qsp-hidden-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    crate::userdata::set_parent(&tmp, false).unwrap();
    h.exec_line("exec default.cfg");
    h.exec_line("cl_startdemos 0");
    h.exec_line("cl_autodemo 0");
    h.exec_line("sv_autosave 1");
    for e in ["entry", "secret", "monsters", "secrets", "key"] {
        h.exec_line(&format!("sv_autosave_{e} 0"));
    }
    h.exec_line("sv_autosave_interval 0");
    h.exec_line("map e1m1");
    let step = |h: &mut Host, secs: f64| {
        for _ in 0..(secs * 72.0) as usize {
            h.frame(1.0 / 72.0, Vec::new());
        }
    };
    step(&mut h, 0.5);
    let dir = h.user_dir();
    let saves = || (0..9).filter(|i| dir.join(format!("autosave{i}.sav")).exists()).count();
    assert_eq!(saves(), 0);

    h.exec_line("sensitivity 7");
    h.page_hidden();
    assert_eq!(saves(), 1);
    let config = std::fs::read_to_string(dir.join("config.cfg")).unwrap();
    assert!(config.contains("sensitivity \"7\""), "{config:.300}");
    // another look at another tab: settings, but no second autosave
    step(&mut h, 1.0);
    h.exec_line("sensitivity 8");
    h.page_hidden();
    assert_eq!(saves(), 1);
    assert!(std::fs::read_to_string(dir.join("config.cfg")).unwrap().contains("sensitivity \"8\""));
    h.exec_line("god");
    step(&mut h, 31.0);
    h.page_hidden();
    assert_eq!(saves(), 2);
    let _ = std::fs::remove_dir_all(&tmp);
}
