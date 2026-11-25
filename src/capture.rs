//! Video capture: frames read back from the renderer and audio painted by
//! the shared mixer go to an encoder on a worker thread. With ffmpeg on the
//! PATH (or `capture_ffmpeg`), video is piped to it as raw RGBA and audio
//! written to a WAV beside it; finishing muxes both into one file in the
//! background. Without ffmpeg the frames are written as numbered PNGs plus
//! `audio.wav` for an external editor. Not available in the web build.

mod ffmpeg;
mod wav;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::thread::JoinHandle;

pub use ffmpeg::Codec;

/// Frames queued for the encoder before the game waits for it.
const QUEUE: usize = 3;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub fps: u32,
    pub codec: Codec,
    /// 0 high, 1 medium, 2 small.
    pub quality: u8,
    pub audio: bool,
    /// ffmpeg executable.
    pub ffmpeg: String,
    /// ffmpeg encoder name overriding the codec's default (e.g. `h264_nvenc`).
    pub encoder: String,
    /// Output size (ffmpeg scales and pads); `None` keeps the window's.
    pub size: Option<(u32, u32)>,
    /// Use a hardware encoder when ffmpeg can (`capture_gpu`).
    pub gpu: bool,
}

/// `capture_size` 1..4: 720p, 1080p, 1440p, 4K; 0 keeps the window size.
pub fn size_from_cvar(v: f32) -> Option<(u32, u32)> {
    match v.round() as i32 {
        1 => Some((1280, 720)),
        2 => Some((1920, 1080)),
        3 => Some((2560, 1440)),
        4 => Some((3840, 2160)),
        _ => None,
    }
}

enum Sink {
    /// Writes raw frames to ffmpeg's stdin; the thread returns its result.
    Ffmpeg { tx: Option<SyncSender<Vec<u8>>>, thread: Option<JoinHandle<Result<(), String>>>, video: PathBuf },
    /// Numbered PNGs in a folder.
    Png { tx: Option<SyncSender<Vec<u8>>>, thread: Option<JoinHandle<Result<(), String>>>, dir: PathBuf },
}

pub struct Recorder {
    sink: Sink,
    /// The ffmpeg encoder in use (empty for PNG frames).
    pub encoder: String,
    wav: Option<wav::Writer>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub frames: u64,
    /// The finished file (or frame folder).
    pub out: PathBuf,
    settings: Settings,
}

/// `dir/name.ext`, or `name_2.ext`... when that exists.
pub fn unique(dir: &Path, name: &str, ext: &str) -> PathBuf {
    let p = dir.join(format!("{name}{ext}"));
    if !p.exists() {
        return p;
    }
    (2..).map(|i| dir.join(format!("{name}_{i}{ext}"))).find(|p| !p.exists()).unwrap()
}

impl Recorder {
    /// Starts a capture of `w`×`h` frames into `dir` (created if missing).
    /// `rate` is the mixer rate when audio is captured.
    pub fn start(dir: &Path, name: &str, w: u32, h: u32, settings: &Settings, rate: Option<u32>) -> Result<Self, String> {
        Self::start_at(&unique(dir, name, settings.codec.extension()), w, h, settings, rate)
    }

