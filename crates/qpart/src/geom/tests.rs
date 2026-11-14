use super::*;

use crate::system::{EmptyWorld, FrameInput, System};

fn files<'a>(m: &'a [(&'a str, Vec<u8>)]) -> impl FnMut(&str) -> Option<Vec<u8>> + 'a {
    move |p: &str| m.iter().find(|(k, _)| *k == p).map(|(_, v)| v.clone())
}

fn tga(w: u16, h: u16) -> Vec<u8> {
    let mut d = vec![0u8; 18];
    d[2] = 2;
    d[12..14].copy_from_slice(&w.to_le_bytes());
    d[14..16].copy_from_slice(&h.to_le_bytes());
    d[16] = 32;
    d[17] = 0x28;
    d.extend(std::iter::repeat_n([10u8, 20, 30, 128], w as usize * h as usize).flatten());
    d
}

#[test]
fn textures_resolve_with_fallbacks() {
    let fx = "r_part a\n{\ntexture particles/fteparticlefont\ntcoords 0 0 64 64 256\nscale 4\n}\n\
              r_part b\n{\ntexture ball\ntcoords 0 0 64 64 256\nscale 4\n}\n\
              r_part c\n{\ntype spark\n}\n\
              r_part d\n{\ntexture nothing\nscale 1\nblend premul_add\n}\n";
    let data = vec![("particles/x.cfg", fx.as_bytes().to_vec()), ("textures/particles/fteparticlefont.tga", tga(2, 2))];
    let mut f = files(&data);
    let mut r = Registry::new();
    r.load_set("x", false, true, &mut f);
    let tex = |r: &Registry, n: &str| r.textures[r.types.iter().find(|t| t.name == n).unwrap().tex.unwrap()].clone();
    assert_eq!(
        tex(&r, "a"),
        TexRef::File { path: "textures/particles/fteparticlefont.tga".into(), premul: false, nearest: false }
    );
    assert_eq!(r.types[0].s2, 0.25);
    assert_eq!(tex(&r, "b"), TexRef::Builtin(BuiltinTex::Ball));
    // Fallback resets the coords to the whole texture.
    assert_eq!(r.types[1].s2, 1.0);
    assert_eq!(tex(&r, "c"), TexRef::Builtin(BuiltinTex::White));
    assert_eq!(tex(&r, "d"), TexRef::Builtin(BuiltinTex::Fuzzy));
    let img = load_image(&mut f, "textures/particles/fteparticlefont.tga", None, true).unwrap();
    assert_eq!((img.width, img.height), (2, 2));
    assert_eq!(&img.rgba[..4], &[15, 10, 5, 128]);
    for b in [BuiltinTex::White, BuiltinTex::Beam, BuiltinTex::PTri, BuiltinTex::Ball, BuiltinTex::Fuzzy, BuiltinTex::Classic] {
        let i = builtin_image(b);
        assert_eq!(i.rgba.len(), (i.width * i.height * 4) as usize);
    }
    // Ball: opaque centre, transparent corner.
    let ball = builtin_image(BuiltinTex::Ball);
    assert_eq!(ball.rgba[(32 * 64 + 32) * 4 + 3], 255);
    assert_eq!(ball.rgba[3], 0);
}

#[test]
fn png_and_jpg_textures() {
    // Grey+alpha PNG, expanded to RGBA.
    let mut png_bytes = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, 2, 1);
        enc.set_color(png::ColorType::GrayscaleAlpha);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&[100, 255, 50, 0]).unwrap();
    }
    let jpg = include_bytes!("../../testdata/red8.jpg").to_vec();
    let mut files = |p: &str| match p {
        "textures/a.png" => Some(png_bytes.clone()),
        "textures/b.jpg" => Some(jpg.clone()),
        _ => None,
    };
    let img = load_image(&mut files, "textures/a.png", None, false).unwrap();
    assert_eq!((img.width, img.height), (2, 1));
    assert_eq!(img.rgba, [100, 100, 100, 255, 50, 50, 50, 0]);
    let img = load_image(&mut files, "textures/b.jpg", None, false).unwrap();
    assert_eq!((img.width, img.height), (8, 8));
    let px = &img.rgba[..4];
    assert!(px[0] > 180 && px[1] < 70 && px[2] < 70 && px[3] == 255, "{px:?}");
    assert_eq!(find_image(&mut files, "textures/b").as_deref(), Some("textures/b.jpg"));
}

