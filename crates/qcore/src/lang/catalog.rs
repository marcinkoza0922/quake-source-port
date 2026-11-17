//! Translation catalogs, built into the binary from `crates/qcore/lang/*.txt`.
//!
//! Each entry is the English text on an `@ ` line followed by one line per
//! language, prefixed with its code:
//!
//! ```text
//! # comment
//! @ Show FPS
//! fr Afficher les IPS
//! de FPS anzeigen
//! ```
//!
//! `\n` and `\\` are escapes on both sides; text runs to the end of the line.
//! `{}`/`{N}` are arguments ([`super::format`]) and may be reordered.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::Lang;

/// The catalog files, in the order their entries are read.
pub const FILES: &[(&str, &str)] = &[
    ("menu.txt", include_str!("../../lang/menu.txt")),
    ("tooltips.txt", include_str!("../../lang/tooltips.txt")),
    ("ui.txt", include_str!("../../lang/ui.txt")),
    ("hud.txt", include_str!("../../lang/hud.txt")),
    ("console.txt", include_str!("../../lang/console.txt")),
];

/// One parsed entry: English key and (language, text) pairs.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Entry {
    pub key: String,
    pub line: usize,
    pub texts: Vec<(Lang, String)>,
}

/// A parse problem (file, line, message).
pub type Problem = (String, usize, String);

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some(o) => out.push(o),
            None => out.push('\\'),
        }
    }
    out
}

/// Parses one catalog file.
pub fn parse(file: &str, text: &str) -> (Vec<Entry>, Vec<Problem>) {
    let mut entries: Vec<Entry> = Vec::new();
    let mut problems = Vec::new();
    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r');
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(key) = line.strip_prefix("@ ") {
            entries.push(Entry { key: unescape(key), line: n + 1, texts: Vec::new() });
            continue;
        }
        let (code, rest) = line.split_once(' ').unwrap_or((line, ""));
        let Some(lang) = Lang::from_code(code).filter(|l| *l != Lang::English && l.code() == code) else {
            problems.push((file.to_string(), n + 1, format!("unknown line {line:?}")));
            continue;
        };
        let Some(e) = entries.last_mut() else {
            problems.push((file.to_string(), n + 1, "translation before the first @ key".into()));
            continue;
        };
        if e.texts.iter().any(|(l, _)| *l == lang) {
            problems.push((file.to_string(), n + 1, format!("second {code} line for {:?}", e.key)));
            continue;
        }
        e.texts.push((lang, unescape(rest)));
    }
    (entries, problems)
}

/// Every entry of every built-in file, with parse problems.
pub fn all() -> (Vec<(&'static str, Entry)>, Vec<Problem>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    for (name, text) in FILES {
        let (e, p) = parse(name, text);
        out.extend(e.into_iter().map(|e| (*name, e)));
        problems.extend(p);
    }
    (out, problems)
}

type Table = HashMap<String, String>;

fn tables() -> &'static [Table; 6] {
    static T: OnceLock<[Table; 6]> = OnceLock::new();
    T.get_or_init(|| {
        let mut t: [Table; 6] = Default::default();
        for (_, e) in all().0 {
            for (l, s) in e.texts {
                t[l as usize].entry(e.key.clone()).or_insert(s);
            }
        }
        t
    })
}

/// The `l` text for English `key`.
pub fn lookup(l: Lang, key: &str) -> Option<&'static str> {
    tables()[l as usize].get(key).map(String::as_str)
}
