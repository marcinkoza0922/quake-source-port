//! Installed mods: game directories under the basedir, their metadata and
//! their playable maps.
//!
//! Metadata comes from an optional `modinfo.txt` in the mod dir (lines of
//! `key value`, value optionally quoted: `title`, `base` = hipnotic / rogue /
//! quoth or any other installed game dir, `description`), then a built-in
//! table for the official games, then the dir name. The first lines of the
//! mod's readme are shown as a blurb. A map pack without progs or modinfo
//! `base` whose readme names an installed mod ("Mod Needed : Arcane
//! Dimension 1.7") is mounted on that mod.

use std::path::Path;

use qcore::fs::Fs;

/// Game a mod builds on, mounted below it: a mission pack (`-hipnotic`) or
/// any other game dir (`-game ad -game mymap`, as ironwail and vkQuake stack
/// several `-game`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Base {
    Id,
    Hipnotic,
    Rogue,
    Quoth,
    Mod(String),
}

impl Base {
    pub fn dir(&self) -> Option<&str> {
        match self {
            Base::Id => None,
            Base::Hipnotic => Some("hipnotic"),
            Base::Rogue => Some("rogue"),
            Base::Quoth => Some("quoth"),
            Base::Mod(d) => Some(d),
        }
    }
    /// Engine arguments that mount the base itself.
    pub fn args(&self) -> Vec<String> {
        match self {
            Base::Id => Vec::new(),
            Base::Mod(d) => vec!["-game".into(), d.clone()],
            b => vec![format!("-{}", b.dir().unwrap_or_default())],
        }
    }
    fn from_name(s: &str) -> Option<Base> {
        [Base::Hipnotic, Base::Rogue, Base::Quoth].into_iter().find(|b| b.dir().is_some_and(|d| d.eq_ignore_ascii_case(s)))
    }
    /// A modinfo `base` value: a mission pack, else another game dir.
    fn parse(s: &str) -> Base {
        let s = s.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("id1") || s.contains(['/', '\\']) || s.contains("..") {
            return Base::Id;
        }
        Base::from_name(s).unwrap_or_else(|| Base::Mod(s.to_string()))
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
    /// Engine arguments that mount this mod (`-hipnotic -game foo`,
    /// `-game ad -game foo`).
    pub fn launch_args(&self) -> Vec<String> {
        let mut a = Vec::new();
        if self.base.dir().is_some_and(|b| !b.eq_ignore_ascii_case(&self.dir)) {
            a.extend(self.base.args());
        }
        if !self.dir.eq_ignore_ascii_case("id1") {
            a.push("-game".into());
            a.push(self.dir.clone());
        }
        a
    }
    /// Game dirs below id1 in mount order: the base (if any), then the mod.
    pub fn game_dirs(&self) -> Vec<String> {
        let mut v = Vec::new();
        if let Some(b) = self.base.dir().filter(|b| !b.eq_ignore_ascii_case(&self.dir)) {
            v.push(b.to_string());
        }
        if !self.dir.eq_ignore_ascii_case("id1") {
            v.push(self.dir.clone());
        }
        v
    }
}

/// Titles of the official releases.
const KNOWN: &[(&str, &str)] = &[
    ("id1", "Quake"),
    ("ad", "Arcane Dimensions"),
    ("hipnotic", "Scourge of Armagon"),
    ("rogue", "Dissolution of Eternity"),
    ("mg1", "Dimension of the Machine"),
    ("mg3", "Dawn of the Machine"),
    ("ctf", "Capture the Flag"),
    ("dopa", "Dimension of the Past"),
    ("q64", "Quake 64"),
];

fn known_blurb(dir: &str) -> Vec<String> {
    let lines: &[&str] = match dir.to_ascii_lowercase().as_str() {
        "id1" => &["id Software (1996)", "The original four-episode Quake campaign."],
        "hipnotic" => &["Ritual Entertainment (1997)", "Mission Pack 1: Scourge of Armagon."],
        "rogue" => &["Rogue Entertainment (1997)", "Mission Pack 2: Dissolution of Eternity."],
        "dopa" => &["MachineGames (2016)", "20th anniversary episode with classic Quake callbacks."],
        "mg1" => &["MachineGames (2021)", "Five-realm expansion culminating in a rematch with Chthon."],
        "mg3" => &["MachineGames & id Software (2026)", "30th anniversary expansion for Quake."],
        _ => &[],
    };
    lines.iter().map(|s| (*s).to_string()).collect()
}

/// True for a directory with game data (paks, pk3s, progs or maps).
pub fn is_game_dir(path: &Path) -> bool {
    let Ok(rd) = qcore::vfs::read_dir(path) else { return false };
    rd.into_iter().any(|e| {
        let n = e.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().to_ascii_lowercase());
        let dir = e.is_dir;
        (!dir && (n.ends_with(".pak") || n.ends_with(".pk3") || n == "progs.dat" || n == "csprogs.dat")) || (dir && n == "maps")
    })
}

