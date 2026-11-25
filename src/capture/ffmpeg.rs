//! The ffmpeg side of capture: the encoder (a GPU one when ffmpeg can use
//! it, else the CPU), its arguments per codec and quality, the raw-frame
//! pipe, and the final mux with the WAV. ffmpeg runs at low priority with
//! two cores left free, so the game keeps running smoothly while it encodes.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, OnceLock};

use super::Settings;

/// The video-only temporary file next to the output.
pub const TEMP_EXT: &str = ".mkv";
/// The VA-API device (Linux).
const VAAPI_DEVICE: &str = "/dev/dri/renderD128";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    H264,
    Hevc,
    Av1,
    Vp9,
}

impl Codec {
    /// `capture_codec` 0..3.
    pub fn from_cvar(v: f32) -> Self {
        match v.round() as i32 {
            1 => Self::Hevc,
            2 => Self::Av1,
            3 => Self::Vp9,
            _ => Self::H264,
        }
    }

    pub fn extension(self) -> &'static str {
        if self == Self::Vp9 { ".webm" } else { ".mp4" }
    }

    fn cpu_encoder(self) -> &'static str {
        match self {
            Self::H264 => "libx264",
            Self::Hevc => "libx265",
            Self::Av1 => "libsvtav1",
            Self::Vp9 => "libvpx-vp9",
        }
    }

    /// Hardware encoders to try, in order.
    fn gpu_encoders(self) -> &'static [&'static str] {
        match self {
            Self::H264 => &["h264_nvenc", "h264_vaapi", "h264_videotoolbox", "h264_amf", "h264_qsv", "h264_v4l2m2m"],
            Self::Hevc => &["hevc_nvenc", "hevc_vaapi", "hevc_videotoolbox", "hevc_amf", "hevc_qsv", "hevc_v4l2m2m"],
            Self::Av1 => &["av1_nvenc", "av1_vaapi", "av1_amf", "av1_qsv"],
            Self::Vp9 => &["vp9_vaapi", "vp9_qsv"],
        }
    }
}

/// How ffmpeg is run for one encoder.
#[derive(Debug, PartialEq)]
pub struct Plan {
    pub encoder: String,
    /// Before `-i` (device setup).
    pub pre_input: Vec<String>,
    /// `-vf` filters after the scaling (hardware upload).
    pub filters: Vec<String>,
    /// After `-c:v <encoder>`.
    pub args: Vec<String>,
}

/// Arguments for `encoder` at quality 0..2 (high, medium, small).
pub fn plan_for(encoder: &str, quality: u8) -> Plan {
    let q = usize::from(quality.min(2));
    let pick = |v: [u8; 3]| v[q].to_string();
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    let mut p = Plan { encoder: encoder.to_string(), pre_input: Vec::new(), filters: Vec::new(), args: Vec::new() };
    let yuv = s(&["-pix_fmt", "yuv420p"]);
    match encoder {
        "libx264" => p.args = [s(&["-preset", "veryfast", "-crf"]), vec![pick([16, 20, 26])], yuv].concat(),
        "libx265" => p.args = [s(&["-preset", "fast", "-crf"]), vec![pick([20, 24, 30])], s(&["-tag:v", "hvc1"]), yuv].concat(),
        "libsvtav1" => p.args = [s(&["-preset", "8", "-crf"]), vec![pick([28, 35, 45])], yuv].concat(),
        "libvpx-vp9" => {
            p.args = [s(&["-b:v", "0", "-crf"]), vec![pick([24, 32, 40])], s(&["-deadline", "realtime", "-cpu-used", "8", "-row-mt", "1"]), yuv].concat()
        }
        e if e.ends_with("_nvenc") => p.args = [s(&["-preset", "p5", "-rc", "vbr", "-b:v", "0", "-cq"]), vec![pick([19, 24, 30])], yuv].concat(),
        e if e.ends_with("_vaapi") => {
            p.pre_input = s(&["-vaapi_device", VAAPI_DEVICE]);
            p.filters = s(&["format=nv12", "hwupload"]);
            p.args = vec!["-qp".into(), pick([20, 25, 30])];
        }
        e if e.ends_with("_videotoolbox") => p.args = [vec!["-q:v".to_string(), pick([70, 58, 45])], yuv].concat(),
        e if e.ends_with("_amf") => p.args = [s(&["-rc", "cqp", "-qp_i"]), vec![pick([20, 25, 30])], s(&["-qp_p"]), vec![pick([22, 27, 32])], yuv].concat(),
        // Arm boards (Raspberry Pi, Rockchip/Mali): bitrate only
        e if e.ends_with("_v4l2m2m") => p.args = [vec!["-b:v".to_string(), format!("{}M", [24, 12, 6][q])], yuv].concat(),
        e if e.ends_with("_qsv") => p.args = [vec!["-global_quality".to_string(), pick([20, 25, 30])], s(&["-pix_fmt", "nv12"])].concat(),
        // other encoders (capture_encoder): ffmpeg's defaults
        _ => p.args = yuv,
    }
    // CPU encoders leave two cores to the game
    if encoder.starts_with("lib") {
        let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
        p.args.extend(["-threads".into(), cores.saturating_sub(2).max(1).to_string()]);
    }
    p
}

