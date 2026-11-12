//! High-resolution UI text and pictures.
//!
//! `ui_font` picks the conchars atlas used by every `Draw2D::Char`:
//! 0 = classic (the 8×8 conchars, nearest sampled), 1 = smooth (default: the
//! game's own conchars upscaled 8× with Scale2x and drawn bilinear with mips,
//! so it stays the mod's font but is clean at any size), 2 = a TrueType/
//! OpenType font from `ui_fontfile` (game path such as `fonts/x.ttf`, or a
//! filesystem path), rasterised into the same 16×16 layout. Characters the
//! font lacks (control/box-drawing glyphs, slider parts) keep the smooth
//! conchars; the high half (128+) is tinted like the conchars' gold set.
//!
//! `ui_smoothpics` (default 1) upscales UI pictures (menu plaques, status
//! bar, ...) 4× with Scale2x the same way.

use qrender::Renderer;

/// Glyph cell size of the hi-res atlas (8× the conchars' 8 pixels).
pub const CELL: usize = 64;

/// Scale2x (EPX) on palette indices, `w`×`h` → `2w`×`2h`; neighbours outside
/// the image (or outside the `cell`-sized block when given) count as the
/// centre texel.
pub fn scale2x(src: &[u8], w: usize, h: usize, cell: Option<usize>) -> Vec<u8> {
    let mut out = vec![0u8; w * h * 4];
    let block = |v: usize| cell.map_or(0, |c| v / c * c);
    for y in 0..h {
        for x in 0..w {
            let e = src[y * w + x];
            let (bx, by) = (block(x), block(y));
            let (xmax, ymax) = cell.map_or((w - 1, h - 1), |c| ((bx + c - 1).min(w - 1), (by + c - 1).min(h - 1)));
            let at = |xx: isize, yy: isize| -> u8 {
                if xx < bx as isize || yy < by as isize || xx > xmax as isize || yy > ymax as isize {
                    e
                } else {
                    src[yy as usize * w + xx as usize]
                }
            };
            let (xi, yi) = (x as isize, y as isize);
            let (b, d, f, hh) = (at(xi, yi - 1), at(xi - 1, yi), at(xi + 1, yi), at(xi, yi + 1));
            let (mut e0, mut e1, mut e2, mut e3) = (e, e, e, e);
            if b != hh && d != f {
                if d == b {
                    e0 = d;
                }
                if b == f {
                    e1 = f;
                }
                if d == hh {
                    e2 = d;
                }
                if hh == f {
                    e3 = f;
                }
            }
            let o = (2 * y) * 2 * w + 2 * x;
            out[o] = e0;
            out[o + 1] = e1;
            out[o + 2 * w] = e2;
            out[o + 2 * w + 1] = e3;
        }
    }
    out
}

/// RGBA mip chain from a base level down to (and including) `min_w` wide.
fn mips(base: Vec<u8>, w: usize, h: usize, min_w: usize) -> (u32, u32, Vec<Vec<u8>>) {
    let mut levels = vec![base];
    let (mut cw, mut ch) = (w, h);
    while cw / 2 >= min_w.max(1) && ch / 2 >= 1 {
        let next = qrender::downsample_rgba(levels.last().unwrap(), cw, ch, true);
        cw /= 2;
        ch /= 2;
        levels.push(next);
    }
    (w as u32, h as u32, levels)
}

fn to_rgba(pal: &qrender::Palette, src: &[u8], transparent: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() * 4);
    for &i in src {
        if i == transparent {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let c = pal[i as usize];
            out.extend_from_slice(&[c[0], c[1], c[2], 255]);
        }
    }
    out
}

/// Conchars (128×128 indices, 0 transparent) upscaled 8×, as RGBA mips
/// (1024² down to 128²).
pub fn smooth_conchars(pal: &qrender::Palette, idx: &[u8]) -> (u32, u32, Vec<Vec<u8>>) {
    let mut px = idx[..128 * 128].to_vec();
    let (mut s, mut cell) = (128, 8);
    for _ in 0..3 {
        px = scale2x(&px, s, s, Some(cell));
        s *= 2;
        cell *= 2;
    }
    mips(to_rgba(pal, &px, 0), s, s, 128)
}

