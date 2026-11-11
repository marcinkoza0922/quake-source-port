//! Malformed-input robustness and synthetic format tests.

use qformats::{Bsp, Mdl, Spr, Wad, Wav, bsp, entities, image};

fn all_parsers(d: &[u8]) {
    let _ = Bsp::parse(d);
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
