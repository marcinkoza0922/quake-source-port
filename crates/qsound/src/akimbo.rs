//! Akimbo pistol cues: CC0 recordings processed by `tools/gun_sounds.py`
//! (see `assets/gun-sounds/README.md`).
use crate::{Sfx, load_sfx};

/// Shots of the left and the right pistol (different takes).
pub const SHOT: usize = 0;
pub const DRY: usize = 2;
pub const MAG_OUT: usize = 3;
pub const MAG_IN: usize = 4;
pub const SLIDE: usize = 5;
pub const COUNT: usize = 6;

macro_rules! wav {
    ($name:literal) => {
        include_bytes!(concat!("../../../assets/gun-sounds/", $name, ".wav"))
    };
}

const CUES: [&[u8]; COUNT] = [
    wav!("pistol_shot_l"),
    wav!("pistol_shot_r"),
    wav!("pistol_dry"),
    wav!("pistol_magout"),
    wav!("pistol_magin"),
    wav!("pistol_slide"),
];

/// Cue `kind` at the device rate.
pub fn render(kind: usize, rate: u32) -> Sfx {
    let kind = kind.min(COUNT - 1);
    load_sfx(&format!("akimbo/{kind}"), CUES[kind], rate).expect("bundled pistol sound")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cues_are_distinct_and_bounded() {
        for rate in [11025, 48000] {
            let s: Vec<_> = (0..COUNT).map(|k| render(k, rate)).collect();
            for (i, c) in s.iter().enumerate() {
                assert!(c.data.iter().any(|x| x.unsigned_abs() > 4000), "{i}");
                assert!(c.data.last().unwrap().unsigned_abs() < 300, "{i}");
                assert!(s[..i].iter().all(|o| o.data != c.data));
            }
        }
    }
}
