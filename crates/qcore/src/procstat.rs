//! This process's CPU time and resident memory, for the HUD's performance
//! metrics. Linux and Android read `/proc/self`; Windows asks kernel32.
//! Elsewhere (macOS, the web) both are unavailable.

/// A snapshot of the process's resource use.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ProcStat {
    /// User + system CPU time used so far, in seconds (all threads).
    pub cpu_secs: Option<f64>,
    /// Resident memory (working set), in bytes.
    pub rss: Option<u64>,
}

/// Reads the current snapshot. Cheap enough for twice a second.
pub fn sample() -> ProcStat {
    imp::sample()
}

/// CPU use between two snapshots `wall` seconds apart, as a percentage of
/// the whole machine (`cores` logical CPUs), like a task manager shows it.
pub fn cpu_percent(prev: &ProcStat, now: &ProcStat, wall: f64, cores: usize) -> Option<f32> {
    let used = now.cpu_secs? - prev.cpu_secs?;
    (wall > 0.0).then(|| (used / wall / cores.max(1) as f64 * 100.0).clamp(0.0, 100.0) as f32)
}

/// utime + stime in clock ticks from `/proc/<pid>/stat` text.
#[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
fn stat_ticks(text: &str) -> Option<u64> {
    // the command name may hold spaces and parentheses: fields follow the last ')'
    let rest = &text[text.rfind(')')? + 1..];
    let mut f = rest.split_whitespace();
    // after ')' come state (field 3) .. utime (14), stime (15)
    let utime: u64 = f.nth(11)?.parse().ok()?;
    let stime: u64 = f.next()?.parse().ok()?;
    Some(utime + stime)
}

/// `VmRSS` from `/proc/<pid>/status` text, in bytes.
#[cfg_attr(not(any(target_os = "linux", target_os = "android")), allow(dead_code))]
fn status_rss(text: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kb * 1024)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod imp {
    use super::*;

    /// USER_HZ: 100 on every Linux ABI that exposes /proc to user space.
    const CLK_TCK: f64 = 100.0;

    pub fn sample() -> ProcStat {
        let read = |p: &str| std::fs::read_to_string(p).ok();
        ProcStat {
            cpu_secs: read("/proc/self/stat").as_deref().and_then(stat_ticks).map(|t| t as f64 / CLK_TCK),
            rss: read("/proc/self/status").as_deref().and_then(status_rss),
        }
    }
}

#[cfg(windows)]
mod imp {
    use super::*;
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Default)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    impl FileTime {
        /// 100 ns units to seconds.
        fn secs(&self) -> f64 {
            ((u64::from(self.high) << 32) | u64::from(self.low)) as f64 * 1e-7
        }
    }

    /// PROCESS_MEMORY_COUNTERS.
    #[repr(C)]
    #[derive(Default)]
    struct MemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> *mut c_void;
        fn GetProcessTimes(process: *mut c_void, creation: *mut FileTime, exit: *mut FileTime, kernel: *mut FileTime, user: *mut FileTime) -> i32;
        fn K32GetProcessMemoryInfo(process: *mut c_void, counters: *mut MemoryCounters, cb: u32) -> i32;
    }

    pub fn sample() -> ProcStat {
        let (mut c, mut e, mut k, mut u) = Default::default();
        let mut mem = MemoryCounters { cb: size_of::<MemoryCounters>() as u32, ..Default::default() };
        // SAFETY: the pseudo handle needs no closing; every out pointer is a
        // valid, writable struct of the type the API expects.
        let (times, info) = unsafe {
            let p = GetCurrentProcess();
            (GetProcessTimes(p, &mut c, &mut e, &mut k, &mut u), K32GetProcessMemoryInfo(p, &mut mem, mem.cb))
        };
        ProcStat {
            cpu_secs: (times != 0).then(|| k.secs() + u.secs()),
            rss: (info != 0).then_some(mem.working_set_size as u64),
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
mod imp {
    use super::*;

    pub fn sample() -> ProcStat {
        ProcStat::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_proc_text() {
        let stat = "1234 (my (odd) game) S 1 2 3 4 5 6 7 8 9 10 250 50 0 0 20 0 9 0 100";
        assert_eq!(stat_ticks(stat), Some(300));
        assert_eq!(stat_ticks("garbage"), None);
        assert_eq!(status_rss("Name:\tq\nVmRSS:\t  2048 kB\nThreads: 9\n"), Some(2048 * 1024));
        let a = ProcStat { cpu_secs: Some(10.0), rss: None };
        let b = ProcStat { cpu_secs: Some(10.5), rss: None };
        assert_eq!(cpu_percent(&a, &b, 1.0, 4), Some(12.5));
        assert_eq!(cpu_percent(&a, &ProcStat::default(), 1.0, 4), None);
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn samples_this_process() {
        let s = sample();
        assert!(s.cpu_secs.is_some_and(|t| t >= 0.0));
        assert!(s.rss.is_some_and(|r| r > 0));
    }
}
