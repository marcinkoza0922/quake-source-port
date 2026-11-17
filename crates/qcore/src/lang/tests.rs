use super::*;

#[test]
fn format_positional_and_sequential() {
    assert_eq!(format("{} got {}", &[&"A", &2]), "A got 2");
    assert_eq!(format("{1} by {0}", &[&"x", &"y"]), "y by x");
    assert_eq!(format("{{lit}} {} {9}", &[&1]), "{lit} 1 ");
    assert_eq!(format("open { brace", &[]), "open { brace");
}

#[test]
fn resolve_auto_and_names() {
    let sys = vec!["xx".to_string(), "de-AT".to_string(), "fr".to_string()];
    assert_eq!(resolve("auto", &sys), Lang::German);
    assert_eq!(resolve("auto", &["pl_PL.UTF-8".to_string()]), Lang::Polish);
    assert_eq!(resolve("Russian", &sys), Lang::Russian);
    assert_eq!(resolve("auto", &[]), Lang::English);
    assert_eq!(Lang::from_code("es_ES.UTF-8"), Some(Lang::Spanish));
}

#[test]
fn locale_codes_with_regions() {
    for (s, l) in [
        ("pt_BR.UTF-8", Lang::Brazilian),
        ("pt-PT", Lang::Portuguese),
        ("pt", Lang::Portuguese),
        ("zh-Hant-TW", Lang::TChinese),
        ("zh_HK", Lang::TChinese),
        ("zh-Hans", Lang::SChinese),
        ("zh_CN", Lang::SChinese),
        ("nn-NO", Lang::Norwegian),
        ("no", Lang::Norwegian),
        ("nb", Lang::Norwegian),
        ("tl-PH", Lang::Filipino),
        ("fil", Lang::Filipino),
        ("ja_JP", Lang::Japanese),
        ("th_TH.UTF-8", Lang::Thai),
        ("iw-IL", Lang::Hebrew),
        ("he", Lang::Hebrew),
        ("fa-IR", Lang::Persian),
        ("ur_PK", Lang::Urdu),
        ("pa-IN", Lang::Punjabi),
        ("km-KH", Lang::Khmer),
        ("am_ET", Lang::Amharic),
        ("yue-Hant-HK", Lang::Cantonese),
        ("zh-yue", Lang::Cantonese),
    ] {
        assert_eq!(Lang::from_code(s), Some(l), "{s}");
    }
    assert_eq!(Lang::from_code("lo"), None);
    assert_eq!(Lang::from_name("Farsi"), Some(Lang::Persian));
    let rtl: Vec<Lang> = Lang::ALL.into_iter().filter(|l| l.rtl()).collect();
    assert_eq!(rtl, [Lang::Arabic, Lang::Hebrew, Lang::Persian, Lang::Urdu]);
}

#[test]
fn every_language_round_trips() {
    for (i, l) in Lang::ALL.into_iter().enumerate() {
        assert_eq!(l as usize, i, "{l:?} out of order");
        assert_eq!(Lang::from_name(l.name()), Some(l));
        assert_eq!(Lang::from_code(l.code()), Some(l), "{}", l.code());
    }
    for (l, ..) in catalog::LANG_FILES {
        assert!(!Lang::RERELEASE.contains(l));
    }
    assert_eq!(catalog::LANG_FILES.len() + Lang::RERELEASE.len(), Lang::ALL.len());
}

#[test]
fn parse_entries_and_problems() {
    let (e, p) = catalog::parse("t", "# c\n@ Show FPS\\n\nfr Afficher\nde Anzeigen\nzz bad\nfr again\n");
    assert_eq!(e.len(), 1);
    assert_eq!(e[0].key, "Show FPS\n");
    assert_eq!(e[0].texts, vec![(Lang::French, "Afficher".into()), (Lang::German, "Anzeigen".into())]);
    assert_eq!(p.len(), 2);
}

#[test]
fn builtin_catalogs_parse_cleanly() {
    let (entries, problems) = catalog::all();
    assert!(problems.is_empty(), "{problems:?}");
    let mut seen = std::collections::HashSet::new();
    for (file, e) in &entries {
        assert!(seen.insert(e.key.as_str()), "{file}:{} duplicate key {:?}", e.line, e.key);
        for (l, s) in &e.texts {
            let args = |s: &str| {
                let mut v: Vec<String> = Vec::new();
                let mut rest = s;
                while let Some(i) = rest.find('{') {
                    let t = &rest[i..];
                    let end = t.find('}').map_or(t.len(), |e| e + 1);
                    v.push(t[..end].to_string());
                    rest = &t[end..];
                }
                v.sort();
                v
            };
            assert_eq!(args(&e.key), args(s), "{file}:{} {:?} {} has other arguments", e.line, e.key, l.code());
            assert_eq!(e.key.ends_with('\n'), s.ends_with('\n'), "{file}:{} {:?} {} newline", e.line, e.key, l.code());
        }
    }
}

#[test]
fn tr_keeps_trailing_newlines_and_misses() {
    set(Lang::French);
    let (entries, _) = catalog::all();
    if let Some((_, e)) = entries.iter().find(|(_, e)| !e.key.ends_with('\n') && e.texts.iter().any(|(l, _)| *l == Lang::French)) {
        let fr = &e.texts.iter().find(|(l, _)| *l == Lang::French).unwrap().1;
        assert_eq!(tr(&format!("{}\n", e.key)), format!("{fr}\n"));
    }
    assert_eq!(tr("no such text 123"), "no such text 123");
    set(Lang::English);
    assert_eq!(tr("Gameplay"), "Gameplay");
}

#[test]
fn quake_encoding_roundtrips_utf8() {
    let q = quake_encode("Jouabilité");
    assert_eq!(crate::text::encode(&q), "Jouabilité".as_bytes());
    assert_eq!(quake_encode("plain"), "plain");
}

#[test]
fn ascii_folding() {
    assert_eq!(to_ascii("ÜBER KILL!"), "UBER KILL!");
    assert_eq!(to_ascii("QUAKE™ l’été"), "QUAKE(TM) l'ete");
    assert_eq!(to_ascii("Привет"), "Privet");
}

/// Thai and Khmer have no spaces between words: lines can only break where
/// the text marks word boundaries with U+200B (see `wrap`).
#[test]
fn thai_and_khmer_mark_word_breaks() {
    for (_, e) in catalog::all().0 {
        for (l, s) in &e.texts {
            let letters = match l {
                Lang::Thai => 0x0E00..=0x0E7F,
                Lang::Khmer => 0x1780..=0x17FF,
                _ => continue,
            };
            let mut run = 0;
            for c in s.chars() {
                run = if letters.contains(&(c as u32)) { run + 1 } else { 0 };
                assert!(run <= 18, "{} text for {:?} needs U+200B word breaks: {s:?}", l.code(), e.key);
            }
        }
    }
}