/// `-vf` value: scaling to `size` (keeping the shape) and the plan's filters.
pub fn filter_chain(size: Option<(u32, u32)>, from: (u32, u32), plan: &Plan) -> Option<String> {
    let mut f = Vec::new();
    if let Some((w, h)) = size.filter(|&s| s != from) {
        f.push(format!("scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos"));
        f.push(format!("pad={w}:{h}:(ow-iw)/2:(oh-ih)/2"));
    }
    f.extend(plan.filters.iter().cloned());
    (!f.is_empty()).then(|| f.join(","))
}

/// A command at low priority: `nice` on Unix, below normal on Windows.
fn low_priority(program: &str) -> Command {
    #[cfg(unix)]
    if Command::new("nice").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok() {
        let mut c = Command::new("nice");
        c.args(["-n", "10", program]);
        return c;
    }
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        c.creation_flags(BELOW_NORMAL_PRIORITY_CLASS);
    }
    c
}

/// True when `ffmpeg -version` runs.
pub fn available(ffmpeg: &str) -> bool {
    !ffmpeg.trim().is_empty()
        && Command::new(ffmpeg.trim()).arg("-version").stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success())
}

/// Whether ffmpeg can really use `encoder` here: a tiny test encode
/// (an encoder can be built in without the hardware). Cached.
fn works(ffmpeg: &str, encoder: &str) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<(String, String), bool>>> = OnceLock::new();
    let key = (ffmpeg.to_string(), encoder.to_string());
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(&ok) = cache.lock().unwrap().get(&key) {
        return ok;
    }
    let plan = plan_for(encoder, 1);
    let mut c = Command::new(ffmpeg);
    c.args(["-hide_banner", "-loglevel", "error"]).args(&plan.pre_input).args(["-f", "lavfi", "-i", "color=c=black:s=256x256:d=0.1"]);
    if let Some(f) = filter_chain(None, (256, 256), &plan) {
        c.args(["-vf", &f]);
    }
    let ok = c.args(["-c:v", encoder]).args(&plan.args).args(["-f", "null", "-"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status().is_ok_and(|s| s.success());
    cache.lock().unwrap().insert(key, ok);
    ok
}

/// The encoder for these settings: the override, else the first working
/// GPU encoder when `gpu` is on, else the CPU one.
pub fn choose(s: &Settings) -> Plan {
    let over = s.encoder.trim();
    if !over.is_empty() {
        return plan_for(over, s.quality);
    }
    if s.gpu
        && let Some(e) = s.codec.gpu_encoders().iter().find(|e| works(s.ffmpeg.trim(), e))
    {
        return plan_for(e, s.quality);
    }
    plan_for(s.codec.cpu_encoder(), s.quality)
}

pub fn spawn_encoder(s: &Settings, plan: &Plan, w: u32, h: u32, fps: u32, video: &Path) -> Result<Child, String> {
    let mut c = low_priority(s.ffmpeg.trim());
    c.args(["-hide_banner", "-loglevel", "error", "-y"]).args(&plan.pre_input).args(["-f", "rawvideo", "-pix_fmt", "rgba"]);
    c.args(["-s", &format!("{w}x{h}"), "-r", &fps.to_string(), "-i", "-", "-an"]);
    if let Some(f) = filter_chain(s.size, (w, h), plan) {
        c.args(["-vf", &f]);
    }
    c.args(["-c:v", &plan.encoder]).args(&plan.args).arg(video).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::piped());
    c.spawn().map_err(|e| format!("couldn't start {}: {e}", s.ffmpeg))
}

