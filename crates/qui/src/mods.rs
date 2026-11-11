//! Installed mods: game directories under the basedir, their metadata and
//! their playable maps.
//!
//! Metadata comes from an optional `modinfo.txt` in the mod dir (lines of
//! `key value`, value optionally quoted: `title`, `base` = hipnotic / rogue /
//! quoth, `description`), then a built-in table for the official games, then
//! the dir name. The first lines of the mod's readme are shown as a blurb.

use std::path::Path;

use qcore::fs::Fs;

/// Mission pack a mod builds on (mounted below it, like `-hipnotic`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Base {
    Id,
    Hipnotic,
    Rogue,
    Quoth,
}

impl Base {
    pub fn dir(self) -> Option<&'static str> {
        match self {
            Base::Id => None,
            Base::Hipnotic => Some("hipnotic"),
            Base::Rogue => Some("rogue"),
            Base::Quoth => Some("quoth"),
        }
    }
    fn from_name(s: &str) -> Option<Base> {
        [Base::Hipnotic, Base::Rogue, Base::Quoth].into_iter().find(|b| b.dir().is_some_and(|d| d.eq_ignore_ascii_case(s)))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModInfo {
    /// Directory name under the basedir.
    pub dir: String,
    pub title: String,
    pub base: Base,
    /// Short description: modinfo `description`, else the readme's first lines.
    pub blurb: Vec<String>,
}

impl ModInfo {
    /// Engine arguments that mount this mod (`-hipnotic -game foo`).
    pub fn launch_args(&self) -> Vec<String> {
        let mut a = Vec::new();
        if let Some(b) = self.base.dir()
            && !b.eq_ignore_ascii_case(&self.dir)
        {
            a.push(format!("-{b}"));
        }
        if !self.dir.eq_ignore_ascii_case("id1") {
            a.push("-game".into());
            a.push(self.dir.clone());
        }
        a
    }
}

/// Titles of the official releases.
const KNOWN: &[(&str, &str)] = &[
    ("id1", "Quake"),
    ("ad", "Arcane Dimensions"),
    ("hipnotic", "Scourge of Armagon"),
    ("rogue", "Dissolution of Eternity"),
    ("mg1", "Dimension of the Machine"),
    ("ctf", "Capture the Flag"),
    ("dopa", "Dimension of the Past"),
    ("q64", "Quake 64"),
];

/// True for a directory with game data (paks, pk3s, progs or maps).
pub fn is_game_dir(path: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(path) else { return false };
    rd.flatten().any(|e| {
        let n = e.file_name().to_string_lossy().to_ascii_lowercase();
        let dir = e.path().is_dir();
        (!dir && (n.ends_with(".pak") || n.ends_with(".pk3") || n == "progs.dat" || n == "csprogs.dat")) || (dir && n == "maps")
    })
}

/// All mods under `basedir`: id1 first, then by title.
pub fn scan(basedir: &Path) -> Vec<ModInfo> {
    let mut out: Vec<ModInfo> = std::fs::read_dir(basedir)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| is_game_dir(&e.path()))
        .filter_map(|e| e.file_name().to_str().map(|d| info(&e.path(), d)))
        .collect();
    out.sort_by_key(|m| (!m.dir.eq_ignore_ascii_case("id1"), m.title.to_ascii_lowercase()));
    out
}

fn info(path: &Path, dir: &str) -> ModInfo {
    let mut m = ModInfo {
        dir: dir.to_string(),
        title: KNOWN.iter().find(|k| k.0.eq_ignore_ascii_case(dir)).map_or_else(|| dir.to_string(), |k| k.1.to_string()),
        base: Base::from_name(dir).unwrap_or(Base::Id),
        blurb: Vec::new(),
    };
    if let Ok(t) = std::fs::read(path.join("modinfo.txt")) {
        for line in String::from_utf8_lossy(&t).lines() {
            let line = line.trim();
            let (k, v) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let v = v.trim().trim_matches('"').trim();
            match k.to_ascii_lowercase().as_str() {
                "title" if !v.is_empty() => m.title = v.to_string(),
                "base" => m.base = Base::from_name(v).unwrap_or(Base::Id),
                "description" if !v.is_empty() => m.blurb.push(v.to_string()),
                _ => {}
            }
        }
    }
    if m.blurb.is_empty()
        && let Some(r) = readme(path, dir)
    {
        m.blurb = blurb(&r, 6);
    }
    m
}

/// The mod's readme: a `.txt`/`.md` whose name says readme, else one named
/// after the mod, else none (changelogs and credits are no use as a blurb).
fn readme(path: &Path, dir: &str) -> Option<String> {
    let mut files: Vec<(u8, std::path::PathBuf)> = std::fs::read_dir(path)
        .ok()?
        .flatten()
        .filter_map(|e| {
            let n = e.file_name().to_string_lossy().to_ascii_lowercase();
            if !(n.ends_with(".txt") || n.ends_with(".md")) {
                return None;
            }
            let rank = if n.contains("readme") {
                0
            } else if n.starts_with(&dir.to_ascii_lowercase()) {
                1
            } else {
                return None;
            };
            Some((rank, e.path()))
        })
        .collect();
    files.sort();
    let data = std::fs::read(&files.first()?.1).ok()?;
    Some(String::from_utf8_lossy(&data).into_owned())
}

