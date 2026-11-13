//! Slang screen shaders (RetroArch `.slangp` presets via librashader):
//! cvars `r_shader_preset`, `r_shader_scope`, `r_shader_params`, and the
//! `r_shader_*` commands. Parameter overrides persist in `r_shader_params`
//! as `name=value;name=value`.

use super::*;
use std::path::{Path, PathBuf};

/// Bundled presets shipped in `assets/slang` (see LICENSES.md there).
const BUNDLED: &str = include_str!("../../assets/slang/presets.json");

#[derive(Default)]
pub(super) struct ShaderState {
    /// Preset string and reload generation last handed to the renderer.
    loaded: Option<(String, u32)>,
    /// `r_shader_params` last applied.
    params: Option<String>,
    /// Bumped by `r_shader_reload`.
    reload: u32,
}

/// Bundled preset names (relative to `assets/slang`, without `.slangp`).
pub(crate) fn bundled_presets() -> Vec<String> {
    let mut v: Vec<String> = BUNDLED
        .split('"')
        .filter(|s| s.ends_with(".slangp"))
        .map(|s| s.trim_end_matches(".slangp").to_string())
        .collect();
    v.sort();
    v
}

/// Directories holding the bundled presets: next to the executable, then
/// the source tree (cargo runs).
fn bundled_roots() -> Vec<PathBuf> {
    let mut v = Vec::new();
    if let Some(d) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        v.push(d.join("assets/slang"));
        v.push(d.join("slang"));
    }
    v.push(Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/slang"));
    v
}

use crate::menu::shader::{parse_params, with_param};

impl Host {
    /// Finds a preset: a path as given, then `<game>/shaders/`, then the
    /// bundled set. `.slangp` is optional.
    fn resolve_preset(&self, name: &str) -> Option<PathBuf> {
        let name = if name.ends_with(".slangp") { name.to_string() } else { format!("{name}.slangp") };
        let p = Path::new(&name);
        if p.is_absolute() {
            return p.is_file().then(|| p.to_path_buf());
        }
        let fs = self.fs();
        let mut roots: Vec<PathBuf> = fs.game_dirs().iter().rev().map(|g| fs.basedir().join(g).join("shaders")).collect();
        roots.extend(bundled_roots());
        roots.into_iter().map(|r| r.join(&name)).find(|f| f.is_file())
    }

    /// Bundled presets plus `.slangp` files under `<game>/shaders/`.
    pub(crate) fn shader_presets(&self) -> Vec<String> {
        let mut v = bundled_presets();
        let fs = self.fs();
        for g in fs.game_dirs() {
            let root = fs.basedir().join(g).join("shaders");
            let mut stack = vec![root.clone()];
            while let Some(d) = stack.pop() {
                let Ok(rd) = std::fs::read_dir(&d) else { continue };
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.extension().is_some_and(|x| x == "slangp")
                        && let Ok(rel) = p.strip_prefix(&root)
                    {
                        let n = rel.with_extension("").to_string_lossy().replace('\\', "/");
                        if !v.contains(&n) {
                            v.push(n);
                        }
                    }
                }
            }
        }
        v
    }

    /// Loads or drops the preset when `r_shader_preset` changes and applies
    /// `r_shader_params`. Called before each frame is rendered.
    pub(super) fn sync_shader(&mut self) {
        if self.menu.shader_presets.is_empty() {
            self.menu.shader_presets = self.shader_presets();
        }
        let want = (self.cvs("r_shader_preset").trim().to_string(), self.shader.reload);
        if self.shader.loaded.as_ref() != Some(&want) {
            self.shader.loaded = Some(want.clone());
            self.shader.params = None;
            let path = if want.0.is_empty() {
                None
            } else {
                match self.resolve_preset(&want.0) {
                    Some(p) => Some(p),
                    None => {
                        self.con.print(&format!("r_shader_preset: \"{}\" not found\n", want.0));
                        let _ = self.r.set_shader_preset(None);
                        self.menu.shader_params.clear();
                        return;
                    }
                }
            };
            match self.r.set_shader_preset(path.as_deref()) {
                Ok(params) => self.menu.shader_params = params,
                Err(e) => {
                    self.menu.shader_params.clear();
                    self.con.print(&format!("r_shader_preset \"{}\": {e}\n", want.0));
                }
            }
        }
        let ps = self.cvs("r_shader_params").to_string();
        if self.shader.params.as_deref() != Some(ps.as_str()) {
            // reset to the preset's values, then apply the overrides
            if let Some((_, params)) = self.r.shader_preset() {
                let init: Vec<(String, f32)> = params.iter().map(|p| (p.id.clone(), p.initial)).collect();
                for (k, v) in init.into_iter().chain(parse_params(&ps)) {
                    self.r.set_shader_param(&k, v);
                }
            }
            self.shader.params = Some(ps);
        }
    }

    /// Current value of a preset parameter (override or preset default).
    pub(crate) fn shader_param_value(&self, id: &str) -> Option<f32> {
        let (_, params) = self.r.shader_preset()?;
        let p = params.iter().find(|p| p.id == id)?;
        let over = parse_params(self.cvs("r_shader_params")).into_iter().rev().find(|(k, _)| k == id);
        Some(over.map_or(p.initial, |(_, v)| v))
    }

    /// Stores an override (None removes it) in `r_shader_params`.
    pub(crate) fn set_shader_param_override(&mut self, id: &str, value: Option<f32>) {
        let new = with_param(self.cvs("r_shader_params"), id, value);
        self.server.core.cvars.set("r_shader_params", &new);
    }
}

