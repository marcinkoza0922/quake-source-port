//! Help screen (main menu → Help): the user manual, `docs/MANUAL.md`,
//! compiled into the binary and shown as a section list beside a scrolling,
//! word-wrapped text pane.

use std::cell::Cell;

use super::*;

/// The manual per language, in [`qcore::lang::Lang::ALL`] order.
const MANUALS: [&str; 6] = [
    include_str!("../../docs/MANUAL.md"),
    include_str!("../../docs/MANUAL.fr.md"),
    include_str!("../../docs/MANUAL.de.md"),
    include_str!("../../docs/MANUAL.it.md"),
    include_str!("../../docs/MANUAL.es.md"),
    include_str!("../../docs/MANUAL.ru.md"),
];

/// Section list (left) and text pane (right), in 640x360 menu units.
const LIST_X: f32 = 24.0;
const LIST_W: f32 = 150.0;
const TEXT_X: f32 = 186.0;
const TEXT_W: f32 = 430.0;
const TOP: f32 = 48.0;
const BOTTOM: f32 = 320.0;
const LINE_H: f32 = 11.0;
const PAD: f32 = 8.0;

/// One displayable block of a section.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Block {
    Heading(String),
    /// Paragraph text; list items carry their bullet ("-" or "1.").
    Para { text: String, bullet: Option<String> },
    Gap,
}

#[derive(Clone, Debug)]
pub(super) struct Section {
    pub title: String,
    pub blocks: Vec<Block>,
}

/// Help screen state: selected section and first visible text line.
#[derive(Default)]
pub(super) struct Help {
    pub section: usize,
    pub scroll: Cell<usize>,
    /// Wrapped line count of the shown section (set while drawing).
    lines: Cell<usize>,
}

/// Inline markdown to plain text: drops emphasis and code markers and
/// turns `[text](link)` into `text`.
fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(i) = rest.find('[') {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        match (tail.find("]("), tail.find(')')) {
            (Some(a), Some(b)) if a < b => {
                out.push_str(&tail[1..a]);
                rest = &tail[b + 1..];
            }
            _ => {
                out.push('[');
                rest = &tail[1..];
            }
        }
    }
    out.push_str(rest);
    out.replace("**", "").replace('`', "")
}

fn cells(line: &str) -> Vec<String> {
    line.trim().trim_matches('|').split('|').map(|c| plain(c.trim())).collect()
}

/// Splits the manual into `##` sections (the text before the first one is
/// "About"). `###` become headings; tables become one paragraph per row.
/// The manual follows the current language; every translation has the same
/// sections, so the selected one stays valid when the language changes.
pub(super) fn sections() -> &'static [Section] {
    static S: [std::sync::OnceLock<Vec<Section>>; 6] = [const { std::sync::OnceLock::new() }; 6];
    let i = qcore::lang::Lang::ALL.iter().position(|&l| l == qcore::lang::current()).unwrap_or(0);
    S[i].get_or_init(|| parse(MANUALS[i], qcore::lang::Lang::ALL[i]))
}

fn parse(manual: &str, lang: qcore::lang::Lang) -> Vec<Section> {
    let mut out = vec![Section { title: qcore::lang::tr_in(lang, qcore::t!("About")).into_owned(), blocks: Vec::new() }];
    let mut para: Option<(String, Option<String>)> = None;
    let mut header: Option<Vec<String>> = None;
    fn flush(sec: &mut Section, para: &mut Option<(String, Option<String>)>) {
        if let Some((text, bullet)) = para.take() {
            sec.blocks.push(Block::Para { text, bullet });
        }
    }
    for line in manual.lines() {
        let t = line.trim();
        let sec = out.last_mut().unwrap();
        if t.starts_with("# ") {
            continue;
        }
        if let Some(h) = t.strip_prefix("## ") {
            flush(sec, &mut para);
            out.push(Section { title: plain(h), blocks: Vec::new() });
            continue;
        }
        if let Some(h) = t.strip_prefix("### ") {
            flush(sec, &mut para);
            sec.blocks.push(Block::Heading(plain(h)));
            continue;
        }
        if t.starts_with('|') {
            flush(sec, &mut para);
            let row = cells(t);
            if row.iter().all(|c| c.chars().all(|ch| ch == '-' || ch == ':')) {
                continue;
            }
            match &header {
                None => header = Some(row),
                Some(h) => {
                    let mut text = row[0].clone();
                    for (k, v) in h.iter().zip(&row).skip(1) {
                        if v != "—" && !v.is_empty() {
                            text.push_str(&format!(". {k}: {v}"));
                        }
                    }
                    sec.blocks.push(Block::Para { text, bullet: Some("-".into()) });
                }
            }
            continue;
        }
        header = None;
        if t.is_empty() {
            flush(sec, &mut para);
            if !matches!(sec.blocks.last(), None | Some(Block::Gap)) {
                sec.blocks.push(Block::Gap);
            }
            continue;
        }
        let item = t.strip_prefix("- ").map(|r| ("-".to_string(), r)).or_else(|| {
            let (n, r) = t.split_once(". ")?;
            n.chars().all(|c| c.is_ascii_digit()).then(|| (format!("{n}."), r))
        });
        if let Some((b, rest)) = item {
            flush(sec, &mut para);
            para = Some((plain(rest), Some(b)));
        } else if let Some((text, _)) = &mut para {
            text.push(' ');
            text.push_str(&plain(t));
        } else {
            para = Some((plain(t), None));
        }
    }
    let last = out.last_mut().unwrap();
    flush(last, &mut para);
    out.retain(|s| !s.blocks.is_empty());
    out
}

