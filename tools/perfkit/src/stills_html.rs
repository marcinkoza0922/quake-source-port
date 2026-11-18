//! The side-by-side gallery `perfkit stills` writes as `index.html`: per
//! view, each engine's thumbnail (linked to the full PNG), ours' difference
//! images against both references (brightened ×4), and the DSSIM numbers.

use perfkit::machine::Machine;
use perfkit::stills::View;
use std::fmt::Write as _;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

pub fn gallery<'a>(m: &Machine, rows: impl Iterator<Item = (&'a View, [Option<f64>; 3], &'static str)>) -> String {
    let mut h = String::from(
        "<!doctype html><html><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\">\
<title>Parity stills</title><style>\
:root{--bg:#fafafa;--fg:#1c1c1c;--muted:#666;--line:#ddd;--off:#c62828;--ok:#2e7d32}\
@media (prefers-color-scheme:dark){:root{--bg:#161616;--fg:#e8e8e8;--muted:#9a9a9a;--line:#333;--off:#ef5350;--ok:#66bb6a}}\
body{background:var(--bg);color:var(--fg);font:14px/1.4 system-ui,sans-serif;margin:16px}\
h2{margin:28px 0 4px;font-size:16px}.m{color:var(--muted)}\
.g{display:grid;grid-template-columns:repeat(auto-fill,minmax(300px,1fr));gap:8px}\
figure{margin:0}img{width:100%;display:block;border:1px solid var(--line)}figcaption{color:var(--muted);font-size:12px}\
.OFF{color:var(--off);font-weight:600}.ok{color:var(--ok)}</style></head><body><h1>Parity stills</h1>",
    );
    let _ = write!(h, "<p class=\"m\">{}</p>", esc(&m.summary()));
    let f = |x: Option<f64>| x.map_or("–".to_string(), |x| format!("{x:.4}"));
    for (v, d, verdict) in rows {
        let _ = write!(
            h,
            "<h2>{} <span class=\"{verdict}\">{verdict}</span></h2><p class=\"m\">{} {} · {} · DSSIM ours↔ironwail {} · ours↔vkQuake {} · \
             ironwail↔vkQuake {}</p><div class=\"g\">",
            esc(v.name), v.game, v.map, esc(v.what), f(d[0]), f(d[1]), f(d[2])
        );
        for (file, label) in [
            ("ours", "ours"), ("ironwail", "ironwail"), ("vkquake", "vkQuake"),
            ("diff-ironwail", "|ours − ironwail| ×4"), ("diff-vkquake", "|ours − vkQuake| ×4"),
        ] {
            let full = if file.starts_with("diff") { format!("{file}.jpg") } else { format!("{file}.png") };
            let _ = write!(
                h,
                "<figure><a href=\"{n}/{full}\"><img loading=\"lazy\" src=\"{n}/{file}.jpg\" alt=\"{label}\"></a><figcaption>{label}</figcaption></figure>",
                n = esc(v.name)
            );
        }
        h.push_str("</div>");
    }
    h.push_str("</body></html>\n");
    h
}
