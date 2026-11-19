//! Video recording (`capture`) and demo clip export (`demo_export`). While
//! a capture runs the host steps at exactly 1/`capture_fps` per frame, live
//! captures paced to real time, exports as fast as the machine renders.
//! Frames come from the renderer's read-back after each render; audio is
//! painted from the shared mixer in step with them. Encoding: `crate::capture`.

use std::path::Path;

use super::*;
use crate::capture::{Codec, Recorder, Settings as CaptureSettings};
use crate::demo::playback::{format_time, parse_time};

pub(super) struct CaptureState {
    /// `None` until an export reaches its first frame.
    rec: Option<Recorder>,
    name: String,
    /// Exact output file (export dialog); else `videos/<name>`.
    out: Option<PathBuf>,
    audio_frac: f64,
    pub export: Option<Export>,
}

/// What the export dialog will export.
pub(super) struct ExportReq {
    demo: String,
    /// The highlighted clip, if any.
    clip: Option<(f64, f64)>,
    resume: Option<(String, f64)>,
    from_menu: bool,
}

pub(super) struct Export {
    start: f64,
    end: Option<f64>,
    /// Demo and time to go back to (exported from the playback bar).
    resume: Option<(String, f64)>,
    /// Open the Demos menu afterwards.
    pub from_menu: bool,
    last_title: i32,
    /// The first exported frame is the export screen's still.
    frozen: bool,
    /// Wall clock when the first frame went to the encoder (time left).
    started_at: Option<qcore::time::Instant>,
}

impl Host {
    fn capture_settings(&self) -> CaptureSettings {
        CaptureSettings {
            fps: self.cv("capture_fps").round().clamp(10.0, 240.0) as u32,
            codec: Codec::from_cvar(self.cv("capture_codec")),
            quality: self.cv("capture_quality").round().clamp(0.0, 2.0) as u8,
            audio: self.cv("capture_audio") != 0.0,
            ffmpeg: self.cvs("capture_ffmpeg").to_string(),
            encoder: self.cvs("capture_encoder").to_string(),
            size: crate::capture::size_from_cvar(self.cv("capture_size")),
            gpu: self.cv("capture_gpu") != 0.0,
        }
    }

    fn videos_dir(&self) -> PathBuf {
        self.user_dir().join("videos")
    }

    /// The fixed step while capturing and whether to keep real-time pace.
    pub fn capture_step(&self) -> Option<(f64, bool)> {
        let c = self.capture.as_ref()?;
        Some((1.0 / self.capture_settings().fps as f64, c.export.is_none()))
    }

    fn start_recorder(&mut self) -> bool {
        let Some(c) = &self.capture else { return false };
        let s = self.capture_settings();
        let (w, h) = self.r.size();
        let rate = self.mixer.as_ref().map(|m| m.rate());
        let rate = rate.filter(|_| s.audio);
        let started = match &c.out {
            Some(out) => Recorder::start_at(out, w, h, &s, rate),
            None => Recorder::start(&self.videos_dir(), &c.name, w, h, &s, rate),
        };
        match started {
            Ok(rec) => {
                if let Some(m) = &self.mixer
                    && rec.has_audio()
                {
                    m.capture_begin(c.export.is_none());
                }
                if c.export.is_none() {
                    self.con.print(&qcore::ctr!("Recording video to {}\n", rec.out.display()));
                }
                if let Some(c) = &mut self.capture {
                    c.rec = Some(rec);
                }
                true
            }
            Err(e) => {
                self.con.print(&format!("capture failed: {e}\n"));
                self.capture = None;
                false
            }
        }
    }

    pub(super) fn capture_start(&mut self, name: &str) {
        if self.capture.is_some() {
            return self.con.print(&qcore::ctr!("Already recording video.\n"));
        }
        if cfg!(target_arch = "wasm32") {
            return self.con.print(&qcore::ctr!("Video capture is not available in this build.\n"));
        }
        let name = if name.is_empty() { format!("capture_{}", crate::demo::stamp_now()) } else { sanitize(name) };
        self.capture = Some(CaptureState { rec: None, name, out: None, audio_frac: 0.0, export: None });
        self.start_recorder();
    }

    /// Stops recording; the encoder finishes in the background.
    pub(super) fn capture_stop(&mut self) {
        let Some(c) = self.capture.take() else { return };
        if let Some(m) = &self.mixer {
            m.capture_end();
        }
        if let Some(rec) = c.rec {
            self.demo_jobs.push(rec.finish());
        }
        self.end_export_screen();
    }

    /// Back to presenting every frame after an export.
    fn end_export_screen(&mut self) {
        let _ = self.r.set_offscreen(false);
        self.export_ui_at = None;
        if let Some(w) = &self.window {
            let _ = w.set_title("Quake");
        }
    }