/// All mods under `basedir`: id1 first, then by title.
pub fn scan(basedir: &Path) -> Vec<ModInfo> {
    scan_roots(&[basedir])
}

/// All mods across `roots` (e.g. `basedir` and `<user data>/mods`): id1 first,
/// then by title. A dir present in more than one root is merged into one entry.
pub fn scan_roots(roots: &[&Path]) -> Vec<ModInfo> {
    let mut out: Vec<ModInfo> = Vec::new();
    let mut paths: Vec<Vec<std::path::PathBuf>> = Vec::new();
    for root in roots {
        for e in qcore::vfs::read_dir(root).into_iter().flatten().filter(|e| e.is_dir && is_game_dir(&e.path)) {
            let Some(d) = e.path.file_name().and_then(|n| n.to_str()) else { continue };
            if let Some(idx) = out.iter().position(|m| m.dir.eq_ignore_ascii_case(d)) {
                paths[idx].push(e.path);
            } else {
                out.push(info(&e.path, d));
                paths.push(vec![e.path]);
            }
        }
    }
    // a modinfo base that isn't installed would fail to mount: run on id1
    let dirs: Vec<String> = out.iter().map(|m| m.dir.to_ascii_lowercase()).collect();
    for m in &mut out {
        if let Base::Mod(d) = &m.base
            && !dirs.contains(&d.to_ascii_lowercase())
        {
            m.base = Base::Id;
        }
    }
    // map packs for another mod (no progs, no modinfo base)
    let found: Vec<Option<Base>> = out
        .iter()
        .zip(&paths)
        .map(|(m, ps)| {
            if m.base != Base::Id || ps.iter().any(|p| has_progs(p)) {
                return None;
            }
            let need = ps.iter().flat_map(|p| text_files(p)).find_map(|t| needed_mod(&t))?;
            needed_dir(&need, &out, &m.dir).map(|d| Base::from_name(&d).unwrap_or(Base::Mod(d)))
        })
        .collect();
    for (m, b) in out.iter_mut().zip(found) {
        if let Some(b) = b {
            m.base = b;
        }
    }
    out.sort_by_key(|m| (!m.dir.eq_ignore_ascii_case("id1"), m.title.to_ascii_lowercase()));
    out
}

/// True when the dir has its own game code: a progs.dat, or a pak that
/// holds one.
fn has_progs(path: &Path) -> bool {
    let mut fs = Fs::new(path.parent().unwrap_or(path));
    let Some(dir) = path.file_name().and_then(|d| d.to_str()) else { return false };
    fs.add_game_dir(dir) && ["progs.dat", "qwprogs.dat", "csprogs.dat"].iter().any(|f| fs.exists(f))
}