pub(super) fn register_shader_commands(c: &mut Commands<Host>) {
    c.add("r_shader_list", |h, _| {
        h.print("Presets (r_shader_preset <name>):\n");
        for p in h.shader_presets() {
            h.print(&format!("  {p}\n"));
        }
        h.print("Add your own .slangp presets under <game>/shaders/.\n");
    });
    c.add("r_shader_reload", |h, _| h.shader.reload = h.shader.reload.wrapping_add(1));
    c.add("r_shader_param", |h, a| {
        let Some((_, params)) = h.r.shader_preset() else {
            h.print("no shader preset loaded (r_shader_preset)\n");
            return;
        };
        if a.argc() < 2 {
            let lines: Vec<String> = params
                .iter()
                .map(|p| {
                    let v = h.shader_param_value(&p.id).unwrap_or(p.initial);
                    format!("  {} = {v} ({}..{}) {}\n", p.id, p.min, p.max, p.description.trim())
                })
                .collect();
            for l in lines {
                h.print(&l);
            }
            return;
        }
        let id = a.argv(1).to_string();
        let Some(p) = params.iter().find(|p| p.id == id).cloned() else {
            h.print(&format!("no parameter \"{id}\" in this preset\n"));
            return;
        };
        if a.argc() < 3 {
            let v = h.shader_param_value(&id).unwrap_or(p.initial);
            h.print(&format!("{id} = {v} (default {}, {}..{})\n", p.initial, p.min, p.max));
        } else if a.argv(2).eq_ignore_ascii_case("default") {
            h.set_shader_param_override(&id, None);
        } else if let Ok(v) = a.argv(2).parse::<f32>() {
            h.set_shader_param_override(&id, Some(v.clamp(p.min, p.max)));
        }
    });
    c.add("r_shader_param_reset", |h, _| {
        h.server.core.cvars.set("r_shader_params", "");
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::shader::format_params;

    #[test]
    fn params_round_trip() {
        let p = parse_params("a=1;b = 0.5;;bad;c=x");
        assert_eq!(p, vec![("a".into(), 1.0), ("b".into(), 0.5)]);
        assert_eq!(format_params(&p), "a=1;b=0.5");
        assert_eq!(with_param("a=1;b=2", "a", Some(3.0)), "b=2;a=3");
        assert_eq!(with_param("a=1;b=2", "b", None), "a=1");
    }

    #[test]
    fn bundled_list_present() {
        let v = bundled_presets();
        assert!(v.iter().any(|p| p == "crt/crt-lottes"));
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/slang");
        for p in &v {
            assert!(root.join(format!("{p}.slangp")).is_file(), "{p}");
        }
    }
}