/// A UI picture (indices, 255 transparent) upscaled 4×, as RGBA mips.
pub fn smooth_pic(pal: &qrender::Palette, w: u32, h: u32, idx: &[u8]) -> (u32, u32, Vec<Vec<u8>>) {
    let (mut w, mut h) = (w as usize, h as usize);
    let mut px = idx.to_vec();
    for _ in 0..2 {
        px = scale2x(&px, w, h, None);
        w *= 2;
        h *= 2;
    }
    mips(to_rgba(pal, &px, 255), w, h, w / 4)
}

/// Average colour of a glyph's opaque texels in the 1024² RGBA base level.
fn glyph_color(atlas: &[u8], ch: usize) -> [f32; 3] {
    let (gx, gy) = ((ch & 15) * CELL, (ch >> 4) * CELL);
    let (mut sum, mut n) = ([0f32; 3], 0f32);
    for y in gy..gy + CELL {
        for x in gx..gx + CELL {
            let i = (y * 16 * CELL + x) * 4;
            if atlas[i + 3] > 0 {
                for k in 0..3 {
                    sum[k] += atlas[i + k] as f32;
                }
                n += 1.0;
            }
        }
    }
    if n == 0.0 { [255.0; 3] } else { sum.map(|s| s / n) }
}

/// Rasterises printable ASCII from `font` over the smooth atlas `base`
/// (1024² RGBA): white-ish for 32..127 (the conchars' text colour), gold for
/// 160..255. Each glyph gets a soft dark drop shadow for legibility.
pub fn ttf_conchars(font_data: &[u8], base: &mut [u8]) -> Result<(), String> {
    use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
    let font = FontRef::try_from_slice(font_data).map_err(|e| e.to_string())?;
    let white = glyph_color(base, b'A' as usize);
    let gold = glyph_color(base, b'A' as usize + 128);
    // Size by cap height ('H' fills 70% of the cell, like the conchars'
    // capitals), then stretch horizontally so the widest advance fills the
    // cell: the layout is a fixed 8-unit grid, so narrow glyphs read as
    // gappy otherwise.
    let unit = PxScale::from(CELL as f32);
    let hb = font.outline_glyph(font.glyph_id('H').with_scale(unit)).map(|o| o.px_bounds());
    let cap = hb.map_or(CELL as f32 * 0.7, |b| b.height().max(1.0));
    let sy = CELL as f32 * 0.70 / cap * CELL as f32;
    let widest = (32u8..127)
        .map(|c| font.as_scaled(PxScale::from(sy)).h_advance(font.glyph_id(c as char)))
        .fold(1f32, f32::max);
    let sx = sy * (CELL as f32 * 0.98 / widest).min(1.6);
    let scale = PxScale { x: sx, y: sy };
    let sf = font.as_scaled(scale);
    let baseline = CELL as f32 * 0.79;
    for c in 32u8..127 {
        let id = font.glyph_id(c as char);
        if id.0 == 0 && c != b' ' {
            continue;
        }
        let mut cov = vec![0f32; CELL * CELL];
        let x0 = (CELL as f32 - sf.h_advance(id)) / 2.0;
        let g = id.with_scale_and_position(scale, ab_glyph::point(x0, baseline));
        if let Some(o) = font.outline_glyph(g) {
            let b = o.px_bounds();
            o.draw(|x, y, v| {
                let (x, y) = (b.min.x as i32 + x as i32, b.min.y as i32 + y as i32);
                if (0..CELL as i32).contains(&x) && (0..CELL as i32).contains(&y) {
                    cov[y as usize * CELL + x as usize] = v.min(1.0);
                }
            });
        }
        // faux bold: the classic font's strokes are ~1/4 of the cell thick
        let src = cov.clone();
        for y in 0..CELL {
            for x in 0..CELL {
                let mut m = src[y * CELL + x];
                for (dx, dy) in [(1, 0), (2, 0), (0, 1), (1, 1), (2, 1)] {
                    if x >= dx && y >= dy {
                        m = m.max(src[(y - dy) * CELL + x - dx]);
                    }
                }
                cov[y * CELL + x] = m;
            }
        }
        for (ch, col) in [(c as usize, white), (c as usize + 128, gold)] {
            let (gx, gy) = ((ch & 15) * CELL, (ch >> 4) * CELL);
            for y in 0..CELL {
                for x in 0..CELL {
                    let a = cov[y * CELL + x];
                    let sh = if x >= 3 && y >= 3 { cov[(y - 3) * CELL + x - 3] * 0.75 } else { 0.0 };
                    // glyph over shadow
                    let out_a = a + sh * (1.0 - a);
                    let rgb = if out_a > 0.0 { col.map(|v| v * a / out_a) } else { [0.0; 3] };
                    let i = ((gy + y) * 16 * CELL + gx + x) * 4;
                    base[i..i + 4].copy_from_slice(&[rgb[0] as u8, rgb[1] as u8, rgb[2] as u8, (out_a * 255.0) as u8]);
                }
            }
        }
    }
    Ok(())
}

