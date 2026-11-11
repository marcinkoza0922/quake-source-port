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
    // 2D only frame
    r.render(&RenderFrame::default()).unwrap();
}
