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

use std::borrow::Cow;
use std::fmt::Display;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub use translit::to_ascii;

/// The re-release's languages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Lang {
    English,
    French,
    German,
    Italian,
    Spanish,
    Russian,
}

impl Lang {
    pub const ALL: [Lang; 6] = [Lang::English, Lang::French, Lang::German, Lang::Italian, Lang::Spanish, Lang::Russian];

    /// `language` cvar value and `loc_<name>.txt` suffix.
    pub fn name(self) -> &'static str {
        match self {
            Lang::English => "english",
            Lang::French => "french",
            Lang::German => "german",
            Lang::Italian => "italian",
            Lang::Spanish => "spanish",
            Lang::Russian => "russian",
        }
    }

    /// ISO 639-1 code (catalog line prefix, system locale match).
    pub fn code(self) -> &'static str {
        match self {
            Lang::English => "en",
            Lang::French => "fr",
            Lang::German => "de",
            Lang::Italian => "it",
            Lang::Spanish => "es",
            Lang::Russian => "ru",
        }
    }

    /// The language's own name, for the menu.
    pub fn native(self) -> &'static str {
        match self {
            Lang::English => "English",
            Lang::French => "Français",
            Lang::German => "Deutsch",
            Lang::Italian => "Italiano",
            Lang::Spanish => "Español",
            Lang::Russian => "Русский",
        }
    }

    pub fn from_name(s: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.name().eq_ignore_ascii_case(s.trim()))
    }

    pub fn from_code(s: &str) -> Option<Lang> {
        let s = s.trim();
        let base = s.split(['-', '_', '.', '@']).next().unwrap_or(s);
        Lang::ALL.into_iter().find(|l| l.code().eq_ignore_ascii_case(base))
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
