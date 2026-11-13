//! Bounded local discovery and conventional retail/Steam install locations.
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::resolve_loose;

pub(super) fn has_pak(dir: &Path) -> bool {
    resolve_loose(dir, "pak0.pak").is_some_and(|p| p.is_file())
}

/// Search directories breadth first, including at most `depth` child levels.
/// Canonical identities prevent symlink cycles; unreadable directories are skipped.
fn search(root: &Path, depth: usize) -> Option<PathBuf> {
    let mut queue = VecDeque::from([(root.to_path_buf(), 0)]);
    let mut seen = HashSet::new();
    while let Some((dir, level)) = queue.pop_front() {
        let Ok(identity) = dir.canonicalize() else { continue };
        if !seen.insert(identity) { continue; }
        if has_pak(&dir) || (level < depth && has_pak(&dir.join("id1"))) {
            return Some(dir);
        }
        if level == depth { continue; }
        let mut children: Vec<_> = std::fs::read_dir(&dir).into_iter().flatten().flatten()
            .map(|e| e.path()).filter(|p| p.is_dir()).collect();
        children.sort();
        queue.extend(children.into_iter().map(|p| (p, level + 1)));
    }
    None
}

fn steam_installs(steam: &Path, candidates: &mut Vec<PathBuf>) {
    let mut libraries = vec![steam.to_path_buf()];
    // Valve's libraryfolders.vdf stores additional library roots as quoted paths.
    if let Ok(text) = std::fs::read_to_string(steam.join("steamapps/libraryfolders.vdf")) {
        for line in text.lines() {
            let fields: Vec<_> = line.split('"').collect();
            if fields.get(1) == Some(&"path") && let Some(path) = fields.get(3) {
                libraries.push(PathBuf::from(path.replace("\\\\", "\\")));
            }
        }
    }
    for library in libraries {
        candidates.push(library.join("steamapps/common/Quake"));
    }
}

pub(super) fn discover() -> PathBuf {
    if let Some(dir) = search(Path::new("."), 2) { return dir; }
    let mut candidates = Vec::new();
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = PathBuf::from(home);
        for name in [".steam/steam", ".local/share/Steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam", "Library/Application Support/Steam"] {
            steam_installs(&home.join(name), &mut candidates);
        }
        for name in ["Quake", "quake", "Games/Quake", "Games/quake", "games/quake", "GOG Games/Quake", "GOG Games/Quake The Offering"] {
            candidates.push(home.join(name));
        }
    }
    for variable in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(root) = std::env::var_os(variable) {
            let root = PathBuf::from(root);
            steam_installs(&root.join("Steam"), &mut candidates);
            candidates.push(root.join("GOG Galaxy/Games/Quake"));
        }
    }
    for root in ["C:/Quake", "C:/Games/Quake", "C:/GOG Games/Quake", "/usr/share/games/quake", "/usr/local/share/games/quake", "/opt/quake"] {
        candidates.push(PathBuf::from(root));
    }
    candidates.into_iter().find_map(|dir| search(&dir, 2)).unwrap_or_else(|| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_search_and_flat_paks() {
        let root = std::env::temp_dir().join(format!("qcore-discovery-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let deep = root.join("one/two/three/id1");
        std::fs::create_dir_all(&deep).unwrap();
        std::fs::write(deep.join("pak0.pak"), []).unwrap();
        assert_eq!(search(&root, 2), None);
        assert_eq!(search(&root, 4), Some(root.join("one/two/three")));
        let local = root.join("one/id1");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(local.join("PAK0.PAK"), []).unwrap();
        assert_eq!(search(&root, 2), Some(root.join("one")));
        std::fs::write(root.join("pak0.pak"), []).unwrap();
        assert_eq!(search(&root, 2), Some(root.clone()));
        let _ = std::fs::remove_dir_all(root);
    }
}
