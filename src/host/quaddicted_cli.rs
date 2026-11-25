//! Console access to the Quaddicted installer, for batches and headless
//! runs: `quaddicted_list <filter>` prints matching single-player releases
//! (title, case-insensitive) with their size and whether they are already
//! here; `quaddicted_install <filter | =stem ...>` installs the ones that are
//! not, with any missing dependency first, one download at a time, through
//! the same installer as Map Packs > Get more maps.
//!
//! "Already here" is the installer's own record (`installed.txt`) or a game
//! dir with the release's zip name, compared without version suffixes
//! (`qbj_1.05` is the `qbj` dir, `sewerjam2_v1.2.1` is `sewerjam2`).

use super::*;
use crate::quaddicted::{Job, Release, Status};

#[derive(Default)]
pub(super) struct QdCli {
    /// What was asked: a title filter, or exact `=stem`s.
    request: Vec<String>,
    install: bool,
    queue: Vec<Release>,
    current: Option<Release>,
    done: Vec<String>,
    failed: Vec<String>,
}

/// `qbj_1.05` -> `qbj`, `sewerjam2_v1.2.1` -> `sewerjam2`, `two_tone_jam.v2` -> `two_tone_jam`.
fn base_name(s: &str) -> String {
    let mut s = s.to_ascii_lowercase();
    loop {
        let before = s.len();
        for suffix in ["_fixed", "_rezipped", "_final", "_full"] {
            if let Some(t) = s.strip_suffix(suffix) {
                s = t.to_string();
            }
        }
        // a trailing version: [_.-]v?digits[.digits...][letter] or _r<n>
        if let Some(i) = s.rfind(['_', '.', '-']) {
            let tail = s[i + 1..].trim_start_matches('v').trim_start_matches('r');
            if !tail.is_empty() && tail.chars().next().is_some_and(|c| c.is_ascii_digit()) && tail.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_ascii_lowercase() && c != 'v') && tail.len() <= 9 && i > 0 {
                s.truncate(i);
            }
        }
        if s.len() == before {
            return s;
        }
    }
}

impl Host {
    fn qd_have(&self, r: &Release) -> Option<String> {
        let o = &self.menu.mods.online.data;
        if let Some(d) = o.installed(&r.sha) {
            return Some(d);
        }
        let want = base_name(&r.stem);
        // a game dir, not a stray folder (a failed install's readme)
        let mods_dir = crate::user_mods_dir();
        [self.fs_snapshot.0.as_path(), mods_dir.as_path()].into_iter().find_map(|root| {
            qcore::vfs::read_dir(root).ok()?.into_iter().filter(|e| e.is_dir && qui::mods::is_game_dir(&e.path)).find_map(|e| {
                let n = e.path.file_name()?.to_string_lossy().into_owned();
                (base_name(&n) == want).then_some(n)
            })
        })
    }

    fn qd_matches(&self, request: &[String]) -> Vec<Release> {
        let rel = &self.menu.mods.online.data.releases;
        if request.iter().all(|r| r.starts_with('=')) {
            return request.iter().filter_map(|r| rel.iter().find(|x| x.stem.eq_ignore_ascii_case(&r[1..])).cloned()).collect();
        }
        let f = request.join(" ").to_ascii_lowercase();
        let mut v: Vec<Release> = rel.iter().filter(|r| r.singleplayer && r.title.to_ascii_lowercase().contains(&f)).cloned().collect();
        v.sort_by(|a, b| a.stem.cmp(&b.stem));
        v
    }

