//! Malformed-input robustness and synthetic format tests.

use qformats::{Bsp, Md3, Mdl, Spr, Wad, Wav, bsp, entities, image};

fn all_parsers(d: &[u8]) {
    let _ = Bsp::parse(d);
    let _ = Md3::parse(d);
    let _ = Mdl::parse(d);
    let _ = Spr::parse(d);
    let _ = Wad::parse(d);
    let _ = Wav::parse(d);
    let _ = image::parse_pic(d);
    let _ = image::parse_palette(d);
    let _ = image::parse_colormap(d);
    let _ = image::parse_pcx(d);
    let _ = image::parse_tga(d);
    let _ = bsp::parse_lit(d);
    let _ = entities::parse_entities(&String::from_utf8_lossy(d));
    let _ = bsp::decompress_vis(d, 0, 4000);
}

/// Truncate at many lengths and corrupt bytes with a deterministic PRNG.
fn fuzz(d: &[u8]) {
    let step = (d.len() / 300).max(1);
    for len in (0..d.len()).step_by(step) {
        all_parsers(&d[..len]);
    }
    let mut s: u64 = 0x2545_f491_4f6c_dd1d;
    for _ in 0..200 {
        let mut c = d.to_vec();
        for _ in 0..8 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            if !c.is_empty() {
                let i = (s as usize) % c.len();
                c[i] = (s >> 32) as u8;
            }
        }
        all_parsers(&c);
    }
}

fn wav_bytes() -> Vec<u8> {
    let mut fmt = Vec::new();
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(11025u32.to_le_bytes());
    fmt.extend(11025u32.to_le_bytes());
    fmt.extend(1u16.to_le_bytes());
    fmt.extend(8u16.to_le_bytes());
    let mut cue = vec![0u8; 24];
    cue.extend(5u32.to_le_bytes()); // sample offset at body+24 (chunk+32)
    let mut list = vec![0u8; 16];
    list.extend(10u32.to_le_bytes()); // chunk+24
    list.extend(b"mark");
    let mut body = b"WAVE".to_vec();
    for (n, c) in [
        (b"fmt ", fmt),
        (b"cue ", cue),
        (b"LIST", list),
        (b"data", vec![128u8; 20]),
    ] {
        body.extend(n);
        body.extend((c.len() as u32).to_le_bytes());
        body.extend(c);
    }
    let mut out = b"RIFF".to_vec();
    out.extend((body.len() as u32).to_le_bytes());
    out.extend(body);
    out
}

#[test]
fn wav_loop_markers() {
    let w = Wav::parse(&wav_bytes()).unwrap();
    assert_eq!((w.rate, w.width, w.channels), (11025, 1, 1));
    assert_eq!(w.loop_start, Some(5));
    assert_eq!(w.samples, 15);
    fuzz(&wav_bytes());
}

#[test]
fn tga_and_pcx() {
    // 2x2 RLE truecolor 24-bit, bottom-up.
    let mut t = vec![0, 0, 10, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 2, 0, 24, 0];
    t.extend([0x83, 1, 2, 3]);
    let img = image::parse_tga(&t).unwrap();
    assert_eq!(img.pixels[..4], [3, 2, 1, 255]);
    fuzz(&t);
    // 2x1 PCX
    let mut p = vec![0u8; 128];
    p[0] = 0x0a;
    p[2] = 1;
    p[3] = 8;
    p[8] = 1; // xmax
    p[66] = 2;
    p.extend([0xc2, 7]);
    p.push(0x0c);
    p.extend(vec![9u8; 768]);
    let (pic, pal) = image::parse_pcx(&p).unwrap();
    assert_eq!(
        (pic.width, pic.height, pic.pixels.clone()),
        (2, 1, vec![7, 7])
    );
    assert_eq!(pal[0], [9, 9, 9]);
    fuzz(&p);
}

#[test]
fn entities_and_vis() {
    let e = entities::parse_entities(
        "// c\n{\n\"classname\" \"worldspawn\"\n\"wad\" \"a{b}\"\n}\n{ \"x\" \"1\" }",
    )
    .unwrap();
    assert_eq!(e.len(), 2);
    assert_eq!(e[0].get("wad"), Some("a{b}"));
    assert!(entities::parse_entities("{ \"a\" ").is_err());
    assert_eq!(
        bsp::decompress_vis(&[0xff, 0, 2, 0x01], 0, 32),
        vec![0xff, 0, 0, 1]
    );
    assert_eq!(bsp::decompress_vis(&[], -1, 9), vec![0xff, 0xff]);
}

