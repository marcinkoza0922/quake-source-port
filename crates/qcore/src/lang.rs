//! Engine text translation (the port's own menus, HUD and messages).
//!
//! The 2021 re-release's game text (`$qc_*` keys) lives in its own
//! `localization/loc_<lang>.txt` files and is handled by `qserver::loc`.
//! This module covers what the port adds: catalogs keyed by the English
//! source text (see `crates/qcore/lang/*.txt` and [`catalog`]), the current
//! language, and `{}`/`{N}` formatting in the re-release's style.
//!
//! - [`tr`]: translated UI text (Unicode), for the TTF menus.
//! - [`tr!`](crate::tr): the same with `{}` arguments.
//! - [`qtr!`](crate::qtr): Quake-encoded (see [`crate::text`]) text for the
//!   centerprint, notify and HUD paths; non-ASCII arrives there as UTF-8 bytes.
//! - [`ctr!`](crate::ctr): console messages; English unless `con_translate`.
//! - [`t!`](crate::t): no-op marker for literals that are translated where
//!   they are drawn, so the catalog test finds them.

pub mod catalog;
mod translit;
pub mod wrap;

use std::borrow::Cow;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub use translit::to_ascii;
pub use wrap::wrap;

/// The supported languages: the 2021 re-release's six first, then the
/// port's own translations (`crates/qcore/lang/languages/<code>.txt`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    English,
    French,
    German,
    Italian,
    Spanish,
    Russian,
    Polish,
    Ukrainian,
    Czech,
    Slovak,
    Croatian,
    Bulgarian,
    Swedish,
    Finnish,
    Danish,
    Norwegian,
    Dutch,
    Greek,
    Hungarian,
    Romanian,
    Portuguese,
    Brazilian,
    Japanese,
    Korean,
    SChinese,
    TChinese,
    Turkish,
    Indonesian,
    Malay,
    Filipino,
    Vietnamese,
    Serbian,
    Lithuanian,
    Latvian,
    Estonian,
    Maltese,
    Albanian,
    Armenian,
    Georgian,
    Icelandic,
    Irish,
    Welsh,
    Basque,
    Catalan,
    Galician,
    Latin,
    Esperanto,
    Belarusian,
    Macedonian,
    Slovenian,
    Swahili,
}

/// (language, `language` cvar name, catalog code, own name).
const INFO: [(Lang, &str, &str, &str); 51] = [
    (Lang::English, "english", "en", "English"),
    (Lang::French, "french", "fr", "Français"),
    (Lang::German, "german", "de", "Deutsch"),
    (Lang::Italian, "italian", "it", "Italiano"),
    (Lang::Spanish, "spanish", "es", "Español"),
    (Lang::Russian, "russian", "ru", "Русский"),
    (Lang::Polish, "polish", "pl", "Polski"),
    (Lang::Ukrainian, "ukrainian", "uk", "Українська"),
    (Lang::Czech, "czech", "cs", "Čeština"),
    (Lang::Slovak, "slovak", "sk", "Slovenčina"),
    (Lang::Croatian, "croatian", "hr", "Hrvatski"),
    (Lang::Bulgarian, "bulgarian", "bg", "Български"),
    (Lang::Swedish, "swedish", "sv", "Svenska"),
    (Lang::Finnish, "finnish", "fi", "Suomi"),
    (Lang::Danish, "danish", "da", "Dansk"),
    (Lang::Norwegian, "norwegian", "nb", "Norsk"),
    (Lang::Dutch, "dutch", "nl", "Nederlands"),
    (Lang::Greek, "greek", "el", "Ελληνικά"),
    (Lang::Hungarian, "hungarian", "hu", "Magyar"),
    (Lang::Romanian, "romanian", "ro", "Română"),
    (Lang::Portuguese, "portuguese", "pt", "Português"),
    (Lang::Brazilian, "brazilian", "pt-BR", "Português (Brasil)"),
    (Lang::Japanese, "japanese", "ja", "日本語"),
    (Lang::Korean, "korean", "ko", "한국어"),
    (Lang::SChinese, "schinese", "zh-CN", "简体中文"),
    (Lang::TChinese, "tchinese", "zh-TW", "繁體中文"),
    (Lang::Turkish, "turkish", "tr", "Türkçe"),
    (Lang::Indonesian, "indonesian", "id", "Bahasa Indonesia"),
    (Lang::Malay, "malay", "ms", "Bahasa Melayu"),
    (Lang::Filipino, "filipino", "fil", "Filipino"),
    (Lang::Vietnamese, "vietnamese", "vi", "Tiếng Việt"),
    (Lang::Serbian, "serbian", "sr", "Српски"),
    (Lang::Lithuanian, "lithuanian", "lt", "Lietuvių"),
    (Lang::Latvian, "latvian", "lv", "Latviešu"),
    (Lang::Estonian, "estonian", "et", "Eesti"),
    (Lang::Maltese, "maltese", "mt", "Malti"),
    (Lang::Albanian, "albanian", "sq", "Shqip"),
    (Lang::Armenian, "armenian", "hy", "Հայերեն"),
    (Lang::Georgian, "georgian", "ka", "ქართული"),
    (Lang::Icelandic, "icelandic", "is", "Íslenska"),
    (Lang::Irish, "irish", "ga", "Gaeilge"),
    (Lang::Welsh, "welsh", "cy", "Cymraeg"),
    (Lang::Basque, "basque", "eu", "Euskara"),
    (Lang::Catalan, "catalan", "ca", "Català"),
    (Lang::Galician, "galician", "gl", "Galego"),
    (Lang::Latin, "latin", "la", "Latina"),
    (Lang::Esperanto, "esperanto", "eo", "Esperanto"),
    (Lang::Belarusian, "belarusian", "be", "Беларуская"),
    (Lang::Macedonian, "macedonian", "mk", "Македонски"),
    (Lang::Slovenian, "slovenian", "sl", "Slovenščina"),
    (Lang::Swahili, "swahili", "sw", "Kiswahili"),
];

