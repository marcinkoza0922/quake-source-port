//! Shrinking textures that are larger than the GPU or the player allows.

use std::borrow::Cow;

/// What a texture is for: decides whether the player's size limit applies
/// and how its texels average.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum TexUse {
    /// World texture, sky layer or skin.
    Base,
    /// Fullbright layer, black where there is none.
    Glow,
    /// Ramp positions, not colours (player colour masks).
    Index,
    /// Skybox face.
    Sky,
    /// Only the GPU's limit applies, as in vkQuake (TEXPREF_NOPICMIP).
    Sprite,
    /// 2D art, kept across maps; only the GPU's limit applies.
    Pic,
}

impl TexUse {
    /// Freed with the level.
    pub(crate) fn level(self) -> bool {
        self != TexUse::Pic
    }

    /// Largest side for this texture: `user` (0 = no limit) or the GPU's.
    pub(crate) fn limit(self, user: u32, gpu: u32) -> u32 {
        match self {
            TexUse::Sprite | TexUse::Pic => gpu,
            _ if user == 0 => gpu,
            _ => user.min(gpu),
        }
    }
}

/// Size and mip levels of a texture that fits its limit.
pub(crate) type Fitted<'a> = (u32, u32, Vec<Cow<'a, [u8]>>);

/// Halves the RGBA texture until neither side exceeds `max`, with as many
/// mip levels as before (the dropped ones' lower levels are reused). `None`
/// when it fits already.
pub(crate) fn fit<'a>(w: u32, h: u32, mips: &[&'a [u8]], max: u32, usage: TexUse) -> Option<Fitted<'a>> {
    let max = max.max(1);
    let (mut w, mut h) = (w as usize, h as usize);
    if w.max(h) <= max as usize || mips.first().is_none_or(|m| m.len() < w * h * 4) {
        return None;
    }
    let half = |w: usize, h: usize| ((w / 2).max(1), (h / 2).max(1));
    let mut rest = mips[1..].iter();
    let mut next = |cur: &[u8], w: usize, h: usize| -> Cow<'a, [u8]> {
        let (nw, nh) = half(w, h);
        match rest.next() {
            Some(&m) if m.len() >= nw * nh * 4 => Cow::Borrowed(m),
            _ => {
                rest = [].iter();
                Cow::Owned(halve(cur, w, h, usage))
            }
        }
    };
    let mut top = Cow::Borrowed(mips[0]);
    while w.max(h) > max as usize {
        top = next(&top, w, h);
        (w, h) = half(w, h);
    }
    let size = (w as u32, h as u32);
    let mut out = vec![top];
    while out.len() < mips.len() && w.max(h) > 1 {
        let m = next(&out[out.len() - 1], w, h);
        out.push(m);
        (w, h) = half(w, h);
    }
    Some((size.0, size.1, out))
}

/// Half-size copy (2x2 box). Base colours weigh by alpha, so cut-out edges
/// don't darken; where all four texels are transparent they count equally,
/// since some skins carry an alpha channel nothing reads.
fn halve(src: &[u8], w: usize, h: usize, usage: TexUse) -> Vec<u8> {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    let mut out = Vec::with_capacity(nw * nh * 4);
    for y in 0..nh {
        for x in 0..nw {
            let at = |dx: usize, dy: usize| &src[((y * 2 + dy).min(h - 1) * w + (x * 2 + dx).min(w - 1)) * 4..][..4];
            if usage == TexUse::Index {
                out.extend_from_slice(at(0, 0));
                continue;
            }
            let px = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
            let alpha: u32 = px.iter().map(|p| p[3] as u32).sum();
            let weighted = alpha > 0 && usage != TexUse::Glow;
            let sum = if weighted { alpha } else { 4 };
            for c in 0..3 {
                let v: u32 = px.iter().map(|p| p[c] as u32 * if weighted { p[3] as u32 } else { 1 }).sum();
                out.push((v / sum) as u8);
            }
            out.push((alpha / 4) as u8);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: usize, h: usize, p: [u8; 4]) -> Vec<u8> {
        p.repeat(w * h)
    }

    #[test]
    fn small_enough_textures_are_untouched() {
        let t = solid(64, 32, [1; 4]);
        assert!(fit(64, 32, &[&t], 64, TexUse::Base).is_none());
        assert!(fit(64, 32, &[&t[..8]], 16, TexUse::Base).is_none(), "truncated data");
        assert!(fit(64, 32, &[], 16, TexUse::Base).is_none());
    }

    #[test]
    fn sides_halve_together_and_mips_are_reused() {
        let m: Vec<Vec<u8>> = (0..4).map(|i| solid(1024 >> i, 256 >> i, [i as u8; 4])).collect();
        let refs: Vec<&[u8]> = m.iter().map(|v| v.as_slice()).collect();
        let (w, h, out) = fit(1024, 256, &refs, 300, TexUse::Base).unwrap();
        assert_eq!((w, h, out.len()), (256, 64, 4));
        assert_eq!([out[0][0], out[1][0]], [2, 3], "existing levels");
        assert!(matches!(out[1], Cow::Borrowed(_)) && matches!(out[2], Cow::Owned(_)));
        for (i, o) in out.iter().enumerate() {
            assert_eq!(o.len(), (256 >> i) * (64 >> i) * 4);
        }
    }

    #[test]
    fn one_level_stays_one_level_and_chains_end_at_one_texel() {
        let t = solid(40, 10, [9, 8, 7, 255]);
        let (w, h, out) = fit(40, 10, &[&t], 16, TexUse::Base).unwrap();
        assert_eq!((w, h, out.len()), (10, 2, 1));
        assert_eq!(&out[0][..4], &[9, 8, 7, 255]);
        let m: Vec<Vec<u8>> = (0..4).map(|i| solid(8 >> i, 8 >> i, [0; 4])).collect();
        let refs: Vec<&[u8]> = m.iter().map(|v| v.as_slice()).collect();
        let (w, _, out) = fit(8, 8, &refs, 2, TexUse::Base).unwrap();
        assert_eq!((w, out.len()), (2, 2));
        assert_eq!(fit(4096, 1, &[&solid(4096, 1, [0; 4])], 1, TexUse::Base).unwrap().0, 1);
    }

    #[test]
    fn texels_average_by_use() {
        let t: Vec<u8> = [[200, 100, 0, 255], [0, 0, 0, 0], [200, 100, 0, 255], [0, 0, 0, 0]].concat();
        let one = |u| fit(2, 2, &[&t], 1, u).unwrap().2[0].to_vec();
        assert_eq!(one(TexUse::Base), [200, 100, 0, 127], "cut-out edges keep their colour");
        assert_eq!(one(TexUse::Glow), [100, 50, 0, 127], "glow fades");
        assert_eq!(one(TexUse::Index), [200, 100, 0, 255], "indices never mix");
        let clear = solid(2, 2, [50, 60, 70, 0]);
        assert_eq!(&fit(2, 2, &[&clear], 1, TexUse::Base).unwrap().2[0][..], [50, 60, 70, 0]);
    }

    #[test]
    fn limits_apply_by_use() {
        assert_eq!(TexUse::Base.limit(0, 4096), 4096);
        assert_eq!(TexUse::Sky.limit(256, 4096), 256);
        assert_eq!(TexUse::Base.limit(8192, 4096), 4096);
        assert_eq!(TexUse::Pic.limit(256, 4096), 4096);
        assert_eq!(TexUse::Sprite.limit(256, 4096), 4096);
    }
}
