use super::*;
use std::cell::RefCell;
use std::rc::Rc;

use crate::glyphs::{Family, Glyphs};

#[test]
fn speeds_print_short() {
    assert_eq!(trim(2.0), "2");
    assert_eq!(trim(0.25), "0.25");
    assert_eq!(trim(1.5), "1.5");
}

#[test]
fn layout_keeps_buttons_apart_and_on_the_panel() {
    for pad in [false, true] {
        for (vw, vh) in [(320.0, 240.0), (426.0, 240.0), (480.0, 270.0), (640.0, 360.0)] {
            let l = layout(vw, vh, true, pad);
            assert!(l.panel.1 >= 0.0 && l.panel.1 + l.panel.3 <= vh);
            for (i, &(_, a)) in l.buttons.iter().enumerate() {
                assert!(a.0 >= l.panel.0 && a.0 + a.2 <= l.panel.0 + l.panel.2, "{vw} pad={pad}");
                assert!(a.1 >= l.panel.1 && a.1 + a.3 <= l.panel.1 + l.panel.3, "{vw} pad={pad}");
                for &(_, b) in &l.buttons[i + 1..] {
                    assert!(a.0 + a.2 <= b.0 || b.0 + b.2 <= a.0, "overlap at {vw} pad={pad}");
                }
            }
            // the transport buttons always fit
            assert!(l.buttons.iter().any(|(b, _)| *b == Btn::Faster));
        }
        // wide screens get every text button
        assert_eq!(layout(640.0, 360.0, true, pad).buttons.len(), 15);
        assert_eq!(layout(426.0, 240.0, true, pad).buttons.len(), 15);
    }
    assert!(lift(true) > lift(false));
    assert!(inside((0.0, 0.0, 2.0, 2.0), (1.0, 1.0)) && !inside((0.0, 0.0, 2.0, 2.0), (2.0, 1.0)));
}

#[test]
fn pad_glyphs_fit_on_every_button_in_every_family() {
    let mut c = Canvas::new(1280, 720);
    c.ui = Some(Rc::new(RefCell::new(crate::font::UiText::default())));
    for family in [Family::Xbox, Family::PlayStation, Family::Nintendo] {
        for (vw, vh) in [(320.0, 240.0), (426.67, 240.0), (480.0, 270.0), (640.0, 360.0)] {
            let l = layout(vw, vh, true, true);
            for &(b, r) in &l.buttons {
                let gw = crate::glyphs::width(&c, b.pad_key(), family, GLYPH_H);
                assert!(gw <= r.2, "{family:?} {b:?} glyph {gw} wider than button {} at {vw}x{vh}", r.2);
            }
        }
    }
}

#[test]
fn pad_mode_draws_glyphs_on_buttons_without_above_bar_clutter() {
    let ui = Rc::new(RefCell::new(crate::font::UiText::default()));
    let sample = |pad: bool| Bar {
        state: "Play 1x".into(),
        map: "The Slipgate Complex (e1m1)".into(),
        timeline: Some(Timeline { now: 24.0, len: 65.0, maps: vec![0.0, 35.0], marks: [Some(12.0), Some(40.0)] }),
        auto: true,
        playing: true,
        speed: "1".into(),
        pointer: None,
        held: None,
        pad,
    };
    let mut c_kb = Canvas::new(1280, 720);
    c_kb.ui = Some(ui.clone());
    draw(&mut c_kb, &sample(false), &Glyphs { family: Family::Xbox });
    let mut c_pad = Canvas::new(1280, 720);
    c_pad.ui = Some(ui.clone());
    draw(&mut c_pad, &sample(true), &Glyphs { family: Family::Xbox });
    // controller mode draws button glyphs on the panel (more 2D primitives than keyboard mode)
    assert!(c_pad.list.len() > c_kb.list.len());
    // on 16:9 (1280x720) all 15 buttons fit, so nothing draws above the panel
    let panel_top_px = layout(c_pad.vw(), c_pad.vh(), true, true).panel.1 * c_pad.scale;
    for d in &c_pad.list {
        let (qrender::Draw2D::Pic { y, .. } | qrender::Draw2D::Char { y, .. } | qrender::Draw2D::Fill { y, .. } | qrender::Draw2D::Tile { y, .. }) = *d;
        assert!(y >= panel_top_px - 1.0, "unexpected draw above panel: y={y} < {panel_top_px}");
    }

    // optional visual dump when QSP_DEMOBAR_SHOTS is set
    let Some(dir) = std::env::var_os("QSP_DEMOBAR_SHOTS") else { return };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pal = [[0u8; 3]; 256];
    let atlas_n = crate::font::UiText::atlas_size();
    for (name, w, h, family, pad) in [
        ("demobar-720p-xbox", 1280u32, 720u32, Family::Xbox, true),
        ("demobar-720p-ps", 1280, 720, Family::PlayStation, true),
        ("demobar-720p-nintendo", 1280, 720, Family::Nintendo, true),
        ("demobar-1080p-xbox", 1920, 1080, Family::Xbox, true),
        ("demobar-4x3-xbox", 960, 720, Family::Xbox, true),
        ("demobar-720p-kb", 1280, 720, Family::Xbox, false),
    ] {
        let Ok(mut r) = Renderer::headless(w, h, &pal) else { return };
        let ui = Rc::new(RefCell::new(crate::font::UiText::default()));
        {
            let mut u = ui.borrow_mut();
            u.pic = r.register_pic_rgba(atlas_n, atlas_n, &u.px).ok();
        }
        let mut c = Canvas::new(w, h);
        c.ui = Some(ui.clone());
        // dark slate backdrop so the bar stands out clearly
        c.fill(0.0, 0.0, c.vw(), c.vh(), [0.14, 0.16, 0.20, 1.0]);
        draw(&mut c, &sample(pad), &Glyphs { family });
        {
            let mut u = ui.borrow_mut();
            if u.dirty
                && let Some(pic) = u.pic
            {
                r.update_pic(pic, &u.px).unwrap();
                u.after_upload();
            }
        }
        let frame = qrender::RenderFrame {
            gun_portals: &[],
            time: 0.0,
            view: None,
            entities: &[],
            particles: &[],
            fte_vertices: &[],
            fte_indices: &[],
            fte_batches: &[],
            dlights: &[],
            lightstyles: &[],
            fog: Default::default(),
            blend: [0.0; 4],
            draw2d: &c.list,
            settings: Default::default(),
            shot_impacts: &[],
            dyn_verts: &[],
            splits: &[],
            player: None,
        };
        r.request_capture();
        r.render(&frame).unwrap();
        let px = r.read_pixels().unwrap();
        std::fs::write(dir.join(format!("{name}.png")), crate::png::encode_rgba(w, h, &px)).unwrap();
    }
}
