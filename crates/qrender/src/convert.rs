//! Palette → RGBA conversion and model preprocessing (no GPU).

use qformats::bsp::{MipTex, TextureKind};
use qformats::mdl::Mdl;

pub type Palette = [[u8; 3]; 256];

pub fn is_fullbright(i: u8) -> bool {
    i >= 224
}

fn px(pal: &Palette, i: u8, a: u8) -> [u8; 4] {
    let c = pal[i as usize];
    [c[0], c[1], c[2], a]
}

/// Converts indices to RGBA; `transparent` index gets alpha 0 (and black rgb).
pub fn to_rgba(pal: &Palette, src: &[u8], transparent: Option<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(src.len() * 4);
    for &i in src {
        if Some(i) == transparent {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            out.extend_from_slice(&px(pal, i, 255));
        }
    }
    out
}

/// 2x2 box filter of an RGBA image (`w`x`h`, halved, min 1). With
/// `alpha_weighted`, colour is averaged over texels with alpha only, so
/// cut-out edges don't bleed black.
pub fn downsample_rgba(src: &[u8], w: usize, h: usize, alpha_weighted: bool) -> Vec<u8> {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    if w == 0 || h == 0 || src.len() < w * h * 4 {
        return vec![0; nw * nh * 4];
    }
    let mut out = Vec::with_capacity(nw * nh * 4);
    for y in 0..nh {
        for x in 0..nw {
            let (mut rgb, mut a, mut wsum) = ([0u32; 3], 0u32, 0u32);
            for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let (sx, sy) = ((x * 2 + dx).min(w - 1), (y * 2 + dy).min(h - 1));
                let p = &src[(sy * w + sx) * 4..][..4];
                let pa = p[3] as u32;
                let wt = if alpha_weighted { pa } else { 1 };
                for c in 0..3 {
                    rgb[c] += p[c] as u32 * wt;
                }
                a += pa;
                wsum += wt;
            }
            let c = |v: u32| v.checked_div(wsum).unwrap_or(0) as u8;
            out.extend_from_slice(&[c(rgb[0]), c(rgb[1]), c(rgb[2]), (a / 4) as u8]);
        }
    }
    out
}

/// World texture mips: (base, fullbright) where base has fullbright texels
/// zeroed and fullbright is `None` when the texture has none.
pub fn world_texture(pal: &Palette, t: &MipTex) -> (Vec<Vec<u8>>, Option<Vec<Vec<u8>>>) {
    let fence = t.kind == TextureKind::Fence;
    let lit = t.kind == TextureKind::Normal || fence;
    let has_fb = lit
        && t.mips
            .first()
            .is_some_and(|m| m.iter().any(|&i| is_fullbright(i) && !(fence && i == 255)));
    let mut base = Vec::new();
    let mut fb = Vec::new();
    // Like ironwail, only mip 0 is used; smaller levels are built from its
    // RGBA. Compiler-made mips can't be trusted: fence textures in the wild
    // (MG1 {mgrate4) fill their transparent texels with an opaque colour.
    if let Some(m) = t.mips.first() {
        let mut b = Vec::with_capacity(m.len() * 4);
        let mut f = Vec::with_capacity(if has_fb { m.len() * 4 } else { 0 });
        for &i in m {
            if fence && i == 255 {
                b.extend_from_slice(&[0, 0, 0, 0]);
                if has_fb {
                    f.extend_from_slice(&[0, 0, 0, 0]);
                }
            } else if has_fb && is_fullbright(i) {
                b.extend_from_slice(&[0, 0, 0, 255]);
                f.extend_from_slice(&px(pal, i, 255));
            } else {
                b.extend_from_slice(&px(pal, i, 255));
                if has_fb {
                    f.extend_from_slice(&[0, 0, 0, 0]);
                }
            }
        }
        base.push(b);
        fb.push(f);
    }
    let (w, h) = (t.width as usize, t.height as usize);
    for lvl in 1..t.mips.len() {
        let (pw, ph) = ((w >> (lvl - 1)).max(1), (h >> (lvl - 1)).max(1));
        base.push(downsample_rgba(&base[lvl - 1], pw, ph, fence));
        if has_fb {
            fb.push(downsample_rgba(&fb[lvl - 1], pw, ph, false));
        } else {
            fb.push(Vec::new());
        }
    }
    (base, has_fb.then_some(fb))
}

