use crate::{Result, Server, schema};
use base64::Engine;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

impl Server {
    pub(crate) fn artifact(&self, name: &str, extension: &str) -> Result<PathBuf> {
        let p = self
            .root
            .join(name)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        let allowed = [self.root.join("target"), self.root.join("tools/qbot")];
        if !allowed.iter().any(|root| p.starts_with(root))
            || p.extension().and_then(|e| e.to_str()) != Some(extension)
        {
            return Err(format!(
                "expected a .{extension} artifact inside target/ or tools/qbot/"
            ));
        }
        if p.metadata().map_err(|e| e.to_string())?.len() > 8 * 1024 * 1024 {
            return Err("artifact exceeds 8 MiB".into());
        }
        Ok(p)
    }

    pub(crate) fn read_json(&self, name: &str) -> Result<Value> {
        let p = self.artifact(name, "json")?;
        serde_json::from_slice(&std::fs::read(p).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())
    }

    pub(crate) fn report(&self, a: &Value) -> Result<Value> {
        let v = self.read_json(schema::string(a, "path")?)?;
        if let Some(map) = a["map"].as_str() {
            v.get(map)
                .cloned()
                .ok_or_else(|| format!("map {map} absent"))
        } else {
            Ok(v)
        }
    }

    pub(crate) fn screenshot(&self, name: &str) -> Result<Value> {
        let p = self.artifact(name, "png")?;
        let bytes = std::fs::read(p).map_err(|e| e.to_string())?;
        if !bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err("invalid PNG signature".into());
        }
        Ok(
            json!({"content":[{"type":"image","mimeType":"image/png","data":base64::engine::general_purpose::STANDARD.encode(bytes)}],"isError":false}),
        )
    }

    pub(crate) fn list(&self, a: &Value) -> Result<Value> {
        let mut paths = Vec::new();
        for dir in ["target", "tools/qbot"] {
            walk(&self.root.join(dir), &mut paths, 0)?;
        }
        let kind = a["kind"].as_str().unwrap_or("all");
        paths.retain(|p| match kind {
            "reports" => p.extension().is_some_and(|e| e == "json"),
            "screenshots" => p.extension().is_some_and(|e| e == "png"),
            _ => true,
        });
        let mut rows: Vec<_> = paths.into_iter().filter_map(|p| {
            let canonical = p.canonicalize().ok()?;
            if !canonical.starts_with(&self.root) { return None; }
            let m = p.metadata().ok()?;
            let modified = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_secs();
            Some((modified, json!({"path":p.strip_prefix(&self.root).ok()?.to_string_lossy(),"bytes":m.len(),"modified_unix":modified})))
        }).collect();
        rows.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then_with(|| a.1["path"].as_str().cmp(&b.1["path"].as_str()))
        });
        let total = rows.len();
        let offset = a["offset"].as_u64().unwrap_or(0) as usize;
        let limit = a["limit"].as_u64().unwrap_or(50) as usize;
        Ok(
            json!({"total":total,"offset":offset,"artifacts":rows.into_iter().skip(offset).take(limit).map(|r|r.1).collect::<Vec<_>>()}),
        )
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    if depth > 12 || !dir.exists() {
        return Ok(());
    }
    for e in std::fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let ty = e.file_type().map_err(|e| e.to_string())?;
        if ty.is_symlink() {
            continue;
        }
        let p = e.path();
        if ty.is_dir() {
            // Build caches are not run artifacts and can contain millions of files.
            if ![
                "incremental",
                "deps",
                "build",
                ".fingerprint",
                "node_modules",
            ]
            .contains(&e.file_name().to_string_lossy().as_ref())
            {
                walk(&p, out, depth + 1)?;
            }
        } else if p.extension().is_some_and(|e| e == "json" || e == "png") {
            out.push(p);
            if out.len() > 100_000 {
                return Err("too many artifacts; narrow the artifact directory".into());
            }
        }
    }
    Ok(())
}
