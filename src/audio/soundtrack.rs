//! Soundtrack switcher: discovers music packs across installed Quake game
//! directories (id1, hipnotic, rogue, q64, mg1, mg3, mods) and custom folders
//! under `<user data>/music/<pack>/`, and serves CD track requests live.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use qcore::fs::Fs;
use qcore::vfs;
use qsound::{SoundSystem, music::MUSIC_EXTS};

/// Where a discovered soundtrack loads its audio files from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PackSource {
    /// A game directory under basedir or mod_roots (reads loose + PAK/PK3 `music/`).
    GameDir(String),
    /// A user folder under `<user data>/music/<name>/`.
    UserFolder(PathBuf),
}

/// One selectable soundtrack pack in Options → Audio.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundtrackPack {
    /// Stored in the `s_soundtrack` cvar (e.g. `"id1"`, `"q64"`, `"user:mytracks"`).
    pub id: String,
    /// Human-friendly display label.
    pub label: String,
    pub source: PackSource,
}

const KNOWN_PACKS: &[(&str, &str)] = &[
    ("id1", "Original (id1)"),
    ("hipnotic", "Scourge of Armagon"),
    ("rogue", "Dissolution of Eternity"),
    ("dopa", "Dimension of the Past"),
    ("mg1", "Dimension of the Machine"),
    ("mg3", "Call of the Machine"),
    ("q64", "Quake 64"),
    ("ad", "Arcane Dimensions"),
    ("quoth", "Quoth"),
];

fn known_rank(dir: &str) -> (usize, String) {
    let lower = dir.to_ascii_lowercase();
    let idx = KNOWN_PACKS
        .iter()
        .position(|(k, _)| *k == lower)
        .unwrap_or(KNOWN_PACKS.len());
    (idx, lower)
}

fn friendly_label(dir: &str) -> String {
    let lower = dir.to_ascii_lowercase();
    if let Some((_, name)) = KNOWN_PACKS.iter().find(|(k, _)| *k == lower) {
        return (*name).to_owned();
    }
    dir.to_owned()
}

fn music_ext(path: &str) -> Option<&'static str> {
    let (_, ext) = path.rsplit_once('.')?;
    let lower = ext.to_ascii_lowercase();
    MUSIC_EXTS.iter().copied().find(|&e| e == lower)
}

fn dir_has_music(fs_template: &Fs, gamedir: &str) -> bool {
    let mut probe = Fs::new(fs_template.basedir());
    for r in fs_template.mod_roots() {
        probe.add_mod_root(r);
    }
    probe.add_game_dir(gamedir);
    MUSIC_EXTS.iter().any(|ext| !probe.list("music/", ext).is_empty())
}

fn user_folder_tracks(dir: &Path) -> Vec<(PathBuf, &'static str)> {
    let Ok(entries) = vfs::read_dir(dir) else {
        return Vec::new();
    };
    let mut tracks = Vec::new();
    for e in entries {
        if e.is_dir {
            continue;
        }
        if let Some(name) = e.path.file_name().and_then(|n| n.to_str())
            && let Some(ext) = music_ext(name)
        {
            tracks.push((e.path, ext));
        }
    }
    tracks.sort_by(|a, b| a.0.cmp(&b.0));
    tracks
}

/// Discover all soundtrack packs from game dirs and `<user_root>/music/`.
pub fn discover(fs: &Fs, user_root: &Path) -> Vec<SoundtrackPack> {
    let mut packs = Vec::new();
    let mut seen_dirs = HashSet::new();

    // 1. Scan basedir and mod_roots for gamedirs containing music/.
    let mut roots = vec![fs.basedir().to_path_buf()];
    roots.extend(fs.mod_roots().iter().cloned());
    for root in roots {
        let Ok(entries) = vfs::read_dir(&root) else { continue };
        for e in entries {
            if !e.is_dir {
                continue;
            }
            let Some(name) = e.path.file_name().and_then(|n| n.to_str()) else { continue };
            if name.starts_with('.') {
                continue;
            }
            let key = name.to_ascii_lowercase();
            if !seen_dirs.insert(key) {
                continue;
            }
            if dir_has_music(fs, name) {
                packs.push(SoundtrackPack {
                    id: name.to_owned(),
                    label: friendly_label(name),
                    source: PackSource::GameDir(name.to_owned()),
                });
            }
        }
    }
    packs.sort_by_key(|p| known_rank(&p.id));

    // 2. Scan `<user_root>/music/<pack>/` for custom user soundtrack folders.
    let user_music = user_root.join("music");
    if let Ok(entries) = vfs::read_dir(&user_music) {
        let mut custom = Vec::new();
        for e in entries {
            if !e.is_dir {
                continue;
            }
            let Some(name) = e.path.file_name().and_then(|n| n.to_str()) else { continue };
            if name.starts_with('.') || user_folder_tracks(&e.path).is_empty() {
                continue;
            }
            let collides = packs.iter().any(|p| p.id.eq_ignore_ascii_case(name));
            let id = if collides { format!("user:{name}") } else { name.to_owned() };
            let label = if collides { format!("{name} (custom)") } else { name.to_owned() };
            custom.push(SoundtrackPack {
                id,
                label,
                source: PackSource::UserFolder(e.path),
            });
        }
        custom.sort_by_key(|a| a.label.to_ascii_lowercase());
        packs.extend(custom);
    }

    packs
}

