//! Offscreen render of a map from info_player_start to `target/qrender-<map>.ppm`.
//! Usage: cargo run -p qrender --example offscreen [map] [width height] [game]
use qformats::{Bsp, Mdl, Spr, Wad, parse_entities};
use qrender::*;

fn vec3(s: &str) -> [f32; 3] {
    let v: Vec<f32> = s
        .split_whitespace()
        .filter_map(|t| t.parse().ok())
        .collect();
    [
        v.first().copied().unwrap_or(0.0),
        v.get(1).copied().unwrap_or(0.0),
        v.get(2).copied().unwrap_or(0.0),
    ]
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let map = args.get(1).cloned().unwrap_or_else(|| "start".into());
    let w: u32 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(1280);
    let h: u32 = args.get(3).and_then(|s| s.parse().ok()).unwrap_or(720);
    let mut fs = qcore::fs::Fs::new(
        std::env::var_os("QUAKE_BASEDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../vkQuake directory with maps")
            }),
    );
    fs.add_game_dir("id1");
    if let Some(g) = args.get(4) {
        fs.add_game_dir(g);
    }
    let pal =
        qformats::image::parse_palette(&fs.open("gfx/palette.lmp").expect("palette")).unwrap();
    let bsp = Bsp::parse(&fs.open(&format!("maps/{map}.bsp")).expect("map")).unwrap();
    let lit = fs
        .open(&format!("maps/{map}.lit"))
        .and_then(|d| qformats::bsp::parse_lit(&d).ok());
    let mut r = Renderer::headless(w, h, &pal).expect("renderer");
    eprintln!("device: {}", r.device_name());
    let t0 = std::time::Instant::now();
    let subs = r.load_world(&bsp, lit.as_deref()).unwrap();
    eprintln!("world loaded in {:?}", t0.elapsed());
    let ents = parse_entities(&bsp.entities).unwrap();
    let mut fog = Fog::default();
    let mut view = View {
        fov_x: 90.0,
        ..Default::default()
    };
    let mut rents = Vec::new();
    let mut models = std::collections::HashMap::new();
    for e in &ents {
        match e.classname() {
            Some("worldspawn") => {
                if let Some(f) = e.get("fog") {
                    fog = parse_fog(f);
                }
                if let Some(sky) = e.get("sky").or(e.get("skyname"))
                    && let Some(faces) = load_skybox(&fs, sky)
                {
                    r.set_skybox(Some(&faces)).unwrap();
                }
            }
            Some("info_player_start") => {
                view.origin = vec3(e.get("origin").unwrap_or(""));
                view.origin[2] += 22.0;
                view.angles = [
                    0.0,
                    e.get("angle").and_then(|a| a.parse().ok()).unwrap_or(0.0),
                    0.0,
                ];
            }
            _ => {}
        }
        let mdl = match e.classname() {
            Some("light_torch_small_walltorch") => Some("progs/flame.mdl"),
            Some("light_flame_large_yellow") | Some("light_flame_small_yellow") => {
                Some("progs/flame2.mdl")
            }
            Some(c) if c.starts_with("item_armor") => Some("progs/armor.mdl"),
            _ => None,
        };
        let origin = vec3(e.get("origin").unwrap_or(""));
        if let Some(m) = e
            .get("model")
            .and_then(|m| m.strip_prefix('*'))
            .and_then(|n| n.parse::<usize>().ok())
            && !e.classname().is_some_and(|c| c.starts_with("trigger_"))
            && let Some(id) = subs.get(m.wrapping_sub(1))
        {
            rents.push(RenderEntity {
                model: *id,
                origin,
                ..Default::default()
            });
        }
        if let Some(name) = mdl {
            let id = *models.entry(name).or_insert_with(|| {
                let m = Mdl::parse(&fs.open(name).unwrap()).unwrap();
                r.load_alias(&m).unwrap()
            });
            rents.push(RenderEntity {
                model: id,
                origin,
                fullbright: name.contains("flame"),
                ..Default::default()
            });
        }
    }
    // a player in front of the camera, a sprite, and particles
    let (f, _, _) = qrender_fwd(view.angles);
    let at = |d: f32| {
        [
            view.origin[0] + f[0] * d,
            view.origin[1] + f[1] * d,
            view.origin[2] + f[2] * d - 22.0,
        ]
    };
    if let Some(d) = fs.open("progs/player.mdl") {
        let id = r.load_alias(&Mdl::parse(&d).unwrap()).unwrap();
        let mut a = view.angles;
        a[1] += 180.0;
        rents.push(RenderEntity {
            model: id,
            origin: at(96.0),
            angles: a,
            colormap: Some([4, 12]),
            player: true,
            ..Default::default()
        });
    }
    if let Some(d) = fs.open("progs/s_explod.spr") {
        let id = r.load_sprite(&Spr::parse(&d).unwrap()).unwrap();
        let mut o = at(160.0);
        o[2] += 40.0;
        rents.push(RenderEntity {
            model: id,
            origin: o,
            frame: 2,
            ..Default::default()
        });
    }
    if let Some(d) = fs.open("progs/v_shot.mdl") {
        let id = r.load_alias(&Mdl::parse(&d).unwrap()).unwrap();
        rents.push(RenderEntity {
            model: id,
            origin: view.origin,
            angles: view.angles,
            viewmodel: true,
            ..Default::default()
        });
    }
    let mut particles = Vec::new();
    for i in 0..200 {
        let a = i as f32 * 0.31;
        let mut o = at(128.0);
        o[0] += a.cos() * 20.0;
        o[1] += a.sin() * 20.0;
        o[2] += 30.0 + (i % 20) as f32;
        particles.push(Particle {
            origin: o,
            color: 0xe0 + (i % 8) as u8,
            alpha: 1.0,
        });
    }
    let mut draw2d = Vec::new();
    if let Some(wad) = fs.open("gfx.wad").and_then(|d| Wad::parse(&d).ok()) {
        if let Some(c) = wad.get("conchars") {
            r.set_conchars(&c.data[..128 * 128.min(c.data.len() / 128)])
                .unwrap();
            for (i, ch) in format!("qrender {map} {}", r.device_name())
                .bytes()
                .enumerate()
            {
                draw2d.push(Draw2D::Char {
                    x: 8.0 + i as f32 * 16.0,
                    y: 8.0,
                    size: 16.0,
                    ch,
                    color: [1.0; 4],
                });
            }
        }
        if let Some(sb) = wad
            .get("sbar")
            .and_then(|l| qformats::image::parse_pic(&l.data).ok())
        {
            let id = r.register_pic(&sb).unwrap();
            let (pw, ph) = (sb.width as f32 * 2.0, sb.height as f32 * 2.0);
            draw2d.push(Draw2D::Pic {
                x: (w as f32 - pw) / 2.0,
                y: h as f32 - ph,
                w: pw,
                h: ph,
                pic: id,
                uv: [0.0, 0.0, 1.0, 1.0],
                color: [1.0; 4],
            });
        }
    }
    draw2d.push(Draw2D::Fill {
        x: 8.0,
        y: 30.0,
        w: 100.0,
        h: 4.0,
        color: [1.0, 0.5, 0.0, 0.8],
    });
    let styles: Vec<i32> = (0..64).map(|_| 264).collect();
    let dl = [DLight {
        origin: at(64.0),
        radius: 200.0,
        minlight: 0.0,
        color: [1.0, 0.6, 0.3],
    }];
    let frame = RenderFrame {
        time: 1.0,
        view: Some(view),
        entities: &rents,
        particles: &particles,
        dlights: &dl,
        lightstyles: &styles,
        fog,
        draw2d: &draw2d,
        settings: Settings {
            wateralpha: 0.5,
            ..Default::default()
        },
        ..Default::default()
    };
    let t1 = std::time::Instant::now();
    let n = 60;
    for _ in 0..n {
        r.render(&frame).unwrap();
    }
    let px = r.read_pixels().unwrap();
    eprintln!("{n} frames in {:?}", t1.elapsed());
    let out = format!(
        "{}/../../target/qrender-{map}.ppm",
        env!("CARGO_MANIFEST_DIR")
    );
    let mut ppm = format!("P6\n{w} {h}\n255\n").into_bytes();
    for p in px.as_chunks::<4>().0 {
        ppm.extend_from_slice(&p[..3]);
    }
    std::fs::write(&out, ppm).unwrap();
    eprintln!("wrote {out}");
}

fn qrender_fwd(a: [f32; 3]) -> ([f32; 3], (), ()) {
    let (sy, cy) = a[1].to_radians().sin_cos();
    let (sp, cp) = a[0].to_radians().sin_cos();
    ([cp * cy, cp * sy, -sp], (), ())
}