    /// Cancel on the export screen (or quitting during an export): stop and
    /// delete everything written, then return like a finished export.
    pub(super) fn cancel_export(&mut self) {
        let Some(c) = self.capture.take() else { return };
        if let Some(m) = &self.mixer {
            m.capture_end();
        }
        if let Some(rec) = c.rec {
            rec.abort();
        }
        self.end_export_screen();
        self.con.print(&qcore::ctr!("Export cancelled.\n"));
        if let Some(e) = c.export {
            self.return_after_export(e);
        }
    }

    /// The export screen's numbers.
    pub(super) fn export_progress(&self) -> Option<export_ui::Progress> {
        let c = self.capture.as_ref()?;
        let (e, rec) = (c.export.as_ref()?, c.rec.as_ref()?);
        let len = e.end.or_else(|| self.demo.as_ref().and_then(|d| d.play.duration()))? - e.start;
        let at = (self.demo_clock()? - e.start).clamp(0.0, len.max(0.0));
        let frac = if len > 0.0 { at / len } else { 1.0 };
        let left = e.started_at.map(|t| t.elapsed().as_secs_f64()).filter(|_| frac > 0.02).map(|el| el * (1.0 - frac) / frac);
        let file = rec.out.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        let encoder = if rec.encoder.is_empty() { "PNG".to_string() } else { rec.encoder.clone() };
        Some(export_ui::Progress { frac, at, len, left, file, encoder })
    }

    /// Whether the export screen shows the still yet.
    pub(super) fn export_frozen(&self) -> bool {
        self.capture.as_ref().and_then(|c| c.export.as_ref()).is_some_and(|e| e.frozen)
    }

    /// After a render: hand the frame (read back by the caller) and one
    /// frame of audio to the encoder.
    pub(super) fn capture_frame(&mut self, frame: Option<(u32, u32, &[u8])>) {
        let fps = self.capture_settings().fps;
        let freeze = self.capture.as_ref().is_some_and(|c| c.rec.is_some() && c.export.as_ref().is_some_and(|e| !e.frozen));
        if freeze && let Some((w, h, px)) = frame {
            self.freeze_export_frame(w, h, px);
            if let Some(e) = self.capture.as_mut().and_then(|c| c.export.as_mut()) {
                e.frozen = true;
                e.started_at = Some(qcore::time::Instant::now());
            }
        }
        let Some(c) = &mut self.capture else { return };
        let Some(rec) = &mut c.rec else { return };
        if let Some((w, h, px)) = frame
            && let Err(e) = rec.frame(w, h, px)
        {
            self.con.print(&format!("capture failed: {e}\n"));
            return self.capture_stop();
        }
        if rec.has_audio()
            && let Some(m) = &self.mixer
        {
            c.audio_frac += f64::from(m.rate()) / f64::from(fps);
            let n = c.audio_frac.floor();
            c.audio_frac -= n;
            rec.audio(&m.capture_paint(n as usize));
        }
    }

    /// Export bookkeeping before a frame is drawn: start the encoder once
    /// the demo stands at the clip start, stop at its end, show progress.
    pub(super) fn update_export(&mut self) {
        let Some(c) = &self.capture else { return };
        let Some(e) = &c.export else { return };
        let ready = self.demo.as_ref().is_some_and(|d| d.play.index.is_some() && d.play.pending_seek.is_none()) && self.cl.signon == 4;
        if self.demo.is_none() {
            return self.finish_export();
        }
        let now = self.demo_clock();
        if c.rec.is_none() {
            if ready && now.is_some_and(|t| t + 0.05 >= e.start) {
                self.menu.screen = Screen::None;
                self.con.open = false;
                self.start_recorder();
            }
            return;
        }
        let (start, end) = (e.start, e.end.or_else(|| self.demo.as_ref().and_then(|d| d.play.duration())));
        if let (Some(now), Some(end)) = (now, end) {
            if now >= end {
                return self.finish_export();
            }
            let pct = ((now - start) / (end - start).max(0.001) * 100.0).clamp(0.0, 100.0) as i32;
            if let Some(c) = &mut self.capture
                && let Some(e) = &mut c.export
                && e.last_title != pct
            {
                e.last_title = pct;
                if let Some(w) = &self.window {
                    let _ = w.set_title(&qcore::tr!("Quake - exporting video {}%", pct));
                }
            }
        }
    }

    /// Ends an export: close the file, then go back where it was started.
    pub(super) fn finish_export(&mut self) {
        let Some(e) = self.capture.as_mut().and_then(|c| c.export.take()) else { return };
        let started = self.capture.as_ref().is_some_and(|c| c.rec.is_some());
        self.capture_stop();
        if !started {
            self.con.print("demo_export: nothing to export\n");
        }
        self.return_after_export(e);
        if self.headless {
            self.quit_after_jobs = true;
        }
    }