/// The encoder, for the thread that feeds it and whoever must end it.
pub type Shared = Arc<Mutex<Child>>;

pub fn share(child: Child) -> Shared {
    Arc::new(Mutex::new(child))
}

pub fn kill(child: &Shared) {
    let _ = child.lock().unwrap_or_else(|e| e.into_inner()).kill();
}

/// Reads all of it, so that the encoder never waits; gives the end.
fn tail(r: Option<impl Read>) -> String {
    const KEEP: usize = 16 << 10;
    let mut kept = Vec::new();
    let mut buf = [0u8; 4096];
    if let Some(mut r) = r {
        while let Ok(n @ 1..) = r.read(&mut buf) {
            kept.extend_from_slice(&buf[..n]);
            if kept.len() > 2 * KEEP {
                kept.drain(..kept.len() - KEEP);
            }
        }
    }
    String::from_utf8_lossy(&kept).into_owned()
}

/// Writes frames to the encoder until the channel closes.
pub fn feed(child: &Shared, rx: Receiver<Vec<u8>>) -> Result<(), String> {
    let lock = || child.lock().unwrap_or_else(|e| e.into_inner());
    let (stdin, stderr) = {
        let mut c = lock();
        (c.stdin.take(), c.stderr.take())
    };
    let mut stdin = stdin.ok_or("no ffmpeg stdin")?;
    let errors = std::thread::spawn(move || tail(stderr));
    let mut err = None;
    for f in rx {
        if let Err(e) = stdin.write_all(&f) {
            err = Some(e.to_string());
            break;
        }
    }
    drop(stdin);
    // (polled: `kill` needs the lock while it hangs)
    let status = loop {
        if let Some(s) = lock().try_wait().map_err(|e| e.to_string())? {
            break s;
        }
        std::thread::sleep(qcore::time::Duration::from_millis(5));
    };
    let stderr = errors.join().unwrap_or_default();
    if !status.success() || err.is_some() {
        let msg = stderr.lines().last().unwrap_or_default().to_string();
        return Err(if msg.is_empty() { err.unwrap_or_else(|| format!("ffmpeg exited with {status}")) } else { msg });
    }
    Ok(())
}

/// Copies the video into `out`, adding the WAV as AAC (Opus in WebM).
/// `out` is only replaced by a finished file.
pub fn mux(s: &Settings, video: &Path, wav: Option<&Path>, out: &Path) -> Result<(), String> {
    // same extension: ffmpeg picks the container by it
    let ext = out.extension().map_or_else(String::new, |e| e.to_string_lossy().into_owned());
    let part = out.with_extension(format!("part.{ext}"));
    let r = mux_to(s, video, wav, &part).and_then(|()| std::fs::rename(&part, out).map_err(|e| format!("{}: {e}", out.display())));
    if r.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    r
}

