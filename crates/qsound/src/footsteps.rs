//! Surface-aware player footstep and landing cues backed by embedded CC0 1.0
//! (public domain) OGG Vorbis recordings in `crates/qsound/assets/footsteps/`.

use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::Sfx;

/// Floor surface kind, matching `hitfx`'s BSP texture material classifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FootstepSurface {
    Stone,
    Metal,
    Wood,
    Dirt,
    Water,
    Slime,
    Lava,
}

impl FootstepSurface {
    pub const ALL: [Self; 7] = [
        Self::Stone,
        Self::Metal,
        Self::Wood,
        Self::Dirt,
        Self::Water,
        Self::Slime,
        Self::Lava,
    ];

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Stone => "stone",
            Self::Metal => "metal",
            Self::Wood => "wood",
            Self::Dirt => "dirt",
            Self::Water => "water",
            Self::Slime => "slime",
            Self::Lava => "lava",
        }
    }
}

/// Number of distinct alternating footstep variations per surface.
pub const VARIANTS: usize = 4;

const STEP_CLIPS: [[&[u8]; VARIANTS]; 7] = [
    [
        include_bytes!("../assets/footsteps/stone_0.ogg"),
        include_bytes!("../assets/footsteps/stone_1.ogg"),
        include_bytes!("../assets/footsteps/stone_2.ogg"),
        include_bytes!("../assets/footsteps/stone_3.ogg"),
    ],
    [
        include_bytes!("../assets/footsteps/metal_0.ogg"),
        include_bytes!("../assets/footsteps/metal_1.ogg"),
        include_bytes!("../assets/footsteps/metal_2.ogg"),
        include_bytes!("../assets/footsteps/metal_3.ogg"),
    ],
    [
        include_bytes!("../assets/footsteps/wood_0.ogg"),
        include_bytes!("../assets/footsteps/wood_1.ogg"),
        include_bytes!("../assets/footsteps/wood_2.ogg"),
        include_bytes!("../assets/footsteps/wood_3.ogg"),
    ],
    [
        include_bytes!("../assets/footsteps/dirt_0.ogg"),
        include_bytes!("../assets/footsteps/dirt_1.ogg"),
        include_bytes!("../assets/footsteps/dirt_2.ogg"),
        include_bytes!("../assets/footsteps/dirt_3.ogg"),
    ],
    [
        include_bytes!("../assets/footsteps/water_0.ogg"),
        include_bytes!("../assets/footsteps/water_1.ogg"),
        include_bytes!("../assets/footsteps/water_2.ogg"),
        include_bytes!("../assets/footsteps/water_3.ogg"),
    ],
    [
        include_bytes!("../assets/footsteps/slime_0.ogg"),
        include_bytes!("../assets/footsteps/slime_1.ogg"),
        include_bytes!("../assets/footsteps/slime_2.ogg"),
        include_bytes!("../assets/footsteps/slime_3.ogg"),
    ],
    [
        include_bytes!("../assets/footsteps/lava_0.ogg"),
        include_bytes!("../assets/footsteps/lava_1.ogg"),
        include_bytes!("../assets/footsteps/lava_2.ogg"),
        include_bytes!("../assets/footsteps/lava_3.ogg"),
    ],
];

const LAND_CLIPS: [&[u8]; 7] = [
    include_bytes!("../assets/footsteps/stone_land.ogg"),
    include_bytes!("../assets/footsteps/metal_land.ogg"),
    include_bytes!("../assets/footsteps/wood_land.ogg"),
    include_bytes!("../assets/footsteps/dirt_land.ogg"),
    include_bytes!("../assets/footsteps/water_land.ogg"),
    include_bytes!("../assets/footsteps/slime_land.ogg"),
    include_bytes!("../assets/footsteps/lava_land.ogg"),
];

