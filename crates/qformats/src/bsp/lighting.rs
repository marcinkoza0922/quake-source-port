//! Colored, HDR (`LIT2` / `LIGHTING_E5BGR9`), and directional (`.dlit` / `LIGHTINGDIR`)
//! lightmap formats.

use crate::{Error, Reader, Result};

/// Decoded contents of a `.lit` or `.lit2` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LitData {
    /// Standard 24-bit RGB8 lightmap (`"QLIT"` v1, 3 bytes per sample).
    Rgb8(Vec<u8>),
    /// 32-bit shared-exponent HDR lightmap (`E5BGR9`, 1 `u32` per sample).
    HdrE5Bgr9(Vec<u32>),
}

/// Decode one `E5BGR9` (`GL_RGB9_E5` / `VK_FORMAT_E5B9G9R9_UFLOAT_PACK32`) sample to linear RGB.
///
/// Layout: bits `0..9` R, `9..18` G, `18..27` B, `27..32` 5-bit shared exponent (bias 15).
/// Neutral Quake brightness (8-bit 128) corresponds to `1.0`.
#[inline]
pub fn decode_e5bgr9(v: u32) -> [f32; 3] {
    let exp = ((v >> 27) & 0x1f) as i32 - 15 - 9;
    let scale = 2.0f32.powi(exp);
    [
        (v & 0x1ff) as f32 * scale,
        ((v >> 9) & 0x1ff) as f32 * scale,
        ((v >> 18) & 0x1ff) as f32 * scale,
    ]
}

/// Encode linear non-negative RGB to `E5BGR9` (`GL_RGB9_E5`).
pub fn encode_e5bgr9(rgb: [f32; 3]) -> u32 {
    const MAX_VAL: f32 = ((1 << 9) - 1) as f32 / (1 << 9) as f32 * (1 << 16) as f32;
    let r = rgb[0].clamp(0.0, MAX_VAL);
    let g = rgb[1].clamp(0.0, MAX_VAL);
    let b = rgb[2].clamp(0.0, MAX_VAL);
    let max_c = r.max(g).max(b);
    if max_c <= 0.0 || !max_c.is_finite() {
        return 0;
    }
    let exp_shared = ((max_c.log2().floor() as i32) + 16).clamp(0, 31);
    let mut denom = 2.0f32.powi(exp_shared - 15 - 9);
    let max_m = (max_c / denom + 0.5).floor() as i32;
    let exp_shared = if max_m == 512 {
        denom *= 2.0;
        (exp_shared + 1).min(31)
    } else {
        exp_shared
    };
    let rm = ((r / denom + 0.5).floor() as u32).min(511);
    let gm = ((g / denom + 0.5).floor() as u32).min(511);
    let bm = ((b / denom + 0.5).floor() as u32).min(511);
    rm | (gm << 9) | (bm << 18) | ((exp_shared as u32) << 27)
}

/// Parse a `.lit` colored lightmap file (`"QLIT"`, version 1). Returns RGB triplets
/// matching the BSP lighting lump (3 bytes per mono sample).
pub fn parse_lit(data: &[u8]) -> Result<Vec<u8>> {
    match parse_lit_any(data)? {
        LitData::Rgb8(rgb) => Ok(rgb),
        LitData::HdrE5Bgr9(_) => Err(Error::new("lit", "HDR LIT2 file; use parse_lit_any")),
    }
}

/// Parse a `.lit` (v1 RGB8 or v2 / `0x00010001` E5BGR9 HDR) or `.lit2` (`"LIT2"`) file.
pub fn parse_lit_any(data: &[u8]) -> Result<LitData> {
    let mut r = Reader::new(data, "lit");
    let magic = r.arr::<4>()?;
    let v = r.i32()?;
    match (&magic, v) {
        (b"QLIT", 1) => Ok(LitData::Rgb8(data[8..].to_vec())),
        // FTEQW / QuakeSpasm-Spiked / ericw-tools HDR E5BGR9 lightmap versions
        (b"QLIT", 0x0001_0001 | 2) | (b"LIT2", 1 | 2) => {
            let body = &data[8..];
            if !body.len().is_multiple_of(4) {
                return Err(r.err(format!("HDR lit size {} not multiple of 4", body.len())));
            }
            let out = body
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| u32::from_le_bytes(*c))
                .collect();
            Ok(LitData::HdrE5Bgr9(out))
        }
        (b"QLIT" | b"LIT2", _) => Err(r.err(format!("unsupported version {v}"))),
        _ => Err(r.err("bad magic")),
    }
}