/// `cd` console command state (vkQuake `cd_sdl.c` / `CD_f`).
pub struct CdState {
    /// `music_loop 0`.
    pub music_once: bool,
    pub enabled: bool,
    pub paused: bool,
    pub playing: Option<(u8, bool)>,
    pub remap: [u8; 100],
}

impl Default for CdState {
    fn default() -> Self {
        Self::new()
    }
}

impl CdState {
    pub fn new() -> Self {
        Self { music_once: false, enabled: true, paused: false, playing: None, remap: std::array::from_fn(|i| i as u8) }
    }

    pub fn map(&self, track: u8) -> u8 {
        self.remap.get(track as usize).copied().unwrap_or(track)
    }

    pub fn reset_remap(&mut self) {
        self.remap = std::array::from_fn(|i| i as u8);
    }
}

#[derive(Default)]
pub struct SoundtrackManager {
    packs: Vec<SoundtrackPack>,
    /// Last requested CD track and loop flag, so switching soundtracks updates live.
    current_track: Option<(u8, bool)>,
}

impl SoundtrackManager {
    pub fn refresh(&mut self, fs: &Fs, user_root: &Path) {
        self.packs = discover(fs, user_root);
    }

    pub fn packs(&self) -> &[SoundtrackPack] {
        &self.packs
    }

    /// Menu entries: `(id, label)` with empty id = Default (Current Mod).
    pub fn menu_entries(&self) -> Vec<(String, String)> {
        self.packs.iter().map(|p| (p.id.clone(), p.label.clone())).collect()
    }

    pub fn clear_track(&mut self) {
        self.current_track = None;
    }

    /// Play CD track `track` using `soundtrack_id` (empty = active mod's default).
    pub fn play_cd_track(
        &mut self,
        track: u8,
        looping: bool,
        soundtrack_id: &str,
        fs: &Fs,
        snd: &mut SoundSystem,
    ) -> bool {
        self.current_track = Some((track, looping));
        let id = soundtrack_id.trim();
        if !id.is_empty()
            && let Some(pack) = self.packs.iter().find(|p| p.id.eq_ignore_ascii_case(id))
            && play_from_pack(pack, track, looping, fs, snd)
        {
            return true;
        }
        // BGM_PlayCDtrack: the highest search path wins, then the order of types
        let mut best: Option<(usize, String)> = None;
        for ext in MUSIC_EXTS {
            let name = format!("music/track{track:02}.{ext}");
            if let Some(p) = fs.priority(&name)
                && best.as_ref().is_none_or(|b| p > b.0)
            {
                best = Some((p, name));
            }
        }
        match best {
            Some((_, name)) => (looping && snd.resume_music(&name)) || snd.play_music(&name, looping),
            None => {
                snd.stop_music();
                false
            }
        }
    }

    /// Re-play the current CD track when the user changes `s_soundtrack` live.
    pub fn replay_current(&mut self, soundtrack_id: &str, fs: &Fs, snd: &mut SoundSystem) {
        if let Some((track, looping)) = self.current_track {
            self.play_cd_track(track, looping, soundtrack_id, fs, snd);
        }
    }
}