/// Decode a single footstep variation for `surface` resampled to `rate`.
pub fn render_step(surface: FootstepSurface, variant: usize, rate: u32) -> Sfx {
    let v = variant % VARIANTS;
    let data = decode_ogg_mono(STEP_CLIPS[surface.index()][v], rate.max(8000));
    Sfx {
        name: format!("footsteps/{}_{}", surface.name(), v),
        data,
        loop_start: None,
    }
}

/// Decode the heavier landing thud for `surface` resampled to `rate`.
pub fn render_land(surface: FootstepSurface, rate: u32) -> Sfx {
    let data = decode_ogg_mono(LAND_CLIPS[surface.index()], rate.max(8000));
    Sfx {
        name: format!("footsteps/{}_land", surface.name()),
        data,
        loop_start: None,
    }
}

fn decode_ogg_mono(bytes: &'static [u8], out_rate: u32) -> Vec<i16> {
    let src = Cursor::new(bytes);
    let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    hint.with_extension("ogg");
    let Ok(probed) = symphonia::default::get_probe().format(
        &hint,
        mss,
        &FormatOptions::default(),
        &MetadataOptions::default(),
    ) else {
        return Vec::new();
    };
    let mut format = probed.format;
    let Some(track) = format.default_track() else {
        return Vec::new();
    };
    let in_rate = track.codec_params.sample_rate.unwrap_or(out_rate);
    let track_id = track.id;
    let Ok(mut decoder) =
        symphonia::default::get_codecs().make(&track.codec_params, &DecoderOptions::default())
    else {
        return Vec::new();
    };

    let mut mono = Vec::<f32>::new();
    while let Ok(pkt) = format.next_packet() {
        if pkt.track_id() != track_id {
            continue;
        }
        let Ok(decoded) = decoder.decode(&pkt) else {
            continue;
        };
        let spec = *decoded.spec();
        let ch = spec.channels.count().max(1);
        let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sb.copy_interleaved_ref(decoded);
        let inv_ch = 1.0 / ch as f32;
        for frame in sb.samples().chunks_exact(ch) {
            let sum: f32 = frame.iter().copied().sum();
            mono.push(sum * inv_ch);
        }
    }
    if mono.is_empty() {
        return Vec::new();
    }

    if in_rate == out_rate {
        return mono
            .into_iter()
            .map(|s| (s * 32767.0).clamp(-32768.0, 32767.0) as i16)
            .collect();
    }

    let step = f64::from(in_rate) / f64::from(out_rate);
    let out_len = ((mono.len() as f64) / step).ceil() as usize;
    let last = mono.len() - 1;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * step;
        let idx = (pos as usize).min(last);
        let next = (idx + 1).min(last);
        let frac = (pos - idx as f64) as f32;
        let sample = mono[idx] + (mono[next] - mono[idx]) * frac;
        out.push((sample * 32767.0).clamp(-32768.0, 32767.0) as i16);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_all_surfaces_and_variants_non_empty() {
        for &surf in &FootstepSurface::ALL {
            for v in 0..VARIANTS {
                let s = render_step(surf, v, 48000);
                assert!(!s.data.is_empty(), "{surf:?} v{v} empty");
                assert!(s.data.iter().any(|&x| x.abs() > 3000), "{surf:?} v{v} too quiet");
                assert!(s.name.starts_with("footsteps/"));
            }
            let l = render_land(surf, 48000);
            assert!(!l.data.is_empty(), "{surf:?} land empty");
            assert!(l.data.iter().any(|&x| x.abs() > 4000), "{surf:?} land too quiet");
        }
    }

    #[test]
    fn resamples_proportionally_to_output_rate() {
        let lo = render_step(FootstepSurface::Stone, 0, 22050);
        let hi = render_step(FootstepSurface::Stone, 0, 44100);
        let ratio = hi.data.len() as f32 / lo.data.len() as f32;
        assert!((ratio - 2.0).abs() < 0.05, "unexpected resample ratio {ratio}");
    }
}
