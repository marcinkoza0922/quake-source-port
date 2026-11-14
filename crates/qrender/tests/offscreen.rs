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
    // 2D only frame
    r.render(&RenderFrame::default()).unwrap();
}
