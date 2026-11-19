//! Where elements go: anchor points, rectangles, and keeping elements
//! apart. An element that would overlap another moves to the nearest
//! free spot on screen.

use super::layout::{Anchor, Elem};

/// Distance of the anchor points from the screen edges, in HUD units.
pub const MARGIN: f32 = 8.0;
/// Space left between elements pushed apart.
pub const GAP: f32 = 2.0;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }
    pub fn contains(&self, (px, py): (f32, f32)) -> bool {
        (self.x..self.x + self.w).contains(&px) && (self.y..self.y + self.h).contains(&py)
    }
    pub fn right(&self) -> f32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> f32 {
        self.y + self.h
    }
}

fn frac(i: usize) -> f32 {
    i as f32 * 0.5
}

/// The screen point of `a` on a `vw`×`vh` HUD.
pub fn anchor_point(a: Anchor, vw: f32, vh: f32) -> (f32, f32) {
    let along = |i: usize, len: f32| match i {
        0 => MARGIN,
        1 => (len / 2.0).floor(),
        _ => len - MARGIN,
    };
    (along(a.col(), vw), along(a.row(), vh))
}

/// Where `e` asks to be, for a scaled size `w`×`h`.
pub fn wanted(e: &Elem, (w, h): (f32, f32), vw: f32, vh: f32) -> Rect {
    let (ax, ay) = anchor_point(e.anchor, vw, vh);
    Rect { x: (ax - frac(e.anchor.col()) * w + e.x).round(), y: (ay - frac(e.anchor.row()) * h + e.y).round(), w, h }
}

/// The offset that puts `e` at `r`.
pub fn offset_for(e: &Elem, r: Rect, vw: f32, vh: f32) -> (f32, f32) {
    let (ax, ay) = anchor_point(e.anchor, vw, vh);
    ((r.x - ax + frac(e.anchor.col()) * r.w).round(), (r.y - ay + frac(e.anchor.row()) * r.h).round())
}

fn clamp_on_screen(r: Rect, vw: f32, vh: f32) -> Rect {
    Rect { x: r.x.clamp(0.0, (vw - r.w).max(0.0)), y: r.y.clamp(0.0, (vh - r.h).max(0.0)), ..r }
}

/// `want` moved on screen and off `others`, as little as possible. When
/// there is no room at all it stays put (on screen).
pub fn nearest_free(want: Rect, others: &[Rect], vw: f32, vh: f32) -> Rect {
    let start = clamp_on_screen(want, vw, vh);
    if !others.iter().any(|o| o.overlaps(&start)) {
        return start;
    }
    // the best spot lines up with the wish or touches a screen edge or another element
    let (w, h) = (want.w, want.h);
    let mut xs = vec![want.x, 0.0, vw - w];
    let mut ys = vec![want.y, 0.0, vh - h];
    for o in others {
        xs.extend([o.right() + GAP, o.x - w - GAP]);
        ys.extend([o.bottom() + GAP, o.y - h - GAP]);
    }
    let fits = |v: f32, len: f32, size: f32| v >= 0.0 && v + size <= len.max(size);
    xs.retain(|&x| fits(x, vw, w));
    ys.retain(|&y| fits(y, vh, h));
    let mut best: Option<(f32, Rect)> = None;
    for &x in &xs {
        for &y in &ys {
            let r = Rect { x, y, w, h };
            let d = (x - want.x).powi(2) + (y - want.y).powi(2);
            if best.is_some_and(|(bd, _)| bd <= d) || others.iter().any(|o| o.overlaps(&r)) {
                continue;
            }
            best = Some((d, r));
        }
    }
    best.map_or(start, |(_, r)| r)
}

