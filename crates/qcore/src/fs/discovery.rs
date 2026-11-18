//! Bounded local discovery and conventional retail/Steam install locations.
use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};

use super::resolve_loose;

pub(super) fn has_pak(dir: &Path) -> bool {
    resolve_loose(dir, "pak0.pak").is_some_and(|p| crate::vfs::is_file(&p))
}

/// A flat directory of PAKs counts as the base game only when its pak0
/// holds the game's palette: a mod or a mapping kit unpacked next to the
/// game data (e.g. Alkaline's dev kit, a flat `pak0.pak` with maps and
/// models) must not win the search over the real `id1` beside it.
fn has_base_pak(dir: &Path) -> bool {
    resolve_loose(dir, "pak0.pak")
        .and_then(|p| super::Pack::load_pak(&p))
        .is_some_and(|p| p.contains("gfx/palette.lmp"))
}

/// Search directories breadth first, including at most `depth` child levels.
/// Canonical identities prevent symlink cycles; unreadable directories are skipped.
fn search(root: &Path, depth: usize) -> Option<PathBuf> {
    let mut queue = VecDeque::from([(root.to_path_buf(), 0)]);
    let mut seen = HashSet::new();
    while let Some((dir, level)) = queue.pop_front() {
        let Ok(identity) = dir.canonicalize() else { continue };
        if !seen.insert(identity) { continue; }
        // A Steam/GOG/Epic install holds the 2021 re-release in rerelease/
        // next to the original data; the re-release is the one to run.
        let rerelease = dir.join("rerelease");
        if has_pak(&rerelease.join("id1")) {
            return Some(rerelease);
        }
        if has_base_pak(&dir) || (level < depth && has_pak(&dir.join("id1"))) {
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
        // the 2021 re-release lives in rerelease/; the original data sits above it
        candidates.push(library.join("steamapps/common/Quake/rerelease"));
        candidates.push(library.join("steamapps/common/Quake"));
    }
}

/// Folders where the 2021 re-release (KEX) keeps its saves: Windows
/// `Saved Games\\Nightdive Studios\\Quake`, or the same inside Steam's Proton
/// prefix (app 2310) on Linux. ironwail loads saves from there as well.
pub fn kex_save_dirs() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = std::env::var_os("USERPROFILE") {
        out.push(PathBuf::from(p).join("Saved Games/Nightdive Studios/Quake"));
    }
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        let mut libs = Vec::new();
        for name in [".steam/steam", ".local/share/Steam", ".var/app/com.valvesoftware.Steam/.local/share/Steam"] {
            let mut c = Vec::new();
            steam_installs(&home.join(name), &mut c);
            // candidates end in steamapps/common/Quake[/rerelease]; the library is three up
            libs.extend(c.iter().filter_map(|p| p.ancestors().nth(if p.ends_with("rerelease") { 4 } else { 3 }).map(Path::to_path_buf)));
        }
        libs.sort();
        libs.dedup();
        for lib in libs {
            out.push(lib.join("steamapps/compatdata/2310/pfx/drive_c/users/steamuser/Saved Games/Nightdive Studios/Quake"));
        }
    }
    out.retain(|d| d.is_dir());
    out
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
        // a flat pak0 without the base game's palette (a mod, a dev kit)
        // doesn't win over the id1 below it; one with the palette does
        std::fs::write(root.join("pak0.pak"), pak_with("maps/test.bsp")).unwrap();
        assert_eq!(search(&root, 2), Some(root.join("one")));
        std::fs::write(root.join("pak0.pak"), pak_with("gfx/palette.lmp")).unwrap();
        assert_eq!(search(&root, 2), Some(root.clone()));
        let _ = std::fs::remove_dir_all(root);
    }

    /// A PAK holding one 4-byte file called `name`.
    fn pak_with(name: &str) -> Vec<u8> {
        let mut v = b"PACK".to_vec();
        v.extend_from_slice(&16i32.to_le_bytes());
        v.extend_from_slice(&64i32.to_le_bytes());
        v.extend_from_slice(&[0; 4]);
        let mut entry = [0u8; 64];
        entry[..name.len()].copy_from_slice(name.as_bytes());
        entry[56..60].copy_from_slice(&12i32.to_le_bytes());
        entry[60..64].copy_from_slice(&4i32.to_le_bytes());
        v.extend_from_slice(&entry);
        v
    }

    #[test]
    fn prefers_rerelease_folder() {
        let root = std::env::temp_dir().join(format!("qcore-rerelease-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        for d in ["Quake/id1", "Quake/rerelease/id1"] {
            std::fs::create_dir_all(root.join(d)).unwrap();
            std::fs::write(root.join(d).join("pak0.pak"), []).unwrap();
        }
        assert_eq!(search(&root.join("Quake"), 2), Some(root.join("Quake/rerelease")));
        assert_eq!(search(&root, 2), Some(root.join("Quake/rerelease")));
        let _ = std::fs::remove_dir_all(root);
    }
}
