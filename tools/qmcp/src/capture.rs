use crate::{Result, Server, data::identifier, schema};
use serde_json::{Value, json};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

impl Server {
    pub(crate) fn capture(&self, a: &Value) -> Result<Value> {
        let map = schema::string(a, "map")?;
        identifier(map)?;
        let game = a["game"].as_str().unwrap_or("id1");
        identifier(game)?;
        let dir = self.root.join("target/qmcp");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        // Resolve the output directory so a symlink cannot redirect writes out of the checkout.
        if !dir
            .canonicalize()
            .map_err(|e| e.to_string())?
            .starts_with(&self.root)
        {
            return Err("capture directory escapes checkout".into());
        }
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        let stem = format!("capture-{}-{id}", std::process::id());
        let path = dir.join(format!("{stem}.png"));
        let log = dir.join(format!("{stem}.log"));
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&log)
            .map_err(|e| e.to_string())?;
        let mut cmd = Command::new(self.root.join("target/release/quake-source-port"));
        cmd.current_dir(&self.root).args([
            "-headless",
            "-autoquit",
            "6",
            "-width",
            &a["width"].as_u64().unwrap_or(1280).to_string(),
            "-height",
            &a["height"].as_u64().unwrap_or(720).to_string(),
        ]);
        if ["hipnotic", "rogue", "quoth"].contains(&game) {
            cmd.arg(format!("-{game}"));
        } else if game != "id1" {
            cmd.args(["-game", game]);
        }
        cmd.args([
            "+sv_autosave",
            "0",
            "+map",
            map,
            "+delay 1 god",
            "+delay 1 notarget",
        ]);
        if let Some(camera) = a["camera"].as_array() {
            let numbers = camera
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join(" ");
            cmd.arg(format!("+delay 2 setpos {numbers}"));
        }
        // Quote the path for the engine console, independently of process argv.
        if path.to_string_lossy().contains(['"', '\n', '\r', ';']) {
            return Err("unsupported checkout path for console screenshot command".into());
        }
        cmd.arg(format!("+delay 3 screenshot \"{}\"", path.display()));
        cmd.stdin(Stdio::null())
            .stdout(file.try_clone().map_err(|e| e.to_string())?)
            .stderr(file);
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("build the release engine first: {e}"))?;
        let start = Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => {
                    if !status.success() {
                        return Err(format!("engine exited {status}; see {}", log.display()));
                    }
                    break;
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(e.to_string());
                }
            }
            if start.elapsed() > Duration::from_secs(45) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("capture timed out; see {}", log.display()));
            }
            std::thread::sleep(Duration::from_millis(25));
        }
        let mut result = self.screenshot(path.to_str().ok_or("non-UTF8 screenshot path")?)?;
        result["content"].as_array_mut().unwrap().push(json!({"type":"text","text":json!({"path":path.strip_prefix(&self.root).unwrap().to_string_lossy(),"log":log.strip_prefix(&self.root).unwrap().to_string_lossy()}).to_string()}));
        Ok(result)
    }
}
