//! Automatic demos (`cl_autodemo`): every level visit is recorded to
//! `autodemos/<stamp>_<map>.dem`, from its serverinfo to the next map or
//! disconnect. Only the newest `cl_autodemo_slots` are kept (0: all); a
//! level shorter than [`MIN_SECS`] is discarded. `demo_keep` moves one into
//! the saved demos, where rotation never touches it. With only
//! `cl_autodemo_pb` on, levels are still recorded but kept only when they
//! set a personal best (`level_time`).

use std::path::Path;

use super::*;
use crate::demo::library::{self, AUTO_DIR};

/// Levels shorter than this (server time) are not worth a slot.
const MIN_SECS: f64 = 2.0;

pub(super) struct AutoDemo {
    writer: crate::demo::DemoWriter,
    /// First and last server time recorded in game (signon 4).
    span: Option<(f64, f64)>,
    /// `demo_keep` during the level: the saved name to use when it ends.
    keep: Option<String>,
    /// Recorded only for `cl_autodemo_pb` (auto-demos off): dropped unless
    /// the level sets a personal best.
    pb_only: bool,
}

#[cfg(test)]
impl AutoDemo {
    pub(super) fn writer_path(&self) -> &Path {
        self.writer.path()
    }
}

impl Host {
    fn autodemo_allowed(&self) -> bool {
        (self.cv("cl_autodemo") != 0.0 || self.cv("cl_autodemo_pb") != 0.0 || self.cv("cl_demo_ghost") != 0.0)
            && self.demo.is_none()
            && self.timedemo.is_none()
            && !self.dedicated
            && self.capture.as_ref().is_none_or(|c| c.export.is_none())
    }

    /// Called with every server message after it is parsed: a serverinfo
    /// ends the level's demo and starts the next one, then the message is
    /// recorded.
    pub(super) fn autodemo_message(&mut self, msg: &[u8], new_map: bool) {
        if new_map {
            self.autodemo_finish();
            if self.autodemo_allowed() {
                self.autodemo_start();
            }
        }
        let (signon, t) = (self.cl.signon, self.cl.mtime[0]);
        let Some(a) = &mut self.autodemo else { return };
        if signon == 4 {
            a.span = Some(a.span.map_or((t, t), |(t0, t1)| (t0, t1.max(t))));
        }
        if let Err(e) = a.writer.write(self.cl.viewangles, msg) {
            self.con.print(&format!("auto-demo write failed: {e}\n"));
            self.autodemo = None;
        }
    }

    fn autodemo_start(&mut self) {
        let dir = self.user_dir().join(AUTO_DIR);
        let slots = self.cv("cl_autodemo_slots").max(0.0) as usize;
        let pb_only = self.cv("cl_autodemo") == 0.0;
        // room for the new one
        if slots > 0 && !pb_only {
            library::prune(&dir, slots - 1);
        }
        let map: String = self.cl.mapname.chars().map(|c| if c.is_alphanumeric() || "-_".contains(c) { c } else { '_' }).collect();
        let stamp = crate::demo::stamp_now();
        let map_part = if map.is_empty() { "map" } else { &map };
        let mut path = dir.join(format!("{stamp}_{map_part}.dem"));
        let mut n = 2u32;
        while qcore::vfs::exists(&path) {
            path = dir.join(format!("{stamp}r{n:02}_{map_part}.dem"));
            n += 1;
        }
        match crate::demo::DemoWriter::create(&path, -1) {
            Ok(writer) => self.autodemo = Some(AutoDemo { writer, span: None, keep: None, pb_only }),
            Err(e) => self.con.print(&qcore::ctr!("ERROR: couldn't create {}: {}\n", path.display(), e)),
        }
    }

    /// Ends the current auto-demo: dropped when too short, kept when asked.
    pub(super) fn autodemo_finish(&mut self) {
        let Some(mut a) = self.autodemo.take() else { return };
        let path = a.writer.path().to_path_buf();
        let ok = a.writer.write(self.cl.viewangles, &[qnet::protocol::SVC_DISCONNECT]).and_then(|_| a.writer.finish()).is_ok();
        let long = a.span.is_some_and(|(t0, t1)| t1 - t0 >= MIN_SECS);
        if !ok || !long || (a.pb_only && a.keep.is_none()) {
            let _ = qcore::vfs::remove_file(&path);
            return;
        }
        self.last_autodemo = Some(path.clone());
        if let Some(name) = a.keep {
            self.keep_autodemo(&path, &name);
        }
    }

    fn keep_autodemo(&mut self, path: &Path, name: &str) -> Option<PathBuf> {
        match library::keep(path, &self.user_dir(), name) {
            Ok(dest) => {
                let shown = dest.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                self.con.print(&qcore::ctr!("Kept demo {}\n", shown));
                if self.last_autodemo.as_deref() == Some(path) {
                    self.last_autodemo = None;
                }
                self.convert_demo(dest.clone());
                Some(dest)
            }
            Err(e) => {
                self.con.print(&format!("demo_keep: {e}\n"));
                None
            }
        }
    }

