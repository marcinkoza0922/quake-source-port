//! Text parsers: configs, particle scripts, catalogs, JSON. First byte picks one.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    let Some((&k, d)) = data.split_first() else { return };
    let s = String::from_utf8_lossy(d);
    match k {
        b'0' => {
            let mut t: &str = &s;
            while let Some((_, rest)) = qcore::cmd::com_parse(t) {
                t = rest;
            }
            let _ = qcore::cmd::Args::tokenize(&s);
            let mut b = qcore::cmd::Cbuf::new();
            b.add_text(&s);
            while b.next_line().is_some() {}
        }
        b'1' | b'2' => {
            let name = if k == b'1' { "x" } else { "effectinfo" };
            let mut files = |p: &str| (p == "particles/x.cfg" || p == "effectinfo.txt").then(|| d.to_vec());
            let mut r = qpart::Registry::new();
            r.load_set(name, false, true, &mut files);
        }
        b'3' => drop(qui::json::parse(&s)),
        b'4' => drop(qui::stats::Stats::parse(&s)),
        b'5' => drop(qcore::lang::catalog::parse("f", &s)),
        b'6' => drop(qcore::text::decode(d)),
        b'7' => drop(qserver::weaponswitch::parse_priority(&s)),
        _ => {}
    }
});