/// Loads `path` from the game filesystem, else from disk.
fn read_font(fs: &qcore::fs::Fs, path: &str) -> Option<Vec<u8>> {
    fs.open(path).or_else(|| std::fs::read(path).ok())
}

/// Installs the conchars for `mode` (see the module docs). Returns a
/// warning when a custom font could not be used.
pub fn apply(r: &mut Renderer, fs: &qcore::fs::Fs, conchars: &[u8], mode: i32, file: &str) -> Option<String> {
    if conchars.len() < 128 * 128 {
        return None;
    }
    if mode <= 0 {
        let _ = r.set_conchars(&conchars[..128 * 128]);
        return None;
    }
    let pal = *r.palette();
    let (w, h, mut levels) = smooth_conchars(&pal, conchars);
    let mut warn = None;
    if mode >= 2 {
        match read_font(fs, file).ok_or_else(|| format!("can't open \"{file}\"")).and_then(|d| ttf_conchars(&d, &mut levels[0])) {
            Ok(()) => {
                let base = levels.swap_remove(0);
                levels = mips(base, w as usize, h as usize, 128).2;
            }
            Err(e) => warn = Some(format!("ui_fontfile: {e}; using the smooth font\n")),
        }
    }
    let refs: Vec<&[u8]> = levels.iter().map(|l| l.as_slice()).collect();
    let _ = r.set_conchars_mips(w, h, &refs);
    warn
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale2x_rounds_diagonals() {
        // 2×2 diagonal: top-left and bottom-right set
        let src = [1, 0, 0, 1];
        let out = scale2x(&src, 2, 2, None);
        assert_eq!(out.len(), 16);
        // plain blocks stay blocks: no neighbours agree across the diagonal
        let flat = scale2x(&[7; 4], 2, 2, None);
        assert!(flat.iter().all(|&v| v == 7));
        // an L corner fills in: texel (1,1)=0 with B=0? use a 3×3 staircase
        let st = [1, 0, 0, 1, 1, 0, 0, 1, 1];
        let o = scale2x(&st, 3, 3, None);
        // centre texel (1,1)=1: its top-right sub-texel follows B==F (0)
        assert_eq!(o[2 * 6 + 3], 0);
    }

    #[test]
    fn cells_do_not_bleed() {
        // two 2-wide cells: left all 1, right all 2
        let src = [1, 1, 2, 2, 1, 1, 2, 2];
        let o = scale2x(&src, 4, 2, Some(2));
        assert!(o[..4].iter().all(|&v| v == 1) && o[4..8].iter().all(|&v| v == 2));
    }

    #[test]
    fn smooth_atlas_has_mips() {
        let pal = [[200u8; 3]; 256];
        let mut idx = vec![0u8; 128 * 128];
        for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
            idx[y * 128 + x] = 5;
        }
        let (w, h, l) = smooth_conchars(&pal, &idx);
        assert_eq!((w, h, l.len()), (1024, 1024, 4));
        assert_eq!(l[3].len(), 128 * 128 * 4);
        // the 2×2 block became 16×16 (centre opaque, outside clear)
        assert_eq!(l[0][(24 * 1024 + 24) * 4 + 3], 255);
        assert_eq!(l[0][(10 * 1024 + 10) * 4 + 3], 0);
    }
}
