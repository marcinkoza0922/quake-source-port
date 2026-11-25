//! HUD elements add-ons bring. They are shared: every gun with a magazine
//! uses the one Magazine element, as only the held gun's shows, so a
//! combination is a set of elements ([`Combo`]), not of add-ons.
//!
//! The HUD in play is `hud_layout` plus the running combination's elements,
//! at predefined places unless the layout has them already. Changes the
//! player made for a combination (Adjust HUD in an add-on's settings) are
//! kept in `hud_addon_layouts` as patches to that: `<combo>:` followed by
//! the elements that differ, in `hud_layout` form, and `-kind` for removed
//! ones, so later edits of the layout still reach every combination. A
//! combination without a patch of its own takes those of its parts. Example:
//! `magazine: magazine@br,y-40 bullettime+magazine: -clock`.

use qcore::cvar::Cvars;

use super::draw::STATUS_BAR_H;
use super::kind::Kind;
use super::layout::{Anchor, Elem, Layout};
use super::place::{self, Rect};

/// Archived patches of `hud_layout` per combination.
pub const CVAR: &str = "hud_addon_layouts";

/// Each shared element and the toggles of the add-ons that use it.
const ELEMENTS: [(Kind, &[&str]); 2] =
    [(Kind::BulletTime, &["addon_bullet_time"]), (Kind::Magazine, &["addon_akimbo", "addon_rifle"])];

/// A set of add-on HUD elements (bits of `ELEMENTS`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Combo(u8);

impl Combo {
    fn of_toggles(on: impl Fn(&str) -> bool) -> Self {
        let bits = ELEMENTS.iter().enumerate().filter(|(_, (_, cvars))| cvars.iter().any(|c| on(c)));
        Combo(bits.fold(0, |b, (i, _)| b | 1 << i))
    }

    /// The elements of the add-ons switched on.
    pub fn enabled(cv: &Cvars) -> Self {
        Self::of_toggles(|c| cv.value_of(c) != 0.0 && (c != "addon_bullet_time" || cv.value_of("addon_bullet_time_meter") != 0.0))
    }

    /// The elements of the add-on with toggle `cvar`.
    pub fn of(cvar: &str) -> Self {
        Self::of_toggles(|c| c == cvar)
    }

    pub fn union(self, o: Self) -> Self {
        Combo(self.0 | o.0)
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn has(self, k: Kind) -> bool {
        self.kinds().any(|x| x == k)
    }

    pub fn kinds(self) -> impl Iterator<Item = Kind> {
        ELEMENTS.iter().enumerate().filter(move |(i, _)| self.0 >> i & 1 != 0).map(|(_, e)| e.0)
    }

    /// Name in `hud_addon_layouts`: `bullettime+magazine`.
    fn id(self) -> String {
        self.kinds().map(Kind::id).collect::<Vec<_>>().join("+")
    }

    /// The smaller combinations within this one, smallest first.
    fn parts(self) -> Vec<Combo> {
        let mut v: Vec<Combo> = (1..self.0).filter(|b| b & !self.0 == 0).map(Combo).collect();
        v.sort_by_key(|c| c.0.count_ones());
        v
    }
}

/// The parsed `hud_addon_layouts`: patches by combination name (names this
/// build doesn't know are kept).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patches(Vec<(String, String)>);

impl Patches {
    pub fn parse(s: &str) -> Self {
        let mut out: Vec<(String, String)> = Vec::new();
        for tok in s.split_whitespace() {
            if let Some(id) = tok.strip_suffix(':') {
                out.push((id.to_string(), String::new()));
            } else if let Some((_, p)) = out.last_mut() {
                if !p.is_empty() {
                    p.push(' ');
                }
                p.push_str(tok);
            }
        }
        Patches(out)
    }

    pub fn of(cv: &Cvars) -> Self {
        Self::parse(cv.by_name(CVAR).map_or("", |c| c.string.as_str()))
    }

    /// One line per combination.
    pub fn lines(&self) -> Vec<String> {
        self.0.iter().map(|(id, p)| format!("{id}: {p}")).collect()
    }

    pub fn to_text(&self) -> String {
        self.lines().join(" ")
    }

    fn get(&self, c: Combo) -> Option<&str> {
        let id = c.id();
        self.0.iter().find(|(i, _)| *i == id).map(|(_, p)| p.as_str())
    }

