//! User data directory: where saves, configs, demos, screenshots, videos,
//! bug reports, stats and Quaddicted downloads live.
//!
//! By default this is `$XDG_DATA_HOME/quake-source-port/` (`~/.local/share/quake-source-port/`).
//! The user can choose another parent folder in Options > Gameplay; the engine
//! always stores its data inside a `quake-source-port/` subfolder of that
//! location (without double-nesting if the selected folder is already named
//! `quake-source-port`). A one-line pointer file (`userdata.path`) at the
//! default location records a non-default choice across restarts.

use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Subdirectory created inside the user-selected folder.
pub const SUBDIR: &str = "quake-source-port";
/// Pointer file at the default location storing a custom parent directory.
const POINTER_FILE: &str = "userdata.path";

static CLI_OVERRIDE: RwLock<Option<PathBuf>> = RwLock::new(None);
static CONFIGURED: RwLock<Option<PathBuf>> = RwLock::new(None);

/// Default parent folder (`$XDG_DATA_HOME`, else `$HOME/.local/share`, else `.`).
pub fn default_parent() -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from(qcore::fs::WEB_USER_ROOT);
    }
    if cfg!(target_os = "android") && std::env::var_os("XDG_DATA_HOME").is_none() {
        return PathBuf::from(qcore::fs::ANDROID_DATA_DIR).join("userdata");
    }
    std::env::var_os("XDG_DATA_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").filter(|v| !v.is_empty()).map(|h| PathBuf::from(h).join(".local/share")))
        .or_else(|| std::env::var_os("APPDATA").filter(|v| !v.is_empty()).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Default effective user data root (`<default_parent>/quake-source-port` on desktop).
pub fn default_root() -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from(qcore::fs::WEB_USER_ROOT);
    }
    if cfg!(target_os = "android") && std::env::var_os("XDG_DATA_HOME").is_none() {
        return PathBuf::from(qcore::fs::ANDROID_DATA_DIR).join("userdata");
    }
    resolve_root(&default_parent())
}

fn is_subdir_name(p: &Path) -> bool {
    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.eq_ignore_ascii_case(SUBDIR))
}

/// Normalises a user-picked folder to the parent shown in the settings field:
/// selecting `.../quake-source-port` directly maps back to its parent so we
/// never create `.../quake-source-port/quake-source-port`.
pub fn normalize_parent(selected: &Path) -> PathBuf {
    if is_subdir_name(selected)
        && let Some(p) = selected.parent().filter(|p| !p.as_os_str().is_empty())
    {
        return p.to_path_buf();
    }
    selected.to_path_buf()
}

/// Effective user data root for a selected parent directory: always ends with
/// `quake-source-port` on desktop (without double-nesting).
pub fn resolve_root(parent: &Path) -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from(qcore::fs::WEB_USER_ROOT);
    }
    if is_subdir_name(parent) { parent.to_path_buf() } else { parent.join(SUBDIR) }
}

fn pointer_path() -> PathBuf {
    default_root().join(POINTER_FILE)
}

/// Loads any saved user data directory pointer at startup (`-userdir` wins
/// for this session without overwriting the saved pointer).
pub fn init(cli_override: Option<&Path>) {
    if let Ok(mut g) = CLI_OVERRIDE.write() {
        *g = cli_override.filter(|p| !p.as_os_str().is_empty()).map(normalize_parent);
    }
    if cfg!(target_arch = "wasm32") {
        return;
    }
    let loaded = qcore::vfs::read_to_string(&pointer_path())
        .ok()
        .and_then(|s| s.lines().map(str::trim).find(|l| !l.is_empty()).map(|l| normalize_parent(Path::new(l))));
    if let Ok(mut g) = CONFIGURED.write() {
        *g = loaded;
    }
}

/// Active selected parent directory (prefilled with the default location).
pub fn parent() -> PathBuf {
    if let Ok(g) = CLI_OVERRIDE.read()
        && let Some(p) = g.as_ref()
    {
        return p.clone();
    }
    if let Ok(g) = CONFIGURED.read()
        && let Some(p) = g.as_ref()
    {
        return p.clone();
    }
    default_parent()
}

