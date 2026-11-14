//! Headless render of start.bsp; skips without game data or a Vulkan device.
use qformats::{Bsp, parse_entities};
use qrender::*;

#[test]
fn start_bsp_renders_non_black() {
    let base = std::env::var_os("QUAKE_BASEDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../vkQuake directory with maps")
        });
    if !base.join("id1").exists() {
        eprintln!("skip: no game data");
        return;
    }
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    let (Some(pal), Some(map)) = (fs.open("gfx/palette.lmp"), fs.open("maps/start.bsp")) else {
        eprintln!("skip: no start.bsp");
        return;
    };
    let pal = qformats::image::parse_palette(&pal).unwrap();
    let bsp = Bsp::parse(&map).unwrap();
    let mut r = match Renderer::headless(320, 200, &pal) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skip: no Vulkan device: {e}");
            return;
        }
    };
    r.load_world(&bsp, None).unwrap();
    let ents = parse_entities(&bsp.entities).unwrap();
    let start = ents
        .iter()
        .find(|e| e.classname() == Some("info_player_start"))
        .unwrap();
    let o: Vec<f32> = start
        .get("origin")
        .unwrap()
        .split_whitespace()
        .map(|v| v.parse().unwrap())
        .collect();
    let yaw: f32 = start
        .get("angle")
        .and_then(|a| a.parse().ok())
        .unwrap_or(0.0);
    let view = View {
        origin: [o[0], o[1], o[2] + 22.0],
        angles: [0.0, yaw, 0.0],
        fov_x: 90.0,
        ..Default::default()
    };
    let frame = RenderFrame {
        time: 0.5,
        view: Some(view),
        ..Default::default()
    };
    r.render(&frame).unwrap();
    r.render(&frame).unwrap();
    let px = r.read_pixels().unwrap();
    assert_eq!(px.len(), 320 * 200 * 4);
    let lit = px
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
        .count();
    assert!(lit > 320 * 200 / 2, "mostly black frame: {lit} lit pixels");
    // light sampling works at the start position
    let l = r.light_point(view.origin, &[]);
    assert!(l.iter().any(|&c| c > 0.0), "{l:?}");
    // Isolation must preserve a destination view and restore the source scene
    // exactly at a fixed camera/time, including the normal GPU culling path.
    let portal = &qrender::portals::inspect(&bsp)[0];
    let portal_frame = RenderFrame {
        view: Some(View {
            origin: std::array::from_fn(|k| portal.center[k] + portal.normal[k] * 110.0),
            angles: [
                0.0,
                (-portal.normal[1]).atan2(-portal.normal[0]).to_degrees(),
                0.0,
            ],
            ..view
        }),
        settings: Settings {
            teleport_windows: true,
            msaa: 2,
            ..Default::default()
        },
        ..frame
    };
    r.render(&portal_frame).unwrap();
    let baseline = r.read_pixels().unwrap();
    let cmd = |words: &[&str]| words.iter().map(|w| w.to_string()).collect::<Vec<_>>();
    assert!(
        r.debug_visibility(&cmd(&["isolate", "0"]))
            .contains("Isolated portal 0")
    );
    r.render(&portal_frame).unwrap();
    let isolated = r.read_pixels().unwrap();
    assert_ne!(baseline, isolated, "isolation should remove source scenery");
    assert!(
        isolated
            .as_chunks::<4>()
            .0
            .iter()
            .any(|p| p[..3].iter().any(|&v| v > 30)),
        "isolation removed the portal destination"
    );
    r.debug_visibility(&cmd(&["isolate", "0", "110"]));
    let relocated_frame = RenderFrame {
        time: portal_frame.time,
        view: Some(View {
            origin: [0.0; 3],
            angles: [0.0; 3],
            ..view
        }),
        settings: portal_frame.settings,
        ..Default::default()
    };
    r.render(&relocated_frame).unwrap();
    assert_eq!(
        isolated,
        r.read_pixels().unwrap(),
        "fixed camera followed player relocation"
    );
    // Suppressing soft particles must still resolve the main MSAA image.
    // Otherwise the portal pass's scratch colour can fill the whole screen.
    let pic = r.register_pic_rgba(1, 1, &[255; 4]).unwrap();
    let vertices = [FteVertex::default(); 3];
    let indices = [0, 1, 2];
    let batches = [FteBatch {
        pic,
        blend: FteBlend::Blend,
        lines: false,
        first_index: 0,
        num_indices: 3,
        soft: true, gel: false,
    }];
    let soft_frame = RenderFrame {
        time: portal_frame.time,
        view: portal_frame.view,
        settings: Settings {
            teleport_windows: true,
            msaa: 2,
            soft_particle_distance: 4.0,
            ..Default::default()
        },
        fte_vertices: &vertices,
        fte_indices: &indices,
        fte_batches: &batches,
        ..Default::default()
    };
    r.render(&soft_frame).unwrap();
    assert_eq!(
        isolated,
        r.read_pixels().unwrap(),
        "soft particles broke isolated scene resolve"
    );
    r.debug_visibility(&cmd(&["reset"]));
    r.render(&portal_frame).unwrap();
    assert_eq!(
        baseline,
        r.read_pixels().unwrap(),
        "reset must restore source scenery"
    );
    // Both players see the same teleporter destination. Sharing the immutable
    // destination ranges must preserve each cell's pixels and rebuild after
    // time changes and frames that use a different layout.
    let portal_splits = [
        SplitView { view: portal_frame.view.unwrap(), rect: [0, 0, 160, 200], blend: [0.0; 4] },
        SplitView { view: portal_frame.view.unwrap(), rect: [160, 0, 160, 200], blend: [0.0; 4] },
    ];
    let portal_single = RenderFrame { splits: &portal_splits[..1], ..portal_frame };
    let portal_double = RenderFrame { splits: &portal_splits, ..portal_frame };
    let cell = |pixels: &[u8], x: usize| pixels.chunks_exact(320 * 4)
        .flat_map(|row| row[x * 4..(x + 160) * 4].iter().copied()).collect::<Vec<_>>();
    r.render(&portal_single).unwrap();
    let cell_expected = cell(&r.read_pixels().unwrap(), 0);
    r.render(&portal_double).unwrap();
    let both_expected = r.read_pixels().unwrap();
    assert_eq!(cell_expected, cell(&both_expected, 0));
    assert_eq!(cell_expected, cell(&both_expected, 160), "shared destination changed the second view");
    r.render(&RenderFrame { time: portal_frame.time + 1.0, ..portal_double }).unwrap();
    r.render(&portal_single).unwrap();
    r.render(&portal_double).unwrap();
    assert_eq!(both_expected, r.read_pixels().unwrap(), "destination ranges survived into another frame");
    // 2D only frame
    r.render(&RenderFrame::default()).unwrap();
    // Profiling must preserve pixels and produce complete queries for AO on,
    // AO off and no-view frames. Capture every reference before enabling it.
    let frames = [
        RenderFrame::default(),
        RenderFrame { time: frame.time, view: frame.view, ..Default::default() },
        RenderFrame {
            time: frame.time,
            view: frame.view,
            settings: Settings { ssao: 2.0, msaa: 4, render_scale: 2.0, ..Default::default() },
            ..Default::default()
        },
    ];
    let expected: Vec<_> = frames.iter().map(|frame| {
        r.render(frame).unwrap();
        r.read_pixels().unwrap()
    }).collect();
    // The MSAA snapshot must survive consecutive views and be recreated after
    // resize/sample changes. Returning to the same scene must restore pixels.
    let splits = [
        SplitView { view, rect: [0, 0, 160, 200], blend: [0.0; 4] },
        SplitView { view: View { angles: [0.0, yaw + 90.0, 0.0], ..view }, rect: [160, 0, 160, 200], blend: [0.0; 4] },
    ];
    let split_frame = RenderFrame {
        time: frame.time,
        splits: &splits,
        settings: Settings { ssao: 2.0, msaa: 4, render_scale: 2.0, ..Default::default() },
        ..Default::default()
    };
    r.render(&split_frame).unwrap();
    let split_expected = r.read_pixels().unwrap();
    r.resize(400, 240);
    r.render(&frames[2]).unwrap();
    r.render(&frames[1]).unwrap(); // drops multisampled targets
    r.resize(320, 200);
    r.render(&split_frame).unwrap();
    assert_eq!(split_expected, r.read_pixels().unwrap(), "AO snapshot changed after target recreation");
    // Capture multi-view pixels before enabling profiling, including a fifth
    // view to force query-pool growth beyond the four-player reservation.
    let view_counts = [2, 4, 5, 1];
    let split_cases: Vec<Vec<SplitView>> = view_counts.iter().map(|&n| {
        (0..n).map(|i| SplitView {
            view: View { angles: [0.0, yaw + i as f32 * 45.0, 0.0], ..view },
            rect: [i as u32 * (320 / n as u32), 0, 320 / n as u32, 200],
            blend: [0.0; 4],
        }).collect()
    }).collect();
    let split_pixels: Vec<_> = split_cases.iter().map(|splits| {
        r.render(&RenderFrame { splits, ..split_frame }).unwrap();
        r.read_pixels().unwrap()
    }).collect();
    r.gpu_times_wanted = true;
    for (frame, expected) in frames.iter().zip(expected) {
        // Drain both frame slots so collection includes this path's queries.
        for _ in 0..4 { r.render(frame).unwrap(); }
        assert_eq!(expected, r.read_pixels().unwrap(), "GPU profiling changed output");
        let (count, stages) = r.take_gpu_times().expect("GPU queries enabled");
        assert!(count >= 2, "incomplete timestamp queries");
        for name in ["ao-prepare", "ao-generate", "ao-to-apply", "ao-apply", "ao-resume", "translucent"] {
            let us = stages.iter().find(|(k, _)| *k == name).expect("missing GPU stage").1;
            assert!(us.is_finite() && us >= 0.0, "invalid {name} timestamp: {us}");
        }
    }
    for (splits, expected) in split_cases.iter().zip(split_pixels) {
        let frame = RenderFrame { splits, ..split_frame };
        for _ in 0..4 { r.render(&frame).unwrap(); }
        let _ = r.take_gpu_times(); // discard queries from the previous view count
        for _ in 0..4 { r.render(&frame).unwrap(); }
        assert_eq!(expected, r.read_pixels().unwrap(), "profiling changed split pixels");
        let views = r.gpu_view_times();
        let (frames, stages) = r.take_gpu_times().unwrap();
        assert_eq!(views.len(), splits.len());
        for view in &views { assert_eq!(view.frames, frames); }
        for (name, us) in &stages {
            assert!(us.is_finite() && *us >= 0.0, "invalid {name}: {us}");
            if *name == "ui+present" { continue; }
            let sum: f64 = views.iter().flat_map(|v| &v.stages)
                .filter(|(k, _)| k == name).map(|v| v.1).sum();
            assert!((sum - us).abs() < 1e-6, "{name} double-counted or missing a view");
        }
        assert!(r.gpu_view_times().is_empty(), "taking totals must reset views too");
    }

}