fn mux_to(s: &Settings, video: &Path, wav: Option<&Path>, out: &Path) -> Result<(), String> {
    let mut c = low_priority(s.ffmpeg.trim());
    c.args(["-hide_banner", "-loglevel", "error", "-y", "-i"]).arg(video);
    if let Some(w) = wav {
        c.arg("-i").arg(w);
        let audio: &[&str] = if s.codec == Codec::Vp9 { &["-c:a", "libopus", "-b:a", "160k"] } else { &["-c:a", "aac", "-b:a", "192k"] };
        c.args(["-map", "0:v", "-map", "1:a", "-c:v", "copy"]).args(audio).arg("-shortest");
    } else {
        c.args(["-c", "copy"]);
    }
    if s.codec != Codec::Vp9 {
        c.args(["-movflags", "+faststart"]);
    }
    let r = c.arg(out).stdin(Stdio::null()).stdout(Stdio::null()).output().map_err(|e| e.to_string())?;
    if r.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&r.stderr).lines().last().unwrap_or("ffmpeg mux failed").to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(codec: Codec, quality: u8, encoder: &str, gpu: bool) -> Settings {
        Settings { fps: 60, codec, quality, audio: true, ffmpeg: "ffmpeg".into(), encoder: encoder.into(), size: None, gpu }
    }

    #[test]
    fn encoder_plans() {
        let p = plan_for("libx264", 1);
        assert_eq!(p.args[..4], ["-preset", "veryfast", "-crf", "20"]);
        assert!(p.args.contains(&"-threads".to_string()));
        assert!(plan_for("libvpx-vp9", 2).args.contains(&"40".to_string()));
        let v = plan_for("h264_vaapi", 0);
        assert_eq!(v.pre_input[0], "-vaapi_device");
        assert_eq!(filter_chain(Some((1920, 1080)), (1280, 720), &v).unwrap().split(',').count(), 4);
        assert!(!plan_for("hevc_nvenc", 0).args.contains(&"-threads".to_string()));
        assert_eq!(plan_for(" h264_nvenc ".trim(), 0).encoder, "h264_nvenc");
        assert_eq!(filter_chain(Some((1280, 720)), (1280, 720), &plan_for("libx264", 0)), None);
        assert_eq!(choose(&settings(Codec::H264, 0, "h264_qsv", true)).encoder, "h264_qsv");
        assert_eq!(choose(&settings(Codec::Vp9, 0, "", false)).encoder, "libvpx-vp9");
        assert_eq!(Codec::from_cvar(3.0).extension(), ".webm");
        assert_eq!(Codec::from_cvar(9.0), Codec::H264);
        assert_eq!(super::super::size_from_cvar(2.0), Some((1920, 1080)));
    }

    /// Encodes and muxes a short clip when ffmpeg with libx264 is installed.
    #[test]
    fn encodes_with_ffmpeg_when_installed() {
        let s = settings(Codec::H264, 2, "", true);
        if !available(&s.ffmpeg) {
            return;
        }
        let dir = std::env::temp_dir().join(format!("qsp-ffmpeg-{}", std::process::id()));
        let mut r = super::super::Recorder::start(&dir, "t", 64, 48, &s, Some(48000)).unwrap();
        for i in 0..10u8 {
            r.frame(64, 48, &vec![i * 20; 64 * 48 * 4]).unwrap();
            r.audio(&vec![0; 800 * 2]);
        }
        let out = r.out.clone();
        std::fs::write(&out, b"old video").unwrap();
        let msg = r.finish().recv().unwrap();
        let ok = std::fs::read(&out).is_ok_and(|d| d != b"old video");
        let left = std::fs::read_dir(&dir).unwrap().count();
        assert!(!ok || left == 1, "only the video stays");
        let _ = std::fs::remove_dir_all(&dir);
        // an ffmpeg without the encoder fails cleanly; one with it must succeed
        assert!(ok || msg.contains("failed"), "{msg}");
    }

    /// A cancelled export to the name of an earlier video leaves that alone,
    /// and so does one that can't be muxed.
    #[test]
    fn an_earlier_video_survives() {
        let s = settings(Codec::H264, 2, "", false);
        let dir = std::env::temp_dir().join(format!("qsp-ffmpeg-keep-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("holiday.mp4");
        std::fs::write(&out, b"holiday").unwrap();
        assert!(mux(&s, &dir.join("missing.mkv"), None, &out).is_err());
        if available(&s.ffmpeg) {
            let mut r = super::super::Recorder::start_at(&out, 64, 48, &s, None).unwrap();
            r.frame(64, 48, &[9; 64 * 48 * 4]).unwrap();
            r.abort();
        }
        let kept = std::fs::read(&out).unwrap();
        let left = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((kept.as_slice(), left), (&b"holiday"[..], 1));
    }

    #[test]
    fn abort_leaves_nothing() {
        let s = settings(Codec::H264, 2, "", false);
        if !available(&s.ffmpeg) {
            return;
        }
        let dir = std::env::temp_dir().join(format!("qsp-ffmpeg-abort-{}", std::process::id()));
        let mut r = super::super::Recorder::start(&dir, "t", 64, 48, &s, Some(48000)).unwrap();
        r.frame(64, 48, &[9; 64 * 48 * 4]).unwrap();
        r.audio(&[0; 1600]);
        r.abort();
        let left: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(left.is_empty(), "{left:?}");
    }
}