/// A wrapped line: x offset, text, heading colour.
fn layout(c: &Canvas, sec: &Section) -> Vec<(f32, String, bool)> {
    let mut out = Vec::new();
    let width = TEXT_W - 2.0 * PAD;
    for b in &sec.blocks {
        match b {
            Block::Gap => out.push((0.0, String::new(), false)),
            Block::Heading(h) => out.push((0.0, h.clone(), true)),
            Block::Para { text, bullet } => {
                let indent = bullet.as_ref().map_or(0.0, |b| c.text_w(b, TXT) + 5.0);
                let mut first = true;
                for l in wrap_lines(c, text, width - indent) {
                    let s = match (first, bullet) {
                        (true, Some(b)) => format!("{b} {l}"),
                        _ => l,
                    };
                    out.push((if first { 0.0 } else { indent }, s, false));
                    first = false;
                }
            }
        }
    }
    while out.last().is_some_and(|l| l.1.is_empty()) {
        out.pop();
    }
    out
}

fn wrap_lines(c: &Canvas, s: &str, max_w: f32) -> Vec<String> {
    let mut lines = vec![String::new()];
    for word in s.split_whitespace() {
        let cur = lines.last_mut().unwrap();
        let cand = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
        if !cur.is_empty() && c.text_w(&cand, TXT) > max_w {
            lines.push(word.to_string());
        } else {
            *cur = cand;
        }
    }
    lines
}

fn visible_lines() -> usize {
    ((BOTTOM - TOP - 2.0 * PAD) / LINE_H) as usize
}

impl Help {
    fn max_scroll(&self) -> usize {
        self.lines.get().saturating_sub(visible_lines())
    }

    fn scroll_by(&self, d: isize) {
        let s = (self.scroll.get() as isize + d).clamp(0, self.max_scroll() as isize);
        self.scroll.set(s as usize);
    }

    fn select(&mut self, i: usize) {
        if i != self.section {
            self.section = i;
            self.scroll.set(0);
        }
    }

    pub fn open_section(&mut self, prefix: &str) {
        let p = prefix.to_ascii_lowercase();
        if let Some(i) = sections().iter().position(|s| !p.is_empty() && s.title.to_ascii_lowercase().starts_with(&p)) {
            self.select(i);
        }
    }