/// Sky texture halves: (solid back layer, alpha front layer), each (w/2)×h.
/// Quake64 skies (`q64`) are stacked vertically instead: front on top with a
/// constant alpha of 128, back below (Sky_LoadTextureQ64).
pub fn sky_layers(pal: &Palette, t: &MipTex, q64: bool) -> Option<(u32, u32, Vec<u8>, Vec<u8>)> {
    let m = t.mips.first()?;
    if q64 {
        let (w, h) = (t.width as usize, t.height as usize / 2);
        if w == 0 || h == 0 || m.len() < w * h * 2 {
            return None;
        }
        let alpha = m[..w * h].iter().flat_map(|&i| px(pal, i, 128)).collect();
        let solid = m[w * h..w * h * 2].iter().flat_map(|&i| px(pal, i, 255)).collect();
        return Some((w as u32, h as u32, solid, alpha));
    }
    let (w, h) = (t.width as usize, t.height as usize);
    let hw = w / 2;
    if hw == 0 || m.len() < w * h {
        return None;
    }
    let mut solid = Vec::with_capacity(hw * h * 4);
    let mut alpha = Vec::with_capacity(hw * h * 4);
    for y in 0..h {
        for x in 0..hw {
            let fi = m[y * w + x];
            alpha.extend_from_slice(&if fi == 0 {
                [0, 0, 0, 0]
            } else {
                px(pal, fi, 255)
            });
            solid.extend_from_slice(&px(pal, m[y * w + x + hw], 255));
        }
    }
    Some((hw as u32, h as u32, solid, alpha))
}

/// Alias skin: (base, fullbright or None, colormap mask or None).
/// The mask holds ramp position+1 for shirt (r, range 16..32) and pants (g, 96..112);
/// those texels are zeroed in base so the shader can re-add them translated.
pub fn alias_skin(
    pal: &Palette,
    src: &[u8],
    holey: bool,
) -> (Vec<u8>, Option<Vec<u8>>, Option<Vec<u8>>) {
    let has_fb = src
        .iter()
        .any(|&i| is_fullbright(i) && !(holey && i == 255));
    let has_cm = src
        .iter()
        .any(|&i| (16..32).contains(&i) || (96..112).contains(&i));
    let n = src.len();
    let mut base = Vec::with_capacity(n * 4);
    let mut fb = Vec::with_capacity(if has_fb { n * 4 } else { 0 });
    let mut cm = Vec::with_capacity(if has_cm { n * 4 } else { 0 });
    for &i in src {
        let transparent = holey && i == 255;
        let full = has_fb && is_fullbright(i) && !transparent;
        let top = (16..32).contains(&i);
        let bot = (96..112).contains(&i);
        if transparent {
            base.extend_from_slice(&[0, 0, 0, 0]);
        } else if full || top || bot {
            base.extend_from_slice(&[0, 0, 0, 255]);
        } else {
            base.extend_from_slice(&px(pal, i, 255));
        }
        if has_fb {
            fb.extend_from_slice(&if full { px(pal, i, 255) } else { [0, 0, 0, 0] });
        }
        if has_cm {
            let r = if top { i - 16 + 1 } else { 0 };
            let g = if bot { i - 96 + 1 } else { 0 };
            cm.extend_from_slice(&[r, g, 0, 255]);
        }
    }
    (base, has_fb.then_some(fb), has_cm.then_some(cm))
}

