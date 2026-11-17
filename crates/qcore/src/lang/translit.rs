//! UTF-8 to plain ASCII for the 8×8 conchars (ironwail's `UTF8_ToQuake`
//! idea, reduced to the letters and punctuation the catalogs and the
//! re-release's English text use).

/// `s` with accents removed, typographic punctuation simplified and
/// Cyrillic romanised; anything else unknown becomes `?`.
pub fn to_ascii(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii() {
            out.push(c);
        } else {
            out.push_str(fold(c));
        }
    }
    out
}

fn fold(c: char) -> &'static str {
    match c {
        'À'..='Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'à'..='å' | 'ā' | 'ă' | 'ą' | 'ª' => "a",
        'Æ' => "AE",
        'æ' => "ae",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ð' | 'Ď' | 'Đ' => "D",
        'ð' | 'ď' | 'đ' => "d",
        'È'..='Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'è'..='ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'Ì'..='Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'ì'..='ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ł' | 'Ĺ' | 'Ļ' | 'Ľ' => "L",
        'ł' | 'ĺ' | 'ļ' | 'ľ' => "l",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'Ò'..='Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "O",
        'ò'..='ö' | 'ø' | 'ō' | 'ŏ' | 'ő' | 'º' => "o",
        'Œ' => "OE",
        'œ' => "oe",
        'Ŕ' | 'Ř' => "R",
        'ŕ' | 'ř' => "r",
        'Ś' | 'Ş' | 'Š' => "S",
        'ś' | 'ş' | 'š' => "s",
        'ß' => "ss",
        'Ţ' | 'Ť' => "T",
        'ţ' | 'ť' => "t",
        'Ù'..='Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ù'..='ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ý' | 'Ÿ' => "Y",
        'ý' | 'ÿ' => "y",
        'Ź' | 'Ż' | 'Ž' => "Z",
        'ź' | 'ż' | 'ž' => "z",
        'Þ' => "Th",
        'þ' => "th",
        '‘' | '’' | '‚' | '′' | '´' => "'",
        '“' | '”' | '„' | '″' | '«' | '»' => "\"",
        '‹' => "<",
        '›' => ">",
        '–' | '—' | '−' | '‐' => "-",
        '…' => "...",
        '•' | '·' => "*",
        '×' => "x",
        '°' => "o",
        '™' => "(TM)",
        '©' => "(C)",
        '®' => "(R)",
        '\u{a0}' | '\u{2009}' | '\u{202f}' => " ",
        '¡' => "!",
        '¿' => "?",
        c if ('\u{400}'..='\u{4ff}').contains(&c) => cyrillic(c),
        _ => "?",
    }
}

fn cyrillic(c: char) -> &'static str {
    const UPPER: [&str; 32] = [
        "A", "B", "V", "G", "D", "E", "Zh", "Z", "I", "Y", "K", "L", "M", "N", "O", "P", "R", "S", "T", "U", "F", "Kh",
        "Ts", "Ch", "Sh", "Shch", "", "Y", "", "E", "Yu", "Ya",
    ];
    const LOWER: [&str; 32] = [
        "a", "b", "v", "g", "d", "e", "zh", "z", "i", "y", "k", "l", "m", "n", "o", "p", "r", "s", "t", "u", "f", "kh",
        "ts", "ch", "sh", "shch", "", "y", "", "e", "yu", "ya",
    ];
    match c {
        'А'..='Я' => UPPER[c as usize - 'А' as usize],
        'а'..='я' => LOWER[c as usize - 'а' as usize],
        'Ё' => "Yo",
        'ё' => "yo",
        _ => "?",
    }
}
