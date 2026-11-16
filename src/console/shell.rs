//! Interactive `bash` PTY session and non-blocking shell / pipe execution
//! rooted in the game's install directory (`basedir`).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use qcore::time::{Duration, Instant};

use qinput::keys::*;

use super::Console;
use super::MAX_LINES;
use super::ansi::StyledCell;
use super::vt::VtGrid;

/// Context passed when spawning `bash` or a piped command.
#[derive(Clone, Debug, Default)]
pub struct ShellEnv {
    pub basedir: PathBuf,
    pub gamedir: String,
    pub userdir: PathBuf,
    pub mapname: String,
}

impl ShellEnv {
    fn apply(&self, cmd: &mut Command) {
        if self.basedir.is_dir() {
            cmd.current_dir(&self.basedir);
        }
        cmd.env("TERM", "xterm-256color")
            .env("COLORTERM", "truecolor")
            .env("QUAKE_BASEDIR", &self.basedir)
            .env("QUAKE_GAMEDIR", &self.gamedir)
            .env("QUAKE_USERDIR", &self.userdir)
            .env("QUAKE_MAP", &self.mapname);
    }
}

enum JobMsg {
    Chunk(String),
    Done,
}

/// Non-blocking one-shot shell command (`!cmd`) or Quake-to-bash pipeline (`quake_cmd | bash_cmd`).
pub struct ShellJob {
    rx: Receiver<JobMsg>,
    pub finished: bool,
}

impl ShellJob {
    pub fn spawn(shell_cmd: &str, stdin_data: Option<String>, env: &ShellEnv) -> Result<Self, String> {
        if cfg!(target_arch = "wasm32") {
            return Err("no shell on the web".into());
        }
        let mut cmd = Command::new("bash");
        cmd.arg("-c").arg(shell_cmd);
        env.apply(&mut cmd);
        cmd.stdin(if stdin_data.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| format!("failed to spawn bash: {e}"))?;

        if let Some(input) = stdin_data
            && let Some(mut stdin) = child.stdin.take()
        {
            std::thread::spawn(move || {
                let _ = stdin.write_all(input.as_bytes());
            });
        }

        let (tx, rx) = mpsc::channel();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();

        let tx_err = tx.clone();
        let err_handle = std::thread::spawn(move || {
            if let Some(mut r) = stderr {
                let mut buf = [0u8; 4096];
                while let Ok(n) = r.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let _ = tx_err.send(JobMsg::Chunk(String::from_utf8_lossy(&buf[..n]).into_owned()));
                }
            }
        });

        std::thread::spawn(move || {
            if let Some(mut r) = stdout {
                let mut buf = [0u8; 4096];
                while let Ok(n) = r.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    let _ = tx.send(JobMsg::Chunk(String::from_utf8_lossy(&buf[..n]).into_owned()));
                }
            }
            let _ = err_handle.join();
            let _ = child.wait();
            let _ = tx.send(JobMsg::Done);
        });

        Ok(Self { rx, finished: false })
    }

    /// Wait briefly (up to `budget`) so fast commands (`grep`, `wc`, `ls`) complete
    /// within the same frame, without stalling the frame loop on slow commands.
    pub fn drain_with_budget(&mut self, budget: Duration) -> String {
        let deadline = Instant::now() + budget;
        let mut out = String::new();
        while !self.finished {
            let now = Instant::now();
            if now >= deadline {
                break;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(JobMsg::Chunk(s)) => out.push_str(&s),
                Ok(JobMsg::Done) => {
                    self.finished = true;
                    break;
                }
                Err(_) => break,
            }
        }
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                JobMsg::Chunk(s) => out.push_str(&s),
                JobMsg::Done => self.finished = true,
            }
        }
        out
    }

    /// Non-blocking poll called once per frame.
    pub fn poll(&mut self) -> String {
        let mut out = String::new();
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                JobMsg::Chunk(s) => out.push_str(&s),
                JobMsg::Done => self.finished = true,
            }
        }
        out
    }
}

/// Interactive `bash` session attached to a Unix pseudo-terminal (PTY).
pub struct PtySession {
    pub vt: VtGrid,
    pub cwd: PathBuf,
    #[cfg(unix)]
    master_fd: i32,
    child: Child,
}

#[cfg(unix)]
mod unix_pty {
    use std::ffi::{c_int, c_uchar, c_ulong, c_ushort, c_void};

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct Winsize {
        pub ws_row: c_ushort,
        pub ws_col: c_ushort,
        pub ws_xpixel: c_ushort,
        pub ws_ypixel: c_ushort,
    }

    pub const F_GETFL: c_int = 3;
    pub const F_SETFL: c_int = 4;
    pub const F_SETFD: c_int = 2;
    pub const FD_CLOEXEC: c_int = 1;
    pub const O_NONBLOCK: c_int = 0o4000;
    pub const TIOCSWINSZ: c_ulong = 0x5414;
    pub const TIOCSCTTY: c_ulong = 0x540E;

