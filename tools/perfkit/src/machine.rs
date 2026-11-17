//! What the numbers were measured on, for every report's header.

use crate::json_str;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct Machine {
    pub gpu: String,
    pub driver: String,
    pub cpu: String,
    pub kernel: String,
    /// cpufreq governor / energy-performance preference / power profile.
    pub governor: String,
    pub epp: String,
    pub power_profile: String,
    pub gamescope: String,
    pub mangohud: String,
    pub commit: String,
}

fn out(cmd: &str, args: &[&str]) -> String {
    Command::new(cmd)
        .args(args)
        .output()
        .ok()
        .map(|o| {
            let s = if o.stdout.is_empty() { o.stderr } else { o.stdout };
            String::from_utf8_lossy(&s).trim().to_string()
        })
        .unwrap_or_default()
}

fn read(path: &str) -> String {
    std::fs::read_to_string(path).map(|s| s.trim().to_string()).unwrap_or_default()
}

impl Machine {
    pub fn detect() -> Machine {
        let smi = out("nvidia-smi", &["--query-gpu=name,driver_version", "--format=csv,noheader"]);
        let (gpu, driver) = smi.split_once(',').map(|(a, b)| (a.trim().to_string(), b.trim().to_string())).unwrap_or((smi, String::new()));
        let cpu = read("/proc/cpuinfo")
            .lines()
            .find_map(|l| l.strip_prefix("model name").and_then(|r| r.split_once(':')).map(|(_, v)| v.trim().to_string()))
            .unwrap_or_default();
        let dirty = !out("git", &["status", "--porcelain", "--untracked-files=no"]).is_empty();
        let gamescope = out("gamescope", &["--version"]);
        Machine {
            gpu,
            driver,
            cpu,
            kernel: out("uname", &["-r"]),
            governor: read("/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"),
            epp: read("/sys/devices/system/cpu/cpu0/cpufreq/energy_performance_preference"),
            power_profile: out("powerprofilesctl", &["get"]),
            gamescope: gamescope.split("version ").nth(1).unwrap_or(&gamescope).split_whitespace().next().unwrap_or("").to_string(),
            mangohud: out("mangohud", &["--version"]),
            commit: format!("{}{}", out("git", &["rev-parse", "--short", "HEAD"]), if dirty { "-dirty" } else { "" }),
        }
    }

    /// Warnings about settings that make numbers noisy or slow.
    pub fn warnings(&self) -> Vec<String> {
        let mut w = Vec::new();
        if self.epp != "performance" && self.governor != "performance" {
            w.push(format!(
                "CPU is not in performance mode (governor {}, EPP {}): run `powerprofilesctl set performance`",
                self.governor, self.epp
            ));
        }
        w
    }

    pub fn to_json(&self) -> String {
        let f = [
            ("gpu", &self.gpu),
            ("driver", &self.driver),
            ("cpu", &self.cpu),
            ("kernel", &self.kernel),
            ("governor", &self.governor),
            ("epp", &self.epp),
            ("power_profile", &self.power_profile),
            ("gamescope", &self.gamescope),
            ("mangohud", &self.mangohud),
            ("commit", &self.commit),
        ];
        let body: Vec<String> = f.iter().map(|(k, v)| format!("{}: {}", json_str(k), json_str(v))).collect();
        format!("{{{}}}", body.join(", "))
    }

    /// One Markdown line for report headers.
    pub fn summary(&self) -> String {
        format!(
            "{} (driver {}), {}, kernel {}, CPU {}/{} ({}), gamescope {}, MangoHud {}, commit {}",
            self.gpu, self.driver, self.cpu, self.kernel, self.governor, self.epp, self.power_profile,
            self.gamescope, self.mangohud, self.commit
        )
    }
}