    /// Sets `c`'s patch; an empty one is dropped.
    pub fn set(&mut self, c: Combo, patch: String) {
        let id = c.id();
        self.0.retain(|(i, _)| *i != id);
        if !patch.is_empty() {
            self.0.push((id, patch));
            self.0.sort();
        }
    }
}

/// Where `k` goes until the player moves it: clear of the elements of the
/// built-in HUDs, above a status bar at the bottom.
fn predefined(k: Kind, base: &Layout) -> Elem {
    let bars = base.elems.iter().filter(|e| e.anchor.row() == 2 && !e.hidden);
    let lift = bars
        .map(|e| match e.kind {
            Kind::StatusBar => STATUS_BAR_H[usize::from(e.a == 1)] * e.scale,
            Kind::ModHud => super::draw::MOD_HUD_SIZE.1 * e.scale,
            _ => 0.0,
        })
        .fold(0.0, f32::max);
    let mut e = Elem::new(k, k.default_anchor());
    e.y = if e.anchor == Anchor::BottomCenter { -16.0 - lift.ceil() } else { 2.0 };
    e
}

/// `base` with `c`'s elements and the patches of its parts.
fn inherited(base: &Layout, patches: &Patches, c: Combo) -> Layout {
    let mut l = base.clone();
    for k in c.kinds() {
        if !base.elems.iter().any(|e| e.kind == k) {
            l.elems.push(predefined(k, base));
        }
    }
    for p in c.parts() {
        apply(&mut l, patches.get(p).unwrap_or(""));
    }
    l
}

/// The layout shown while `c`'s elements are in use.
pub fn compose(base: &Layout, patches: &Patches, c: Combo) -> Layout {
    if c.is_empty() {
        return base.clone();
    }
    let mut l = inherited(base, patches, c);
    apply(&mut l, patches.get(c).unwrap_or(""));
    l
}

/// Makes `edited` the layout of `c`.
pub fn store(base: &Layout, patches: &mut Patches, c: Combo, edited: &Layout) {
    let patch = diff(&inherited(base, patches, c), edited);
    patches.set(c, patch);
}

/// The kinds `patch` names get its elements, in place of their old ones.
fn apply(l: &mut Layout, patch: &str) {
    let mut gone: Vec<Kind> = patch.split_whitespace().filter_map(|t| Kind::from_id(t.strip_prefix('-')?)).collect();
    let mut new: Vec<Elem> = patch.split_whitespace().filter_map(Elem::parse).collect();
    gone.extend(new.iter().map(|e| e.kind));
    let mut out = Vec::with_capacity(l.elems.len() + new.len());
    for e in l.elems.drain(..) {
        if !gone.contains(&e.kind) {
            out.push(e);
        } else if let Some(i) = new.iter().position(|n| n.kind == e.kind) {
            out.push(new.remove(i));
        }
    }
    out.append(&mut new);
    l.elems = out;
    l.ensure_permanent();
}

fn diff(from: &Layout, to: &Layout) -> String {
    let of = |l: &Layout, k: Kind| -> Vec<String> { l.elems.iter().filter(|e| e.kind == k).map(Elem::to_text).collect() };
    let mut toks: Vec<String> = Vec::new();
    for k in Kind::ALL {
        if of(from, k) != of(to, k) && of(to, k).is_empty() {
            toks.push(format!("-{}", k.id()));
        }
    }
    toks.extend(to.elems.iter().filter(|e| of(from, e.kind) != of(to, e.kind)).map(Elem::to_text));
    toks.join(" ")
}

/// `c`'s elements that don't get their place, each with the element in the
/// way. The messages don't count: they are short-lived, and small screens
/// have little room beside their boxes.
pub fn conflicts(l: &Layout, sizes: &[(f32, f32)], c: Combo, vw: f32, vh: f32) -> Vec<(Kind, Kind)> {
    let rects = place::place_all(&l.elems, sizes, vw, vh, None);
    let mut out = Vec::new();
    for (i, e) in l.elems.iter().enumerate() {
        if e.hidden || !c.has(e.kind) {
            continue;
        }
        let want = place::clamp_on_screen(place::wanted(e, sizes[i], vw, vh), vw, vh);
        let other = |(j, r): (usize, &Rect)| (j != i && !l.elems[j].kind.is_message() && r.overlaps(&want)).then(|| l.elems[j].kind);
        if want != rects[i]
            && let Some(k) = rects.iter().enumerate().find_map(other)
        {
            out.push((e.kind, k));
        }
    }
    out
}

/// What the player is told about a conflict.
pub fn conflict_text(k: Kind, by: Kind) -> String {
    qcore::tr!("{} overlaps {}. Move it with Adjust HUD.", qcore::lang::tr(k.label()), qcore::lang::tr(by.label()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cvars() -> Cvars {
        let mut cv = Cvars::new();
        for n in ["addon_akimbo", "addon_rifle", "addon_bullet_time", "addon_railgun"] {
            cv.register(n, "0", 0);
        }
        cv.register("addon_bullet_time_meter", "1", 0);
        cv
    }

    fn at(l: &Layout, k: Kind) -> &Elem {
        l.elems.iter().find(|e| e.kind == k).unwrap()
    }

    #[test]
    fn add_ons_share_elements() {
        let mut cv = cvars();
        cv.set("addon_railgun", "1");
        assert!(Combo::enabled(&cv).is_empty());
        cv.set("addon_rifle", "1");
        let rifle = Combo::enabled(&cv);
        assert!(rifle.kinds().eq([Kind::Magazine]));
        cv.set("addon_akimbo", "1");
        assert_eq!(Combo::enabled(&cv), rifle, "both guns use the one magazine");
        cv.set("addon_bullet_time", "1");
        assert_eq!(Combo::enabled(&cv).id(), "bullettime+magazine");
        // without its meter bullet time shows nothing
        cv.set("addon_bullet_time_meter", "0");
        assert_eq!(Combo::enabled(&cv), rifle);
    }

    #[test]
    fn patches_survive_layout_edits_and_reach_bigger_combinations() {
        let mut base = Layout::default_layout();
        let (mag, bt) = (Combo::of("addon_rifle"), Combo::of("addon_bullet_time"));
        let both = mag.union(bt);
        let mut p = Patches::default();
        let mut edited = compose(&base, &p, mag);
        let i = edited.elems.iter().position(|e| e.kind == Kind::Magazine).unwrap();
        (edited.elems[i].anchor, edited.elems[i].y) = (Anchor::TopLeft, 40.0);
        let h = edited.elems.iter().position(|e| e.kind == Kind::Health).unwrap();
        edited.elems[h].x = 50.0;
        edited.elems.retain(|e| e.kind != Kind::Face && e.kind != Kind::Weapons);
        store(&base, &mut p, mag, &edited);
        // text round trip, as in the cvar
        p = Patches::parse(&format!("later+unknown: thing@tl {}", p.to_text()));
        assert_eq!(compose(&base, &p, mag), edited);
        assert!(p.to_text().contains("later+unknown: thing@tl"));
        // the plain HUD is untouched, and edits of it reach the combination
        assert_eq!(compose(&base, &p, Combo::default()), base);
        base.elems.push(Elem::new(Kind::Clock, Anchor::TopRight));
        let now = compose(&base, &p, mag);
        assert!(now.elems.iter().any(|e| e.kind == Kind::Clock) && !now.elems.iter().any(|e| e.kind == Kind::Weapons));
        assert_eq!(at(&now, Kind::Health).x, 50.0);
        // together with bullet time the magazine stays where it was put
        let l = compose(&base, &p, both);
        assert_eq!(at(&l, Kind::Magazine).anchor, Anchor::TopLeft);
        assert!(l.elems.iter().any(|e| e.kind == Kind::BulletTime));
        // a change for that combination alone
        let mut e2 = l.clone();
        e2.elems.iter_mut().find(|e| e.kind == Kind::Magazine).unwrap().scale = 2.0;
        store(&base, &mut p, both, &e2);
        assert_eq!(at(&compose(&base, &p, both), Kind::Magazine).scale, 2.0);
        assert_eq!(at(&compose(&base, &p, mag), Kind::Magazine).scale, 1.0);
        // back to what it inherits: no patch left
        store(&base, &mut p, both, &l);
        assert_eq!(p.get(both), None);
    }

    #[test]
    fn deleting_an_add_on_element_keeps_it_away() {
        let base = Layout::classic_layout();
        let mag = Combo::of("addon_akimbo");
        let mut p = Patches::default();
        let mut e = compose(&base, &p, mag);
        e.elems.retain(|e| e.kind != Kind::Magazine);
        store(&base, &mut p, mag, &e);
        assert_eq!(p.to_text(), "magazine: -magazine");
        assert_eq!(compose(&base, &p, mag), base);
        // one already in the layout is used as it is
        let mut own = base.clone();
        own.elems.push(Elem::new(Kind::Magazine, Anchor::TopRight));
        assert_eq!(compose(&own, &Patches::default(), mag), own);
    }

    #[test]
    fn an_element_in_the_way_is_a_conflict() {
        let base = Layout::parse("clock@bc,y-16");
        let mag = Combo::of("addon_rifle");
        let l = compose(&base, &Patches::default(), mag);
        let sizes: Vec<_> = l.elems.iter().map(|_| (60.0, 18.0)).collect();
        assert_eq!(conflicts(&l, &sizes, mag, 640.0, 360.0), [(Kind::Magazine, Kind::Clock)]);
        let mut hidden = l.clone();
        hidden.elems.last_mut().unwrap().hidden = true;
        assert!(conflicts(&hidden, &sizes, mag, 640.0, 360.0).is_empty());
        let l = compose(&Layout::parse("clock@bl"), &Patches::default(), mag);
        assert!(conflicts(&l, &sizes, mag, 640.0, 360.0).is_empty());
    }
}