/// Every top-level `.txt` in the dir (map packs name their readme after
/// the map: `ad_heresp1.txt`), readmes first.
fn text_files(path: &Path) -> Vec<String> {
    let Ok(rd) = qcore::vfs::read_dir(path) else { return Vec::new() };
    let mut files: Vec<(bool, std::path::PathBuf)> = rd
        .into_iter()
        .filter(|e| !e.is_dir)
        .filter_map(|e| {
            let n = e.path.file_name()?.to_string_lossy().to_ascii_lowercase();
            n.ends_with(".txt").then(|| (!n.contains("readme"), e.path))
        })
        .collect();
    files.sort();
    files.iter().filter_map(|(_, p)| qcore::vfs::read(p).ok()).map(|d| String::from_utf8_lossy(&d).into_owned()).collect()
}

/// The value of a readme's "Mod Needed :" / "Requires :" line, if any.
fn needed_mod(readme: &str) -> Option<String> {
    readme.lines().find_map(|l| {
        let (k, v) = l.split_once(':')?;
        let k = k.trim().to_ascii_lowercase();
        let v = v.trim();
        let key = matches!(k.as_str(), "mod needed" | "mod required" | "requires" | "required mod" | "needs" | "base mod");
        (key && v.chars().any(|c| c.is_ascii_alphabetic()) && !v.eq_ignore_ascii_case("none")).then(|| v.to_string())
    })
}

/// The installed game dir a readme's needed mod names: by dir name or by
/// title, ignoring a version ("Arcane Dimension 1.7" -> `ad`, titled
/// "Arcane Dimensions").
fn needed_dir(need: &str, mods: &[ModInfo], own: &str) -> Option<String> {
    let norm = |s: &str| s.chars().filter(char::is_ascii_alphanumeric).collect::<String>().to_ascii_lowercase();
    let first = need.split(|c: char| c.is_ascii_digit() || c == '(' || c == ',').next().unwrap_or("");
    let want = norm(first);
    let word = norm(need.split_whitespace().next().unwrap_or(""));
    if want.len() < 2 {
        return None;
    }
    mods.iter()
        .filter(|m| !m.dir.eq_ignore_ascii_case(own) && !m.dir.eq_ignore_ascii_case("id1"))
        .find(|m| {
            let t = norm(&m.title);
            let d = norm(&m.dir);
            d == word || d == want || (want.len() >= 6 && t.len() >= 6 && (t.starts_with(&want) || want.starts_with(&t)))
        })
        .map(|m| m.dir.clone())
}

