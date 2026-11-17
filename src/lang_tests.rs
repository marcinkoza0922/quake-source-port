//! Catalog completeness: every translatable text of the port has an entry in
//! `crates/qcore/lang/*.txt` for each language, and no entry is stale.
//!
//! Texts are found in three ways: the options pages ([`crate::menu::lang_keys`]),
//! the markers `t!`/`tr!`/`qtr!`/`ctr!`/`tr(`, and string literals on lines
//! that draw modern or 8×8 text (`.text(`, `txt(`, `gold(`, `.ptext(`, hint
//! arrays, ...), which [`crate::draw::Canvas`] translates as it draws them.
//! A failure lists the missing keys in catalog format.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use qcore::lang::{Lang, catalog};

const MARKERS: &[&str] = &["t!(", "tr!(", "qtr!(", "ctr!(", "lang::tr(", " tr(", "(tr(", "tr_ctx("];
const DRAW_CALLS: &[&str] = &[
    ".text(", ".text_right(", ".text_center(", "txt(", "txt_gold(", "gold(", "label(c, ", ".ptext(", ".ptext_w(",
    ".ptext_gold(", "center_gold(", "center(c, ", "(K_", "hint(", "title(c, ", "heading(",
];

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if p.is_dir() {
            if name != "tests" {
                rust_files(&p, out);
            }
        } else if name.ends_with(".rs") && name != "tests.rs" && !name.ends_with("_tests.rs") && name != "lang_keys.rs" {
            out.push(p);
        }
    }
}

/// Parses the Rust string literal starting at `s[0] == '"'`; returns the
/// value and the byte length consumed.
fn literal(s: &str) -> Option<(String, usize)> {
    let mut out = String::new();
    let mut it = s.char_indices().skip(1);
    while let Some((i, c)) = it.next() {
        match c {
            '"' => return Some((out, i + 1)),
            '\\' => match it.next()?.1 {
                'n' => out.push('\n'),
                't' => out.push('\t'),
                '\n' => {
                    // line continuation: skip leading whitespace
                    let rest = &s[i + 2..];
                    let skip = rest.len() - rest.trim_start().len();
                    for _ in 0..rest[..skip].chars().count() {
                        it.next();
                    }
                }
                'u' => {
                    let rest = &s[i + 2..];
                    let end = rest.find('}')?;
                    out.push(char::from_u32(u32::from_str_radix(&rest[1..end], 16).ok()?)?);
                    for _ in 0..=end {
                        it.next();
                    }
                }
                o => out.push(o),
            },
            c => out.push(c),
        }
    }
    None
}

fn normalize(k: &str) -> String {
    k.trim_end_matches('\n').to_string()
}

/// Numbers, percentages and sizes ("100%", "16x") read the same everywhere.
fn needs_translation(k: &str) -> bool {
    let words: String = k.chars().filter(|c| c.is_alphabetic()).collect();
    !words.is_empty() && words != "x"
}

/// Draw-line literals that are data, not words: paths, cvars, format strings.
fn is_text(s: &str) -> bool {
    s.chars().any(|c| c.is_ascii_alphabetic())
        && !s.contains('{')
        && !(s.contains('_') && !s.contains(' '))
        && !(s.contains('/') && !s.contains(' ') && s.len() > 1)
        && !s.ends_with(".lmp")
        && !s.ends_with(".wav")
}

fn scan(text: &str, file: &str, keys: &mut BTreeMap<String, String>) {
    let body = text.split("#[cfg(test)]\nmod tests").next().unwrap_or(text);
    let mut add = |k: String, line: usize| {
        if needs_translation(&k) {
            keys.entry(normalize(&k)).or_insert_with(|| format!("{file}:{line}"));
        }
    };
    // markers (the literal may follow on the next line)
    for m in MARKERS {
        let mut from = 0;
        while let Some(i) = body[from..].find(m) {
            let start = from + i;
            let at = start + m.len();
            from = at;
            // word boundary: `t!(` must not be the end of `format!(`
            let prev = body[..start].chars().last();
            if m.starts_with(|c: char| c.is_ascii_alphabetic()) && prev.is_some_and(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let rest = body[at..].trim_start();
            if rest.starts_with('"')
                && let Some((v, _)) = literal(rest)
            {
                let line = body[..at].lines().count();
                if !body[..at].lines().last().is_some_and(|l| l.trim_start().starts_with("//")) {
                    add(v, line);
                }
            }
        }
    }
    // literals on drawing lines
    for (n, line) in body.lines().enumerate() {
        let t = line.trim_start();
        if t.starts_with("//") || !DRAW_CALLS.iter().any(|d| line.contains(d)) {
            continue;
        }
        let mut rest = line;
        while let Some(i) = rest.find('"') {
            let Some((v, len)) = literal(&rest[i..]) else { break };
            // skip `b"..."` byte strings and `'"'` chars
            let prev = rest[..i].chars().last();
            if prev != Some('b') && prev != Some('\'') && is_text(&v) {
                add(v, n + 1);
            }
            rest = &rest[i + len..];
        }
    }
}

fn collect() -> BTreeMap<String, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    rust_files(&root.join("src"), &mut files);
    if let Ok(rd) = std::fs::read_dir(root.join("crates")) {
        for e in rd.flatten() {
            rust_files(&e.path().join("src"), &mut files);
        }
    }
    files.sort();
    let mut keys = BTreeMap::new();
    for f in files {
        let rel = f.strip_prefix(root).unwrap_or(&f).display().to_string();
        if rel.contains("qcore/src/lang") {
            continue;
        }
        scan(&std::fs::read_to_string(&f).unwrap_or_default(), &rel, &mut keys);
    }
    for k in crate::menu::lang_keys::menu_keys().into_iter().filter(|k| needs_translation(k)) {
        keys.entry(normalize(k)).or_insert_with(|| "menu".into());
    }
    keys
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('\n', "\\n")
}

#[test]
fn every_text_is_translated() {
    let keys = collect();
    let (entries, problems) = catalog::all();
    assert!(problems.is_empty(), "{problems:?}");
    let have: BTreeMap<&str, &catalog::Entry> = entries.iter().map(|(_, e)| (e.key.as_str(), e)).collect();
    let mut missing = String::new();
    let mut n = 0;
    for (k, at) in &keys {
        let langs: Vec<Lang> = Lang::ALL[1..]
            .iter()
            .copied()
            .filter(|l| !have.get(k.as_str()).is_some_and(|e| e.texts.iter().any(|(x, _)| x == l)))
            .collect();
        if !langs.is_empty() {
            n += 1;
            let codes: Vec<&str> = langs.iter().map(|l| l.code()).collect();
            missing += &format!("# {at} needs {}\n@ {}\n", codes.join(" "), escape(k));
        }
    }
    let used: BTreeSet<&str> = keys.keys().map(String::as_str).collect();
    let stale: Vec<String> =
        entries.iter().filter(|(_, e)| !used.contains(e.key.as_str())).map(|(f, e)| format!("{f}:{} {:?}", e.line, e.key)).collect();
    if let Ok(path) = std::env::var("QSP_LANG_KEYS") {
        let all: Vec<String> = keys.keys().map(|k| escape(k)).collect();
        std::fs::write(path, all.join("\n")).expect("write key list");
    }
    if let Ok(path) = std::env::var("QSP_LANG_MISSING") {
        std::fs::write(path, &missing).expect("write missing list");
    }
    assert!(n == 0 && stale.is_empty(), "{n} untranslated texts:\n{missing}\nstale catalog entries:\n{}", stale.join("\n"));
}
