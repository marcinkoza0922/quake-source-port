//! LMP pictures, palette, colormap, PCX, TGA.

use crate::{Error, Reader, Result};

/// 8-bit paletted picture (qpic / .lmp).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pic {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// RGBA8 image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Parse a qpic (`.lmp` or WAD QPIC lump): u32 width, u32 height, pixels.
pub fn parse_pic(data: &[u8]) -> Result<Pic> {
    let mut r = Reader::new(data, "lmp");
    let w = r.u32()?;
    let h = r.u32()?;
    let n = (w as usize)
        .checked_mul(h as usize)
        .ok_or_else(|| r.err("size overflow"))?;
    Ok(Pic {
        width: w,
        height: h,
        pixels: r.bytes(n)?.to_vec(),
    })
}

/// 256-entry RGB palette (palette.lmp).
pub fn parse_palette(data: &[u8]) -> Result<[[u8; 3]; 256]> {
    if data.len() < 768 {
        return Err(Error::new(
            "palette",
            format!("too short ({} bytes)", data.len()),
        ));
    }
    Ok(std::array::from_fn(|i| {
        [data[i * 3], data[i * 3 + 1], data[i * 3 + 2]]
    }))
}

/// Colormap (colormap.lmp): 64 light levels x 256 indices.
pub fn parse_colormap(data: &[u8]) -> Result<Vec<[u8; 256]>> {
    if data.len() < 64 * 256 {
        return Err(Error::new(
            "colormap",
            format!("too short ({} bytes)", data.len()),
        ));
    }
    Ok(data[..64 * 256].as_chunks::<256>().0.to_vec())
}

/// 8-bit RLE PCX with trailing 256-color palette.
pub fn parse_pcx(data: &[u8]) -> Result<(Pic, [[u8; 3]; 256])> {
    let mut r = Reader::new(data, "pcx");
    let hdr = r.bytes(128)?;
    if hdr[0] != 0x0a || hdr[2] != 1 || hdr[3] != 8 {
        return Err(r.err("unsupported pcx (need 8-bit RLE)"));
    }
    let u16at = |o: usize| u16::from_le_bytes([hdr[o], hdr[o + 1]]) as u32;
    let (xmin, ymin, xmax, ymax) = (u16at(4), u16at(6), u16at(8), u16at(10));
    if xmax < xmin || ymax < ymin {
        return Err(r.err("bad dimensions"));
    }
    let (w, h) = (xmax - xmin + 1, ymax - ymin + 1);
    let bpl = (u16at(66) as usize).max(w as usize);
    if data.len() < 128 + 769 {
        return Err(r.err("missing palette"));
    }
    let pal = parse_palette(&data[data.len() - 768..])?;
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    let mut row = Vec::with_capacity(bpl);
    for _ in 0..h {
        row.clear();
        while row.len() < bpl {
            let b = r.u8()?;
            if b & 0xc0 == 0xc0 {
                let v = r.u8()?;
                let n = ((b & 0x3f) as usize).min(bpl - row.len());
                row.resize(row.len() + n, v);
            } else {
                row.push(b);
            }
        }
        pixels.extend_from_slice(&row[..w as usize]);
    }
    Ok((
        Pic {
            width: w,
            height: h,
            pixels,
        },
        pal,
    ))
}

/// TGA: types 1/9 (colormapped, 8-bit index, 24/32 map), 2/10 (24/32-bit), 3/11 (8-bit gray). Output RGBA, top-down.
pub fn parse_tga(data: &[u8]) -> Result<Rgba> {
    let mut r = Reader::new(data, "tga");
    let id_len = r.u8()? as usize;
    let cmap_type = r.u8()?;
    let ty = r.u8()?;
    let _cmap_first = r.u16()?;
    let cmap_len = r.u16()? as usize;
    let cmap_bits = r.u8()?;
    let _xo = r.u16()?;
    let _yo = r.u16()?;
    let w = r.u16()? as u32;
    let h = r.u16()? as u32;
    let bits = r.u8()?;
    let desc = r.u8()?;
    r.bytes(id_len)?;
    let mut cmap = Vec::new();
    if cmap_type == 1 {
        let cb = match cmap_bits {
            24 => 3,
            32 => 4,
            b => return Err(r.err(format!("unsupported colormap depth {b}"))),
        };
        for _ in 0..cmap_len {
            let c = r.bytes(cb)?;
            cmap.push([c[2], c[1], c[0], if cb == 4 { c[3] } else { 255 }]);
        }
    }
    let base = ty & !8;
    let rle = ty & 8 != 0;
    let bpp = match (base, bits) {
        (1, 8) | (3, 8) => 1,
        (2, 24) => 3,
        (2, 32) => 4,
        _ => return Err(r.err(format!("unsupported tga type {ty} / {bits} bits"))),
    };
    if base == 1 && cmap.is_empty() {
        return Err(r.err("colormapped tga without colormap"));
    }
    let n = w as usize * h as usize;
    if n > r.remaining().saturating_mul(if rle { 128 } else { 1 }) {
        return Err(r.err("image larger than data"));
    }
    let to_rgba = |p: &[u8]| -> Result<[u8; 4]> {
        Ok(match (base, bpp) {
            (1, _) => *cmap
                .get(p[0] as usize)
                .ok_or_else(|| Error::new("tga", "colormap index out of range"))?,
            (3, _) => [p[0], p[0], p[0], 255],
            (_, 3) => [p[2], p[1], p[0], 255],
            _ => [p[2], p[1], p[0], p[3]],
        })
    };
    let mut px: Vec<[u8; 4]> = Vec::with_capacity(n);
    while px.len() < n {
        if rle {
            let hdr = r.u8()?;
            let cnt = ((hdr & 0x7f) as usize + 1).min(n - px.len());
            if hdr & 0x80 != 0 {
                let c = to_rgba(r.bytes(bpp)?)?;
                px.resize(px.len() + cnt, c);
            } else {
                for _ in 0..cnt {
                    px.push(to_rgba(r.bytes(bpp)?)?);
                }
            }
        } else {
            px.push(to_rgba(r.bytes(bpp)?)?);
        }
    }
    let mut pixels = Vec::with_capacity(n * 4);
    let top_down = desc & 0x20 != 0;
    for y in 0..h as usize {
        let sy = if top_down { y } else { h as usize - 1 - y };
        for p in &px[sy * w as usize..(sy + 1) * w as usize] {
            pixels.extend_from_slice(p);
        }
    }
    Ok(Rgba {
        width: w,
        height: h,
        pixels,
    })
}
