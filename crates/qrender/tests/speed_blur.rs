//! Exercise peripheral blur on the GPU; skip without game data or Vulkan.
use qformats::{Bsp, parse_entities};
use qrender::*;

const WIDTH: usize = 320;
const HEIGHT: usize = 200;

fn start_scene() -> Option<(Renderer, View)> {
    let base = std::env::var_os("QUAKE_BASEDIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../vkQuake directory with maps")
        });
    if !base.join("id1").exists() {
        eprintln!("skip: no game data");
        return None;
    }
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    let (Some(pal), Some(map)) = (fs.open("gfx/palette.lmp"), fs.open("maps/start.bsp")) else {
        eprintln!("skip: no start.bsp");
        return None;
    };
    let pal = qformats::image::parse_palette(&pal).unwrap();
    let bsp = Bsp::parse(&map).unwrap();
    let mut renderer = match Renderer::headless(WIDTH as u32, HEIGHT as u32, &pal) {
        Ok(renderer) => renderer,
        Err(error) => {
            eprintln!("skip: no Vulkan device: {error}");
            return None;
        }
    };
    renderer.load_world(&bsp, None).unwrap();
    let entities = parse_entities(&bsp.entities).unwrap();
    let start = entities
        .iter()
        .find(|entity| entity.classname() == Some("info_player_start"))
        .unwrap();
    let origin: Vec<f32> = start
        .get("origin")
        .unwrap()
        .split_whitespace()
        .map(|value| value.parse().unwrap())
        .collect();
    let yaw = start
        .get("angle")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0.0);
    Some((
        renderer,
        View {
            origin: [origin[0], origin[1], origin[2] + 22.0],
            angles: [0.0, yaw, 0.0],
            fov_x: 90.0,
            ..Default::default()
        },
    ))
}

fn pixel(pixels: &[u8], x: usize, y: usize) -> &[u8] {
    let offset = (y * WIDTH + x) * 4;
    &pixels[offset..offset + 4]
}

fn assert_region_unchanged(before: &[u8], after: &[u8], rect: [usize; 4]) {
    let [x, y, width, height] = rect;
    for py in y..y + height {
        for px in x..x + width {
            assert_eq!(
                pixel(before, px, py),
                pixel(after, px, py),
                "pixel ({px}, {py}) changed"
            );
        }
    }
}

fn changed_edge_pixels(before: &[u8], after: &[u8], rect: [usize; 4]) -> usize {
    let [x, y, width, height] = rect;
    let mut changed = 0;
    for py in 0..height {
        for px in 0..width {
            let peripheral =
                px < width / 8 || px >= width * 7 / 8 || py < height / 8 || py >= height * 7 / 8;
            if peripheral && pixel(before, x + px, y + py) != pixel(after, x + px, y + py) {
                changed += 1;
            }
        }
    }
    changed
}

#[test]
fn speed_blur_preserves_center_ui_and_other_players() {
    let Some((mut renderer, view)) = start_scene() else {
        return;
    };
    let ui = [Draw2D::Fill {
        x: 2.0,
        y: 2.0,
        w: 20.0,
        h: 12.0,
        color: [1.0, 0.0, 1.0, 1.0],
    }];
    let frame = RenderFrame {
        time: 0.5,
        view: Some(view),
        draw2d: &ui,
        settings: Settings {
            dither: 0.0,
            ..Default::default()
        },
        ..Default::default()
    };
    renderer.render(&frame).unwrap();
    renderer.render(&frame).unwrap();
    let sharp = renderer.read_pixels().unwrap();
    renderer
        .render(&RenderFrame {
            view: Some(View {
                speed_blur: 1.0,
                ..view
            }),
            ..frame
        })
        .unwrap();
    let blurred = renderer.read_pixels().unwrap();
    assert_region_unchanged(&sharp, &blurred, [150, 90, 20, 20]);
    assert_region_unchanged(&sharp, &blurred, [2, 2, 20, 12]);
    assert_eq!(
        pixel(&blurred, 10, 8),
        &[255, 0, 255, 255],
        "HUD fill was altered"
    );
    assert!(
        changed_edge_pixels(&sharp, &blurred, [0, 0, WIDTH, HEIGHT]) > 100,
        "speed blur did not visibly affect the screen edges"
    );

    // Each split view samples only its own smaller scene region. A scaled
    // target also exercises the distinction between scene UVs and output UVs.
    let splits = [
        SplitView {
            view,
            rect: [0, 0, 160, 200],
            blend: [0.0; 4],
        },
        SplitView {
            view,
            rect: [160, 0, 160, 200],
            blend: [0.0; 4],
        },
    ];
    let split_frame = RenderFrame {
        settings: Settings {
            render_scale: 0.5,
            ..frame.settings
        },
        splits: &splits,
        ..frame
    };
    renderer.render(&split_frame).unwrap();
    let sharp_split = renderer.read_pixels().unwrap();
    let mut blurred_splits = splits;
    blurred_splits[1].view.speed_blur = 1.0;
    renderer
        .render(&RenderFrame {
            splits: &blurred_splits,
            ..split_frame
        })
        .unwrap();
    let blurred_split = renderer.read_pixels().unwrap();
    assert_region_unchanged(&sharp_split, &blurred_split, [0, 0, 160, HEIGHT]);
    assert_region_unchanged(&sharp_split, &blurred_split, [234, 90, 12, 20]);
    assert!(
        changed_edge_pixels(&sharp_split, &blurred_split, [160, 0, 160, HEIGHT]) > 100,
        "the second player's screen edges were not blurred"
    );
}