/// First `n` lines with real text, cleaned to printable ASCII.
fn blurb(text: &str, n: usize) -> Vec<String> {
    text.lines()
        .map(|l| {
            let s: String = l.chars().map(|c| if c == '\t' { ' ' } else { c }).filter(|c| c.is_ascii() && !c.is_ascii_control()).collect();
            s.trim_matches(|c: char| c.is_whitespace() || "#*=-_~|".contains(c)).to_string()
        })
        .filter(|l| l.chars().filter(|c| c.is_ascii_alphanumeric()).count() >= 3)
        .take(n)
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapInfo {
    /// Name for the `map` command (no `maps/` or `.bsp`).
    pub name: String,
    /// worldspawn `message`, cleaned.
    pub title: String,
}

/// Playable maps the mod itself ships (not inherited from id1 or a base
/// pack): a map is playable when it has an `info_player_*` entity. `start`
/// comes first, then by name.
pub fn maps(basedir: &Path, dir: &str) -> Vec<MapInfo> {
    let mut fs = Fs::new(basedir);
    if !fs.add_game_dir(dir) {
        return Vec::new();
    }
    let mut out: Vec<MapInfo> = fs
        .list("maps/", ".bsp")
        .into_iter()
        .filter_map(|p| {
            let name = p.get(5..p.len() - 4)?.to_ascii_lowercase();
            if name.contains('/') || name.starts_with("b_") {
                return None;
            }
            let data = fs.open(&p)?;
            let ents = qformats::entities::parse_entities(&entity_lump(&data)?).ok()?;
            if !ents.iter().any(|e| e.classname().is_some_and(|c| c.starts_with("info_player_"))) {
                return None;
            }
            let title = ents.first().and_then(|w| w.get("message")).map(clean_message).unwrap_or_default();
            Some(MapInfo { name, title })
        })
        .collect();
    out.sort_by(|a, b| (a.name != "start", &a.name).cmp(&(b.name != "start", &b.name)));
    out
}

/// The entity lump text of a BSP29 / BSP2 / Quake64 file (lump 0 in all).
/// Not always UTF-8 (Quake charset bytes), so converted lossily.
pub fn entity_lump(data: &[u8]) -> Option<std::borrow::Cow<'_, str>> {
    let rd = |o: usize| data.get(o..o + 4).map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]));
    let (ofs, len) = (usize::try_from(rd(4)?).ok()?, usize::try_from(rd(8)?).ok()?);
    let lump = data.get(ofs..ofs.checked_add(len)?)?;
    let end = lump.iter().position(|&b| b == 0).unwrap_or(lump.len());
    Some(String::from_utf8_lossy(&lump[..end]))
}

/// Worldspawn message as one line: `\n` escapes and newlines become spaces,
/// coloured (high-bit) characters become plain.
pub fn clean_message(m: &str) -> String {
    let m = m.replace("\\n", " ");
    let s: String = m.bytes().map(|b| (b & 0x7f) as char).map(|c| if c.is_ascii_control() { ' ' } else { c }).collect();
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn basedir() -> Option<std::path::PathBuf> {
        let d = std::env::var_os("QUAKE_BASEDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
        d.join("id1").is_dir().then_some(d)
    }

    #[test]
    fn launch_args_and_cleaning() {
        let m = |dir: &str, base| ModInfo { dir: dir.into(), title: String::new(), base, blurb: vec![] };
        assert!(m("id1", Base::Id).launch_args().is_empty());
        assert_eq!(m("hipnotic", Base::Hipnotic).launch_args(), ["-game", "hipnotic"]);
        assert_eq!(m("foo", Base::Rogue).launch_args(), ["-rogue", "-game", "foo"]);
        assert_eq!(clean_message("The\\nSlipgate  Complex\n"), "The Slipgate Complex");
        assert_eq!(blurb("=====\n\n  Title: Foo  \n--\nby someone\n", 1), ["Title: Foo"]);
        assert!(entity_lump(&[0; 6]).is_none());
    }

    #[test]
    fn scan_real_basedir() {
        let Some(b) = basedir() else { return };
        let mods = scan(&b);
        assert_eq!(mods[0].dir, "id1");
        assert_eq!(mods[0].title, "Quake");
        let hip = mods.iter().find(|m| m.dir == "hipnotic").unwrap();
        assert_eq!(hip.base, Base::Hipnotic);
        let maps = maps(&b, "id1");
        assert_eq!(maps[0].name, "start");
        let e1m1 = maps.iter().find(|m| m.name == "e1m1").unwrap();
        assert_eq!(e1m1.title, "The Slipgate Complex");
        assert!(!maps.iter().any(|m| m.name.starts_with("b_")));
        if b.join("ad").is_dir() {
            assert!(super::maps(&b, "ad").iter().any(|m| m.name == "ad_sepulcher"));
        }
    }
}