fn info(path: &Path, dir: &str) -> ModInfo {
    let mut m = ModInfo {
        dir: dir.to_string(),
        title: KNOWN.iter().find(|k| k.0.eq_ignore_ascii_case(dir)).map_or_else(|| dir.to_string(), |k| k.1.to_string()),
        base: Base::from_name(dir).unwrap_or(Base::Id),
        blurb: Vec::new(),
    };
    if let Ok(t) = qcore::vfs::read(&path.join("modinfo.txt")) {
        for line in String::from_utf8_lossy(&t).lines() {
            let line = line.trim();
            let (k, v) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
            let v = v.trim().trim_matches('"').trim();
            match k.to_ascii_lowercase().as_str() {
                "title" if !v.is_empty() => m.title = v.to_string(),
                "base" => m.base = Base::parse(v),
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
    if m.blurb.is_empty() {
        m.blurb = known_blurb(dir);
    }
    m
}

/// The mod's readme: a `.txt`/`.md` whose name says readme, else one named
/// after the mod, else none (changelogs and credits are no use as a blurb).
fn readme(path: &Path, dir: &str) -> Option<String> {
    let mut files: Vec<(u8, std::path::PathBuf)> = qcore::vfs::read_dir(path)
        .ok()?
        .into_iter()
        .filter_map(|e| {
            let n = e.path.file_name().map_or_else(String::new, |n| n.to_string_lossy().to_ascii_lowercase());
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
            Some((rank, e.path))
        })
        .collect();
    files.sort();
    let data = qcore::vfs::read(&files.first()?.1).ok()?;
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
    maps_roots(&[basedir], dir)
}

/// Playable maps shipped by `dir` across `roots` (e.g. `basedir` + user `mods/`).
pub fn maps_roots(roots: &[&Path], dir: &str) -> Vec<MapInfo> {
    let Some((&first, rest)) = roots.split_first() else { return Vec::new() };
    let mut fs = Fs::new(first);
    for r in rest {
        fs.add_mod_root(r);
    }
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
        assert_eq!(m("oxy", Base::Mod("ad".into())).launch_args(), ["-game", "ad", "-game", "oxy"]);
        assert_eq!(m("oxy", Base::Mod("ad".into())).game_dirs(), ["ad", "oxy"]);
        assert_eq!(Base::parse("AD"), Base::Mod("AD".into()));
        assert_eq!(Base::parse("quoth"), Base::Quoth);
        assert_eq!(Base::parse("../x"), Base::Id);
        assert_eq!(clean_message("The\\nSlipgate  Complex\n"), "The Slipgate Complex");
        assert_eq!(blurb("=====\n\n  Title: Foo  \n--\nby someone\n", 1), ["Title: Foo"]);
        assert!(entity_lump(&[0; 6]).is_none());
    }

    #[test]
    fn needed_mod_from_readme() {
        let r = "Title : X\nMod Needed\t\t: Arcane Dimension 1.7\nBase : -\n";
        assert_eq!(needed_mod(r).as_deref(), Some("Arcane Dimension 1.7"));
        assert_eq!(needed_mod("Base : -\nRequires : none\n"), None);
        let m = |d: &str, t: &str| ModInfo { dir: d.into(), title: t.into(), base: Base::Id, blurb: vec![] };
        let mods = [m("id1", "Quake"), m("ad", "Arcane Dimensions"), m("quoth", "quoth"), m("oxy", "oxy")];
        assert_eq!(needed_dir("Arcane Dimension 1.7", &mods, "oxy").as_deref(), Some("ad"));
        assert_eq!(needed_dir("Quoth 2.2", &mods, "oxy").as_deref(), Some("quoth"));
        assert_eq!(needed_dir("AD 1.80", &mods, "oxy").as_deref(), Some("ad"));
        assert_eq!(needed_dir("Quake", &mods, "oxy"), None);
        assert_eq!(needed_dir("Something else", &mods, "oxy"), None);
    }

    #[test]
    fn scan_roots_merges_and_resolves_cross_root_base() {
        let base = std::env::temp_dir().join(format!("qui-mods-roots-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let (b, u) = (base.join("base"), base.join("mods"));
        for d in [b.join("id1/maps"), b.join("ad/maps"), u.join("ad/maps"), u.join("pack/maps")] {
            std::fs::create_dir_all(d).unwrap();
        }
        std::fs::write(b.join("ad/progs.dat"), "p").unwrap();
        std::fs::write(u.join("pack/modinfo.txt"), "title \"User Pack\"\nbase ad\n").unwrap();
        let mods = scan_roots(&[&b, &u]);
        assert_eq!(mods.iter().filter(|m| m.dir == "ad").count(), 1, "merged into one entry");
        let pack = mods.iter().find(|m| m.dir == "pack").unwrap();
        assert_eq!(pack.title, "User Pack");
        assert_eq!(pack.base, Base::Mod("ad".into()));
        let _ = std::fs::remove_dir_all(&base);
    }

    #[test]
    fn scan_real_basedir() {
        let Some(b) = basedir() else { return };
        let mods = scan(&b);
        assert_eq!(mods[0].dir, "id1");
        assert_eq!(mods[0].title, "Quake");
        let hip = mods.iter().find(|m| m.dir == "hipnotic").unwrap();
        assert_eq!(hip.base, Base::Hipnotic);
        // an AD map pack without progs, found by its readme's "Mod Needed"
        if let Some(oxy) = mods.iter().find(|m| m.dir == "Oxyblack_Fortress_by_Heresy")
            && b.join("ad").is_dir()
        {
            assert_eq!(oxy.base, Base::Mod("ad".into()));
        }
        // mods with their own progs keep running on id1
        if let Some(ad) = mods.iter().find(|m| m.dir == "ad") {
            assert_eq!(ad.base, Base::Id);
        }
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