#[test]
fn spr32() {
    let mut d = b"IDSP".to_vec();
    for v in [32i32, 2, 0, 1, 1, 1, 0, 0, 0, 0, 0, 1, 1] {
        d.extend(v.to_le_bytes());
    }
    d.extend([1, 2, 3, 4]);
    let s = Spr::parse(&d).unwrap();
    assert!(s.is_rgba());
    assert_eq!(s.frames[0].images[0].pixels, [1, 2, 3, 4]);
    fuzz(&d);
}

#[test]
fn fuzz_real_data() {
    let base = std::env::var("QUAKE_BASEDIR").unwrap_or_else(|_| {
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../vkQuake directory with maps"
        )
        .into()
    });
    let Ok(pak) = std::fs::read(format!("{base}/id1/pak0.pak")) else {
        return;
    };
    let le = |o: usize| u32::from_le_bytes(pak[o..o + 4].try_into().unwrap()) as usize;
    let (ofs, len) = (le(4), le(8));
    let want = [
        "maps/e1m1.bsp",
        "progs/player.mdl",
        "progs/flame2.mdl",
        "progs/s_explod.spr",
        "gfx.wad",
        "sound/ambience/fire1.wav",
        "gfx/conback.lmp",
    ];
    let mut n = 0;
    for i in 0..len / 64 {
        let e = ofs + i * 64;
        let name = qformats::cstr(&pak[e..e + 56]);
        if want.contains(&name.as_str()) {
            fuzz(&pak[le(e + 56)..le(e + 56) + le(e + 60)]);
            n += 1;
        }
    }
    assert_eq!(n, want.len());
}

/// Minimal BSP with only a texture lump and a lighting lump.
fn synth_bsp(version: i32, textures: &[u8], lighting: &[u8]) -> Vec<u8> {
    let mut d = version.to_le_bytes().to_vec();
    let hdr = 4 + 15 * 8;
    let tex_ofs = hdr;
    let light_ofs = hdr + textures.len();
    for i in 0..15 {
        let (o, l) = match i {
            bsp::LUMP_TEXTURES => (tex_ofs, textures.len()),
            bsp::LUMP_LIGHTING => (light_ofs, lighting.len()),
            _ => (hdr, 0),
        };
        d.extend_from_slice(&(o as i32).to_le_bytes());
        d.extend_from_slice(&(l as i32).to_le_bytes());
    }
    d.extend_from_slice(textures);
    d.extend_from_slice(lighting);
    d
}

fn miptex_lump(name: &str, w: u32, h: u32, shift: Option<u32>, offsets: [u32; 4], px: &[u8]) -> Vec<u8> {
    let mut l = 1i32.to_le_bytes().to_vec();
    l.extend_from_slice(&8i32.to_le_bytes());
    let mut n = [0u8; 16];
    n[..name.len()].copy_from_slice(name.as_bytes());
    l.extend_from_slice(&n);
    l.extend_from_slice(&w.to_le_bytes());
    l.extend_from_slice(&h.to_le_bytes());
    if let Some(s) = shift {
        l.extend_from_slice(&s.to_le_bytes());
    }
    for o in offsets {
        l.extend_from_slice(&o.to_le_bytes());
    }
    l.extend_from_slice(px);
    l
}

#[test]
fn miptex_past_lump_end_is_clamped() {
    // 16x16 texture claiming 4 mips but only 100 texels present.
    let px: Vec<u8> = (0..100).map(|i| i as u8 + 1).collect();
    let lump = miptex_lump("broken", 16, 16, None, [40, 296, 360, 376], &px);
    let b = Bsp::parse(&synth_bsp(29, &lump, &[])).unwrap();
    let t = b.textures[0].as_ref().unwrap();
    assert_eq!(t.mips.len(), 4);
    assert_eq!(t.mips[0].len(), 256);
    assert_eq!(&t.mips[0][..100], &px[..]);
    assert!(t.mips[0][100..].iter().all(|&p| p == 0));
    // Derived by point sampling: mip1 texel (1,1) = mip0 texel (2,2).
    assert_eq!(t.mips[1].len(), 64);
    assert_eq!(t.mips[1][8 + 1], t.mips[0][2 * 16 + 2]);
    assert_eq!(t.mips[3].len(), 4);
}

