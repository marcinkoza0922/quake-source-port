//! Background music streaming (bgmusic.c / snd_codec.c equivalent) via symphonia.

use std::io::Cursor;
use std::sync::Arc;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{Decoder, DecoderOptions};
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::{MediaSourceStream, MediaSourceStreamOptions};
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Extensions tried in order, as in bgmusic.c's handler list.
pub const MUSIC_EXTS: [&str; 4] = ["ogg", "mp3", "flac", "wav"];

/// Candidate paths for a CD track (`music/trackNN.ext`).
pub fn cd_track_paths(track: u8) -> Vec<String> {
    MUSIC_EXTS
        .iter()
        .map(|e| format!("music/track{track:02}.{e}"))
        .collect()
}

/// A decoding music stream, resampled to the output rate, stereo.
pub struct MusicStream {
    bytes: Arc<[u8]>,
    ext: String,
    looping: bool,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track_id: u32,
    in_rate: u32,
    out_rate: u32,
    /// Decoded interleaved stereo frames at input rate.
    buf: Vec<[f32; 2]>,
    /// Fractional read position into `buf` (input frames).
    pos: f64,
    pub paused: bool,
    pub finished: bool,
}

type Opened = (Box<dyn FormatReader>, Box<dyn Decoder>, u32, u32);

fn open(bytes: &Arc<[u8]>, ext: &str) -> Option<Opened> {
    let src = Cursor::new(bytes.clone());
    let mss = MediaSourceStream::new(Box::new(src), MediaSourceStreamOptions::default());
    let mut hint = Hint::new();
    hint.with_extension(ext);
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    let format = probed.format;
    let track = format.default_track()?;
    let rate = track.codec_params.sample_rate?;
    let decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .ok()?;
    let id = track.id;
    Some((format, decoder, id, rate))
}

impl MusicStream {
    /// Build a stream from file bytes; `ext` is the file extension (format hint).
    pub fn new(bytes: Vec<u8>, ext: &str, looping: bool, out_rate: u32) -> Option<Self> {
        let bytes: Arc<[u8]> = bytes.into();
        let (format, decoder, track_id, in_rate) = open(&bytes, ext)?;
        Some(Self {
            bytes,
            ext: ext.to_owned(),
            looping,
            format,
            decoder,
            track_id,
            in_rate,
            out_rate,
            buf: Vec::new(),
            pos: 0.0,
            paused: false,
            finished: false,
        })
    }

    /// Decode the next packet into `buf`. Returns false at end of stream.
    fn refill(&mut self) -> bool {
        loop {
            let pkt = match self.format.next_packet() {
                Ok(p) => p,
                Err(_) => return false,
            };
            if pkt.track_id() != self.track_id {
                continue;
            }
            let Ok(decoded) = self.decoder.decode(&pkt) else {
                continue;
            };
            let spec = *decoded.spec();
            let ch = spec.channels.count().max(1);
            let mut sb = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
            sb.copy_interleaved_ref(decoded);
            for f in sb.samples().chunks_exact(ch) {
                let l = f[0];
                let r = if ch > 1 { f[1] } else { l };
                self.buf.push([l, r]);
            }
            return true;
        }
    }

    fn rewind(&mut self) -> bool {
        match open(&self.bytes, &self.ext) {
            Some((f, d, t, r)) => {
                self.format = f;
                self.decoder = d;
                self.track_id = t;
                self.in_rate = r;
                true
            }
            None => false,
        }
    }

    /// Add `frames` output frames into `out` (i32 stereo, 16-bit scale) at `vol` (0..1).
    pub fn mix(&mut self, out: &mut [[i32; 2]], vol: f32) {
        if self.paused || self.finished {
            return;
        }
        let step = f64::from(self.in_rate) / f64::from(self.out_rate);
        let scale = vol * 32767.0;
        for o in out.iter_mut() {
            while self.pos as usize + 1 >= self.buf.len() {
                // keep the last frame for interpolation
                let keep = self.buf.len().saturating_sub(1).min(self.pos as usize);
                self.buf.drain(..keep);
                self.pos -= keep as f64;
                if !self.refill() {
                    if self.looping && self.rewind() && self.refill() {
                        continue;
                    }
                    self.finished = true;
                    return;
                }
            }
            let i = self.pos as usize;
            let t = (self.pos - i as f64) as f32;
            let (a, b) = (self.buf[i], self.buf[i + 1]);
            o[0] += ((a[0] + (b[0] - a[0]) * t) * scale) as i32;
            o[1] += ((a[1] + (b[1] - a[1]) * t) * scale) as i32;
            self.pos += step;
        }
    }
}
