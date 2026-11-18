//! Quaddicted map archive: the release index and installing a release.
//!
//! The index is the `Quaddicted/quaddicted-data` GitHub repository (one JSON
//! file per release under `json/by-sha256/`), fetched as a single zip; the old
//! `quaddicted_database.xml` is frozen. Downloads go through the system `curl`
//! (shipped with Linux, macOS and Windows 10+), so no TLS stack is linked.
//!
//! Installing maps each zip entry to a path under the basedir (the release's
//! `extract` / `extractmapping` rules), then moves plain `id1` content and
//! content for the mission-pack bases (hipnotic, rogue, quoth) into a game
//! dir of its own, so every release shows up as its own entry in Map Packs.
//! Content for other mods (ad, copper, ...) goes into that mod's dir.

mod worker;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use qui::json::Value;

pub use worker::{Job, Online, Status};

pub const INDEX_URL: &str = "https://codeload.github.com/Quaddicted/quaddicted-data/zip/refs/heads/main";

/// How a release's zip is laid out on disk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Install {
    /// `extract`: basedir-relative dir the whole zip is unpacked into.
    pub extract: Option<String>,
    /// `extractmapping`: zip path prefix (or file) -> basedir path, `None` = skip.
    pub mapping: Vec<(String, Option<String>)>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Release {
    pub sha: String,
    pub title: String,
    pub author: String,
    /// `YYYY-MM-DD`.
    pub date: String,
    pub kinds: Vec<String>,
    pub description: String,
    pub bytes: u64,
    pub url: String,
    /// Zip file name without extension (lowercase), used for the game dir.
    pub stem: String,
    pub startmaps: Vec<String>,
    pub deps: Vec<String>,
    pub singleplayer: bool,
    pub install: Install,
}

fn s(v: &Value) -> Option<&str> {
    if let Value::Str(s) = v { Some(s) } else { None }
}

fn arr(v: Option<&Value>) -> &[Value] {
    if let Some(Value::Arr(a)) = v { a } else { &[] }
}

/// Parses one release JSON; `None` for files without a download.
pub fn parse_release(v: &Value) -> Option<Release> {
    let mut r = Release {
        sha: s(v.get("sha256")?)?.to_string(),
        description: strip_html(v.get("description").and_then(s).unwrap_or("")),
        bytes: v.get("bytes").and_then(Value::num).unwrap_or(0.0) as u64,
        url: arr(v.get("urls")).iter().filter_map(s).next()?.to_string(),
        ..Default::default()
    };
    let mut filename = String::new();
    let mut authors = Vec::new();
    for t in arr(v.get("tags")).iter().filter_map(s) {
        let Some((k, val)) = t.split_once('=') else { continue };
        match k {
            "title" => r.title = val.to_string(),
            "author" => authors.push(val.to_string()),
            "release_date" => r.date = val.to_string(),
            "type" => r.kinds.push(val.to_string()),
            "startmap" => r.startmaps.push(val.to_ascii_lowercase()),
            "dependency" => r.deps.push(val.to_string()),
            "game_mode" if val == "singleplayer" || val == "cooperative" => r.singleplayer = true,
            "filename" => filename = val.to_string(),
            _ => {}
        }
    }
    r.author = authors.join(", ");
    if filename.is_empty() {
        filename = r.url.rsplit('/').next().unwrap_or("").to_string();
    }
    r.stem = sanitize(filename.rsplit_once('.').map_or(filename.as_str(), |(a, _)| a));
    if r.stem.is_empty() {
        r.stem = r.sha[..r.sha.len().min(12)].to_string();
    }
    if r.title.is_empty() {
        r.title = r.stem.clone();
    }
    if let Some(i) = v.get("install") {
        r.install.extract = i.get("extract").and_then(s).map(|e| clean(e.trim_start_matches("{base}")));
        if let Some(m) = i.get("extractmapping").and_then(Value::as_obj) {
            r.install.mapping = m.iter().map(|(k, v)| (k.to_ascii_lowercase(), s(v).map(clean))).collect();
        }
    }
    Some(r)
}

/// Reads every release from the index zip, newest first.
pub fn load_index(zip: &Path) -> Option<Vec<Release>> {
    let pack = qcore::fs::Pack::load_pk3(zip)?;
    let mut out: Vec<Release> = pack
        .names()
        .iter()
        .filter(|n| n.contains("json/by-sha256/") && n.ends_with(".json"))
        .filter_map(|n| pack.read(n))
        .filter_map(|d| qui::json::parse(&String::from_utf8_lossy(&d)))
        .filter_map(|v| parse_release(&v))
        .collect();
    out.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.title.cmp(&b.title)));
    (!out.is_empty()).then_some(out)
}