/// Rectangles for every element, in list order: each goes where it asks
/// unless an earlier one is there, then to the nearest free spot. `last`
/// is placed after all the others (the element being edited gives way).
pub fn place_all(elems: &[Elem], sizes: &[(f32, f32)], vw: f32, vh: f32, last: Option<usize>) -> Vec<Rect> {
    let mut out = vec![Rect::default(); elems.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(elems.len());
    let order = (0..elems.len()).filter(|&i| Some(i) != last).chain(last.filter(|&i| i < elems.len()));
    for i in order {
        let r = nearest_free(wanted(&elems[i], sizes[i], vw, vh), &placed, vw, vh);
        placed.push(r);
        out[i] = r;
    }
    out
}

/// Spreads elements that share an anchor away from the screen edge (the
/// old HUD's stacks): used for layouts made from the old settings.
pub fn stack(elems: &mut [Elem], sizes: &[(f32, f32)]) {
    for a in Anchor::ALL {
        let idx: Vec<usize> = (0..elems.len()).filter(|&i| elems[i].anchor == a).collect();
        let total: f32 = idx.iter().map(|&i| sizes[i].1 + GAP * 2.0).sum::<f32>() - GAP * 2.0;
        let mut used = 0.0;
        for &i in &idx {
            let h = sizes[i].1;
            elems[i].y += match a.row() {
                0 => used,
                1 => -total / 2.0 + used + h / 2.0,
                _ => -used,
            }
            .round();
            used += h + GAP * 2.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hud::kind::Kind;

    fn r(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn anchors_put_the_matching_corner_on_the_point() {
        let mut e = Elem::new(Kind::Health, Anchor::BottomRight);
        assert_eq!(wanted(&e, (40.0, 20.0), 400.0, 300.0), r(352.0, 272.0, 40.0, 20.0));
        e.anchor = Anchor::Center;
        (e.x, e.y) = (5.0, -3.0);
        let at = wanted(&e, (40.0, 20.0), 400.0, 300.0);
        assert_eq!(at, r(185.0, 137.0, 40.0, 20.0));
        assert_eq!(offset_for(&e, at, 400.0, 300.0), (5.0, -3.0));
    }

    #[test]
    fn overlapping_element_moves_the_shortest_way() {
        let others = [r(100.0, 100.0, 50.0, 20.0)];
        // mostly to the right of it: slides right
        assert_eq!(nearest_free(r(140.0, 105.0, 30.0, 10.0), &others, 400.0, 300.0), r(152.0, 105.0, 30.0, 10.0));
        // just under its top: slides up
        assert_eq!(nearest_free(r(110.0, 92.0, 20.0, 10.0), &others, 400.0, 300.0), r(110.0, 88.0, 20.0, 10.0));
        // free already, but off screen: pulled back on
        assert_eq!(nearest_free(r(390.0, -5.0, 20.0, 10.0), &others, 400.0, 300.0), r(380.0, 0.0, 20.0, 10.0));
        // against the screen edge: goes the other way
        let wall = [r(0.0, 0.0, 30.0, 300.0)];
        assert_eq!(nearest_free(r(0.0, 10.0, 20.0, 10.0), &wall, 400.0, 300.0).x, 32.0);
    }

    #[test]
    fn placement_never_overlaps_and_the_edited_element_gives_way() {
        let elems: Vec<Elem> = (0..6).map(|_| Elem::new(Kind::Clock, Anchor::TopLeft)).collect();
        let sizes = vec![(60.0, 12.0); 6];
        let rects = place_all(&elems, &sizes, 320.0, 240.0, None);
        for i in 0..6 {
            for j in i + 1..6 {
                assert!(!rects[i].overlaps(&rects[j]), "{i} {j}: {rects:?}");
            }
        }
        assert_eq!(rects[0], r(8.0, 8.0, 60.0, 12.0));
        let edited = place_all(&elems, &sizes, 320.0, 240.0, Some(0));
        assert_eq!(edited[1], r(8.0, 8.0, 60.0, 12.0));
        assert_ne!(edited[0], edited[1]);
    }

    #[test]
    fn stacks_grow_away_from_the_edge() {
        let mut elems = vec![Elem::new(Kind::Health, Anchor::BottomLeft), Elem::new(Kind::Armor, Anchor::BottomLeft), Elem::new(Kind::Clock, Anchor::TopRight)];
        let sizes = [(72.0, 24.0), (100.0, 24.0), (40.0, 12.0)];
        stack(&mut elems, &sizes);
        assert_eq!((elems[0].y, elems[1].y, elems[2].y), (0.0, -28.0, 0.0));
        let rects = place_all(&elems, &sizes, 400.0, 300.0, None);
        assert!(!rects[0].overlaps(&rects[1]));
        assert_eq!(rects[1].y, 300.0 - 8.0 - 24.0 - 28.0);
    }
}