/// Parse a `.dlit` / `.lux` directional lightmap (deluxemap) file (`"QLIT"`, version 1).
/// Returns RGB8 world-space direction triplets (`(dir * 0.5 + 0.5) * 255`).
pub fn parse_dlit(data: &[u8]) -> Result<Vec<u8>> {
    let mut r = Reader::new(data, "dlit");
    if &r.arr::<4>()? != b"QLIT" {
        return Err(r.err("bad magic"));
    }
    let v = r.i32()?;
    if v != 1 {
        return Err(r.err(format!("unsupported version {v}")));
    }
    let body = &data[8..];
    if !body.len().is_multiple_of(3) {
        return Err(r.err(format!("dlit size {} not multiple of 3", body.len())));
    }
    Ok(body.to_vec())
}

/// Parse a BSPX `LIGHTING_E5BGR9` lump (raw little-endian `u32` E5BGR9 samples).
pub fn parse_e5bgr9_lump(data: &[u8], expected_samples: usize) -> Option<Vec<u32>> {
    if data.is_empty() || !data.len().is_multiple_of(4) {
        return None;
    }
    let count = data.len() / 4;
    if expected_samples > 0 && count != expected_samples {
        return None;
    }
    Some(
        data.as_chunks::<4>()
            .0
            .iter()
            .map(|c| u32::from_le_bytes(*c))
            .collect(),
    )
}

/// Parse a BSPX `LIGHTINGDIR` deluxemap lump (raw RGB8 world-space direction triplets).
pub fn parse_deluxemap_lump(data: &[u8], expected_samples: usize) -> Option<Vec<u8>> {
    if data.is_empty() || !data.len().is_multiple_of(3) {
        return None;
    }
    let count = data.len() / 3;
    if expected_samples > 0 && count != expected_samples {
        return None;
    }
    Some(data.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn e5bgr9_roundtrip_and_overbright() {
        for rgb in [
            [0.0, 0.0, 0.0],
            [1.0, 1.0, 1.0],
            [0.5, 0.25, 0.125],
            [4.0, 2.0, 0.5],
            [12.5, 0.75, 3.25],
        ] {
            let packed = encode_e5bgr9(rgb);
            let out = decode_e5bgr9(packed);
            for (a, b) in rgb.into_iter().zip(out) {
                let tol = (a * 0.02).max(1e-3);
                assert!((a - b).abs() <= tol, "rgb={rgb:?} -> out={out:?}");
            }
        }
    }

    #[test]
    fn parse_lit_variants_and_dlit() {
        // v1 RGB8
        let mut v1 = b"QLIT".to_vec();
        v1.extend_from_slice(&1i32.to_le_bytes());
        v1.extend_from_slice(&[10, 20, 30]);
        assert_eq!(parse_lit_any(&v1).unwrap(), LitData::Rgb8(vec![10, 20, 30]));
        assert_eq!(parse_lit(&v1).unwrap(), vec![10, 20, 30]);
        assert_eq!(parse_dlit(&v1).unwrap(), vec![10, 20, 30]);

        // HDR LIT2 (QLIT 0x00010001 and LIT2 v1)
        let sample = encode_e5bgr9([3.0, 1.5, 0.25]);
        let mut hdr = b"QLIT".to_vec();
        hdr.extend_from_slice(&0x0001_0001i32.to_le_bytes());
        hdr.extend_from_slice(&sample.to_le_bytes());
        assert_eq!(parse_lit_any(&hdr).unwrap(), LitData::HdrE5Bgr9(vec![sample]));
        assert!(parse_lit(&hdr).is_err());

        let mut lit2 = b"LIT2".to_vec();
        lit2.extend_from_slice(&1i32.to_le_bytes());
        lit2.extend_from_slice(&sample.to_le_bytes());
        assert_eq!(parse_lit_any(&lit2).unwrap(), LitData::HdrE5Bgr9(vec![sample]));

        // Truncated/invalid
        assert!(parse_lit_any(b"NOPE1234").is_err());
        assert!(parse_lit_any(&hdr[..10]).is_err());
        assert!(parse_dlit(&v1[..10]).is_err());
    }
}
