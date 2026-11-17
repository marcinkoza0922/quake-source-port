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
    let sys = vec!["pl".to_string(), "de-AT".to_string(), "fr".to_string()];
    assert_eq!(resolve("auto", &sys), Lang::German);
    assert_eq!(resolve("Russian", &sys), Lang::Russian);
    assert_eq!(resolve("auto", &[]), Lang::English);
    assert_eq!(Lang::from_code("es_ES.UTF-8"), Some(Lang::Spanish));
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