    /// Like [`Self::start`] writing exactly `out` (its extension follows the
    /// codec; an existing file is replaced). Without ffmpeg the frames go to
    /// `<out stem>_frames/` beside it.
    pub fn start_at(out: &Path, w: u32, h: u32, settings: &Settings, rate: Option<u32>) -> Result<Self, String> {
        if cfg!(target_arch = "wasm32") {
            return Err("video capture is not available in the web build".into());
        }
        let out = out.with_extension(settings.codec.extension().trim_start_matches('.'));
        let dir = out.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
        let name = out.file_stem().map_or_else(|| "video".into(), |s| s.to_string_lossy().into_owned());
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let (w, h) = (w & !1, h & !1);
        if w == 0 || h == 0 {
            return Err("nothing to capture".into());
        }
        let fps = settings.fps.clamp(10, 240);
        let use_ffmpeg = ffmpeg::available(&settings.ffmpeg);
        let mut encoder = String::new();
        let (sink, out) = if use_ffmpeg {
            let video = out.with_extension(format!("video{}", ffmpeg::TEMP_EXT));
            let plan = ffmpeg::choose(settings);
            encoder = plan.encoder.clone();
            let child = ffmpeg::spawn_encoder(settings, &plan, w, h, fps, &video)?;
            let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE);
            let thread = std::thread::spawn(move || ffmpeg::feed(child, rx));
            (Sink::Ffmpeg { tx: Some(tx), thread: Some(thread), video }, out)
        } else {
            let dir = unique(dir, &format!("{name}_frames"), "");
            std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            let (tx, rx) = sync_channel::<Vec<u8>>(QUEUE);
            let d = dir.clone();
            let thread = std::thread::spawn(move || write_pngs(&d, w, h, rx));
            (Sink::Png { tx: Some(tx), thread: Some(thread), dir: dir.clone() }, dir)
        };
        let wav = match (settings.audio, rate) {
            (true, Some(rate)) => {
                let path = match &sink {
                    Sink::Ffmpeg { video, .. } => video.with_extension("wav"),
                    Sink::Png { dir, .. } => dir.join("audio.wav"),
                };
                Some(wav::Writer::create(&path, rate).map_err(|e| format!("{}: {e}", path.display()))?)
            }
            _ => None,
        };
        Ok(Self { sink, encoder, wav, width: w, height: h, fps, frames: 0, out, settings: settings.clone() })
    }

    #[cfg(test)]
    pub fn uses_ffmpeg(&self) -> bool {
        matches!(self.sink, Sink::Ffmpeg { .. })
    }

    /// Queues one RGBA frame; a frame of another size is cropped or padded
    /// to the capture size.
    pub fn frame(&mut self, w: u32, h: u32, px: &[u8]) -> Result<(), String> {
        let frame = fit(w, h, px, self.width, self.height);
        let tx = match &self.sink {
            Sink::Ffmpeg { tx, .. } | Sink::Png { tx, .. } => tx.as_ref(),
        };
        let tx = tx.ok_or("encoder closed")?;
        if tx.send(frame).is_err() {
            return Err(self.join().err().unwrap_or_else(|| "encoder stopped".into()));
        }
        self.frames += 1;
        Ok(())
    }

    pub fn audio(&mut self, samples: &[i16]) {
        if let Some(w) = &mut self.wav
            && w.write(samples).is_err()
        {
            self.wav = None;
        }
    }

    pub fn has_audio(&self) -> bool {
        self.wav.is_some()
    }

    /// Closes the encoder and waits for it.
    fn join(&mut self) -> Result<(), String> {
        let (tx, thread) = match &mut self.sink {
            Sink::Ffmpeg { tx, thread, .. } | Sink::Png { tx, thread, .. } => (tx, thread),
        };
        drop(tx.take());
        thread.take().map_or(Ok(()), |t| t.join().unwrap_or_else(|_| Err("encoder thread panicked".into())))
    }

    /// Stops and deletes everything written: the temporary video and WAV,
    /// a PNG frame folder. The output isn't written before [`Self::finish`],
    /// so a file of that name is somebody's earlier video.
    pub fn abort(mut self) {
        let _ = self.join();
        let wav = self.wav.take().map(|w| w.finish());
        if let Some(Ok(p)) = wav {
            let _ = std::fs::remove_file(p);
        }
        match &self.sink {
            Sink::Ffmpeg { video, .. } => {
                let _ = std::fs::remove_file(video);
            }
            Sink::Png { dir, .. } => {
                let _ = std::fs::remove_dir_all(dir);
            }
        }
    }

    /// Ends the capture. The returned channel reports the outcome once the
    /// final file is written (muxing runs on another thread).
    pub fn finish(mut self) -> Receiver<String> {
        let (tx, rx) = std::sync::mpsc::channel();
        let joined = self.join();
        let wav = self.wav.take().map(|w| w.finish());
        let (frames, fps, out) = (self.frames, self.fps, self.out.clone());
        let secs = frames as f64 / f64::from(fps);
        match (&self.sink, joined) {
            (_, Err(e)) => {
                let _ = tx.send(format!("capture failed: {e}\n"));
            }
            (Sink::Png { .. }, Ok(())) => {
                let _ = tx.send(qcore::ctr!("ffmpeg not found: wrote {} PNG frames and a WAV to {}\n", frames, out.display()));
            }
            (Sink::Ffmpeg { video, .. }, Ok(())) => {
                let (video, settings) = (video.clone(), self.settings.clone());
                let wav = wav.and_then(Result::ok);
                std::thread::spawn(move || {
                    let msg = match ffmpeg::mux(&settings, &video, wav.as_deref(), &out) {
                        Ok(()) => qcore::ctr!("Wrote {} ({}s)\n", out.display(), format!("{secs:.1}")),
                        Err(e) => format!("capture failed: {e}\n"),
                    };
                    let _ = std::fs::remove_file(&video);
                    if let Some(w) = wav {
                        let _ = std::fs::remove_file(w);
                    }
                    let _ = tx.send(msg);
                });
            }
        }
        rx
    }
}

