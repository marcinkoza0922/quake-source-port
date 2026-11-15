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

#[test]
fn vertical_culling_keeps_intersecting_quads_at_varied_fovs() {
    let mut r = Registry::new();
    let mut ty = crate::types::PartType::empty("test", "billboard");
    ty.looks.kind = RenderType::Normal;
    ty.looks.scalefactor = 1.0;
    ty.tex = Some(0);
    r.types.push(ty);
    let view = ViewInfo {
        origin: Vec3::ZERO, forward: Vec3::new(1.0, 0.0, 0.0),
        right: Vec3::new(0.0, -1.0, 0.0), up: Vec3::new(0.0, 0.0, 1.0),
    };
    let eo = |_| None;
    for tan in [0.1, 0.75, 4.0] {
        for sign in [-1.0, 1.0] {
            for angle in [0.0, 0.7] {
                for (offset, kept) in [(0.0, true), (1.0, true), (1000.0, false)] {
                    let mut p = Particle::default();
                    p.org = Vec3::new(100.0, 0.0, sign * (100.0 * tan + offset));
                    p.scale = 8.0;
                    p.angle = angle;
                    p.rgba = [1.0; 4];
                    let mut g = Geometry::new(&r, view, 0.0, &eo);
                    g.cull_behind = true;
                    g.vertical_tan = Some(tan);
                    g.particle(0, &p);
                    assert_eq!(!g.vertices.is_empty(), kept);
                    // Disabling camera culling must keep the shared geometry.
                    let mut shared = Geometry::new(&r, view, 0.0, &eo);
                    shared.vertical_tan = Some(tan);
                    shared.particle(0, &p);
                    assert_eq!(shared.vertices.len(), 4);
                    if kept {
                        assert_eq!(g.indices, shared.indices);
                        for (a, b) in g.vertices.iter().zip(&shared.vertices) {
                            assert_eq!((a.pos, a.uv, a.color), (b.pos, b.uv, b.color));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn horizontal_culling_keeps_edges_for_rotated_cameras_and_quads() {
    let mut r = Registry::new();
    let mut ty = crate::types::PartType::empty("test", "billboard");
    ty.looks.kind = RenderType::Normal;
    ty.looks.scalefactor = 1.0;
    ty.tex = Some(0);
    r.types.push(ty);
    let eo = |_| None;
    for angles in [Vec3::ZERO, Vec3::new(35.0, 123.0, 42.0)] {
        let (forward, right, up) = qcore::math::angle_vectors(angles);
        let view = ViewInfo { origin: Vec3::new(100.0, -50.0, 30.0), forward, right, up };
        for tan in [0.1, 0.75, 1.33, 4.0] {
            for sign in [-1.0, 1.0] {
                for angle in [0.0, 0.7] {
                    for (offset, kept) in [(0.0, true), (1.0, true), (1000.0, false)] {
                        let mut p = Particle::default();
                        p.org = view.origin + forward * 100.0 + right * (sign * (100.0 * tan + offset));
                        p.scale = 8.0;
                        p.angle = angle;
                        p.rgba = [1.0; 4];
                        let mut g = Geometry::new(&r, view, 0.0, &eo);
                        g.cull_behind = true;
                        g.horizontal_tan = Some(tan);
                        g.vertical_tan = Some(0.75);
                        g.particle(0, &p);
                        assert_eq!(!g.vertices.is_empty(), kept);
                        let mut shared = Geometry::new(&r, view, 0.0, &eo);
                        shared.horizontal_tan = Some(tan);
                        shared.vertical_tan = Some(0.75);
                        shared.particle(0, &p);
                        assert_eq!(shared.vertices.len(), 4);
                        if kept {
                            assert_eq!(g.indices, shared.indices);
                            for (a, b) in g.vertices.iter().zip(&shared.vertices) {
                                assert_eq!((a.pos, a.uv, a.color), (b.pos, b.uv, b.color));
                            }
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn recycled_geometry_matches_fresh_after_empty_and_smaller_frames() {
    let mut r = Registry::new();
    for blend in [BlendMode::AddA, BlendMode::Blend, BlendMode::Subtract] {
        let mut ty = crate::types::PartType::empty("test", "recycle");
        ty.tex = Some(0);
        ty.looks.kind = RenderType::Normal;
        ty.looks.blendmode = blend;
        r.types.push(ty);
    }
    let view = ViewInfo {
        forward: Vec3::new(1.0, 0.0, 0.0),
        right: Vec3::new(0.0, -1.0, 0.0),
        up: Vec3::new(0.0, 0.0, 1.0),
        ..Default::default()
    };
    let eo = |_| None;
    let mut buffers = (Vec::with_capacity(256), Vec::with_capacity(512), Vec::with_capacity(64));
    let addresses = (buffers.0.as_ptr(), buffers.1.as_ptr(), buffers.2.as_ptr());
    // Empty frames and changing type/count order must not retain stale batches
    // or offset the next frame's indices. Stable pass ordering must also match.
    for types in [&[0, 1, 2, 0][..], &[], &[2], &[1, 0, 1]] {
        let mut reused = Geometry::new(&r, view, 0.0, &eo);
        reused.reuse_buffers(buffers.0, buffers.1, buffers.2);
        let mut fresh = Geometry::new(&r, view, 0.0, &eo);
        for &ty in types {
            let mut p = Particle::default();
            p.org = Vec3::new(100.0, ty as f32, 0.0);
            p.scale = 8.0;
            p.rgba = [1.0, 0.5, 0.25, 1.0];
            reused.particle(ty, &p);
            fresh.particle(ty, &p);
        }
        reused.batches.sort_by_key(|b| blend_pass(b.blend));
        assert_eq!(reused.vertices, fresh.vertices);
        assert_eq!(reused.indices, fresh.indices);
        assert_eq!(reused.batches, fresh.ordered_batches());
        assert_eq!((reused.vertices.as_ptr(), reused.indices.as_ptr(), reused.batches.as_ptr()), addresses);
        buffers = (reused.vertices, reused.indices, reused.batches);
    }
}

#[test]
fn particles_rev_matches_reverse_single_particle_calls() {
    let kinds = [
        RenderType::Normal,
        RenderType::UDecal,
        RenderType::Spark,
        RenderType::SparkFan,
        RenderType::TexturedSpark,
    ];
    let mut r = Registry::new();
    r.textures.push(TexRef::Builtin(BuiltinTex::Fuzzy));
    r.textures.push(TexRef::Builtin(BuiltinTex::White));
    for (i, &kind) in kinds.iter().enumerate() {
        let mut ty = crate::types::PartType::empty("test", &format!("k{i}"));
        ty.tex = Some(0);
        ty.looks.kind = kind;
        ty.looks.premul = (i % 3) as u8;
        ty.looks.scalefactor = if i == 0 { 0.5 } else { 1.0 };
        ty.looks.invscalefactor = if i == 0 { 0.02 } else { 0.0 };
        ty.looks.blendmode = if i % 2 == 0 { BlendMode::Blend } else { BlendMode::AddA };
        r.types.push(ty);
    }
    let view = ViewInfo {
        origin: Vec3::new(10.0, 20.0, 30.0),
        forward: Vec3::new(1.0, 0.0, 0.0),
        right: Vec3::new(0.0, -1.0, 0.0),
        up: Vec3::new(0.0, 0.0, 1.0),
    };
    let mk = |org, vel, rgba, scale, angle, uv: [f32; 4]| {
        let mut p = Particle::default();
        p.org = org;
        p.vel = vel;
        p.rgba = rgba;
        p.scale = scale;
        p.angle = angle;
        p.s1 = uv[0];
        p.t1 = uv[1];
        p.s2 = uv[2];
        p.t2 = uv[3];
        p
    };
    let particles = [
        mk(Vec3::new(100.0, 10.0, 35.0), Vec3::new(50.0, -10.0, 5.0), [0.9, 0.4, 0.2, 0.8], 12.0, 0.0, [0.0, 0.0, 1.0, 1.0]),
        // Behind view -> culled when cull_behind = true
        mk(Vec3::new(-50.0, 20.0, 30.0), Vec3::new(-20.0, 30.0, 10.0), [0.3, 0.8, 0.6, 1.0], 6.0, 1.25, [0.25, 0.0, 0.5, 0.25]),
        // Side culled when horizontal_tan is set
        mk(Vec3::new(30.0, 200.0, 30.0), Vec3::new(10.0, 40.0, -15.0), [1.0, 1.0, 1.0, 0.5], 4.0, 2.7, [0.0, 0.5, 0.5, 1.0]),
    ];
    let eo = |_| None;
    for cull in [false, true] {
        let mut batched = Geometry::new(&r, view, 1.0, &eo);
        let mut single = Geometry::new(&r, view, 1.0, &eo);
        batched.cull_behind = cull;
        single.cull_behind = cull;
        if cull {
            batched.horizontal_tan = Some(1.0);
            batched.vertical_tan = Some(0.75);
            single.horizontal_tan = Some(1.0);
            single.vertical_tan = Some(0.75);
        }
        for ty in 0..kinds.len() {
            batched.particles_rev(ty, &particles);
            for p in particles.iter().rev() {
                single.particle(ty, p);
            }
        }
        assert_eq!(batched.vertices, single.vertices);
        assert_eq!(batched.indices, single.indices);
        assert_eq!(batched.batches, single.batches);
    }
}
