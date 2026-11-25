//! Background threads for the Quaddicted browser: index fetch and release
//! downloads. The menu reads the shared state every frame.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime};

use super::{INDEX_URL, Release};

/// The cached index is refetched once it is older than this.
const INDEX_MAX_AGE: Duration = Duration::from_secs(24 * 3600);

#[derive(Clone, Debug, Default, PartialEq)]
pub enum Status {
    #[default]
    Idle,
    Loading,
    Ready,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Job {
    /// Fraction downloaded (0 when the size is unknown).
    Downloading(f32),
    Installing,
    Failed(String),
}

#[derive(Default)]
struct Shared {
    status: Status,
    fresh: Option<Vec<Release>>,
    jobs: HashMap<String, Job>,
    installed: HashMap<String, String>,
}

/// Browser state owned by the menu.
#[derive(Default)]
pub struct Online {
    shared: Arc<Mutex<Shared>>,
    pub releases: Vec<Release>,
    basedir: PathBuf,
    install_root: PathBuf,
    cache: PathBuf,
}

impl Online {
    fn lock(&self) -> MutexGuard<'_, Shared> {
        self.shared.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Starts loading the index (once; `force` refetches it).
    pub fn open(&mut self, basedir: &Path, install_root: &Path, cache: &Path, force: bool) {
        let dir_changed = self.install_root != install_root || self.cache != cache;
        self.basedir = basedir.to_path_buf();
        self.install_root = install_root.to_path_buf();
        self.cache = cache.to_path_buf();
        // the browser has no threads or curl
        if cfg!(target_arch = "wasm32") {
            self.lock().status = Status::Failed(qcore::t!("Downloads are not available in the browser build").into());
            return;
        }
        {
            let mut s = self.lock();
            if dir_changed {
                s.installed = super::load_installed(&cache.join("installed.txt"));
            }
            if s.status == Status::Loading || (s.status == Status::Ready && !force && !dir_changed) {
                return;
            }
            s.status = Status::Loading;
            s.installed = super::load_installed(&cache.join("installed.txt"));
        }
        let shared = self.shared.clone();
        let cache = cache.to_path_buf();
        std::thread::spawn(move || {
            let r = load(&cache, force);
            let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
            match r {
                Ok(v) => {
                    s.fresh = Some(v);
                    s.status = Status::Ready;
                }
                Err(e) => s.status = Status::Failed(e),
            }
        });
    }

    /// Takes a newly loaded index; true when the release list changed.
    pub fn poll(&mut self) -> bool {
        let fresh = self.lock().fresh.take();
        fresh.map(|v| self.releases = v).is_some()
    }

    pub fn status(&self) -> Status {
        self.lock().status.clone()
    }

    pub fn job(&self, sha: &str) -> Option<Job> {
        self.lock().jobs.get(sha).cloned()
    }

    /// Game dir of an installed release.
    pub fn installed(&self, sha: &str) -> Option<String> {
        self.lock().installed.get(sha).cloned()
    }

    /// Downloads and installs a release in the background.
    pub fn download(&mut self, r: &Release) {
        if cfg!(target_arch = "wasm32") {
            self.lock().jobs.insert(r.sha.clone(), Job::Failed(qcore::t!("Downloads are not available in the browser build").into()));
            return;
        }
        {
            let mut s = self.lock();
            if matches!(s.jobs.get(&r.sha), Some(Job::Downloading(_) | Job::Installing)) {
                return;
            }
            s.jobs.insert(r.sha.clone(), Job::Downloading(0.0));
        }
        let (shared, r, basedir, install_root, cache) = (
            self.shared.clone(),
            r.clone(),
            self.basedir.clone(),
            self.install_root.clone(),
            self.cache.clone(),
        );
        std::thread::spawn(move || {
            let set = |j: Option<Job>| {
                let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                match j {
                    Some(j) => s.jobs.insert(r.sha.clone(), j),
                    None => s.jobs.remove(&r.sha),
                };
            };
            let zip = cache.join(format!("{}.zip", r.sha));
            let res = fetch(&r.url, &zip, r.bytes, &|f| set(Some(Job::Downloading(f)))).and_then(|()| {
                set(Some(Job::Installing));
                super::install(&r, &zip, &install_root, &basedir)
            });
            let _ = std::fs::remove_file(&zip);
            match res {
                Ok(dir) => {
                    let mut s = shared.lock().unwrap_or_else(|e| e.into_inner());
                    s.jobs.remove(&r.sha);
                    s.installed.insert(r.sha.clone(), dir);
                    super::save_installed(&cache.join("installed.txt"), &s.installed);
                }
                Err(e) => set(Some(Job::Failed(e))),
            }
        });
    }
}

fn load(cache: &Path, force: bool) -> Result<Vec<Release>, String> {
    let zip = cache.join("index.zip");
    let age = std::fs::metadata(&zip).and_then(|m| m.modified()).ok().and_then(|t| SystemTime::now().duration_since(t).ok());
    let stale = force || age.is_none_or(|a| a > INDEX_MAX_AGE);
    let fetched = if stale { fetch(INDEX_URL, &zip, 0, &|_| {}) } else { Ok(()) };
    // offline: an old index still lists what can be installed later
    match super::load_index(&zip) {
        Some(v) => Ok(v),
        None => Err(fetched.err().unwrap_or_else(|| qcore::t!("the map list could not be read").into())),
    }
}

/// No zip of the archive comes near this (curl stops the download there).
const MAX_DOWNLOAD: &str = "4294967296";

/// `url` comes from the index: only ever a web address, never an option, a
/// local file or a glob.
fn curl_args(url: &str, part: &Path) -> Vec<OsString> {
    let mut a: Vec<OsString> = ["-fsSL", "--retry", "2", "--connect-timeout", "20", "--globoff", "--max-filesize", MAX_DOWNLOAD]
        .map(OsString::from)
        .into();
    a.extend(["--proto", "=http,https", "--proto-redir", "=http,https", "-o"].map(OsString::from));
    a.push(part.into());
    a.extend(["--".into(), url.into()]);
    a
}

/// Downloads `url` to `dest` with curl, reporting progress against `size`.
fn fetch(url: &str, dest: &Path, size: u64, progress: &dyn Fn(f32)) -> Result<(), String> {
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    let part = dest.with_extension("part");
    let mut child = Command::new("curl")
        .args(curl_args(url, &part))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|_| qcore::t!("downloading needs the curl program").to_string())?;
    let status = loop {
        if let Some(st) = child.try_wait().map_err(|e| e.to_string())? {
            break st;
        }
        if size > 0 {
            let got = std::fs::metadata(&part).map_or(0, |m| m.len());
            progress((got as f64 / size as f64).min(1.0) as f32);
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    if !status.success() {
        let _ = std::fs::remove_file(&part);
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = std::io::Read::read_to_string(&mut e, &mut err);
        }
        let err = err.trim().trim_start_matches("curl: ").to_string();
        return Err(if err.is_empty() { qcore::t!("download failed").into() } else { err });
    }
    std::fs::rename(&part, dest).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curl_takes_the_url_as_a_url() {
        let a = curl_args("-K/tmp/rc", Path::new("/c/x.part"));
        assert_eq!(a[a.len() - 4..], ["-o", "/c/x.part", "--", "-K/tmp/rc"].map(OsString::from));
        assert!(a.windows(2).any(|w| w[0] == "--proto" && w[1] == "=http,https"));
    }
}
