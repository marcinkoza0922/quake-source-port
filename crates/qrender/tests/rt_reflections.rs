//! Ray-traced water reflections (r_water_reflections 2) on start.bsp's pool;
//! skips without game data, a Vulkan device or ray queries.
use qformats::Bsp;
use qrender::*;

fn differing(a: &[u8], b: &[u8]) -> usize {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0)
        .filter(|(p, q)| p[..3].iter().zip(&q[..3]).any(|(x, y)| x.abs_diff(*y) > 2))
        .count()
}

#[test]
fn ray_traced_reflections_show_world_and_models() {
    let base = std::env::var_os("QUAKE_BASEDIR").map(std::path::PathBuf::from).unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps")
    });
    if !base.join("id1").exists() {
        eprintln!("skip: no game data");
        return;
    }
    let mut fs = qcore::fs::Fs::new(base);
    fs.add_game_dir("id1");
    let (Some(pal), Some(map), Some(mdl)) =
        (fs.open("gfx/palette.lmp"), fs.open("maps/start.bsp"), fs.open("progs/player.mdl"))
    else {
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
    if !r.ray_tracing_supported() {
        eprintln!("skip: no ray queries");
        return;
    }
    r.load_world(&bsp, None).unwrap();
    let player = r.load_alias(&qformats::Mdl::parse(&mdl).unwrap()).unwrap();
    // start.bsp's pool (surface z -136, x 448..639, y 778..1192): look down
    // along it from one end, with a player model standing over the far end
    let view = View { origin: [544.0, 800.0, -80.0], angles: [35.0, 90.0, 0.0], fov_x: 90.0, ..Default::default() };
    let ent = [RenderEntity {
        key: 7,
        model: player,
        origin: [544.0, 940.0, -100.0],
        angles: [0.0, 270.0, 0.0],
        alpha: 1.0,
        ..Default::default()
    }];
    // looking nearly straight down from over the pool's middle
    let down = View { origin: [544.0, 980.0, -80.0], angles: [80.0, 90.0, 0.0], ..view };
    let mut shot = |view: View, mode: u32, msaa: u32, entities: &[RenderEntity]| {
        let settings = Settings { water_reflections: mode, water_reflection_amount: 1.5, msaa, ..Default::default() };
        let frame = RenderFrame { time: 0.5, view: Some(view), entities, settings, ..Default::default() };
        // two frames: both frame slots build their structures
        r.render(&frame).unwrap();
        r.render(&frame).unwrap();
        r.read_pixels().unwrap()
    };
    for msaa in [1, 4] {
        let off = shot(view, 0, msaa, &[]);
        let rt = shot(view, 2, msaa, &[]);
        let water = differing(&off, &rt);
        assert!(water > 320 * 200 / 20, "{msaa}x: reflections changed only {water} pixels");
        // the model changes more of the frame when the water reflects it
        let off_ent = differing(&off, &shot(view, 0, msaa, &ent));
        let rt_ent = differing(&rt, &shot(view, 2, msaa, &ent));
        assert!(off_ent > 0, "{msaa}x: model not visible");
        assert!(rt_ent > off_ent + 50, "{msaa}x: no model reflection ({rt_ent} vs {off_ent} pixels)");
        // the player's own first-person body (portal_only, under the eye):
        // not drawn in the view, but reflected by both planar and ray tracing
        let body = [RenderEntity {
            key: 1,
            origin: [down.origin[0], down.origin[1], down.origin[2] - 22.0],
            portal_only: true,
            ..ent[0]
        }];
        for mode in [0, 1, 2] {
            let n = differing(&shot(down, mode, msaa, &[]), &shot(down, mode, msaa, &body));
            if mode == 0 {
                assert_eq!(n, 0, "{msaa}x: own body drawn in the main view");
            } else {
                assert!(n > 20, "{msaa}x: own body missing from mode {mode} reflection ({n} pixels)");
            }
        }
        // planar and back: switching modes keeps rendering
        let _ = shot(view, 1, msaa, &ent);
        // reflected surfaces outside the view are lit from the first frame
        // (their lightmaps are kept current), so later frames match
        assert_eq!(differing(&rt, &shot(view, 2, msaa, &[])), 0, "{msaa}x: ray-traced frame changed");
    }
    // a new level frees and rebuilds the world's structures
    r.load_world(&bsp, None).unwrap();
    let player = r.load_alias(&qformats::Mdl::parse(&mdl).unwrap()).unwrap();
    let ent = [RenderEntity { model: player, ..ent[0] }];
    let settings = Settings { water_reflections: 2, water_reflection_amount: 1.5, ..Default::default() };
    let frame = RenderFrame { time: 0.5, view: Some(view), entities: &ent, settings, ..Default::default() };
    r.render(&frame).unwrap();
    r.render(&frame).unwrap();
    let _ = r.read_pixels().unwrap();
}
