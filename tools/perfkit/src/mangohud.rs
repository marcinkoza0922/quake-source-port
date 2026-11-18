//! MangoHud per-frame logs of exactly a timedemo's frames.
//!
//! MangoHud (0.8) only writes its log when logging stops, so a SIGKILLed
//! engine would lose it, and `no_display` disables logging altogether. So
//! the runner shows the smallest HUD (`fps_only`) and drives logging over
//! MangoHud's control socket (`control=<name>`, an abstract unix socket):
//! `:logging=1;` when the engine prints "Playing demo from", `:logging=0;`
//! when it prints the timedemo result, then it waits for MangoHud's
//! `:LoggingFinished` reply (the CSV is on disk) before killing the engine.
//!
//! MangoHud handles control commands only at a frame present, and only once
//! per `fps_sampling_period` (default 500 ms, which started logs up to half
//! a second late). With a 1 ms period the command sent at "Playing demo
//! from" takes effect within a frame. The references present a few more
//! console frames before loading, so their log starts with those, then the
//! long frame of the map load, then the demo; ours starts with the demo.
//! Logging stops a few frames after the demo. So the timedemo's frames are
//! the first `frames` frames after the load frame, or from the start when
//! there is none ([`demo_segment`]), checked against the reported seconds
//! ([`aligned`]). The 1 ms period costs frame time; `perfkit calibrate`
//! measures how much.

use std::io::{Read, Write};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixStream};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// `MANGOHUD_CONFIG` for a controlled per-frame log into `out`.
pub fn config(out: &Path, control: &str) -> String {
    format!("output_folder={},log_interval=0,fps_sampling_period=1,fps_only,control={control}", out.display())
}

pub struct Control {
    sock: UnixStream,
}

impl Control {
    /// Connects to the socket `name`, retrying until `timeout` (the layer
    /// creates it when the engine creates its Vulkan device or GL context).
    pub fn connect(name: &str, timeout: Duration) -> std::io::Result<Control> {
        let addr = SocketAddr::from_abstract_name(name.as_bytes())?;
        let end = Instant::now() + timeout;
        loop {
            match UnixStream::connect_addr(&addr) {
                Ok(sock) => {
                    sock.set_read_timeout(Some(Duration::from_millis(100)))?;
                    return Ok(Control { sock });
                }
                Err(e) if Instant::now() > end => return Err(e),
                Err(_) => std::thread::sleep(Duration::from_millis(20)),
            }
        }
    }

    pub fn start_logging(&mut self) -> std::io::Result<()> {
        self.sock.write_all(b":logging=1;")
    }

    /// Stops logging and waits until MangoHud reports the log written.
    pub fn stop_logging(&mut self, timeout: Duration) -> Result<(), String> {
        self.sock.write_all(b":logging=0;").map_err(|e| format!("mangohud control: {e}"))?;
        let end = Instant::now() + timeout;
        let mut got = Vec::new();
        let mut buf = [0u8; 256];
        while Instant::now() < end {
            match self.sock.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    got.extend_from_slice(&buf[..n]);
                    if got.windows(16).any(|w| w == b":LoggingFinished") {
                        return Ok(());
                    }
                }
                Err(e) if matches!(e.kind(), std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut) => {}
                Err(e) => return Err(format!("mangohud control: {e}")),
            }
        }
        Err("mangohud never reported the log written".into())
    }
}

/// The frame log (not the `_summary.csv`) MangoHud wrote into `dir`.
pub fn find_log(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| {
        let n = p.file_name().unwrap_or_default().to_string_lossy();
        n.ends_with(".csv") && !n.ends_with("_summary.csv")
    })
}

/// Frame times (ms) from a MangoHud CSV: two system-info lines, then a
/// header naming the columns, then one row per frame.
pub fn parse_log(text: &str) -> Result<Vec<f32>, String> {
    let mut lines = text.lines();
    let header = lines.by_ref().find(|l| l.starts_with("fps,")).ok_or("no frame header in MangoHud log")?;
    let col = header.split(',').position(|c| c == "frametime").ok_or("no frametime column")?;
    lines
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split(',').nth(col).and_then(|v| v.parse().ok()).ok_or_else(|| format!("bad log row: {l}")))
        .collect()
}

/// The timedemo's `frames` frames: those after the map-load frame (the
/// longest of the first 16, if it is over 20 ms and 10× the log's median),
/// else from the first frame. Engines count the first frame differently
/// (ours 1501 where the references count 1500), so one frame less is
/// accepted; more than 1% + 20 extra frames means logging stopped late and
/// the log can't be trusted.
pub fn demo_segment(ms: &[f32], frames: usize) -> Result<&[f32], String> {
    let typical = crate::stats::percentile(ms, 50.0).unwrap_or(0.0);
    let start = ms[..ms.len().min(16)]
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .filter(|&(_, &x)| x > 20.0 && x as f64 > 10.0 * typical)
        .map_or(0, |(i, _)| i + 1);
    let rest = &ms[start..];
    if rest.len() + 1 < frames {
        return Err(format!("log has {} frames after the load, timedemo counted {frames}: logging started late", rest.len()));
    }
    if rest.len() > frames + frames / 100 + 20 {
        return Err(format!("log has {} frames after the load, timedemo counted {frames}: logging stopped late", rest.len()));
    }
    Ok(&rest[..rest.len().min(frames)])
}

/// Whether the segment's total time matches the timedemo's reported
/// seconds (printed with one decimal): within 60 ms plus 3%.
pub fn aligned(seg: &[f32], secs: f64) -> bool {
    let total: f64 = seg.iter().map(|&x| x as f64).sum::<f64>() / 1000.0;
    (total - secs).abs() <= 0.06 + 0.03 * secs
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "os,cpu,gpu\nLinux,cpu,gpu\nfps,frametime,cpu_load,elapsed\n\
                       400,2.5,1,100\n1,900.0,1,1000\n1000,1.0,1,1001\n1000,1.0,1,1002\n500,2.0,1,1004\n";

    #[test]
    fn parses_frame_times() {
        assert_eq!(parse_log(LOG).unwrap(), vec![2.5, 900.0, 1.0, 1.0, 2.0]);
        assert!(parse_log("garbage").is_err());
    }

    #[test]
    fn segment_starts_after_the_load_frame() {
        let ms = parse_log(LOG).unwrap();
        assert_eq!(demo_segment(&ms, 3).unwrap(), &[1.0, 1.0, 2.0]);
        assert_eq!(demo_segment(&ms, 4).unwrap().len(), 3, "one frame short is accepted");
        assert!(demo_segment(&ms, 5).is_err(), "started late");
    }

    #[test]
    fn segment_without_load_frame_starts_at_once() {
        let mut ms = vec![1.0f32; 1504];
        ms[0] = 2.5; // a slow first frame is not a load
        assert_eq!(demo_segment(&ms, 1500).unwrap().len(), 1500);
        assert_eq!(demo_segment(&ms, 1500).unwrap()[0], 2.5);
        assert!(demo_segment(&vec![1.0f32; 1600], 1500).is_err(), "stopped late");
    }

    #[test]
    fn alignment_tolerance() {
        let seg = vec![1.0f32; 1500];
        assert!(aligned(&seg, 1.5));
        assert!(aligned(&seg, 1.5 + 0.05));
        assert!(!aligned(&seg, 1.8));
    }
}