impl Lang {
    pub const ALL: [Lang; INFO.len()] = {
        let mut a = [Lang::English; INFO.len()];
        let mut i = 0;
        while i < a.len() {
            a[i] = INFO[i].0;
            i += 1;
        }
        a
    };

    /// The 2021 re-release's languages, whose catalogs live in the shared
    /// `crates/qcore/lang/*.txt` files.
    pub const RERELEASE: [Lang; 6] = [Lang::English, Lang::French, Lang::German, Lang::Italian, Lang::Spanish, Lang::Russian];

    /// `language` cvar value and `loc_<name>.txt` suffix.
    pub fn name(self) -> &'static str {
        INFO[self as usize].1
    }

    /// Catalog line prefix and manual suffix: ISO 639-1 (`fil`: 639-2), with
    /// a region where one language has two variants (`pt-BR`, `zh-CN`, `zh-TW`).
    pub fn code(self) -> &'static str {
        INFO[self as usize].2
    }

    /// The language's own name, for the menu.
    pub fn native(self) -> &'static str {
        INFO[self as usize].3
    }

    pub fn from_name(s: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.name().eq_ignore_ascii_case(s.trim()))
    }

    /// A catalog code or a system locale (`pt_BR.UTF-8`, `zh-Hant-TW`,
    /// `nb-NO`, `de-AT`).
    pub fn from_code(s: &str) -> Option<Lang> {
        let s = s.trim().split(['.', '@']).next().unwrap_or("").to_ascii_lowercase().replace('_', "-");
        let mut parts = s.split('-');
        let base = parts.next().unwrap_or("");
        let rest: Vec<&str> = parts.collect();
        let has = |x: &str| rest.contains(&x);
        Some(match base {
            "pt" if has("br") => Lang::Brazilian,
            "zh" if has("hant") || has("tw") || has("hk") || has("mo") => Lang::TChinese,
            "zh" => Lang::SChinese,
            "no" | "nn" => Lang::Norwegian,
            "tl" => Lang::Filipino,
            "in" => Lang::Indonesian,
            _ => return Lang::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(base)),
        })
    }

    fn index(self) -> u8 {
        self as u8
    }
}

/// Resolves the `language` cvar: `auto` (or anything unknown) takes the
/// first known language from the system's preferred list, else English.
pub fn resolve(cvar: &str, system: &[String]) -> Lang {
    Lang::from_name(cvar).unwrap_or_else(|| system.iter().find_map(|s| Lang::from_code(s)).unwrap_or(Lang::English))
}

static CURRENT: AtomicU8 = AtomicU8::new(0);
static CONSOLE: AtomicBool = AtomicBool::new(false);

pub fn current() -> Lang {
    Lang::ALL[CURRENT.load(Ordering::Relaxed) as usize % Lang::ALL.len()]
}

