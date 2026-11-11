//! Sprite (.spr, "IDSP" version 1, and SPR32 version 32 with RGBA pixels).

use crate::{Reader, Result, check_count};

const FMT: &str = "spr";

pub const SPR_VP_PARALLEL_UPRIGHT: i32 = 0;
pub const SPR_FACING_UPRIGHT: i32 = 1;
pub const SPR_VP_PARALLEL: i32 = 2;
pub const SPR_ORIENTED: i32 = 3;
pub const SPR_VP_PARALLEL_ORIENTED: i32 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SprImage {
    pub origin: [i32; 2],
    pub width: u32,
    pub height: u32,
    /// Palette indices (version 1) or RGBA bytes (SPR32).
    pub pixels: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SprFrame {
    /// Group intervals (empty for single frame).
    pub intervals: Vec<f32>,
    pub images: Vec<SprImage>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Spr {
    pub version: i32,
    pub kind: i32,
    pub bounding_radius: f32,
    pub width: i32,
    pub height: i32,
    pub beam_length: f32,
    pub synctype: i32,
    pub frames: Vec<SprFrame>,
}

impl Spr {
    pub fn is_rgba(&self) -> bool {
        self.version == 32
    }

    pub fn parse(data: &[u8]) -> Result<Spr> {
        let mut r = Reader::new(data, FMT);
        if &r.arr::<4>()? != b"IDSP" {
            return Err(r.err("bad magic"));
        }
        let version = r.i32()?;
        let bpp = match version {
            1 => 1,
            32 => 4,
            v => return Err(r.err(format!("unsupported version {v}"))),
        };
        let kind = r.i32()?;
        let bounding_radius = r.f32()?;
        let width = r.i32()?;
        let height = r.i32()?;
        let num_frames = r.count("frame", 4)?;
        let beam_length = r.f32()?;
        let synctype = r.i32()?;
        let image = |r: &mut Reader| -> Result<SprImage> {
            let origin = [r.i32()?, r.i32()?];
            let w = r.i32()?;
            let h = r.i32()?;
            if !(0..=8192).contains(&w) || !(0..=8192).contains(&h) {
                return Err(r.err(format!("bad frame size {w}x{h}")));
            }
            let pixels = r.bytes(w as usize * h as usize * bpp)?.to_vec();
            Ok(SprImage {
                origin,
                width: w as u32,
                height: h as u32,
                pixels,
            })
        };
        let mut frames = Vec::with_capacity(num_frames);
        for _ in 0..num_frames {
            if r.i32()? == 0 {
                frames.push(SprFrame {
                    intervals: Vec::new(),
                    images: vec![image(&mut r)?],
                });
            } else {
                let n = r.i32()?;
                let n = check_count(&r, n, "frame group", 20)?;
                let intervals = (0..n).map(|_| r.f32()).collect::<Result<_>>()?;
                let images = (0..n).map(|_| image(&mut r)).collect::<Result<_>>()?;
                frames.push(SprFrame { intervals, images });
            }
        }
        Ok(Spr {
            version,
            kind,
            bounding_radius,
            width,
            height,
            beam_length,
            synctype,
            frames,
        })
    }
}