#[test]
fn builds_batches_per_kind() {
    let fx = "r_part q\n{\ntexture ball\nscale 8\ncount 4\ndie 1\nalpha 1\nrgbf 1 0 0\n}\n\
              r_part s\n{\ntype spark\ncount 3\ndie 1\nvelbias 0 0 100\n}\n\
              r_part t\n{\ntexture ball\ntype texturedspark\nscale 2\ncount 2\ndie 1\nvelbias 100 0 0\nblend add\n}\n";
    let data = vec![("particles/x.cfg", fx.as_bytes().to_vec())];
    let mut f = files(&data);
    let mut r = Registry::new();
    r.load_set("x", false, true, &mut f);
    let mut s = System::default();
    let id = |n: &str| r.types.iter().position(|t| t.name == n).unwrap();
    for n in ["q", "s", "t"] {
        s.run_effect(&r, &EmptyWorld, Vec3::new(100.0, 0.0, 0.0), None, 1.0, id(n), None);
    }
    let view = ViewInfo {
        origin: Vec3::ZERO,
        forward: Vec3::new(1.0, 0.0, 0.0),
        right: Vec3::new(0.0, -1.0, 0.0),
        up: Vec3::new(0.0, 0.0, 1.0),
    };
    let eo = |_| None;
    let mut g = Geometry::new(&r, view, 0.0, &eo);
    s.frame(&r, &EmptyWorld, &FrameInput { cl_time: 0.01, ..Default::default() }, &mut g);
    assert_eq!(g.batches.len(), 3);
    let by = |lines: bool, blend: BlendMode| g.batches.iter().find(|b| b.lines == lines && b.blend == blend).unwrap();
    assert_eq!(by(false, BlendMode::Blend).num_indices, 4 * 6);
    assert_eq!(by(true, BlendMode::Blend).num_indices, 3 * 2);
    assert_eq!(by(false, BlendMode::AddA).num_indices, 2 * 6);
    assert_eq!(g.vertices.len(), 4 * 4 + 3 * 2 + 2 * 4);
    assert!(g.indices.iter().all(|&i| (i as usize) < g.vertices.len()));
    // Textured quad: scalefactor 0 -> distance scaling; red, opaque.
    let v = g.vertices[g.indices[by(false, BlendMode::Blend).first_index as usize] as usize];
    // (alpha has faded for 0.01 s)
    assert_eq!(&v.color[..3], &[255, 0, 0]);
    assert!(v.color[3] > 250);
    // Draw order: alpha before additive.
    let o = g.ordered_batches();
    assert!(blend_pass(o[0].blend) <= blend_pass(o[2].blend));
}

#[test]
fn behind_eye_culling_keeps_front_and_boundary_quads() {
    let mut r = Registry::new();
    let mut f = files(&[]);
    r.load_set("classic", false, true, &mut f);
    // Use a resolved normal billboard, independent of any game data.
    let mut ty = crate::types::PartType::empty("test", "billboard");
    ty.looks.kind = RenderType::Normal;
    ty.looks.scalefactor = 1.0;
    ty.tex = Some(0);
    r.types.push(ty);
    let id = r.types.len() - 1;
    let view = ViewInfo {
        origin: Vec3::ZERO, forward: Vec3::new(1.0, 0.0, 0.0),
        right: Vec3::new(0.0, -1.0, 0.0), up: Vec3::new(0.0, 0.0, 1.0),
    };
    let eo = |_| None;
    for angle in [0.0, 0.7] {
        for (depth, kept) in [(-100.0, false), (-1.0, true), (0.0, true), (100.0, true)] {
            let mut p = Particle::default();
            p.org = Vec3::new(depth, 0.0, 0.0);
            p.scale = 8.0;
            p.angle = angle;
            p.rgba = [1.0; 4];
            let mut reference = Geometry::new(&r, view, 0.0, &eo);
            reference.particle(id, &p);
            let mut culled = Geometry::new(&r, view, 0.0, &eo);
            culled.cull_behind = true;
            culled.particle(id, &p);
            if kept {
                assert_eq!(reference.indices, culled.indices);
                for (a, b) in reference.vertices.iter().zip(&culled.vertices) {
                    assert_eq!(a.pos, b.pos);
                    assert_eq!(a.color, b.color);
                    assert_eq!(a.uv, b.uv);
                }
                assert_eq!(culled.vertices.len(), 4);
            } else {
                assert!(culled.vertices.is_empty());
                assert!(culled.indices.is_empty());
                assert!(culled.batches.is_empty());
            }
        }
    }
}
