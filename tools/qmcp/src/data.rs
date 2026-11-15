use crate::{Result, Server, schema};
use serde_json::{Value, json};
use std::process::Command;

pub(crate) fn identifier(s: &str) -> Result<()> {
    if s.is_empty()
        || s.len() > 128
        || s == "."
        || s.contains("..")
        || !s
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_-.".contains(&c))
    {
        return Err("invalid map/game identifier".into());
    }
    Ok(())
}

impl Server {
    fn qdata(&self, command: &str, a: &Value) -> Result<String> {
        let mut cmd = Command::new("python3");
        cmd.current_dir(&self.root)
            .arg(self.root.join("tools/qdata.py"))
            .arg(command);
        if let Some(game) = a["game"].as_str() {
            for g in game.split(',') {
                identifier(g.trim_start_matches('+'))?;
            }
            cmd.args(["-g", game]);
        }
        if command == "maps" && a["include_base"] == true {
            cmd.arg("--all");
        }
        if command == "ents" {
            let map = schema::string(a, "map")?;
            identifier(map)?;
            cmd.arg(map).arg("--json");
            if let Some(c) = a["classname"].as_str() {
                cmd.arg(format!("--classname={c}"));
            }
            for k in a["keys"].as_array().into_iter().flatten() {
                cmd.arg(format!("--key={}", k.as_str().unwrap()));
            }
        }
        let output = cmd.output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        if output.stdout.len() > 8 * 1024 * 1024 {
            return Err("qdata result exceeds 8 MiB; filter the query".into());
        }
        String::from_utf8(output.stdout).map_err(|e| e.to_string())
    }

    pub(crate) fn map(&self, a: &Value) -> Result<Value> {
        let raw = self.qdata("ents", a)?;
        let entities: Vec<Value> = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        let offset = a["offset"].as_u64().unwrap_or(0) as usize;
        let limit = a["limit"].as_u64().unwrap_or(50) as usize;
        Ok(
            json!({"map":a["map"],"game":a["game"].as_str().unwrap_or("id1"),"total":entities.len(),"offset":offset,"entities":entities.into_iter().skip(offset).take(limit).collect::<Vec<_>>()}),
        )
    }

    pub(crate) fn data_list(&self, name: &str, a: &Value) -> Result<Value> {
        let maps = name == "list_maps";
        let raw = self.qdata(if maps { "maps" } else { "mods" }, a)?;
        let rows: Vec<Value> = raw
            .lines()
            .map(|line| {
                let mut words = line.split_whitespace();
                let name = words.next().unwrap_or("");
                if maps {
                    let game = words.next().unwrap_or("");
                    // Source paths may contain spaces.
                    let source = line
                        .trim_start()
                        .strip_prefix(name)
                        .unwrap_or("")
                        .trim_start()
                        .strip_prefix(game)
                        .unwrap_or("")
                        .trim_start();
                    json!({"map":name,"game":game,"source":source})
                } else {
                    json!({"game":name,"kinds":words.collect::<Vec<_>>()})
                }
            })
            .collect();
        Ok(json!({"entries":rows}))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reject_traversal_and_console_injection() {
        for s in ["../id1", "foo/bar", "e1m1;quit", "+map", "..", ""] {
            assert!(identifier(s).is_err());
        }
        assert!(identifier("ad_sepulcher").is_ok());
    }
}