/// Copies `px` (w×h RGBA) into a `cw`×`ch` frame, cropping or padding black.
fn fit(w: u32, h: u32, px: &[u8], cw: u32, ch: u32) -> Vec<u8> {
    if (w, h) == (cw, ch) && px.len() == (w * h * 4) as usize {
        return px.to_vec();
    }
    let mut out = vec![0u8; (cw * ch * 4) as usize];
    let (cols, rows) = (w.min(cw) as usize, h.min(ch) as usize);
    for y in 0..rows {
        let src = &px[y * w as usize * 4..][..cols * 4];
        out[y * cw as usize * 4..][..cols * 4].copy_from_slice(src);
    }
    out.as_chunks_mut::<4>().0.iter_mut().for_each(|p| p[3] = 255);
    out
}

fn write_pngs(dir: &Path, w: u32, h: u32, rx: Receiver<Vec<u8>>) -> Result<(), String> {
    for (i, f) in rx.into_iter().enumerate() {
        let p = dir.join(format!("frame_{:06}.png", i + 1));
        std::fs::write(&p, crate::png::encode_rgba(w, h, &f)).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fit_crops_and_pads() {
        let px: Vec<u8> = (0..3 * 2 * 4).map(|i| i as u8).collect();
        let out = fit(3, 2, &px, 2, 3);
        assert_eq!(out.len(), 2 * 3 * 4);
        assert_eq!(&out[..8], &[0, 1, 2, 255, 4, 5, 6, 255]);
        assert_eq!(&out[8..12], &[12, 13, 14, 255]);
        assert_eq!(&out[16..], &[0, 0, 0, 255, 0, 0, 0, 255]);
        assert_eq!(fit(3, 2, &px, 3, 2), px);
    }

    #[test]
    fn png_fallback_writes_frames_and_wav() {
        let dir = std::env::temp_dir().join(format!("qsp-capture-{}", std::process::id()));
        let s = Settings { fps: 30, codec: Codec::H264, quality: 0, audio: true, ffmpeg: "qsp-no-such-ffmpeg".into(), encoder: String::new(), size: None, gpu: false };
        let mut r = Recorder::start(&dir, "clip", 5, 4, &s, Some(48000)).unwrap();
        assert!(!r.uses_ffmpeg());
        assert_eq!((r.width, r.height), (4, 4));
        r.frame(5, 4, &[7; 5 * 4 * 4]).unwrap();
        r.audio(&[1, -1, 2, -2]);
        let msg = r.finish().recv().unwrap();
        let frames = dir.join("clip_frames");
        assert!(frames.join("frame_000001.png").exists(), "{msg}");
        let wav = std::fs::read(frames.join("audio.wav")).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(wav.len(), 44 + 8);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