/// Active effective user data root (`<parent>/quake-source-port` on desktop).
pub fn root() -> PathBuf {
    if cfg!(target_arch = "wasm32") {
        return PathBuf::from(qcore::fs::WEB_USER_ROOT);
    }
    if let Ok(g) = CLI_OVERRIDE.read()
        && let Some(p) = g.as_ref()
    {
        return resolve_root(p);
    }
    if let Ok(g) = CONFIGURED.read()
        && let Some(p) = g.as_ref()
    {
        return resolve_root(p);
    }
    default_root()
}

/// Per-game user data directory (`<root>/<game>`): config, saves, demos, stats,
/// screenshots, videos, bug reports.
pub fn game_dir(game: &str) -> PathBuf {
    root().join(game.to_ascii_lowercase())
}

/// User-installed map packs / Quaddicted installs (`<root>/mods`).
pub fn mods_dir() -> PathBuf {
    root().join("mods")
}

/// Quaddicted index cache and `installed.txt` (`<root>/quaddicted`).
pub fn quaddicted_cache_dir() -> PathBuf {
    root().join("quaddicted")
}

/// True when `root` already contains user files or folders other than the
/// bootstrap pointer file.
pub fn has_existing_data(root: &Path) -> bool {
    let Ok(entries) = qcore::vfs::read_dir(root) else { return false };
    entries.into_iter().any(|e| {
        e.path.file_name().and_then(|n| n.to_str()).is_none_or(|n| !n.eq_ignore_ascii_case(POINTER_FILE))
    })
}

/// Checks whether `target_root` can be created and written to without leaving
/// stray files behind.
pub fn check_writable(target_root: &Path) -> Result<(), String> {
    if cfg!(target_arch = "wasm32") {
        return Err("User data directory cannot be changed in the browser build".into());
    }
    let mut probe_dir = target_root;
    while !probe_dir.exists() {
        let Some(p) = probe_dir.parent().filter(|p| !p.as_os_str().is_empty()) else {
            return Err(format!("Cannot access {}", target_root.display()));
        };
        probe_dir = p;
    }
    if !probe_dir.is_dir() {
        return Err(format!("Not a directory: {}", probe_dir.display()));
    }
    let probe = probe_dir.join(format!(".qsp_write_probe_{}", std::process::id()));
    match std::fs::OpenOptions::new().write(true).create_new(true).open(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(e) => Err(format!("Directory is not writable: {e}")),
    }
}

/// Recursively copies all user data from `from` to `to`, skipping the
/// bootstrap pointer file and guarding against copying `to` into itself.
pub fn copy_data(from: &Path, to: &Path) -> Result<(), String> {
    if !from.is_dir() {
        return Ok(());
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let from_canon = std::fs::canonicalize(from).unwrap_or_else(|_| from.to_path_buf());
    let to_canon = std::fs::canonicalize(to).unwrap_or_else(|_| to.to_path_buf());
    if from_canon == to_canon {
        return Ok(());
    }
    copy_dir_inner(from, to, &to_canon, true)
}

fn copy_dir_inner(src: &Path, dst: &Path, skip_canon: &Path, top_level: bool) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    let rd = std::fs::read_dir(src).map_err(|e| format!("{}: {e}", src.display()))?;
    for entry in rd {
        let entry = entry.map_err(|e| format!("{}: {e}", src.display()))?;
        let name = entry.file_name();
        if top_level && name.to_str().is_some_and(|n| n.eq_ignore_ascii_case(POINTER_FILE)) {
            continue;
        }
        let sp = entry.path();
        if let Ok(canon) = std::fs::canonicalize(&sp)
            && canon == skip_canon
        {
            continue;
        }
        let Ok(ft) = entry.file_type() else { continue };
        let dp = dst.join(&name);
        if ft.is_dir() {
            copy_dir_inner(&sp, &dp, skip_canon, false)?;
        } else if ft.is_file() {
            std::fs::copy(&sp, &dp).map_err(|e| format!("{} -> {}: {e}", sp.display(), dp.display()))?;
        }
    }
    Ok(())
}