fn play_from_pack(
    pack: &SoundtrackPack,
    track: u8,
    looping: bool,
    fs: &Fs,
    snd: &SoundSystem,
) -> bool {
    match &pack.source {
        PackSource::GameDir(dir) => {
            let mut pfs = Fs::new(fs.basedir());
            for r in fs.mod_roots() {
                pfs.add_mod_root(r);
            }
            pfs.add_game_dir(dir);
            // Try exact `music/trackNN.<ext>` first.
            for ext in MUSIC_EXTS {
                let path = format!("music/track{track:02}.{ext}");
                let key = format!("{}:{path}", pack.id);
                if (looping && snd.resume_music(&key))
                    || pfs.open(&path).is_some_and(|b| snd.play_music_bytes(&key, b, ext, looping))
                {
                    return true;
                }
            }
            // Otherwise map deterministically across whatever tracks the pack ships.
            let mut all: Vec<(String, &'static str)> = Vec::new();
            for ext in MUSIC_EXTS {
                for p in pfs.list("music/", ext) {
                    all.push((p, ext));
                }
            }
            all.sort_by(|a, b| a.0.cmp(&b.0));
            if all.is_empty() {
                return false;
            }
            let idx = (track.saturating_sub(2) as usize) % all.len();
            let (ref path, ext) = all[idx];
            let key = format!("{}:{path}", pack.id);
            (looping && snd.resume_music(&key)) || pfs.open(path).is_some_and(|b| snd.play_music_bytes(&key, b, ext, looping))
        }
        PackSource::UserFolder(dir) => {
            // Try exact `trackNN.<ext>` first.
            for ext in MUSIC_EXTS {
                let p = dir.join(format!("track{track:02}.{ext}"));
                let key = p.to_string_lossy();
                if (looping && snd.resume_music(&key))
                    || vfs::read(&p).is_ok_and(|b| snd.play_music_bytes(&key, b, ext, looping))
                {
                    return true;
                }
            }
            // Otherwise map across all audio files in the folder sorted by name.
            let all = user_folder_tracks(dir);
            if all.is_empty() {
                return false;
            }
            let idx = (track.saturating_sub(2) as usize) % all.len();
            let (ref path, ext) = all[idx];
            let key = path.to_string_lossy();
            (looping && snd.resume_music(&key)) || vfs::read(path).is_ok_and(|b| snd.play_music_bytes(&key, b, ext, looping))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Half a second of a WAV, silent but for its length.
    fn wav() -> Vec<u8> {
        let mut b = b"RIFF".to_vec();
        b.extend(44136u32.to_le_bytes());
        b.extend(b"WAVEfmt ");
        b.extend(16u32.to_le_bytes());
        for v in [1u16, 1] {
            b.extend(v.to_le_bytes());
        }
        for v in [44100u32, 88200] {
            b.extend(v.to_le_bytes());
        }
        for v in [2u16, 16] {
            b.extend(v.to_le_bytes());
        }
        b.extend(b"data");
        b.extend(44100u32.to_le_bytes());
        b.resize(44 + 44100, 1);
        b
    }

    #[test]
    fn a_mods_track_beats_the_base_games_of_another_type() {
        let tmp = std::env::temp_dir().join(format!("qsp_st_prio_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        for d in ["id1/music", "mod/music"] {
            std::fs::create_dir_all(tmp.join(d)).unwrap();
        }
        // ogg comes before wav, but id1 after the mod
        std::fs::write(tmp.join("id1/music/track02.ogg"), b"not an ogg").unwrap();
        std::fs::write(tmp.join("mod/music/track02.wav"), wav()).unwrap();
        let mut fs = Fs::new(&tmp);
        fs.add_game_dir("id1");
        fs.add_game_dir("mod");
        let mut sfs = Fs::new(&tmp);
        sfs.add_game_dir("id1");
        sfs.add_game_dir("mod");
        let (mut snd, mut mixer) = SoundSystem::new(48000, move |p| sfs.open(p));
        let mut st = SoundtrackManager::default();
        let mut out = vec![0i16; 256];
        assert!(st.play_cd_track(2, true, "", &fs, &mut snd));
        mixer.paint(&mut out);
        assert!(mixer.music_playing());
        assert!(snd.resume_music("music/track02.wav"));
        // no such track: silence, not the last one
        assert!(!st.play_cd_track(9, true, "", &fs, &mut snd));
        mixer.paint(&mut out);
        assert!(!mixer.music_playing());
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn discovers_gamedir_and_user_soundtracks() {
        let tmp = std::env::temp_dir().join(format!("qsp_st_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        let base = tmp.join("base");
        let user = tmp.join("user");
        std::fs::create_dir_all(base.join("id1/music")).unwrap();
        std::fs::write(base.join("id1/music/track02.ogg"), b"fake").unwrap();
        std::fs::create_dir_all(base.join("q64/music")).unwrap();
        std::fs::write(base.join("q64/music/track02.ogg"), b"fake").unwrap();
        std::fs::create_dir_all(user.join("music/Synthwave")).unwrap();
        std::fs::write(user.join("music/Synthwave/01_intro.mp3"), b"fake").unwrap();

        let fs = Fs::new(&base);
        let packs = discover(&fs, &user);
        let ids: Vec<&str> = packs.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, vec!["id1", "q64", "Synthwave"]);
        assert_eq!(packs[0].label, "Original (id1)");
        assert_eq!(packs[1].label, "Quake 64");
        assert_eq!(packs[2].label, "Synthwave");

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