/// Lowercase `[a-z0-9_.-]` only, for a game dir name.
fn sanitize(s: &str) -> String {
    s.chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') { c } else { '_' })
        .collect::<String>()
        .trim_matches('.')
        .to_string()
}

/// Normalises a path to `a/b/c` (no leading or doubled slashes), lowercase.
fn clean(p: &str) -> String {
    p.replace('\\', "/").split('/').filter(|c| !c.is_empty() && *c != ".").collect::<Vec<_>>().join("/").to_ascii_lowercase()
}

/// Drops tags and decodes the common entities of a description.
pub fn strip_html(s: &str) -> String {
    let s = s.replace("<br />", " ").replace("<br>", " ").replace("<br/>", " ");
    let mut out = String::new();
    let mut tag = false;
    for c in s.chars() {
        match c {
            '<' => tag = true,
            '>' if tag => tag = false,
            _ if !tag => out.push(c),
            _ => {}
        }
    }
    let out = out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'");
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Basedir-relative destination of a zip entry (`None` = not installed).
pub fn map_entry(install: &Install, name: &str) -> Option<String> {
    let name = clean(name);
    let full = format!("/{name}");
    // the longest matching prefix (dir keys end in '/') or exact file wins
    let best = install
        .mapping
        .iter()
        .filter(|(k, _)| if k.ends_with('/') { full.starts_with(k.as_str()) } else { full == *k })
        .max_by_key(|(k, _)| k.len());
    let dest = match best {
        Some((k, Some(v))) if k.ends_with('/') => format!("{v}/{}", &full[k.len()..]),
        Some((_, Some(v))) => v.clone(),
        Some((_, None)) => return None,
        None => format!("{}/{name}", install.extract.as_deref()?),
    };
    let dest = clean(&dest);
    let bad = dest.is_empty() || dest.split('/').any(|c| c == ".." || c.contains(':'));
    (!bad).then_some(dest)
}

/// Engine subdirs: a path starting with one of these is `id1` content.
const DATA_DIRS: &[&str] = &["maps", "progs", "gfx", "sound", "music", "textures", "env", "sprites", "models", "locs", "lits", "demos"];
const BASES: &[&str] = &["hipnotic", "rogue", "quoth"];

/// The game dir a release's dependencies name: a mission pack by prefix
/// (`quoth2pt2full_2`), else an installed dir followed by a version or `_`
/// (`ad_v1_80p1final` -> `ad`).
pub fn dep_base(deps: &[String], installed: &[String], own: &str) -> Option<String> {
    let deps: Vec<String> = deps.iter().map(|d| d.to_ascii_lowercase()).collect();
    if let Some(b) = BASES.iter().find(|b| deps.iter().any(|d| d.starts_with(*b))) {
        return Some((*b).to_string());
    }
    let mut dirs: Vec<&String> = installed.iter().filter(|d| !d.eq_ignore_ascii_case("id1") && !d.eq_ignore_ascii_case(own)).collect();
    // the longest name wins (`ad_xmas` over `ad`)
    dirs.sort_by_key(|d| std::cmp::Reverse(d.len()));
    dirs.into_iter()
        .find(|dir| {
            deps.iter().any(|d| {
                d.strip_prefix(&dir.to_ascii_lowercase())
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with(['_', '-', 'v']) || rest.starts_with(|c: char| c.is_ascii_digit()))
            })
        })
        .cloned()
}

/// Final layout: destination paths with id1/base content moved into `own`.
/// Returns (path, zip entry) pairs and the base the own dir needs.
pub fn plan(install: &Install, names: &[String], own: &str) -> (Vec<(String, String)>, Option<&'static str>) {
    let mut base = None;
    let mut out = Vec::new();
    // A release that ships a base pack's own paks or progs.dat is that pack
    // (Quoth itself): its files stay in `quoth/` for everything built on it.
    let is_pack = |b: &str| {
        names.iter().filter_map(|n| map_entry(install, n)).any(|d| {
            d.strip_prefix(b).and_then(|r| r.strip_prefix('/')).is_some_and(|r| {
                let r = r.to_ascii_lowercase();
                r == "progs.dat" || (r.starts_with("pak") && r.ends_with(".pak") && !r.contains('/'))
            })
        })
    };
    let packs: Vec<&str> = BASES.iter().copied().filter(|b| is_pack(b)).collect();
    for n in names {
        let Some(dest) = map_entry(install, n) else { continue };
        let (first, rest) = dest.split_once('/').unwrap_or(("", dest.as_str()));
        let dest = if first.is_empty() && dest.ends_with(".bsp") {
            format!("{own}/maps/{dest}")
        } else if first.is_empty() || DATA_DIRS.contains(&first) {
            format!("{own}/{dest}")
        } else if first == "id1" {
            format!("{own}/{rest}")
        } else if packs.contains(&first) {
            dest
        } else if let Some(b) = BASES.iter().find(|b| **b == first) {
            base = Some(*b);
            format!("{own}/{rest}")
        } else {
            dest
        };
        out.push((dest, n.clone()));
    }
    (out, base)
}