/// Sets the language; returns true when it changed.
pub fn set(l: Lang) -> bool {
    CURRENT.swap(l.index(), Ordering::Relaxed) != l.index()
}

/// `con_translate`: whether console messages are translated.
pub fn set_console(on: bool) {
    CONSOLE.store(on, Ordering::Relaxed);
}

pub fn console() -> bool {
    CONSOLE.load(Ordering::Relaxed)
}

/// True when text may hold non-ASCII (UTF-8) characters, which the 8×8
/// conchars can't draw: 8×8 text then switches to the TTF font.
pub fn unicode_text() -> bool {
    current() != Lang::English
}

/// The translation of `s` (exact English text; trailing newlines are kept
/// outside the lookup), or `s` itself.
pub fn tr(s: &str) -> Cow<'_, str> {
    tr_in(current(), s)
}

pub fn tr_in(l: Lang, s: &str) -> Cow<'_, str> {
    if l == Lang::English || s.is_empty() {
        return Cow::Borrowed(s);
    }
    let body = s.trim_end_matches('\n');
    match catalog::lookup(l, body) {
        Some(t) if body.len() == s.len() => Cow::Borrowed(t),
        Some(t) => Cow::Owned(format!("{t}{}", &s[body.len()..])),
        None => Cow::Borrowed(s),
    }
}

/// For English words with several meanings ("Save" the button, "Save" the
/// kind of save): English shows `english`, other languages translate the
/// unambiguous catalog `key` (falling back to `english`).
pub fn tr_ctx<'a>(key: &'static str, english: &'a str) -> Cow<'a, str> {
    let l = current();
    if l == Lang::English {
        return Cow::Borrowed(english);
    }
    match catalog::lookup(l, key) {
        Some(t) => Cow::Borrowed(t),
        None => tr_in(l, english),
    }
}

/// Replaces `{}` (next argument) and `{N}` (argument N) in `template`;
/// `{{` and `}}` are literal braces. Missing arguments become empty.
pub fn format(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut next = 0;
    let mut rest = template;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        if let Some(end) = tail.strip_prefix('{').and_then(|t| t.find('}'))
            && tail[1..1 + end].bytes().all(|b| b.is_ascii_digit())
        {
            let digits = &tail[1..1 + end];
            let n = if digits.is_empty() {
                next += 1;
                next - 1
            } else {
                digits.parse().unwrap_or(usize::MAX)
            };
            if let Some(a) = args.get(n) {
                use std::fmt::Write;
                let _ = write!(out, "{a}");
            }
            rest = &tail[end + 2..];
            continue;
        }
        out.push_str(&tail[..1]);
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

/// Unicode text in the Quake text encoding: ASCII stays, anything else is
/// carried as its UTF-8 bytes (one char per byte, see [`crate::text`]), which
/// the 8×8 text paths decode again when [`unicode_text`] is on.
pub fn quake_encode(s: &str) -> Cow<'_, str> {
    if s.is_ascii() { Cow::Borrowed(s) } else { Cow::Owned(crate::text::decode(s.as_bytes())) }
}

/// Translated + formatted UI text: `tr!("Saved {}", name)`.
#[macro_export]
macro_rules! tr {
    ($s:literal) => { $crate::lang::tr($s).into_owned() };
    ($s:literal, $($a:expr),+ $(,)?) => { $crate::lang::format(&$crate::lang::tr($s), &[$(&$a as &dyn ::std::fmt::Display),+]) };
}

/// [`tr!`] for the Quake-text paths (centerprint, notify, 8×8 HUD).
#[macro_export]
macro_rules! qtr {
    ($($t:tt)+) => { $crate::lang::quake_encode(&$crate::tr!($($t)+)).into_owned() };
}

/// Console message: translated only with `con_translate`, Quake-encoded.
#[macro_export]
macro_rules! ctr {
    ($s:literal) => {
        if $crate::lang::console() { $crate::qtr!($s) } else { ::std::string::String::from($s) }
    };
    ($s:literal, $($a:expr),+ $(,)?) => {
        if $crate::lang::console() {
            $crate::qtr!($s, $($a),+)
        } else {
            $crate::lang::format($s, &[$(&$a as &dyn ::std::fmt::Display),+])
        }
    };
}

/// Marks a literal that is translated where it is drawn.
#[macro_export]
macro_rules! t {
    ($s:literal) => {
        $s
    };
}

#[cfg(test)]
mod tests;