    /// Back to the demo and time the export started from (paused), or to
    /// the Demos list.
    fn return_after_export(&mut self, e: Export) {
        match e.resume {
            Some((name, t)) => {
                self.play_demo(&name);
                if let Some(d) = &mut self.demo {
                    d.paused = true;
                    d.play.pending_seek = Some(t);
                }
            }
            None => {
                if self.demo.is_some() {
                    self.disconnect_client();
                }
                if e.from_menu {
                    self.cbuf.add_text("menu_demos\n");
                }
            }
        }
    }

    /// `demo_export [demo [start [end]]] [name]` with no demo: the playing
    /// one, between its marks, then back to where it was.
    fn demo_export(&mut self, a: &Args) {
        if self.capture.is_some() {
            return self.con.print(&qcore::ctr!("Already recording video.\n"));
        }
        let (demo, start, end, resume) = if a.argc() < 2 {
            let Some(d) = &self.demo else {
                return self.print("demo_export <demo> [start] [end] : render a demo (or the part between two times) to a video\n");
            };
            let (s, e) = d.play.clip();
            (d.play.name.clone(), s, e, self.demo_clock().map(|t| (d.play.name.clone(), t)))
        } else {
            // times may hold colons, which the tokenizer splits off
            let rest: Vec<String> = a.args().split_whitespace().skip(1).map(|s| s.trim_matches('"').to_string()).collect();
            let t = |i: usize| rest.get(i).and_then(|s| parse_time(s)).map(|(t, _)| t);
            (a.argv(1).to_string(), t(0).unwrap_or(0.0), t(1), None)
        };
        let from_menu = self.menu.screen == Screen::Demos;
        self.start_export(ExportReq { demo, clip: None, resume, from_menu }, start, end, None);
    }

    /// Plays `req.demo` and renders `start..end` (end of demo when `None`)
    /// to `out` (default: `videos/<demo>.<ext>`).
    fn start_export(&mut self, req: ExportReq, start: f64, end: Option<f64>, out: Option<PathBuf>) {
        let ExportReq { demo, resume, from_menu, .. } = req;
        if demo.contains("..") {
            return self.print(&qcore::ctr!("Relative pathnames are not allowed.\n"));
        }
        let stem = Path::new(&demo).file_stem().map_or("demo".into(), |s| s.to_string_lossy().into_owned());
        self.demonum = -1;
        self.play_demo(&demo);
        let Some(d) = &mut self.demo else { return };
        d.play.pending_seek = (start > 0.0).then_some(start);
        self.capture = Some(CaptureState {
            rec: None,
            name: sanitize(&stem),
            out,
            audio_frac: 0.0,
            export: Some(Export { start, end, resume, from_menu, last_title: -1, frozen: false, started_at: None }),
        });
        // export frames are drawn offscreen; the window shows the export screen
        let _ = self.r.set_offscreen(true);
        self.export_ui_at = None;
        self.con.print(&qcore::ctr!("Exporting {} to video\n", demo));
    }

    /// `menu_export [demo]`: the export dialog for a demo from the Demos
    /// list, or (no argument) the playing one with its highlighted clip.
    fn menu_export(&mut self, a: &Args) {
        if self.capture.is_some() {
            return self.con.print(&qcore::ctr!("Already recording video.\n"));
        }
        let from_menu = self.menu.screen == Screen::Demos;
        let req = if a.argc() < 2 {
            let now = self.demo_clock();
            let Some(d) = &mut self.demo else { return };
            let len = d.play.duration().unwrap_or(0.0);
            let clip = match d.play.marks {
                [Some(x), Some(y)] => Some((x.min(y), x.max(y))),
                [Some(x), None] => Some((x, len)),
                [None, Some(y)] => Some((0.0, y)),
                [None, None] => None,
            }
            .filter(|(x, y)| y > x);
            // the dialog pauses the demo; it stays paused if cancelled
            d.paused = true;
            ExportReq { demo: d.play.name.clone(), clip, resume: now.map(|t| (d.play.name.clone(), t)), from_menu }
        } else {
            ExportReq { demo: a.argv(1).to_string(), clip: None, resume: None, from_menu }
        };
        let stem = sanitize(&Path::new(&req.demo).file_stem().map_or("demo".into(), |s| s.to_string_lossy().into_owned()));
        let ext = self.capture_settings().codec.extension();
        let dir = self.videos_dir();
        let name = crate::capture::unique(&dir, &stem, ext).file_stem().map_or(stem.clone(), |s| s.to_string_lossy().into_owned());
        self.server.core.cvars.set("_export_dest", &dir.to_string_lossy());
        self.server.core.cvars.set("_export_name", &name);
        self.server.core.cvars.set("_export_hasclip", if req.clip.is_some() { "1" } else { "0" });
        let mut info = vec![qcore::tr!("Demo: {}", stem)];
        if let Some((x, y)) = req.clip {
            info.push(qcore::tr!("Highlighted clip: {} to {} ({})", format_time(x), format_time(y), format_time(y - x)));
        }
        let clip = req.clip.is_some();
        self.pending_export = Some(req);
        self.menu.open_export(if from_menu { Screen::Demos } else { Screen::None }, info, clip);
        self.sync_key_dest();
    }