/// Game dirs that a plan writes to but that do not exist yet and would only
/// get maps: these are mods the release needs installed first.
pub fn missing_mods(planned: &[(String, String)], basedir: &Path, own: &str) -> Vec<String> {
    let mut dirs: HashMap<&str, bool> = HashMap::new();
    for (p, _) in planned {
        let (d, rest) = p.split_once('/').unwrap_or((p, ""));
        let maps_only = rest.starts_with("maps/");
        *dirs.entry(d).or_insert(true) &= maps_only;
    }
    let mut v: Vec<String> = dirs
        .into_iter()
        .filter(|&(d, maps_only)| maps_only && d != own && !basedir.join(d).is_dir())
        .map(|(d, _)| d.to_string())
        .collect();
    v.sort();
    v
}

/// Unpacks `zip` into `basedir`; returns the game dir to show the release
/// under. A `modinfo.txt` names the own dir (and its base).
pub fn install(r: &Release, zip: &Path, basedir: &Path) -> Result<String, String> {
    let pack = qcore::fs::Pack::load_pk3(zip).ok_or(qcore::t!("the download is not a valid zip"))?;
    let own = r.stem.clone();
    let (planned, base) = plan(&r.install, pack.names(), &own);
    if planned.is_empty() {
        return Err(qcore::t!("nothing to install for this engine").into());
    }
    let missing = missing_mods(&planned, basedir, &own);
    if !missing.is_empty() {
        return Err(qcore::tr!("needs {} installed first", missing.join(", ")));
    }
    let existed: Vec<String> = planned
        .iter()
        .filter_map(|(d, _)| d.split('/').next())
        .filter(|d| basedir.join(d).is_dir())
        .map(str::to_string)
        .collect();
    // a mod built on a mission pack or another mod (deps like
    // quoth2pt2full_2, ad_v1_80p1final) needs it mounted
    let installed: Vec<String> = qui::mods::scan(basedir).into_iter().map(|m| m.dir).collect();
    let dep_base = dep_base(&r.deps, &installed, &own);
    let base = base.map(str::to_string).or_else(|| dep_base.clone());
    for (dest, name) in &planned {
        let data = pack.read(name).ok_or_else(|| qcore::tr!("couldn't unpack {}", name))?;
        let path: PathBuf = basedir.join(dest);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p).map_err(|e| format!("{}: {e}", p.display()))?;
        }
        std::fs::write(&path, data).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for (d, _) in &planned {
        *counts.entry(d.split('/').next().unwrap_or("")).or_default() += 1;
    }
    // the own dir counts only when it is playable (not just a stray readme)
    if qui::mods::is_game_dir(&basedir.join(&own)) {
        let q = |t: &str| t.replace('"', "'");
        let mut info = format!("title \"{}\"\n", q(&r.title));
        if let Some(b) = base {
            info += &format!("base {b}\n");
        }
        if !r.description.is_empty() {
            info += &format!("description \"{}\"\n", q(&r.description));
        }
        std::fs::write(basedir.join(&own).join("modinfo.txt"), info).map_err(|e| e.to_string())?;
        return Ok(own);
    }
    counts.remove(own.as_str());
    let shown = counts.into_iter().max_by_key(|&(_, n)| n).map(|(d, _)| d.to_string()).unwrap_or(own);
    // a game dir this release created itself (e.g. `bluemonday/`) gets its
    // title and base too; dirs that were already there (`ad/`) are left alone
    if let Some(b) = dep_base
        && !existed.contains(&shown)
        && !BASES.contains(&shown.as_str())
        && !shown.eq_ignore_ascii_case(&b)
        && qui::mods::is_game_dir(&basedir.join(&shown))
        && !basedir.join(&shown).join("modinfo.txt").exists()
    {
        let info = format!("title \"{}\"\nbase {b}\n", r.title.replace('"', "'"));
        std::fs::write(basedir.join(&shown).join("modinfo.txt"), info).map_err(|e| e.to_string())?;
    }
    Ok(shown)
}

/// `installed.txt`: one `sha256 dir` line per installed release.
pub fn load_installed(path: &Path) -> HashMap<String, String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once(' '))
        .map(|(a, b)| (a.to_string(), b.trim().to_string()))
        .collect()
}

pub fn save_installed(path: &Path, m: &HashMap<String, String>) {
    let mut v: Vec<_> = m.iter().map(|(a, b)| format!("{a} {b}\n")).collect();
    v.sort();
    let _ = std::fs::write(path, v.concat());
}

#[cfg(test)]
mod tests;