/// Alias mesh: per-vertex (pose vertex index, s, t) and triangle indices;
/// back-facing seam vertices are duplicated with s += skinwidth/2.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct AliasVertex {
    pub vert: u32,
    pub st: [f32; 2],
}

pub fn alias_mesh(m: &Mdl) -> (Vec<AliasVertex>, Vec<u32>) {
    let mut map = std::collections::HashMap::new();
    let mut verts = Vec::new();
    let mut idx = Vec::with_capacity(m.triangles.len() * 3);
    let sw = m.skin_width.max(1) as f32;
    let sh = m.skin_height.max(1) as f32;
    for t in &m.triangles {
        for &v in &t.verts {
            let v = v.max(0) as usize;
            let Some(st) = m.stverts.get(v) else { continue };
            let seam = t.faces_front == 0 && st.onseam != 0;
            let key = (v, seam);
            let i = *map.entry(key).or_insert_with(|| {
                let mut s = st.s as f32;
                if seam {
                    s += sw / 2.0;
                }
                verts.push(AliasVertex {
                    vert: v as u32,
                    st: [(s + 0.5) / sw, (st.t as f32 + 0.5) / sh],
                });
                verts.len() as u32 - 1
            });
            idx.push(i);
        }
    }
    (verts, idx)
}

/// Packs all poses: x | y<<8 | z<<16 | normal<<24, pose-major.
/// Frame: (first pose, pose count, group intervals).
pub type FrameInfo = (u32, u32, Vec<f32>);

pub fn alias_poses(m: &Mdl) -> (Vec<u32>, Vec<FrameInfo>) {
    let mut out = Vec::new();
    let mut frames = Vec::with_capacity(m.frames.len());
    let mut pose = 0u32;
    for f in &m.frames {
        let first = pose;
        for p in &f.poses {
            for i in 0..m.num_verts {
                let v = p.verts.get(i).copied().unwrap_or(qformats::mdl::TriVert {
                    v: [0; 3],
                    normal: 0,
                });
                out.push(
                    v.v[0] as u32
                        | (v.v[1] as u32) << 8
                        | (v.v[2] as u32) << 16
                        | (v.normal as u32) << 24,
                );
            }
            pose += 1;
        }
        frames.push((first, pose - first, f.intervals.clone()));
    }
    (out, frames)
}

/// Picks the image of a timed group (frame/skin groups).
pub fn group_index(intervals: &[f32], n: usize, time: f64) -> usize {
    if n <= 1 || intervals.is_empty() {
        return 0;
    }
    let full = *intervals.last().unwrap_or(&0.1) as f64;
    if full <= 0.0 {
        return 0;
    }
    let t = time - (time / full).floor() * full;
    intervals
        .iter()
        .position(|&iv| iv as f64 > t)
        .unwrap_or(n - 1)
        .min(n - 1)
}

#[cfg(test)]
mod downsample_tests {
    use super::downsample_rgba;

    #[test]
    fn alpha_weighted_ignores_transparent_colour() {
        // 2x2: one opaque red texel, three transparent (garbage colour).
        let src = [255, 0, 0, 255, 0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0];
        assert_eq!(downsample_rgba(&src, 2, 2, true), vec![255, 0, 0, 63]);
        assert_eq!(downsample_rgba(&src, 2, 2, false), vec![63, 191, 0, 63]);
        // fully transparent block stays transparent black
        assert_eq!(downsample_rgba(&[0; 16], 2, 2, true), vec![0, 0, 0, 0]);
        // short or empty input doesn't panic
        assert_eq!(downsample_rgba(&[], 0, 4, true), vec![0; 8]);
        assert_eq!(downsample_rgba(&[1; 8], 4, 4, true).len(), 16);
        // odd sizes clamp at the edge
        assert_eq!(downsample_rgba(&[10, 20, 30, 255], 1, 1, true), vec![10, 20, 30, 255]);
    }
}
