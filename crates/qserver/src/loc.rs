//! Re-release localization (`localization/loc_<language>.txt`, LOC_LoadFile):
//! `key = "value"` lines; `$key` strings are translated on print.
//!
//! The chosen language ([`qcore::lang::current`]) is read over English, so
//! keys it lacks stay English; each `loc_*_mod.txt` (mod additions) follows
//! its base file. English values are folded to ASCII for the 8×8 conchars
//! (as ironwail does); other languages keep their UTF-8 bytes, which the
//! client's text paths decode (`qcore::text::glyphs_utf8`).

use std::collections::HashMap;

use qcore::lang::Lang;

#[derive(Default)]
pub struct Localization {
    map: HashMap<String, Vec<u8>>,
    /// Language the strings were loaded for.
    pub lang: Option<Lang>,
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
    /// Loads the current language's strings over English.
    pub fn load(fs: &qcore::fs::Fs) -> Self {
        Self::load_lang(fs, qcore::lang::current())
    }

    pub fn load_lang(fs: &qcore::fs::Fs, lang: Lang) -> Self {
        let mut l = Self { lang: Some(lang), ..Self::default() };
        let mut langs = vec![Lang::English];
        if lang != Lang::English {
            langs.push(lang);
        }
        for lg in langs {
            for f in [format!("localization/loc_{}.txt", lg.name()), format!("localization/loc_{}_mod.txt", lg.name())] {
                let Some(d) = fs.open(&f) else { continue };
                l.read(&String::from_utf8_lossy(&d), lg);
            }
        }
        l
    }

    fn read(&mut self, text: &str, lang: Lang) {
        for line in text.trim_start_matches('\u{feff}').lines() {
            let line = line.trim_start();
            if line.starts_with("//") {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                let v = unescape(v);
                let bytes = if lang == Lang::English { qcore::lang::to_ascii(&v).into_bytes() } else { v.into_bytes() };
                self.map.insert(k.trim().to_string(), bytes);
            }
        }
    }

    /// LOC_GetRawString: the value for `$key`, if any.
    pub fn get(&self, key: &[u8]) -> Option<&[u8]> {
        let k = key.strip_prefix(b"$")?;
        self.map.get(std::str::from_utf8(k).ok()?).map(|s| s.as_slice())
    }

    /// LOC_GetString: the translation of `$key`, else `s` unchanged.
    pub fn get_string<'a>(&'a self, s: &'a [u8]) -> &'a [u8] {
        self.get(s).unwrap_or(s)
    }

    /// Client-side LOC_GetString for whole print strings (a trailing
    /// newline is kept). `s` and the result are Quake text ([`qcore::text`]).
    pub fn translate(&self, s: &str) -> String {
        let body = s.trim_end_matches('\n');
        match self.get(&qcore::text::encode(body)) {
            Some(v) => format!("{}{}", qcore::text::decode(v), &s[body.len()..]),
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
        l.map.insert("k".into(), b"{0} got {1}{}\n".to_vec());
        let f = l.get_string(b"$k");
        assert!(l.has_placeholders(f));
        let args: [&[u8]; 2] = [b"A", b"B"];
        assert_eq!(format(f, |i| args.get(i).copied().unwrap_or(b"")), b"A got BA\n");
        assert_eq!(l.translate("$k\n"), "{0} got {1}{}\n\n");
        l.map.insert("u".into(), "été".as_bytes().to_vec());
        assert_eq!(qcore::text::encode(&l.translate("$u")), "été".as_bytes());
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
        assert_eq!(l.get(b"$qc_horde_streak_6"), Some(&b"UBER KILL!\n"[..]));
        let fr = Localization::load_lang(&fs, qcore::lang::Lang::French);
        let f = fr.get_string(b"$qc_death_ax");
        assert!(std::str::from_utf8(f).is_ok() && f != b"$qc_death_ax");
    }
}