    unsafe extern "C" {
        pub fn openpty(
            amaster: *mut c_int,
            aslave: *mut c_int,
            name: *mut c_uchar,
            termp: *const c_void,
            winp: *const Winsize,
        ) -> c_int;
        pub fn fcntl(fd: c_int, cmd: c_int, ...) -> c_int;
        pub fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
        pub fn read(fd: c_int, buf: *mut c_void, count: usize) -> isize;
        pub fn write(fd: c_int, buf: *const c_void, count: usize) -> isize;
        pub fn close(fd: c_int) -> c_int;
        pub fn setsid() -> c_int;
    }
}

impl PtySession {
    #[cfg(unix)]
    pub fn spawn(cols: usize, rows: usize, env: &ShellEnv) -> Result<Self, String> {
        use std::os::fd::{FromRawFd, OwnedFd};
        use std::os::unix::process::CommandExt;
        use unix_pty::*;

        let cols = cols.max(20);
        let rows = rows.max(5);
        let ws = Winsize {
            ws_row: rows as u16,
            ws_col: cols as u16,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let (mut master, mut slave) = (-1, -1);
        // SAFETY: valid out pointers and Winsize struct; termp and name are optional null pointers.
        let rc = unsafe { openpty(&mut master, &mut slave, std::ptr::null_mut(), std::ptr::null(), &ws) };
        if rc != 0 || master < 0 || slave < 0 {
            return Err("openpty failed".into());
        }

        // SAFETY: master is a valid open file descriptor from openpty.
        unsafe {
            let flags = fcntl(master, F_GETFL);
            if flags >= 0 {
                fcntl(master, F_SETFL, flags | O_NONBLOCK);
            }
            fcntl(master, F_SETFD, FD_CLOEXEC);
        }

        // SAFETY: slave is a valid open fd; OwnedFd takes ownership and we clone it for stdout/stderr.
        let slave_in = unsafe { OwnedFd::from_raw_fd(slave) };
        let slave_out = slave_in.try_clone().map_err(|e| format!("dup slave pty: {e}"))?;
        let slave_err = slave_in.try_clone().map_err(|e| format!("dup slave pty: {e}"))?;

        let mut cmd = Command::new("bash");
        cmd.arg("-i");
        env.apply(&mut cmd);
        cmd.stdin(Stdio::from(slave_in))
            .stdout(Stdio::from(slave_out))
            .stderr(Stdio::from(slave_err));

        // SAFETY: setsid and ioctl(0, TIOCSCTTY, 0) are async-signal-safe session setup calls.
        unsafe {
            cmd.pre_exec(|| {
                setsid();
                ioctl(0, TIOCSCTTY, 0 as std::ffi::c_int);
                Ok(())
            });
        }

        match cmd.spawn() {
            Ok(child) => Ok(Self {
                vt: VtGrid::new(cols, rows),
                cwd: env.basedir.clone(),
                master_fd: master,
                child,
            }),
            Err(e) => {
                // SAFETY: master is valid and not yet owned by PtySession.
                unsafe { close(master) };
                Err(format!("failed to start bash: {e}"))
            }
        }
    }

    #[cfg(not(unix))]
    pub fn spawn(_cols: usize, _rows: usize, _env: &ShellEnv) -> Result<Self, String> {
        Err("interactive PTY is only supported on Unix".into())
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        let cols = cols.max(20);
        let rows = rows.max(5);
        if cols == self.vt.cols && rows == self.vt.rows {
            return;
        }
        self.vt.resize(cols, rows);
        #[cfg(unix)]
        {
            let ws = unix_pty::Winsize {
                ws_row: rows as u16,
                ws_col: cols as u16,
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            // SAFETY: master_fd is valid while PtySession is alive; ws is a valid Winsize.
            unsafe {
                unix_pty::ioctl(self.master_fd, unix_pty::TIOCSWINSZ, &ws);
            }
        }
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        #[cfg(not(unix))]
        let _ = data;
        #[cfg(unix)]
        if !data.is_empty() {
            // SAFETY: master_fd is valid; data pointer and length are valid.
            unsafe {
                let _ = unix_pty::write(self.master_fd, data.as_ptr() as *const std::ffi::c_void, data.len());
            }
        }
    }

    pub fn poll(&mut self) -> bool {
        #[cfg(unix)]
        {
            let mut buf = [0u8; 8192];
            loop {
                // SAFETY: master_fd is valid and O_NONBLOCK; buf is writable.
                let n = unsafe { unix_pty::read(self.master_fd, buf.as_mut_ptr() as *mut std::ffi::c_void, buf.len()) };
                if n > 0 {
                    self.vt.feed(&buf[..n as usize]);
                } else {
                    break;
                }
            }
        }
        !matches!(self.child.try_wait(), Ok(Some(_)))
    }

    pub fn snapshot_lines(&self) -> Vec<Vec<StyledCell>> {
        let mut out: Vec<Vec<StyledCell>> = self.vt.scrollback.iter().cloned().collect();
        for row in &self.vt.cells {
            let end = row.iter().rposition(|c| c.ch != ' ').map_or(0, |i| i + 1);
            if end > 0 {
                out.push(row[..end].to_vec());
            }
        }
        out
    }

    pub fn send_key(&mut self, key: i32, ctrl: bool, alt: bool) -> bool {
        let app = self.vt.app_cursor_keys;
        if ctrl && (b'a' as i32..=b'z' as i32).contains(&key) {
            let c = (key - b'a' as i32 + 1) as u8;
            self.write_bytes(&[c]);
            return true;
        }
        let seq: &[u8] = match key {
            K_ENTER | K_KP_ENTER => b"\r",
            K_BACKSPACE => b"\x7f",
            K_TAB => b"\t",
            K_DEL | K_KP_DEL => b"\x1b[3~",
            K_UPARROW | K_KP_UPARROW => if app { b"\x1bOA" } else { b"\x1b[A" },
            K_DOWNARROW | K_KP_DOWNARROW => if app { b"\x1bOB" } else { b"\x1b[B" },
            K_RIGHTARROW | K_KP_RIGHTARROW => {
                if ctrl { b"\x1b[1;5C" } else if app { b"\x1bOC" } else { b"\x1b[C" }
            }
            K_LEFTARROW | K_KP_LEFTARROW => {
                if ctrl { b"\x1b[1;5D" } else if app { b"\x1bOD" } else { b"\x1b[D" }
            }
            K_HOME | K_KP_HOME => if app { b"\x1bOH" } else { b"\x1b[H" },
            K_END | K_KP_END => if app { b"\x1bOF" } else { b"\x1b[F" },
            K_PGUP | K_KP_PGUP => b"\x1b[5~",
            K_PGDN | K_KP_PGDN => b"\x1b[6~",
            _ if alt && (32..127).contains(&key) => {
                self.write_bytes(&[0x1b, key as u8]);
                return true;
            }
            _ => return false,
        };
        self.write_bytes(seq);
        true
    }

    pub fn send_paste(&mut self, text: &str) {
        if self.vt.bracketed_paste {
            self.write_bytes(b"\x1b[200~");
            self.write_bytes(text.as_bytes());
            self.write_bytes(b"\x1b[201~");
        } else {
            self.write_bytes(text.as_bytes());
        }
    }

    pub fn cwd_display(&self) -> &Path {
        &self.cwd
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        #[cfg(unix)]
        // SAFETY: master_fd is owned by PtySession and closed once on drop.
        unsafe {
            unix_pty::close(self.master_fd);
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Console {
    /// Redirect subsequent `print` calls into an internal buffer (for `quake_cmd | bash_cmd`).
    pub fn begin_capture(&mut self) {
        self.capture = Some(String::new());
    }

    /// Stop capturing and return all captured output.
    pub fn end_capture(&mut self) -> String {
        self.capture.take().unwrap_or_default()
    }

    /// Start an interactive `bash` PTY session in the game's install directory.
    pub fn start_pty(&mut self) {
        if self.pty.is_some() {
            return;
        }
        let cols = self.grid_cols.max(80);
        let rows = self.grid_rows.max(24);
        match PtySession::spawn(cols, rows, &self.shell_env) {
            Ok(pty) => {
                self.print(&format!(
                    "\x1b[1;32m--- Starting bash in {} (type 'exit' or Ctrl+D to return to Quake) ---\x1b[0m\n",
                    self.shell_env.basedir.display()
                ));
                self.backscroll = 0;
                self.pty = Some(pty);
            }
            Err(e) => self.print(&format!("\x1b[1;31mbash: {e}\x1b[0m\n")),
        }
    }

    /// Run a one-shot shell command or pipe (`quake_cmd | bash_cmd`) in `basedir`.
    pub fn run_shell_cmd(&mut self, shell_cmd: &str, stdin_data: Option<String>) {
        match ShellJob::spawn(shell_cmd, stdin_data, &self.shell_env) {
            Ok(mut job) => {
                let initial = job.drain_with_budget(Duration::from_millis(30));
                if !initial.is_empty() {
                    self.print(&initial);
                    if !initial.ends_with('\n') && job.finished {
                        self.print("\n");
                    }
                }
                if !job.finished {
                    self.jobs.push(job);
                }
            }
            Err(e) => self.print(&format!("\x1b[1;31m{e}\x1b[0m\n")),
        }
    }

    /// Poll active background shell jobs and interactive PTY output each frame.
    pub fn poll_shell(&mut self) {
        let mut chunks = Vec::new();
        self.jobs.retain_mut(|job| {
            let s = job.poll();
            if !s.is_empty() {
                chunks.push((s, job.finished));
            }
            !job.finished
        });
        for (s, done) in chunks {
            self.print(&s);
            if done && !s.ends_with('\n') {
                self.print("\n");
            }
        }

        let exited = if let Some(pty) = &mut self.pty {
            !pty.poll()
        } else {
            false
        };
        if exited && let Some(pty) = self.pty.take() {
            for row in pty.snapshot_lines() {
                self.modern_lines.push_back(row);
                while self.modern_lines.len() > MAX_LINES {
                    self.modern_lines.pop_front();
                }
            }
            self.print("\x1b[1;32m--- bash session ended ---\x1b[0m\n");
        }
    }
}
