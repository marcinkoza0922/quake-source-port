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
    let cell = |pixels: &[u8], x: usize| pixels.as_chunks::<{ 320 * 4 }>().0.iter()
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
    let view_counts = [2, 4, 5, 1];
    let split_cases: Vec<Vec<SplitView>> = view_counts.iter().map(|&n| {
        (0..n).map(|i| SplitView {
            view: View { angles: [0.0, yaw + i as f32 * 45.0, 0.0], ..view },
            rect: [i as u32 * (320 / n as u32), 0, 320 / n as u32, 200],
            blend: [0.0; 4],
        }).collect()
    }).collect();
    // Warm every angle's lightmap atlas entries before capturing bit-exact
    // reference frames (tightly packed adjacent surfaces only populate on draw).
    for splits in &split_cases {
        r.render(&RenderFrame { splits, ..split_frame }).unwrap();
    }
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
    r.gpu_times_wanted = false;
    // Cycle all SSAO quality tiers across single-sample/MSAA and 100%/200% scale.
    for msaa in [1, 4] {
        for render_scale in [1.0, 2.0] {
            for ssao_quality in [0, 1, 2] {
                let st = Settings { ssao: 2.0, ssao_quality, msaa, render_scale, ..Default::default() };
                r.render(&RenderFrame { time: frame.time, splits: &splits, settings: st, ..Default::default() }).unwrap();
                let _ = r.read_pixels().unwrap();
            }
        }
    }
    r.render(&split_frame).unwrap();
    assert_eq!(split_expected, r.read_pixels().unwrap(), "SSAO quality cycling corrupted recreated targets");
    // Cycle portal quality tiers across single-sample/MSAA, 100%/200% scale and split-screen.
    for msaa in [1, 4] {
        for render_scale in [1.0, 2.0] {
            for portal_quality in [0, 1, 2] {
                let st = Settings { teleport_windows: true, portal_quality, msaa, render_scale, ..Default::default() };
                r.render(&RenderFrame { settings: st, ..portal_frame }).unwrap();
                let portal_px = r.read_pixels().unwrap();
                assert!(portal_px.as_chunks::<4>().0.iter().any(|p| p[..3].iter().any(|&v| v > 30)));
                r.render(&RenderFrame { time: portal_frame.time, splits: &splits, settings: st, ..Default::default() }).unwrap();
                let _ = r.read_pixels().unwrap();
            }
            for water_quality in [0, 1] {
                let st = Settings {
                    wateralpha: 0.35,
                    water_ripples: 1.0,
                    water_splashes: true,
                    water_caustics: 1.0,
                    water_quality,
                    msaa,
                    render_scale,
                    ..Default::default()
                };
                r.render(&RenderFrame { time: frame.time, view: frame.view, settings: st, ..Default::default() }).unwrap();
                let _ = r.read_pixels().unwrap();
            }
        }
    }
    // Cycle motion blur quality tiers with a moving alias entity so vel.frag and
    // post.frag run across MSAA, render scale and split-screen.
    if let Some(mdl_bytes) = fs.open("progs/player.mdl") {
        let mdl = qformats::Mdl::parse(&mdl_bytes).unwrap();
        let player_id = r.load_alias(&mdl).unwrap();
        let rad = yaw.to_radians();
        let fwd = [rad.cos(), rad.sin(), 0.0];
        let right = [rad.sin(), -rad.cos(), 0.0];
        let base_pos = std::array::from_fn(|k| view.origin[k] + fwd[k] * 96.0);
        let moved_pos = std::array::from_fn(|k| base_pos[k] + right[k] * 14.0);
        let ent = |pos: [f32; 3]| RenderEntity {
            key: 42,
            model: player_id,
            origin: pos,
            angles: [0.0, yaw + 180.0, 0.0],
            alpha: 1.0,
            ..Default::default()
        };
        let e0 = [ent(base_pos)];
        let e1 = [ent(moved_pos)];
        for msaa in [1, 4] {
            for render_scale in [1.0, 2.0] {
                for motion_blur_quality in [0, 1] {
                    let st = Settings {
                        motion_blur: 2.0,
                        motion_blur_quality,
                        msaa,
                        render_scale,
                        ..Default::default()
                    };
                    r.render(&RenderFrame { time: 1.0, view: Some(view), entities: &e0, settings: st, ..Default::default() }).unwrap();
                    r.render(&RenderFrame { time: 1.02, view: Some(view), entities: &e1, settings: st, ..Default::default() }).unwrap();
                    let blurred = r.read_pixels().unwrap();
                    let st_off = Settings { motion_blur: 0.0, ..st };
                    r.render(&RenderFrame { time: 1.04, view: Some(view), entities: &e1, settings: st_off, ..Default::default() }).unwrap();
                    let crisp = r.read_pixels().unwrap();
                    assert_ne!(blurred, crisp, "motion blur quality {motion_blur_quality} ({msaa}x, {render_scale}) produced no change");
                    // Split-screen coverage with recreated velocity targets.
                    r.render(&RenderFrame { time: 1.06, splits: &splits, entities: &e0, settings: st, ..Default::default() }).unwrap();
                    r.render(&RenderFrame { time: 1.08, splits: &splits, entities: &e1, settings: st, ..Default::default() }).unwrap();
                    let _ = r.read_pixels().unwrap();
                }
            }
        }
    }
    // Cycle soft particle quality tiers across MSAA and supersampling with a soft
    // FTE quad near the view.
    let rad = yaw.to_radians();
    let fwd = [rad.cos(), rad.sin(), 0.0];
    let right = [rad.sin(), -rad.cos(), 0.0];
    let q_center = [
        view.origin[0] + fwd[0] * 48.0,
        view.origin[1] + fwd[1] * 48.0,
        view.origin[2],
    ];
    let soft_verts = [
        FteVertex { pos: [q_center[0] - right[0] * 16.0, q_center[1] - right[1] * 16.0, q_center[2] - 16.0], uv: [0.0, 1.0], color: [255; 4] },
        FteVertex { pos: [q_center[0] + right[0] * 16.0, q_center[1] + right[1] * 16.0, q_center[2] - 16.0], uv: [1.0, 1.0], color: [255; 4] },
        FteVertex { pos: [q_center[0] + right[0] * 16.0, q_center[1] + right[1] * 16.0, q_center[2] + 16.0], uv: [1.0, 0.0], color: [255; 4] },
        FteVertex { pos: [q_center[0] - right[0] * 16.0, q_center[1] - right[1] * 16.0, q_center[2] + 16.0], uv: [0.0, 0.0], color: [255; 4] },
    ];
    let soft_idx = [0, 1, 2, 0, 2, 3];
    let soft_batch = [FteBatch { pic, blend: FteBlend::Blend, lines: false, first_index: 0, num_indices: 6, soft: true, gel: false }];
    for msaa in [1, 4] {
        for supersample in [false, true] {
            for soft_particle_quality in [0, 1] {
                let st = Settings {
                    msaa,
                    supersample,
                    soft_particle_distance: 4.0,
                    soft_particle_quality,
                    ..Default::default()
                };
                r.render(&RenderFrame {
                    time: frame.time,
                    view: Some(view),
                    settings: st,
                    fte_vertices: &soft_verts,
                    fte_indices: &soft_idx,
                    fte_batches: &soft_batch,
                    ..Default::default()
                }).unwrap();
                let _ = r.read_pixels().unwrap();
            }
        }
    }
    // Gel shading quality tiers (low single-fetch vs high parallax+gradient)
    // and async staged update_pic_rows + resize_pic across consecutive frames.
    let mut gel_px = vec![0u8; 32 * 32 * 4];
    for y in 0..32usize {
        for x in 0..32usize {
            let (dx, dy) = (x as f32 / 15.5 - 1.0, y as f32 / 15.5 - 1.0);
            let h = ((1.0 - (dx * dx + dy * dy)).max(0.0).sqrt() * 255.0) as u8;
            let o = (y * 32 + x) * 4;
            gel_px[o..o + 4].copy_from_slice(&[64, 160, 255, h]);
        }
    }
    let gel_pic = r.register_pic_rgba_linear(32, 32, &gel_px).unwrap();
    let gel_batch = [FteBatch { pic: gel_pic, blend: FteBlend::Blend, lines: false, first_index: 0, num_indices: 6, soft: false, gel: true }];
    let mut gel_outs = Vec::new();
    for gel_quality in [0u32, 1] {
        // Stage a partial update, render consecutive frames without idle(),
        // then render full update and read back.
        r.update_pic_rows(gel_pic, &gel_px[..32 * 16 * 4], 16).unwrap();
        for _ in 0..2 {
            r.render(&RenderFrame {
                time: frame.time,
                view: Some(view),
                settings: Settings { gel_quality, ..Default::default() },
                fte_vertices: &soft_verts,
                fte_indices: &soft_idx,
                fte_batches: &gel_batch,
                ..Default::default()
            }).unwrap();
        }
        r.update_pic_rows(gel_pic, &gel_px, 32).unwrap();
        r.render(&RenderFrame {
            time: frame.time,
            view: Some(view),
            settings: Settings { gel_quality, ..Default::default() },
            fte_vertices: &soft_verts,
            fte_indices: &soft_idx,
            fte_batches: &gel_batch,
            ..Default::default()
        }).unwrap();
        gel_outs.push(r.read_pixels().unwrap());
    }
    assert_ne!(gel_outs[0], gel_outs[1], "fast gel shader must differ from full parallax+gradient");
    let small_px = vec![255u8; 16 * 16 * 4];
    r.resize_pic(gel_pic, 16, 16).unwrap();
    r.update_pic(gel_pic, &small_px).unwrap();
    r.render(&RenderFrame {
        time: frame.time,
        view: Some(view),
        fte_vertices: &soft_verts,
        fte_indices: &soft_idx,
        fte_batches: &gel_batch,
        ..Default::default()
    }).unwrap();
    let _ = r.read_pixels().unwrap();

    // Goggles post-process glow tiers across night vision (1) and thermal (2).
    for mode in [1u32, 2] {
        let mut outs = Vec::new();
        for quality in [0u32, 1, 2] {
            let st = Settings {
                goggles: qrender::Goggles { mode, intensity: 1.5, noise: 0.5, quality },
                ..Default::default()
            };
            r.render(&RenderFrame { time: 1.0, view: Some(view), settings: st, ..Default::default() }).unwrap();
            let px = r.read_pixels().unwrap();
            assert!(px.as_chunks::<4>().0.iter().any(|p| p[..3].iter().any(|&v| v > 10)));
            outs.push(px);
        }
        assert_ne!(outs[0], outs[1], "mode {mode}: low (no glow) and medium (4-tap glow) must differ");
    }

    // Slang shader input scale: fit output (shader_scale = 0) caps 200% render
    // scale at output size (320x240) while preserving low-res input at 50%;
    // 3D resolution (shader_scale = 1) feeds full 200% scene size (640x480).
    let preset = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/slang/crt/crt-easymode.slangp");
    if preset.is_file() {
        r.set_shader_preset(Some(&preset)).unwrap();
        let cases = [
            (2.0f32, 0u32, (320u32, 200u32)),
            (2.0f32, 1u32, (640u32, 400u32)),
            (0.5f32, 0u32, (160u32, 100u32)),
        ];
        for (render_scale, shader_scale, want_size) in cases {
            let st = Settings { render_scale, shader_scale, ..Default::default() };
            r.render(&RenderFrame { time: frame.time, view: Some(view), settings: st, ..Default::default() }).unwrap();
            assert_eq!(r.shader_src_size(), Some(want_size));
            let px = r.read_pixels().unwrap();
            assert!(px.as_chunks::<4>().0.iter().any(|p| p[..3].iter().any(|&v| v > 10)));
        }
        r.set_shader_preset(None).unwrap();
    }
}
