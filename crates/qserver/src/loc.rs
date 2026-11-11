//! Re-release localization (`localization/loc_english.txt`, LOC_LoadFile):
//! `$key = "value"` lines; `$key` strings are translated on print.

use std::collections::HashMap;

#[derive(Default)]
pub struct Localization {
    map: HashMap<String, String>,
}

fn unescape(v: &str) -> String {
    let v = v.trim();
    let (quoted, body) = match v.strip_prefix('"') {
        Some(b) => (true, b),
        None => (false, v),
    };
    let mut out = String::new();
    let mut it = body.chars();
    while let Some(c) = it.next() {
        match c {
            '\\' => match it.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(o) => out.push(o),
                None => {}
            },
            '"' if quoted => break,
            c => out.push(c),
        }
    }
    if !quoted {
        out.truncate(out.trim_end().len());
    }
    out
}

impl Localization {
    pub fn load(fs: &qcore::fs::Fs) -> Self {
        let mut l = Self::default();
        for f in ["localization/loc_english.txt", "localization/loc_english_mod.txt"] {
            let Some(d) = fs.open(f) else { continue };
            let text = String::from_utf8_lossy(&d);
            for line in text.lines() {
                let line = line.trim_start();
                if line.starts_with("//") {
                    continue;
                }
                if let Some((k, v)) = line.split_once('=') {
                    l.map.insert(k.trim().to_string(), unescape(v));
                }
            }
        }
        l
    }

    /// LOC_GetRawString: the value for `$key`, if any.
    pub fn get(&self, key: &[u8]) -> Option<&str> {
        let k = key.strip_prefix(b"$")?;
        self.map.get(std::str::from_utf8(k).ok()?).map(|s| s.as_str())
    }

    /// LOC_GetString: the translation of `$key`, else `s` unchanged.
    pub fn get_string<'a>(&'a self, s: &'a [u8]) -> &'a [u8] {
        self.get(s).map_or(s, |v| v.as_bytes())
    }

    /// Client-side LOC_GetString for whole print strings (a trailing
    /// newline is kept).
    pub fn translate(&self, s: &str) -> String {
        let body = s.trim_end_matches('\n');
        match self.get(body.as_bytes()) {
            Some(v) => format!("{v}{}", &s[body.len()..]),
            None => s.to_string(),
        }
    }

    /// LOC_HasPlaceholders.
    pub fn has_placeholders(&self, s: &[u8]) -> bool {
        !self.map.is_empty() && (0..s.len()).any(|i| parse_arg(&s[i..]).is_some())
    }
}

/// LOC_ParseArg: `{}` or `{N}` at the start of `s` -> (index, length).
fn parse_arg(s: &[u8]) -> Option<(usize, usize)> {
    if s.first() != Some(&b'{') {
        return None;
    }
    let mut i = 1;
    let mut n = 0usize;
    while let Some(c) = s.get(i).filter(|c| c.is_ascii_digit()) {
        n = n.saturating_mul(10).saturating_add((c - b'0') as usize);
        i += 1;
    }
    (s.get(i) == Some(&b'}')).then_some((n, i + 1))
}

/// LOC_Format: replaces `{}`/`{N}` in `format` with `arg(N)`.
pub fn format<'a>(format: &[u8], mut arg: impl FnMut(usize) -> &'a [u8]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < format.len() {
        match parse_arg(&format[i..]) {
            Some((n, len)) => {
                out.extend_from_slice(arg(n));
                i += len;
            }
            None => {
                out.push(format[i]);
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_args() {
        let mut l = Localization::default();
        l.map.insert("k".into(), "{0} got {1}{}\n".into());
        let f = l.get_string(b"$k");
        assert!(l.has_placeholders(f));
        let args: [&[u8]; 2] = [b"A", b"B"];
        assert_eq!(format(f, |i| args.get(i).copied().unwrap_or(b"")), b"A got BA\n");
        assert_eq!(l.translate("$k\n"), "{0} got {1}{}\n\n");
        assert_eq!(l.translate("plain"), "plain");
    }

    #[test]
    fn rerelease_two_args() {
        use std::path::{Path, PathBuf};
        let base = std::env::var_os("QUAKE_BASEDIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../vkQuake directory with maps"));
        if !base.join("id1").is_dir() {
            return;
        }
        let mut fs = qcore::fs::Fs::new(base);
        fs.add_game_dir("id1");
        let l = Localization::load(&fs);
        let f = l.get_string(b"$qc_death_ax");
        if !l.has_placeholders(f) {
            return; // not the 2021 re-release
        }
        let args: [&[u8]; 2] = [b"victim", b"killer"];
        let out = format(f, |i| args.get(i).copied().unwrap_or(b""));
        assert_eq!(out, b"victim was ax-murdered by killer\n");
    }
}
