//! Exercise the mapper shaders and geometry counters on either renderer.
use qformats::{Bsp, image::parse_palette, parse_entities};
use qrender::{MapperSettings, RenderFrame, Renderer, Settings, View};

#[test]
fn mapper_debug_modes_render_distinct_frames() {
    let mut fs = qcore::fs::Fs::new(qcore::fs::Fs::test_basedir());
    fs.add_game_dir("id1");
    let (Some(palette), Some(map)) = (fs.open("gfx/palette.lmp"), fs.open("maps/e1m1.bsp")) else {
        eprintln!("skip: no e1m1 or palletized game data");
        return;
    };
    let palette = parse_palette(&palette).unwrap();
    let bsp = Bsp::parse(&map).unwrap();
    let Ok(mut renderer) = Renderer::headless(320, 200, &palette) else {
        eprintln!("skip: no headless graphics device");
        return;
    };
    renderer.load_world(&bsp, None).unwrap();
    let entities = parse_entities(&bsp.entities).unwrap();
    let spawn = entities
        .iter()
        .find(|e| e.classname() == Some("info_player_start"))
        .unwrap();
    let mut coords = spawn
        .get("origin")
        .unwrap()
        .split_whitespace()
        .map(|x| x.parse::<f32>().unwrap());
    let mut origin = std::array::from_fn(|_| coords.next().unwrap());
    origin[2] += 22.0;
    let view = View {
        origin,
        angles: [0.0, spawn.get("angle").unwrap_or("0").parse().unwrap(), 0.0],
        fov_x: 90.0,
        ..Default::default()
    };

    let mut render = |mapper: MapperSettings| {
        renderer
            .render(&RenderFrame {
                view: Some(view),
                settings: Settings {
                    mapper,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
        (
            renderer.read_pixels().unwrap(),
            renderer.mapper_stats(),
            renderer.supports_wireframe(),
        )
    };

    let speeds = MapperSettings {
        speeds: true,
        ..Default::default()
    };
    let (normal, stats, supports_wireframe) = render(speeds);
    assert!(stats.world_polys > 0 && stats.leaves > 0 && stats.triangles > 0 && stats.draws > 0);
    let (lightmap, _, _) = render(MapperSettings {
        lightmap: true,
        ..speeds
    });
    assert_ne!(
        normal, lightmap,
        "the lightmap-only view should drop textures"
    );
    if supports_wireframe {
        let (wire, _, _) = render(MapperSettings { wire: 1, ..speeds });
        let (overdraw, _, _) = render(MapperSettings { wire: 2, ..speeds });
        assert_ne!(normal, wire, "the wireframe should draw triangle edges");
        assert_ne!(
            wire, overdraw,
            "overdraw should include occluded triangle edges"
        );
    }
}
