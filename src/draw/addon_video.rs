//! Embedded silent gameplay clips. Frames are independently zlib-compressed RGB;
//! the picture cache uploads each requested frame once, with bounded clip sizes.

pub(crate) const FPS: usize = 12;
pub(crate) const FRAMES: usize = 48;

fn clip(id: &str) -> Option<&'static [u8]> {
    Some(match id {
        "portalgun" => include_bytes!("../../assets/addon-previews/portalgun.qav"),
        "boomerang" => include_bytes!("../../assets/addon-previews/boomerang.qav"),
        "sticky" => include_bytes!("../../assets/addon-previews/sticky.qav"),
        "hookshot" => include_bytes!("../../assets/addon-previews/hookshot.qav"),
        "railgun" => include_bytes!("../../assets/addon-previews/railgun.qav"),
        "goggles" => include_bytes!("../../assets/addon-previews/goggles.qav"),
        _ => return None,
    })
}

pub(super) fn frame(name: &str) -> Option<qformats::image::Rgba> {
    let path = name.strip_prefix("addons/previews/")?;
    let (id, frame) = path.split_once('/')?;
    let number = frame.strip_suffix(".tga")?.parse::<usize>().ok()?;
    decode(clip(id)?, number)
}

fn decode(data: &[u8], number: usize) -> Option<qformats::image::Rgba> {
    if data.get(..4)? != b"QAV1" || number >= FRAMES {
        return None;
    }
    let word = |i| Some(u16::from_le_bytes(data.get(i..i + 2)?.try_into().ok()?) as usize);
    let (w, h) = (word(4)?, word(6)?);
    if w == 0 || h == 0 || w > 512 || h > 512 || word(8)? != FRAMES || word(10)? != FPS {
        return None;
    }
    let mut offset = 12;
    for index in 0..=number {
        let len = u32::from_le_bytes(data.get(offset..offset + 4)?.try_into().ok()?) as usize;
        offset += 4;
        let bytes = data.get(offset..offset.checked_add(len)?)?;
        if index == number {
            let rgb = miniz_oxide::inflate::decompress_to_vec_zlib_with_limit(bytes, w * h * 3).ok()?;
            if rgb.len() != w * h * 3 {
                return None;
            }
            let mut pixels = Vec::with_capacity(w * h * 4);
            for color in rgb.as_chunks::<3>().0 {
                pixels.extend_from_slice(&[color[0], color[1], color[2], 255]);
            }
            return Some(qformats::image::Rgba { width: w as u32, height: h as u32, pixels });
        }
        offset += len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_clips_decode_and_show_motion() {
        for id in ["portalgun", "boomerang", "sticky", "hookshot", "railgun", "goggles"] {
            let data = clip(id).unwrap();
            let first = decode(data, 0).unwrap();
            let mut motion = false;
            for i in 1..FRAMES {
                let frame = decode(data, i).unwrap();
                assert_eq!((frame.width, frame.height), (320, 92));
                motion |= frame.pixels != first.pixels;
            }
            assert!(motion, "{id} is a still image");
            assert!(decode(data, FRAMES).is_none());
        }
        assert!(decode(b"QAV1", 0).is_none());
        assert!(frame("addons/previews/unknown/000.tga").is_none());
    }
}