    /// Section row under the pointer.
    pub fn item_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let n = sections().len();
        ((LIST_X..LIST_X + LIST_W).contains(&x) && (TOP + 4.0..TOP + 4.0 + n as f32 * ROW_H).contains(&y))
            .then(|| ((y - TOP - 4.0) / ROW_H) as usize)
    }

    /// Returns true when the screen should close (back).
    pub fn key(&mut self, key: i32, mouse: Option<(f32, f32)>, out: &mut Vec<Action>) -> bool {
        let n = sections().len();
        let over_text = mouse.is_some_and(|(x, _)| x >= TEXT_X);
        match key {
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => return true,
            K_MWHEELUP | K_MWHEELDOWN if over_text => self.scroll_by(if key == K_MWHEELUP { -3 } else { 3 }),
            K_UPARROW | K_DPAD_UP | K_MWHEELUP => {
                self.select((self.section + n - 1) % n);
                out.push(Action::Sound("misc/menu1.wav"));
            }
            K_DOWNARROW | K_DPAD_DOWN | K_MWHEELDOWN => {
                self.select((self.section + 1) % n);
                out.push(Action::Sound("misc/menu1.wav"));
            }
            K_PGUP | K_LEFTARROW | K_DPAD_LEFT | K_LSHOULDER => self.scroll_by(-(visible_lines() as isize - 2)),
            K_PGDN | K_RIGHTARROW | K_DPAD_RIGHT | K_RSHOULDER | K_ENTER | K_KP_ENTER | K_ABUTTON => {
                self.scroll_by(visible_lines() as isize - 2)
            }
            K_HOME => self.scroll.set(0),
            K_END => self.scroll.set(self.max_scroll()),
            K_MOUSE1 => {
                if let Some(i) = mouse.and_then(|p| self.item_at(p)) {
                    self.select(i);
                    out.push(Action::Sound("misc/menu2.wav"));
                }
            }
            _ => {}
        }
        false
    }

    pub fn draw(&self, c: &mut Canvas) {
        let secs = sections();
        let sel = self.section.min(secs.len() - 1);
        c.text_center(320.0, 20.0, "Help", 14.0, GOLD_FG);
        // section list
        let h = secs.len() as f32 * ROW_H + 8.0;
        c.fill(LIST_X, TOP, LIST_W, h, [0.0, 0.0, 0.0, 0.55]);
        c.frame(LIST_X, TOP, LIST_W, h, [0.35, 0.27, 0.16, 0.9]);
        for (i, s) in secs.iter().enumerate() {
            let y = TOP + 4.0 + i as f32 * ROW_H;
            if i == sel {
                c.fill(LIST_X + 2.0, y, LIST_W - 4.0, ROW_H, HILITE);
            }
            // long (translated) titles are cut to the list width
            let mut title = s.title.clone();
            if c.text_w_raw(&title, TXT) > LIST_W - 12.0 {
                while c.text_w_raw(&format!("{title}…"), TXT) > LIST_W - 12.0 && title.pop().is_some() {}
                title = format!("{}…", title.trim_end());
            }
            c.text_raw(LIST_X + 8.0, y + 2.0, &title, TXT, if i == sel { GOLD_FG } else { FG });
        }
        // text pane
        c.fill(TEXT_X, TOP, TEXT_W, BOTTOM - TOP, [0.0, 0.0, 0.0, 0.6]);
        c.frame(TEXT_X, TOP, TEXT_W, BOTTOM - TOP, [0.35, 0.27, 0.16, 0.9]);
        let lines = layout(c, &secs[sel]);
        self.lines.set(lines.len());
        let sc = self.scroll.get().min(self.max_scroll());
        self.scroll.set(sc);
        let vis = visible_lines();
        for (k, (x, s, head)) in lines.iter().skip(sc).take(vis).enumerate() {
            let y = TOP + PAD + k as f32 * LINE_H;
            c.text(TEXT_X + PAD + x, y, s, TXT, if *head { GOLD_FG } else { FG });
        }
        if lines.len() > vis {
            let track = BOTTOM - TOP - 8.0;
            let th = (track * vis as f32 / lines.len() as f32).max(8.0);
            let ty = TOP + 4.0 + (track - th) * sc as f32 / self.max_scroll().max(1) as f32;
            let x = TEXT_X + TEXT_W - 6.0;
            c.fill(x, TOP + 4.0, 3.0, track, [1.0, 1.0, 1.0, 0.12]);
            c.fill(x, ty, 3.0, th, [0.85, 0.65, 0.3, 0.9]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translated_manuals_have_the_same_sections() {
        let n = parse(MANUALS[0], qcore::lang::Lang::English).len();
        for (i, m) in MANUALS.iter().enumerate() {
            let s = parse(m, qcore::lang::Lang::ALL[i]);
            assert_eq!(s.len(), n, "{:?} manual has {} sections, English {n}", qcore::lang::Lang::ALL[i], s.len());
            assert!(s.iter().all(|x| !x.blocks.is_empty()), "{:?} has an empty section", qcore::lang::Lang::ALL[i]);
        }
    }

    #[test]
    fn manual_parses_into_sections() {
        let s = sections();
        assert!(s.len() > 5);
        assert!(s.iter().any(|s| s.title == "Add-ons"));
        let text = format!("{:?}", s);
        assert!(!text.contains("**") && !text.contains('`'));
        // add-on weapon table rows become list items
        let addons = s.iter().find(|s| s.title == "Add-ons").unwrap();
        assert!(addons.blocks.iter().any(|b| matches!(b, Block::Para { text, .. } if text.starts_with("Portal gun. Fire:"))));
    }

    #[test]
    fn main_menu_help_opens_and_navigates() {
        let mut cv = Cvars::default();
        let mut m = Menu::new();
        m.open(Screen::Main);
        m.main_cursor = 3;
        m.key(K_ENTER, &mut cv, false);
        assert_eq!(m.screen, Screen::Help);
        m.key(K_DOWNARROW, &mut cv, false);
        assert_eq!(m.help.section, 1);
        m.key(K_UPARROW, &mut cv, false);
        m.key(K_UPARROW, &mut cv, false);
        assert_eq!(m.help.section, sections().len() - 1);
        m.key(K_ESCAPE, &mut cv, false);
        assert_eq!(m.screen, Screen::Main);
        m.open_help("split");
        assert_eq!(sections()[m.help.section].title, "Split-screen");
    }

    #[test]
    fn links_become_plain_text() {
        assert_eq!(plain("see [the docs](a/b.md) **now**"), "see the docs now");
    }
}
