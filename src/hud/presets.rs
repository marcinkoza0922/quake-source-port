//! User-saved HUD presets stored as individual named `.hud` files in
//! `<user data>/hud_presets/<name>.hud`, shared across all games and mods.

use std::path::{Path, PathBuf};

/// Archived cvar holding the active saved preset's name (empty for modern/classic/unsaved custom).
pub const PRESET_NAME: &str = "hud_preset_name";

/// Maximum characters in a saved preset's name.
pub const MAX_NAME_LEN: usize = 24;

const EXT: &str = "hud";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedPreset {
    pub name: String,
    pub layout: String,
    /// Its `hud_addon_layouts`: further lines of the file.
    pub addons: String,
}

impl SavedPreset {
    /// Makes this preset the HUD.
    pub fn load(&self, cv: &mut qcore::cvar::Cvars) {
        cv.set(super::editor::CVAR, &self.layout);
        cv.set(super::addons::CVAR, &self.addons);
        cv.set(super::editor::PRESET, "2");
        cv.set(PRESET_NAME, &self.name);
    }
}

/// Directory for user-saved HUD presets (`<user data>/hud_presets`).
pub fn dir() -> PathBuf {
    crate::userdata::hud_presets_dir()
}

/// Built-in names that cannot be overwritten by a user preset.
pub fn is_reserved(name: &str) -> bool {
    let n = name.trim();
    n.eq_ignore_ascii_case("modern")
        || n.eq_ignore_ascii_case("classic")
        || n.eq_ignore_ascii_case("mod")
        || n.eq_ignore_ascii_case("mod hud")
        || n.eq_ignore_ascii_case("custom")
        || n.eq_ignore_ascii_case("default")
}

/// Sanitizes a preset name for safe cross-platform filenames (`<name>.hud`):
/// strips control characters and filesystem-forbidden characters, trims dots/spaces, caps length.
pub fn clean_name(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|&c| !c.is_control() && !matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    cleaned.trim_matches(|c: char| c.is_whitespace() || c == '.').chars().take(MAX_NAME_LEN).collect()
}

fn file_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}.{EXT}"))
}

/// The layout (the first line that isn't a comment) and the add-on patches after it.
fn parse_layout(text: &str) -> Option<(String, String)> {
    let mut lines = text.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#') && !l.starts_with("//"));
    let layout = lines.next()?.to_string();
    Some((layout, lines.collect::<Vec<_>>().join(" ")))
}

/// Loads all saved `*.hud` presets from `dir`, sorted by name.
pub fn load_from(dir: &Path) -> Vec<SavedPreset> {
    let Ok(entries) = qcore::vfs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries {
        if entry.is_dir {
            continue;
        }
        let is_hud = entry.path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case(EXT));
        if !is_hud {
            continue;
        }
        let Some(stem) = entry.path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let name = clean_name(stem);
        if name.is_empty() || is_reserved(&name) {
            continue;
        }
        let Ok(text) = qcore::vfs::read_to_string(&entry.path) else {
            continue;
        };
        let Some((layout, addons)) = parse_layout(&text) else {
            continue;
        };
        if let Some(existing) = out.iter_mut().find(|p: &&mut SavedPreset| p.name.eq_ignore_ascii_case(&name)) {
            existing.name = name;
            existing.layout = layout;
            existing.addons = addons;
        } else {
            out.push(SavedPreset { name, layout, addons });
        }
    }
    out.sort_by_key(|p| p.name.to_ascii_lowercase());
    out
}

pub fn load() -> Vec<SavedPreset> {
    load_from(&dir())
}

/// Writes one preset to `<dir>/<name>.hud` (replacing any case-insensitive match).
pub fn write_one_to(dir: &Path, preset: &SavedPreset) -> std::io::Result<()> {
    let name = clean_name(&preset.name);
    if name.is_empty() || is_reserved(&name) {
        return Ok(());
    }
    // Remove an existing file with different casing so we don't leave duplicates on case-sensitive FS.
    if let Ok(entries) = qcore::vfs::read_dir(dir) {
        for e in entries {
            if !e.is_dir
                && e.path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case(EXT))
                && let Some(stem) = e.path.file_stem().and_then(|s| s.to_str())
                && stem.eq_ignore_ascii_case(&name)
                && stem != name
            {
                let _ = qcore::vfs::remove_file(&e.path);
            }
        }
    }
    let mut body = format!("{}\n", preset.layout.trim());
    for line in super::addons::Patches::parse(&preset.addons).lines() {
        body += &line;
        body.push('\n');
    }
    qcore::vfs::write(&file_path(dir, &name), body.as_bytes())
}

pub fn write_one(preset: &SavedPreset) -> std::io::Result<()> {
    write_one_to(&dir(), preset)
}

/// Removes `<dir>/<name>.hud` (case-insensitive match).
pub fn delete_one_in(dir: &Path, name: &str) -> std::io::Result<()> {
    let target = clean_name(name);
    if let Ok(entries) = qcore::vfs::read_dir(dir) {
        for e in entries {
            if !e.is_dir
                && e.path.extension().and_then(|x| x.to_str()).is_some_and(|x| x.eq_ignore_ascii_case(EXT))
                && e.path.file_stem().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case(&target))
            {
                qcore::vfs::remove_file(&e.path)?;
            }
        }
    }
    Ok(())
}