/// Sets the active user data parent directory, creates `<parent>/quake-source-port`,
/// and persists the pointer file when `persist` is true.
pub fn set_parent(selected: &Path, persist: bool) -> Result<PathBuf, String> {
    let norm = normalize_parent(selected);
    let target_root = resolve_root(&norm);
    qcore::vfs::create_dir_all(&target_root).map_err(|e| format!("{}: {e}", target_root.display()))?;
    let def_root = default_root();
    let is_default = target_root == def_root
        || std::fs::canonicalize(&target_root).ok().zip(std::fs::canonicalize(&def_root).ok()).is_some_and(|(a, b)| a == b);
    if let Ok(mut g) = CLI_OVERRIDE.write() {
        *g = None;
    }
    if let Ok(mut g) = CONFIGURED.write() {
        *g = (!is_default).then_some(norm.clone());
    }
    if persist && !cfg!(target_arch = "wasm32") {
        let ptr = pointer_path();
        if is_default {
            let _ = std::fs::remove_file(&ptr);
        } else {
            let text = format!("{}\n", norm.to_string_lossy());
            qcore::vfs::write(&ptr, text.as_bytes()).map_err(|e| format!("{}: {e}", ptr.display()))?;
        }
    }
    Ok(target_root)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmpdir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("qsp-userdata-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn resolves_subdir_without_double_nesting() {
        assert_eq!(resolve_root(Path::new("/mnt/games")), PathBuf::from("/mnt/games/quake-source-port"));
        assert_eq!(resolve_root(Path::new("/mnt/games/quake-source-port")), PathBuf::from("/mnt/games/quake-source-port"));
        assert_eq!(normalize_parent(Path::new("/mnt/games/quake-source-port")), PathBuf::from("/mnt/games"));
        assert_eq!(normalize_parent(Path::new("/mnt/games")), PathBuf::from("/mnt/games"));
    }

    #[test]
    fn copy_skips_pointer_and_nested_destination() {
        let base = tmpdir("copy");
        let old = base.join("old/quake-source-port");
        let new = base.join("new/quake-source-port");
        std::fs::create_dir_all(old.join("id1")).unwrap();
        std::fs::create_dir_all(old.join("mods/mymap/maps")).unwrap();
        std::fs::write(old.join(POINTER_FILE), "/some/other\n").unwrap();
        std::fs::write(old.join("id1/config.cfg"), "fov 100\n").unwrap();
        std::fs::write(old.join("mods/mymap/maps/test.bsp"), "bsp").unwrap();

        assert!(has_existing_data(&old));
        copy_data(&old, &new).unwrap();
        assert!(!new.join(POINTER_FILE).exists());
        assert_eq!(std::fs::read_to_string(new.join("id1/config.cfg")).unwrap(), "fov 100\n");
        assert_eq!(std::fs::read_to_string(new.join("mods/mymap/maps/test.bsp")).unwrap(), "bsp");

        // Copying into a subfolder of `old` must not recurse infinitely.
        let nested = old.join("sub/quake-source-port");
        copy_data(&old, &nested).unwrap();
        assert_eq!(std::fs::read_to_string(nested.join("id1/config.cfg")).unwrap(), "fov 100\n");
        assert!(!nested.join("sub/quake-source-port").exists());

        // Directory with only pointer file has no user data to copy.
        let only_ptr = base.join("only_ptr");
        std::fs::create_dir_all(&only_ptr).unwrap();
        std::fs::write(only_ptr.join(POINTER_FILE), "/x\n").unwrap();
        assert!(!has_existing_data(&only_ptr));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn check_writable_validates_target_without_creating_it() {
        let base = tmpdir("writable");
        let target = base.join("fresh/quake-source-port");
        assert!(check_writable(&target).is_ok());
        assert!(!target.exists(), "writability probe must not create the target folder early");
        let file = base.join("not_a_dir");
        std::fs::write(&file, "x").unwrap();
        assert!(check_writable(&file.join("quake-source-port")).is_err());
        let _ = std::fs::remove_dir_all(&base);
    }
}