    /// `_demo_rename <file> <name>` (Demos → Rename): a saved demo gets the
    /// new name; an auto-demo is kept under it. The playing demo follows.
    fn demo_rename(&mut self, file: &str, name: &str) {
        let name = name.trim();
        let name = name.strip_suffix(".dem").unwrap_or(name).trim();
        if name.is_empty() || name.contains(['/', '\\', '"']) || name.contains("..") {
            return self.print(&qcore::ctr!("Not a valid demo name.\n"));
        }
        let Some(src) = self.menu_demo_path(file) else { return };
        let auto = src.parent().is_some_and(|p| p.ends_with(library::AUTO_DIR));
        let dest = if auto {
            self.keep_autodemo(&src, name)
        } else {
            match library::keep(&src, &self.user_dir(), name) {
                Ok(dest) => {
                    if dest != src {
                        let shown = dest.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
                        self.con.print(&qcore::ctr!("Renamed demo to {}\n", shown));
                    }
                    Some(dest)
                }
                Err(e) => {
                    self.con.print(&format!("rename: {e}\n"));
                    None
                }
            }
        };
        let Some(dest) = dest else { return };
        let user_dir = self.user_dir();
        if let Some(d) = &mut self.demo
            && user_dir.join(&d.play.name).with_extension("") == src.with_extension("")
        {
            let ext = Path::new(&d.play.name).extension().map_or_else(|| "dem".into(), |e| e.to_string_lossy().into_owned());
            d.play.name = dest.with_extension(ext).file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned());
            d.play.auto = false;
        }
        self.scan_demos();
        self.menu.demos.focus(&dest);
    }

    /// Copies the currently recording auto-demo to `dest` (ended with
    /// `svc_disconnect`), even when recording only for PB/ghost (`pb_only`).
    pub(super) fn autodemo_copy_current(&mut self, dest: &Path) -> bool {
        let angles = self.cl.viewangles;
        self.autodemo.as_mut().is_some_and(|a| a.writer.copy_to(dest, angles).is_ok())
    }

    /// Bug reports with `cl_autodemo` on: the level being recorded (or the
    /// last finished auto-demo) copied to `dest`.
    pub(super) fn autodemo_copy(&mut self, dest: &Path) -> bool {
        if self.cv("cl_autodemo") == 0.0 {
            return false;
        }
        let angles = self.cl.viewangles;
        if let Some(a) = &mut self.autodemo {
            return a.writer.copy_to(dest, angles).is_ok();
        }
        self.last_autodemo.as_ref().and_then(|p| qcore::vfs::read(p).ok()).is_some_and(|d| qcore::vfs::write(dest, &d).is_ok())
    }

    /// The level being recorded is kept as `name` when it ends, unless a
    /// name was already chosen with `demo_keep`.
    pub(super) fn autodemo_keep_as(&mut self, name: String) {
        if let Some(a) = &mut self.autodemo {
            a.keep.get_or_insert(name);
        }
    }

    /// `demo_keep [name]`: the auto-demo being watched, else the level being
    /// recorded (kept when it ends), else the last finished one.
    fn demo_keep(&mut self, name: &str) {
        let stem = |p: &Path| p.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let named = |p: &Path| if name.is_empty() { library::kept_name(&stem(p)) } else { name.to_string() };
        let user_dir = self.user_dir();
        if let Some(d) = &mut self.demo {
            if !d.play.auto {
                return self.con.print(&qcore::ctr!("This demo is already saved.\n"));
            }
            let path = user_dir.join(&d.play.name);
            d.play.auto = false;
            let n = named(&path);
            d.play.name = format!("{}.dem", n.strip_suffix(".dem").unwrap_or(&n));
            self.keep_autodemo(&path, &n);
            return;
        }
        if let Some(a) = &mut self.autodemo {
            let n = named(a.writer.path());
            a.keep = Some(n);
            return self.con.print(&qcore::ctr!("This level's demo will be kept when it ends.\n"));
        }
        match self.last_autodemo.clone() {
            Some(p) if qcore::vfs::exists(&p) => {
                let n = named(&p);
                self.keep_autodemo(&p, &n);
            }
            _ => self.con.print(&qcore::ctr!("No auto-demo to keep.\n")),
        }
    }
}

impl Host {
    /// Re-reads the demo folders into the Demos menu.
    pub(super) fn scan_demos(&mut self) {
        let (saved, auto) = library::list(&self.user_dir());
        let on = self.cv("cl_autodemo") != 0.0;
        self.menu.demos.set(saved, auto, on);
    }

    /// A user-dir relative demo name from the menu, or None if it escapes.
    fn menu_demo_path(&self, name: &str) -> Option<std::path::PathBuf> {
        (!name.is_empty() && !name.contains("..")).then(|| self.user_dir().join(name))
    }
}

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("menu_demos", |h, _| {
        h.scan_demos();
        h.menu.demos.open();
        h.menu.open(Screen::Demos);
        h.sync_key_dest();
    });
    c.add("_scandemos", |h, _| h.scan_demos());
    c.add("_demo_keep_file", |h, a| {
        if let Some(p) = h.menu_demo_path(a.argv(1)) {
            let stem = p.file_stem().map_or_else(String::new, |s| s.to_string_lossy().into_owned());
            h.keep_autodemo(&p, &library::kept_name(&stem));
            h.scan_demos();
        }
    });
    c.add("_demo_rename", |h, a| h.demo_rename(a.argv(1), a.argv(2)));
    c.add("_demo_delete", |h, a| {
        let Some(p) = h.menu_demo_path(a.argv(1)) else { return };
        // a converted demo and its full version go together
        for ext in ["dem", crate::demo::convert::QSD_EXT] {
            let _ = qcore::vfs::remove_file(&p.with_extension(ext));
        }
        if h.last_autodemo.as_deref().is_some_and(|l| l.with_extension("") == p.with_extension("")) {
            h.last_autodemo = None;
        }
        h.scan_demos();
    });
    c.add("demo_keep", |h, a| {
        if a.argv(1).contains("..") || a.argv(1).contains('/') {
            return h.print(&qcore::ctr!("Relative pathnames are not allowed.\n"));
        }
        h.demo_keep(a.argv(1));
    });
}