pub fn delete_one(name: &str) -> std::io::Result<()> {
    delete_one_in(&dir(), name)
}

/// Inserts or overwrites a preset (case-insensitive name match). Returns `None` if invalid/reserved.
pub fn upsert(presets: &mut Vec<SavedPreset>, raw_name: &str, layout: &str, addons: &str) -> Option<usize> {
    let name = clean_name(raw_name);
    let layout = layout.trim();
    if name.is_empty() || is_reserved(&name) || layout.is_empty() {
        return None;
    }
    if let Some(i) = presets.iter().position(|p| p.name.eq_ignore_ascii_case(&name)) {
        presets[i].name = name;
        presets[i].layout = layout.to_string();
        presets[i].addons = addons.to_string();
        Some(i)
    } else {
        presets.push(SavedPreset { name, layout: layout.to_string(), addons: addons.to_string() });
        presets.sort_by_key(|p| p.name.to_ascii_lowercase());
        presets.iter().position(|p| p.name.eq_ignore_ascii_case(&clean_name(raw_name)))
    }
}

/// Removes a preset by name (case-insensitive). Returns true if one was removed.
pub fn remove(presets: &mut Vec<SavedPreset>, name: &str) -> bool {
    let before = presets.len();
    presets.retain(|p| !p.name.eq_ignore_ascii_case(name.trim()));
    presets.len() != before
}

/// Finds a preset by name (case-insensitive).
pub fn find<'a>(presets: &'a [SavedPreset], name: &str) -> Option<&'a SavedPreset> {
    let n = name.trim();
    presets.iter().find(|p| p.name.eq_ignore_ascii_case(n))
}

/// Picks the first unused default name (`Preset 1`, `Preset 2`, ...).
pub fn next_default_name(presets: &[SavedPreset]) -> String {
    (1..)
        .map(|n| format!("Preset {n}"))
        .find(|n| find(presets, n).is_none())
        .unwrap_or_else(|| "Preset 1".into())
}

/// Picks a fresh unused name for Save As: `<base> 2`, `<base> 3`, ... when
/// deriving from an existing preset, else `Preset 1`, `Preset 2`, ...
pub fn next_variant_name(presets: &[SavedPreset], base: Option<&str>) -> String {
    let Some(base) = base.map(clean_name).filter(|b| !b.is_empty() && !is_reserved(b)) else {
        return next_default_name(presets);
    };
    let stem = base.trim_end_matches(|c: char| c.is_ascii_digit() || c.is_whitespace());
    let prefix = if stem.is_empty() { base.as_str() } else { stem };
    (2..)
        .map(|n| {
            let suffix = format!(" {n}");
            let max_prefix = MAX_NAME_LEN.saturating_sub(suffix.len());
            let clipped: String = prefix.chars().take(max_prefix).collect();
            format!("{}{suffix}", clipped.trim_end())
        })
        .find(|n| find(presets, n).is_none())
        .unwrap_or_else(|| next_default_name(presets))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_name_and_upsert_reject_reserved_and_path_chars() {
        let mut list = Vec::new();
        assert_eq!(upsert(&mut list, "modern", "health@bl", ""), None);
        assert_eq!(upsert(&mut list, "Classic", "health@bl", ""), None);
        assert_eq!(upsert(&mut list, "  ", "health@bl", ""), None);
        assert_eq!(clean_name("../evil:name?"), "evilname");
        assert_eq!(upsert(&mut list, "Speedrun", "timer@tr pb@tr,y16", ""), Some(0));
        assert_eq!(upsert(&mut list, "Minimal", "health@bl,i ammo@br,i", ""), Some(0));
        // Sorted alphabetically: Minimal, Speedrun
        assert_eq!(list[0].name, "Minimal");
        assert_eq!(list[1].name, "Speedrun");
        // Case-insensitive overwrite keeps slot
        assert_eq!(upsert(&mut list, "minimal", "health@bl,i", ""), Some(0));
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].name, "minimal");
        assert!(remove(&mut list, "MINIMAL"));
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Speedrun");
    }

    #[test]
    fn next_default_name_skips_taken_slots() {
        let mut list = Vec::new();
        assert_eq!(next_default_name(&list), "Preset 1");
        upsert(&mut list, "Preset 1", "health@bl", "");
        assert_eq!(next_default_name(&list), "Preset 2");
        upsert(&mut list, "preset 2", "health@bl", "");
        assert_eq!(next_default_name(&list), "Preset 3");
    }

    #[test]
    fn named_hud_files_round_trip_in_directory() {
        let dir = std::env::temp_dir().join(format!("qsp_hud_presets_dir_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let p1 = SavedPreset { name: "Immersive".into(), layout: "health@bl,i armor@bl,y-28,i".into(), addons: "bullettime+magazine: -armor magazine: magazine@br".into() };
        let p2 = SavedPreset { name: "Speedrun HUD".into(), layout: "timer@tr pb@tr,y16".into(), ..Default::default() };
        write_one_to(&dir, &p1).unwrap();
        write_one_to(&dir, &p2).unwrap();
        assert!(dir.join("Immersive.hud").is_file());
        assert!(dir.join("Speedrun HUD.hud").is_file());
        assert_eq!(load_from(&dir), vec![p1.clone(), p2.clone()]);
        delete_one_in(&dir, "immersive").unwrap();
        assert!(!dir.join("Immersive.hud").exists());
        assert_eq!(load_from(&dir), vec![p2]);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
