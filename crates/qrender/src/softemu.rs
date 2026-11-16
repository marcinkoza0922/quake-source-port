//! r_softemu / r_dither parameters shared by both backends (ironwail
//! R_SetupView, R_ChooseBModelProgram, GLPalette_UpdateLookupTable).

use crate::Settings;

/// ironwail NOISESCALE.
pub(crate) const NOISESCALE: f32 = 9.0 / 255.0;

/// Scene shader parameters: (screen dither, texture dither, world dither
/// level, alias mode). World levels: 0 off, 1 coarse dither, 2 banded
/// lightmap; alias modes: 0 standard, 1 dither, 2 affine.
pub(crate) fn scene_params(st: &Settings) -> (f32, f32, u32, u32) {
    let (screen, texture) = match st.softemu {
        0 => (st.dither / 255.0, 0.0),
        2 => (NOISESCALE * st.dither * st.dither_screen, NOISESCALE * st.dither * st.dither_texture),
        _ => (0.0, 0.0), // fine dithers in post; banded doesn't dither
    };
    let banding = if st.softemu == 3 { st.lightmap_banding != 0.0 } else { st.lightmap_banding > 0.0 };
    let world = if banding { 2 } else if st.softemu >= 2 { 1 } else { 0 };
    let warp = if st.softemu == 3 { st.mdl_warp != 0.0 } else { st.mdl_warp > 0.0 };
    let alias = if warp { 2 } else if st.softemu == 2 { 1 } else { 0 };
    (screen, texture, world, alias)
}

/// Post pass dither strength: banding suppression (softemu off) or the
/// fine screen dither before palettizing (softemu 1).
pub(crate) fn post_dither(st: &Settings) -> f32 {
    match st.softemu {
        0 => st.dither / 255.0,
        1 => NOISESCALE * st.dither * st.dither_screen,
        _ => 0.0,
    }
}

/// Post pass mode: 0 truecolour, 1 palettize + dither, 2 palettize.
pub(crate) fn post_mode(st: &Settings) -> u32 {
    st.softemu.min(2)
}

/// Softemu 2/3 and lightmap banding force nearest world texture filtering
/// (with linear mips).
pub(crate) fn forces_nearest(st: &Settings) -> bool {
    st.softemu >= 2 || st.lightmap_banding > 0.0
}

/// Palette lookup metric for [`crate::convert::palette_lut`] while softemu
/// is on: auto is OKLab except in banded mode on maps without coloured light.
pub(crate) fn lut_metric(st: &Settings, lit: bool) -> Option<i32> {
    (st.softemu > 0).then(|| {
        if st.softemu_metric < 0 {
            if st.softemu != 3 || lit { 2 } else { 0 }
        } else {
            st.softemu_metric.min(2)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes() {
        let mut st = Settings::default();
        assert_eq!(scene_params(&st), (1.0 / 255.0, 0.0, 0, 0));
        assert_eq!(lut_metric(&st, false), None);
        st.softemu = 2;
        assert_eq!(scene_params(&st).2..=scene_params(&st).3, 1..=1);
        assert!(forces_nearest(&st));
        assert_eq!(post_dither(&st), 0.0);
        st.softemu = 3;
        // banded: auto lightmap banding and model warp, naive metric when unlit
        assert_eq!((scene_params(&st).2, scene_params(&st).3), (2, 2));
        assert_eq!(lut_metric(&st, false), Some(0));
        assert_eq!(lut_metric(&st, true), Some(2));
        assert_eq!(post_mode(&st), 2);
    }
}
