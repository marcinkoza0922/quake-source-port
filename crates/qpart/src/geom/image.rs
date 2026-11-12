use super::*;

/// Builds a builtin texture.
pub fn builtin_image(b: BuiltinTex) -> Image {
    const N: usize = 64;
    let img = |w: usize, h: usize, f: &dyn Fn(usize, usize) -> [u8; 4]| {
        let mut rgba = Vec::with_capacity(w * h * 4);
        for y in 0..h {
            for x in 0..w {
                rgba.extend_from_slice(&f(x, y));
            }
        }
        Image { width: w as u32, height: h as u32, rgba }
    };
    let half = N as f32 * 0.5 - 1.0;
    match b {
        BuiltinTex::White => img(4, 4, &|_, _| [255; 4]),
        BuiltinTex::Beam => img(N, N, &|_, y| {
            let dy = (y as f32 - 0.5 * N as f32) / half;
            let d = (256.0 * (1.0 - dy * dy)).max(0.0);
            [255, 255, 255, d as i32 as u8]
        }),
        BuiltinTex::PTri => img(N, N, &|x, y| {
            let d = (256.0 * (1.0 - (x as f32 / half + y as f32 / half))).max(0.0);
            let v = d as i32 as u8;
            [v, v, v, v / 2]
        }),
        BuiltinTex::Ball => img(N, N, &|x, y| {
            let dx = (x as f32 - 0.5 * N as f32) / half;
            let dy = (y as f32 - 0.5 * N as f32) / half;
            let d = (255.0 * (1.0 - (dx * dx + dy * dy))).max(0.0);
            [255, 255, 255, d as u8]
        }),
        BuiltinTex::Fuzzy => {
            #[rustfmt::skip]
            const EXP: [[u8; 16]; 16] = [
                [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0], [0,0,0,0,1,0,0,0,1,0,0,1,0,0,0,0],
                [0,0,0,1,1,1,1,1,3,1,1,2,1,0,0,0], [0,0,0,1,1,1,1,4,4,4,5,4,2,1,1,0],
                [0,0,1,1,6,5,5,8,6,8,3,6,3,2,1,0], [0,0,1,5,6,7,5,6,8,8,8,3,3,1,0,0],
                [0,0,0,1,6,8,9,9,9,9,4,6,3,1,0,0], [0,0,2,1,7,7,9,9,9,9,5,3,1,0,0,0],
                [0,0,2,4,6,8,9,9,9,9,8,6,1,0,0,0], [0,0,2,2,3,5,6,8,9,8,8,4,4,1,0,0],
                [0,0,1,2,4,1,8,7,8,8,6,5,4,1,0,0], [0,1,1,1,7,8,1,6,7,5,4,7,1,0,0,0],
                [0,1,2,1,1,5,1,3,4,3,1,1,0,0,0,0], [0,0,0,0,0,1,1,1,1,1,0,0,0,0,0,0],
                [0,0,0,0,0,0,0,0,1,0,0,0,0,0,0,0], [0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],
            ];
            img(16, 16, &|x, y| [255, 255, 255, (EXP[x][y] as u32 * 255 / 9) as u8])
        }
        BuiltinTex::Classic => img(N, N, &|x, y| {
            // R_ParticleTextureLookup(x, y, 8), stored column-major in C.
            let (dx, dy) = (y as i32 - 16, x as i32 - 16);
            let r = (dx * dx + dy * dy).min(255);
            [255, 255, 255, (8 * (255 - r)).min(255) as u8]
        }),
    }
}

/// Loads an image file as RGBA (Image_LoadImage: tga, png, jpg, pcx, lmp).
/// `path` must include the extension.
pub fn load_image(files: &mut dyn FileSource, path: &str, palette: Option<&[u8; 768]>, premul: bool) -> Option<Image> {
    let data = files.load(path)?;
    let ext = path.rsplit('.').next()?.to_ascii_lowercase();
    let pal_rgba = |pixels: &[u8], pal: &[[u8; 3]]| -> Vec<u8> {
        pixels
            .iter()
            .flat_map(|&i| {
                let c = pal.get(i as usize).copied().unwrap_or([0; 3]);
                [c[0], c[1], c[2], if i == 255 { 0 } else { 255 }]
            })
            .collect()
    };
    let game_pal = || -> Vec<[u8; 3]> {
        palette.map_or_else(|| vec![[0; 3]; 256], |p| p.chunks(3).map(|c| [c[0], c[1], c[2]]).collect())
    };
    let mut img = match ext.as_str() {
        "tga" => {
            let t = qformats::image::parse_tga(&data).ok()?;
            Image { width: t.width, height: t.height, rgba: t.pixels }
        }
        "pcx" => {
            let (p, pal) = qformats::image::parse_pcx(&data).ok()?;
            Image { width: p.width, height: p.height, rgba: pal_rgba(&p.pixels, &pal) }
        }
        "png" => decode_png(&data)?,
        "jpg" | "jpeg" => decode_jpeg(&data)?,
        "lmp" => {
            let p = qformats::image::parse_pic(&data).ok()?;
            Image { width: p.width, height: p.height, rgba: pal_rgba(&p.pixels, &game_pal()) }
        }
        _ => return None,
    };
    if premul {
        for px in img.rgba.chunks_mut(4) {
            let a = px[3] as u32;
            for c in &mut px[..3] {
                *c = (*c as u32 * a / 255) as u8;
            }
        }
    }
    Some(img)
}

/// Decodes a PNG of any colour type / bit depth to 8-bit RGBA.
pub(super) fn decode_png(data: &[u8]) -> Option<Image> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(data));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    buf.truncate(info.buffer_size());
    let rgba = expand_to_rgba(&buf, match info.color_type {
        png::ColorType::Grayscale => 1,
        png::ColorType::GrayscaleAlpha => 2,
        png::ColorType::Rgb => 3,
        png::ColorType::Rgba => 4,
        png::ColorType::Indexed => return None, // EXPAND removes palettes
    });
    Some(Image { width: info.width, height: info.height, rgba })
}

/// Decodes a baseline or progressive JPEG to RGBA (alpha 255).
pub(super) fn decode_jpeg(data: &[u8]) -> Option<Image> {
    use zune_core::{bytestream::ZCursor, colorspace::ColorSpace, options::DecoderOptions};
    let opts = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGBA);
    let mut dec = zune_jpeg::JpegDecoder::new_with_options(ZCursor::new(data), opts);
    let rgba = dec.decode().ok()?;
    let info = dec.info()?;
    let (width, height) = (info.width as u32, info.height as u32);
    (rgba.len() == (width * height * 4) as usize).then_some(Image { width, height, rgba })
}

/// Expands 1/2/3/4-channel 8-bit pixels to RGBA.
pub(super) fn expand_to_rgba(px: &[u8], channels: usize) -> Vec<u8> {
    px.chunks_exact(channels)
        .flat_map(|c| match channels {
            1 => [c[0], c[0], c[0], 255],
            2 => [c[0], c[0], c[0], c[1]],
            3 => [c[0], c[1], c[2], 255],
            _ => [c[0], c[1], c[2], c[3]],
        })
        .collect()
}

pub(super) fn find_image(files: &mut dyn FileSource, base: &str) -> Option<String> {
    IMAGE_EXTS.iter().map(|e| format!("{base}.{e}")).find(|p| files.load(p).is_some())
}
