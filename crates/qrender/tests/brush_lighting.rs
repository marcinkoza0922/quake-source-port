//! Brush lightmaps must be ready on their first visible frame.
use qformats::Bsp;
use qrender::*;
use std::path::{Path, PathBuf};

#[test]
fn sepulcher_gate_lighting_does_not_depend_on_camera_history() {
    let base = std::env::var_os("QUAKE_BASEDIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
        });
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    fs.add_game_dir("ad");
    let (Some(pal), Some(map)) = (fs.open("gfx/palette.lmp"), fs.open("maps/ad_sepulcher.bsp"))
    else {
        eprintln!("skip: no AD game data");
        return;
    };
    let pal = qformats::image::parse_palette(&pal).unwrap();
    let bsp = Bsp::parse(&map).unwrap();
    let mut r = match Renderer::headless(320, 240, &pal) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skip: no Vulkan device: {e}");
            return;
        }
    };
    let models = r.load_world(&bsp, None).unwrap();
    // The closed wooden portcullis opposite the player start.
    let entities = qformats::parse_entities(&bsp.entities).unwrap();
    let gate_num: usize = entities
        .iter()
        .find(|e| e.get("targetname") == Some("frontdoor"))
        .unwrap()
        .get("model")
        .unwrap()
        .strip_prefix('*')
        .unwrap()
        .parse()
        .unwrap();
    let gate = [RenderEntity {
        model: models[gate_num - 1],
        ..Default::default()
    }];
    let view = View {
        origin: [-2560.0, 1840.0, 0.0],
        angles: [0.0, 270.0, 0.0],
        fov_x: 90.0,
        ..Default::default()
    };
    let frame = RenderFrame {
        time: 1.0,
        view: Some(view),
        entities: &gate,
        settings: Settings {
            teleport_windows: true,
            wateralpha: 0.6,
            ..Default::default()
        },
        ..Default::default()
    };
    r.render(&frame).unwrap();
    r.render(&frame).unwrap();
    let before = r.read_pixels().unwrap();
    for yaw in (0..360).step_by(30) {
        let turn = RenderFrame {
            view: Some(View {
                angles: [0.0, yaw as f32, 0.0],
                ..view
            }),
            ..frame
        };
        r.render(&turn).unwrap();
    }
    r.render(&frame).unwrap();
    r.render(&frame).unwrap();
    let after = r.read_pixels().unwrap();
    let changed = before
        .chunks_exact(4)
        .zip(after.chunks_exact(4))
        .filter(|(a, b)| a != b)
        .count();
    assert_eq!(changed, 0, "camera history changed lighting");
}
