//! Entity lump parser.

use crate::{Error, Result};

/// One entity: ordered key/value pairs (duplicates preserved).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entity {
    pub pairs: Vec<(String, String)>,
}

impl Entity {
    /// First value for `key`.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.pairs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
    pub fn classname(&self) -> Option<&str> {
        self.get("classname")
    }
}

/// Quake COM_Parse-style tokenizer: quoted strings, single-char `{ } ( ) ' :`, `//` comments.
struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
}

impl Lexer<'_> {
    fn next(&mut self) -> Option<String> {
        loop {
            while self.i < self.s.len() && self.s[self.i] <= b' ' {
                self.i += 1;
            }
            if self.s[self.i..].starts_with(b"//") {
                while self.i < self.s.len() && self.s[self.i] != b'\n' {
                    self.i += 1;
                }
                continue;
            }
            if self.s[self.i..].starts_with(b"/*") {
                match self.s[self.i + 2..].windows(2).position(|w| w == b"*/") {
                    Some(p) => self.i += p + 4,
                    None => self.i = self.s.len(),
                }
                continue;
            }
            break;
        }
        let c = *self.s.get(self.i)?;
        if c == b'"' {
            self.i += 1;
            let start = self.i;
            while self.i < self.s.len() && self.s[self.i] != b'"' {
                self.i += 1;
            }
            let t = String::from_utf8_lossy(&self.s[start..self.i]).into_owned();
            self.i = (self.i + 1).min(self.s.len());
            return Some(t);
        }
        if b"{}()':".contains(&c) {
            self.i += 1;
            return Some((c as char).to_string());
        }
        let start = self.i;
        while self.i < self.s.len()
            && self.s[self.i] > b' '
            && !b"{}()':\"".contains(&self.s[self.i])
        {
            self.i += 1;
        }
        Some(String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
    }
}

/// Parse an entity lump string into entities. Raw entity files must first be
/// decoded with `qcore::text::decode` to preserve 8-bit Quake strings.
pub fn parse_entities(text: &str) -> Result<Vec<Entity>> {
    let mut lx = Lexer {
        s: text.as_bytes(),
        i: 0,
    };
    let mut out = Vec::new();
    let err = |m: &str, lx: &Lexer| Error::new("entities", format!("{m} (at offset {})", lx.i));
    while let Some(tok) = lx.next() {
        if tok != "{" {
            return Err(err(&format!("expected '{{', got {tok:?}"), &lx));
        }
        let mut e = Entity::default();
        loop {
            let key = lx
                .next()
                .ok_or_else(|| err("EOF without closing brace", &lx))?;
            if key == "}" {
                break;
            }
            let val = lx
                .next()
                .ok_or_else(|| err("EOF without closing brace", &lx))?;
            if val == "}" {
                return Err(err("closing brace without data", &lx));
            }
            e.pairs.push((key, val));
        }
        out.push(e);
    }
    Ok(out)
}
