//! Every other binary/text asset parser; the first byte picks one.
#![no_main]
use libfuzzer_sys::fuzz_target;
use qformats::*;

fuzz_target!(|data: &[u8]| {
    let Some((&k, d)) = data.split_first() else { return };
    let text = || String::from_utf8_lossy(d);
    match k {
        b'0' => drop(Mdl::parse(d)),
        b'1' => drop(Md3::parse(d)),
        b'2' => drop(Iqm::parse(d)),
        b'3' => drop(Spr::parse(d)),
        b'4' => drop(Wad::parse(d)),
        b'5' => {
            for r in [11025, 44100, 48000] {
                drop(qsound::load_sfx("x", d, r));
            }
        }
        b'6' => drop(image::parse_pic(d)),
        b'7' => drop(image::parse_pcx(d)),
        b'8' => drop(image::parse_tga(d)),
        b'9' => {
            let mut p = d.splitn(2, |&b| b == 0);
            let (mesh, anim) = (p.next().unwrap_or_default(), p.next());
            drop(md5::Md5::parse(mesh, anim));
        }
        b'a' => drop(bsp::parse_lit_any(d)),
        b'b' => drop(bsp::parse_dlit(d)),
        b'c' => drop(bspx::parse_lightgrid_octree(d)),
        b'd' => drop(bspx::parse_brushlist(d)),
        b'e' => drop(bspx::parse_decoupled_lm(d, 16)),
        b'f' => drop(md3::parse_skin(&text(), &["a", "b"])),
        b'g' => drop(parse_entities(&text())),
        b'h' => drop((image::parse_palette(d), image::parse_colormap(d))),
        b'i' => drop(bsp::decompress_vis(d, 0, 4096)),
        _ => {}
    }
});