    /// Per frame: waits for the index, then lists or installs.
    pub(super) fn qd_cli_frame(&mut self) {
        let Some(mut st) = self.qd_cli.take() else { return };
        self.menu.mods.online.data.poll();
        match self.menu.mods.online.data.status() {
            Status::Failed(e) => {
                self.print(&format!("quaddicted: index failed: {e}\n"));
                return;
            }
            Status::Ready => {}
            _ => {
                self.qd_cli = Some(st);
                return;
            }
        }
        if !st.request.is_empty() {
            let found = self.qd_matches(&std::mem::take(&mut st.request));
            let (mut total, mut queue) = (0u64, Vec::new());
            for r in found {
                let have = self.qd_have(&r);
                self.print(&format!(
                    "{:<26} {:>7.1} MB  {}{}{}\n",
                    r.stem,
                    r.bytes as f64 / 1e6,
                    r.title,
                    if r.deps.is_empty() { String::new() } else { format!("  (needs {})", r.deps.join(", ")) },
                    have.as_ref().map_or(String::new(), |d| format!("  [have: {d}]"))
                ));
                if have.is_none() {
                    total += r.bytes;
                    // dependencies the installer would refuse without, first
                    for d in &r.deps {
                        if let Some(dep) = self.menu.mods.online.data.releases.iter().find(|x| x.stem.eq_ignore_ascii_case(d)).cloned()
                            && self.qd_have(&dep).is_none()
                            && !queue.iter().any(|q: &Release| q.sha == dep.sha)
                        {
                            total += dep.bytes;
                            queue.push(dep);
                        }
                    }
                    if !queue.iter().any(|q| q.sha == r.sha) {
                        queue.push(r);
                    }
                }
            }
            self.print(&format!("{} to install, {:.0} MB\n", queue.len(), total as f64 / 1e6));
            if !st.install {
                return;
            }
            queue.reverse();
            st.queue = queue;
        }
        if let Some(r) = st.current.clone() {
            match self.menu.mods.online.data.job(&r.sha) {
                Some(Job::Downloading(_) | Job::Installing) => {
                    self.qd_cli = Some(st);
                    return;
                }
                Some(Job::Failed(e)) => {
                    self.print(&format!("quaddicted: {} failed: {e}\n", r.stem));
                    st.failed.push(r.stem);
                }
                None => {
                    let dir = self.menu.mods.online.data.installed(&r.sha).unwrap_or_default();
                    self.print(&format!("quaddicted: installed {} into {dir}\n", r.stem));
                    st.done.push(r.stem);
                }
            }
            st.current = None;
        }
        if let Some(r) = st.queue.pop() {
            self.print(&format!("quaddicted: downloading {} ({:.0} MB)\n", r.stem, r.bytes as f64 / 1e6));
            self.menu.mods.online.data.download(&r);
            st.current = Some(r);
            self.qd_cli = Some(st);
            return;
        }
        self.print(&format!("quaddicted: done, {} installed, {} failed {:?}\n", st.done.len(), st.failed.len(), st.failed));
        self.scan_mods();
    }
}

pub(super) fn register(c: &mut Commands<Host>) {
    c.add("quaddicted_list", |h, a| start(h, a, false));
    c.add("quaddicted_install", |h, a| start(h, a, true));
}

fn start(h: &mut Host, a: &qcore::cmd::Args, install: bool) {
    if a.argc() < 2 {
        h.print("quaddicted_list | quaddicted_install <title filter> | =<zip stem> ...\n");
        return;
    }
    h.open_quaddicted(false);
    h.qd_cli = Some(QdCli { request: (1..a.argc()).map(|i| a.argv(i).to_string()).collect(), install, ..Default::default() });
}

#[cfg(test)]
mod tests {
    use super::base_name;

    #[test]
    fn version_suffixes_are_dropped() {
        for (a, b) in [("qbj_1.05", "qbj"), ("sewerjam2_v1.2.1", "sewerjam2"), ("two_tone_jam.v2", "two_tone_jam"), ("tcj_r3", "tcj"),
            ("dtintin_fixed", "dtintin"), ("func_xmasjam2017_v2", "func_xmasjam2017"), ("mc_retrojam2.1", "mc_retrojam2"),
            ("sm224_1-0-3", "sm224"), ("ad_v1_80p1final", "ad"), ("ad_v1_70final", "ad"), ("ad_v1_70patch1", "ad"), ("func_mapjam3", "func_mapjam3"), ("xmasjam2018", "xmasjam2018"), ("hwjam2", "hwjam2")]
        {
            assert_eq!(base_name(a), b, "{a}");
        }
    }
}