    /// The dialog's Export buttons.
    fn export_go(&mut self, which: &str) {
        let Some(req) = self.pending_export.take() else { return };
        let (start, end) = match (which, req.clip) {
            ("clip", Some((a, b))) => (a, Some(b)),
            _ => (0.0, None),
        };
        let ext = self.capture_settings().codec.extension();
        let out = export_path(self.cvs("_export_dest"), self.cvs("_export_name"), ext, &self.videos_dir());
        self.start_export(req, start, end, out);
    }

    /// On quit: close the level's auto-demo and a running capture, and
    /// wait for video files to be written.
    pub fn shutdown_recordings(&mut self) {
        self.autodemo_finish();
        // quitting cancels an export (and deletes it); a live capture is kept
        if self.exporting() {
            self.cancel_export();
        } else if self.capture.is_some() {
            self.capture_stop();
        }
        for rx in std::mem::take(&mut self.demo_jobs) {
            if let Ok(msg) = rx.recv_timeout(std::time::Duration::from_secs(120)) {
                eprint!("{msg}");
            }
        }
    }

    /// Takes the frame once for every consumer: video, screenshot, thumbnail.
    pub(super) fn wants_capture(&self) -> bool {
        self.screenshot.is_some() || self.thumb.is_some() || self.capture.as_ref().is_some_and(|c| c.rec.is_some())
    }
}

/// The export dialog's file: `dir` (else `videos`) / `name` + `ext`. Without
/// a name the demo's own (`None`); an extension typed into it is dropped.
fn export_path(dir: &str, name: &str, ext: &str, videos: &Path) -> Option<PathBuf> {
    let name = name.trim();
    let name = name.strip_suffix(ext).unwrap_or(name).trim();
    let name: String = name.chars().filter(|&c| !"/\\:*?<>|\"".contains(c) && !c.is_control()).collect();
    let name = name.trim_matches('.');
    if name.is_empty() {
        return None;
    }
    let dir = dir.trim();
    let dir = if dir.is_empty() { videos.to_path_buf() } else { expand_home(dir) };
    Some(dir.join(format!("{name}{ext}")))
}

/// `~/x` as `$HOME/x`.
fn expand_home(p: &str) -> PathBuf {
    match (p.strip_prefix("~/"), std::env::var_os("HOME")) {
        (Some(rest), Some(home)) => PathBuf::from(home).join(rest),
        _ => PathBuf::from(p),
    }
}

/// A file name part from user text.
fn sanitize(s: &str) -> String {
    let s: String = s.chars().map(|c| if c.is_alphanumeric() || "-_.".contains(c) { c } else { '_' }).collect();
    s.trim_matches('.').to_string()
}

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("capture", |h, a| {
        if h.capture.is_some() { h.capture_stop() } else { h.capture_start(a.argv(1)) }
    });
    c.add("capture_start", |h, a| h.capture_start(a.argv(1)));
    c.add("capture_stop", |h, _| {
        if h.exporting() {
            h.finish_export();
        } else if h.capture.is_some() {
            h.capture_stop();
        } else {
            h.print(&qcore::ctr!("Not recording video.\n"));
        }
    });
    c.add("demo_export", |h, a| h.demo_export(a));
    c.add("menu_export", |h, a| h.menu_export(a));
    c.add("_export_go", |h, a| h.export_go(a.argv(1)));
    c.add("_export_cancel", |h, _| h.pending_export = None);
}

#[cfg(test)]
mod tests {
    #[test]
    fn sanitize_names() {
        assert_eq!(super::sanitize("my clip/../x"), "my_clip_.._x");
        assert_eq!(super::sanitize("e1m1"), "e1m1");
    }

    #[test]
    fn export_paths() {
        use std::path::{Path, PathBuf};
        let v = Path::new("/u/videos");
        assert_eq!(super::export_path("/out", "my clip", ".mp4", v), Some(PathBuf::from("/out/my clip.mp4")));
        assert_eq!(super::export_path("", "x.mp4", ".mp4", v), Some(PathBuf::from("/u/videos/x.mp4")));
        assert_eq!(super::export_path("/out", "a/b", ".webm", v), Some(PathBuf::from("/out/ab.webm")));
        assert_eq!(super::export_path("/out", "  ", ".mp4", v), None);
    }
}
