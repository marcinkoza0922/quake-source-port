//! Quake text uses byte indices into conchars, not UTF-8. In string-based
//! parsers/APIs carry each byte as the same-valued Unicode scalar (Latin-1).
//! Encode it back before handing it to the VM, network, font or save file.

pub fn decode(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| char::from(b)).collect()
}

pub fn glyphs(text: &str) -> impl Iterator<Item = u8> + '_ {
    text.chars().map(|c| u8::try_from(c as u32).unwrap_or(b'?'))
}

pub fn encode(text: &str) -> Vec<u8> {
    glyphs(text).collect()
}

/// One drawable unit of Quake text.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Glyph {
    /// A conchars index (ASCII, the gold set at +128, symbols below 32).
    Quake(u8),
    /// A character decoded from UTF-8 bytes in the text.
    Char(char),
}

/// Splits Quake text bytes into glyphs. With `utf8`, a run of bytes >= 0x80
/// that is valid UTF-8 as a whole (translated text, see
/// `lang::quake_encode`) becomes [`Glyph::Char`]s; other bytes stay conchars
/// indices. Judging whole runs keeps gold (+128) text intact: gold letters
/// next to gold digits or spaces can form valid pairs, but a gold word
/// rarely parses from start to end.
pub fn glyphs_utf8(bytes: &[u8], utf8: bool) -> impl Iterator<Item = Glyph> + '_ {
    let mut i = 0;
    let mut valid_until = 0;
    std::iter::from_fn(move || {
        let b = *bytes.get(i)?;
        if utf8 && b >= 0x80 {
            if i >= valid_until && (i == 0 || bytes[i - 1] < 0x80) {
                let end = i + bytes[i..].iter().take_while(|&&x| x >= 0x80).count();
                if std::str::from_utf8(&bytes[i..end]).is_ok() {
                    valid_until = end;
                }
            }
            if i < valid_until {
                let n = match b {
                    0xc0..=0xdf => 2,
                    0xe0..=0xef => 3,
                    _ => 4,
                };
                let c = std::str::from_utf8(&bytes[i..i + n]).ok().and_then(|s| s.chars().next()).unwrap_or('?');
                i += n;
                return Some(Glyph::Char(c));
            }
        }
        i += 1;
        Some(Glyph::Quake(b))
    })
}

/// Quake text `s` with its UTF-8 runs decoded ([`glyphs_utf8`]), for
/// char-based consumers: conchars bytes >= 0x80 become U+E080..U+E0FF (the
/// private use area, as FTE does) so they stay apart from real characters.
/// None when nothing decodes.
pub fn unicode_view(s: &str) -> Option<String> {
    if !s.chars().all(|c| (c as u32) < 256) {
        return None;
    }
    let bytes = encode(s);
    let glyphs: Vec<Glyph> = glyphs_utf8(&bytes, true).collect();
    if !glyphs.iter().any(|g| matches!(g, Glyph::Char(_))) {
        return None;
    }
    Some(
        glyphs
            .into_iter()
            .map(|g| match g {
                Glyph::Char(c) => c,
                Glyph::Quake(b) if b < 128 => b as char,
                Glyph::Quake(b) => char::from_u32(0xe000 + b as u32).unwrap_or('?'),
            })
            .collect(),
    )
}

/// The conchars byte of a [`unicode_view`] private-use character.
pub fn pua_byte(c: char) -> Option<u8> {
    let u = c as u32;
    (0xe080..0xe100).contains(&u).then(|| (u - 0xe000) as u8)
}

#[cfg(test)]
mod tests {
    #[test]
    fn utf8_sequences_decode_only_when_enabled() {
        use super::Glyph::*;
        let b = "é".as_bytes();
        let v: Vec<_> = super::glyphs_utf8(&[b'a', b[0], b[1], b' ', 0xc1, 0x80], true).collect();
        assert_eq!(v, [Quake(b'a'), Char('é'), Quake(b' '), Quake(0xc1), Quake(0x80)]);
        assert_eq!(super::glyphs_utf8(b, false).count(), 2);
        // a gold word whose first two bytes form a valid pair stays gold
        let gold: Vec<u8> = b"FitzQuake 0.85".iter().map(|b| b | 128).collect();
        assert!(super::glyphs_utf8(&gold, true).all(|g| matches!(g, Quake(_))));
        let ru = "Найдена броня";
        assert_eq!(super::glyphs_utf8(ru.as_bytes(), true).count(), ru.chars().count());
        let view = super::unicode_view(&super::decode(&[b'x', 0xe9, b' ', b[0], b[1]])).unwrap();
        assert_eq!(view, "x\u{e0e9} é");
    }

    #[test]
    fn all_font_indices_roundtrip_even_when_the_bytes_form_valid_utf8() {
        let bytes: Vec<u8> = (0..=255).collect();
        assert_eq!(super::encode(&super::decode(&bytes)), bytes);
        assert_eq!(super::decode(&[0xc3, 0xa9]).chars().count(), 2);
    }
}