#[test]
fn quake64_bsp() {
    let px: Vec<u8> = (0..64).map(|i| i as u8).collect();
    // miptex64 header is 44 bytes; all offsets point at mip 0.
    let lump = miptex_lump("wall", 8, 8, Some(2), [44; 4], &px);
    // One 16-bit sample: R=0b11111 G=0b00000 B=0b111111 -> f8 00 fc
    let light = [0b1111_1000, 0b0011_1111];
    let b = Bsp::parse(&synth_bsp(bsp::BSPVERSION_QUAKE64, &lump, &light)).unwrap();
    assert_eq!(b.version, Some(bsp::BspVersion::Quake64));
    let t = b.textures[0].as_ref().unwrap();
    assert_eq!(t.shift, 2);
    assert_eq!(t.mips[0], px);
    assert_eq!(t.mips[1][0], px[0]);
    assert_eq!(t.mips[1][1], px[2]);
    assert_eq!(b.rgb_lighting.as_deref(), Some(&[0xf8, 0x00, 0xfc][..]));
    assert_eq!(b.lighting.len(), 1);
}

#[test]
fn md3_synth_and_skin() {
    let mut d = Vec::new();
    let pad = |buf: &mut Vec<u8>, s: &str, len: usize| {
        let mut b = vec![0u8; len];
        let n = s.len().min(len);
        b[..n].copy_from_slice(&s.as_bytes()[..n]);
        buf.extend_from_slice(&b);
    };
    let ofs_frames = 108i32;
    let ofs_surfaces = ofs_frames + 56;
    let surf_hdr = 108;
    let ofs_tris = surf_hdr;
    let ofs_shaders = ofs_tris + 12;
    let ofs_st = ofs_shaders + 68;
    let ofs_xyz = ofs_st + 3 * 8;
    let surf_end = ofs_xyz + 3 * 8;
    d.extend_from_slice(b"IDP3");
    d.extend_from_slice(&15i32.to_le_bytes());
    pad(&mut d, "test.md3", 64);
    d.extend_from_slice(&0i32.to_le_bytes()); // flags
    d.extend_from_slice(&1i32.to_le_bytes()); // num_frames
    d.extend_from_slice(&0i32.to_le_bytes()); // num_tags
    d.extend_from_slice(&1i32.to_le_bytes()); // num_surfaces
    d.extend_from_slice(&0i32.to_le_bytes()); // num_skins
    d.extend_from_slice(&ofs_frames.to_le_bytes());
    d.extend_from_slice(&ofs_frames.to_le_bytes());
    d.extend_from_slice(&ofs_surfaces.to_le_bytes());
    d.extend_from_slice(&(ofs_surfaces + surf_end).to_le_bytes());
    // Frame 0
    for f in [-10.0f32, -10.0, -10.0, 10.0, 10.0, 10.0, 0.0, 0.0, 0.0, 17.32] {
        d.extend_from_slice(&f.to_le_bytes());
    }
    pad(&mut d, "idle0", 16);
    // Surface 0
    d.extend_from_slice(b"IDP3");
    pad(&mut d, "body", 64);
    for v in [0i32, 1, 1, 3, 1, ofs_tris, ofs_shaders, ofs_st, ofs_xyz, surf_end] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    for v in [0i32, 1, 2] {
        d.extend_from_slice(&v.to_le_bytes());
    }
    pad(&mut d, "progs/body.tga", 64);
    d.extend_from_slice(&0i32.to_le_bytes());
    for uv in [0.0f32, 0.0, 1.0, 0.0, 0.0, 1.0] {
        d.extend_from_slice(&uv.to_le_bytes());
    }
    for (x, y, z, lat, lng) in [(0i16, 0, 0, 0u8, 0u8), (64, 0, 0, 64, 0), (0, 64, 0, 0, 64)] {
        for c in [x, y, z] {
            d.extend_from_slice(&c.to_le_bytes());
        }
        d.extend_from_slice(&[lat, lng]);
    }
    let m = Md3::parse(&d).unwrap();
    assert_eq!(m.surfaces.len(), 1);
    assert_eq!(m.surfaces[0].name, "body");
    assert_eq!(m.surfaces[0].verts[1].xyz, [64, 0, 0]);
    assert_eq!(m.surfaces[0].shaders, vec!["progs/body.tga"]);
    fuzz(&d);

    let skin = "tag_head,\nbody,progs/body.tga\n";
    assert!(qformats::md3::parse_skin(skin, &["body"]).is_none());
    let valid = "header\nbody,progs/body.tga\n";
    assert_eq!(
        qformats::md3::parse_skin(valid, &["body"]).as_deref(),
        Some(&[("body".to_string(), "progs/body.tga".to_string())][..])
    );
}
