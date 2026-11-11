//! RIFF WAVE PCM loader, matching Quake's GetWavinfo (cue/LIST "mark" loop points).

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wav {
    pub rate: u32,
    /// Bytes per sample (1 = unsigned 8-bit, 2 = signed 16-bit LE).
    pub width: u16,
    pub channels: u16,
    /// Number of sample frames (possibly shortened by a loop length marker).
    pub samples: usize,
    /// Loop start in sample frames, `None` if not looped.
    pub loop_start: Option<usize>,
    /// Raw PCM data (`samples * width * channels` bytes, clamped to available data).
    pub data: Vec<u8>,
}

fn le32(d: &[u8], o: usize) -> Option<u32> {
    d.get(o..o + 4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
}
fn le16(d: &[u8], o: usize) -> Option<u16> {
    d.get(o..o + 2)
        .map(|b| u16::from_le_bytes(b.try_into().unwrap()))
}

/// Iterate chunks in `d[start..]`; returns (offset of chunk header, body len) of the first
/// chunk named `name` at or after `from`.
fn find_chunk(d: &[u8], from: usize, name: &[u8; 4]) -> Option<(usize, usize)> {
    let mut p = from;
    while p + 8 <= d.len() {
        let len = le32(d, p + 4)? as usize;
        if len > d.len() - (p + 8) {
            return None;
        }
        if &d[p..p + 4] == name {
            return Some((p, len));
        }
        p += 8 + ((len + 1) & !1);
    }
    None
}

impl Wav {
    pub fn parse(d: &[u8]) -> Result<Wav> {
        let e = |m: &str| Error::new("wav", m.to_string());
        if d.len() < 12 || &d[0..4] != b"RIFF" || &d[8..12] != b"WAVE" {
            return Err(e("missing RIFF/WAVE chunks"));
        }
        let base = 12;
        let (fmt, flen) = find_chunk(d, base, b"fmt ").ok_or_else(|| e("missing fmt chunk"))?;
        if flen < 16 {
            return Err(e("fmt chunk too short"));
        }
        let f = fmt + 8;
        if le16(d, f) != Some(1) {
            return Err(e("not PCM format"));
        }
        let channels = le16(d, f + 2).ok_or_else(|| e("truncated fmt"))?;
        let rate = le32(d, f + 4).ok_or_else(|| e("truncated fmt"))?;
        let bits = le16(d, f + 14).ok_or_else(|| e("truncated fmt"))?;
        if bits != 8 && bits != 16 {
            return Err(e("unsupported bit depth"));
        }
        if channels == 0 {
            return Err(e("zero channels"));
        }
        let width = bits / 8;

        let mut loop_start = None;
        let mut loop_samples = None;
        if let Some((cue, clen)) = find_chunk(d, base, b"cue ")
            && let Some(ls) = le32(d, cue + 32)
        {
            {
                loop_start = Some(ls as usize);
                let next = cue + 8 + ((clen + 1) & !1);
                if let Some((list, _)) = find_chunk(d, next, b"LIST")
                    && d.get(list + 28..list + 32) == Some(b"mark")
                    && let Some(n) = le32(d, list + 24)
                {
                    loop_samples = Some(ls as usize + n as usize);
                }
            }
        }
        let (data, dlen) = find_chunk(d, base, b"data").ok_or_else(|| e("missing data chunk"))?;
        let frame = width as usize * channels as usize;
        let total = dlen / frame;
        let mut samples = match loop_samples {
            Some(s) if s > total => return Err(e("bad loop length")),
            Some(s) => s,
            None => total,
        };
        if loop_start.is_some_and(|l| l >= samples) {
            loop_start = None;
            samples = total;
        }
        let pcm = d[data + 8..data + 8 + samples * frame].to_vec();
        Ok(Wav {
            rate,
            width,
            channels,
            samples,
            loop_start,
            data: pcm,
        })
    }
}
