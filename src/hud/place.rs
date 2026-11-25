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

pub(super) fn clamp_on_screen(r: Rect, vw: f32, vh: f32) -> Rect {
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

/// Reflow candidate for `want` while another element is being dragged:
/// stays put when clear, otherwise shifts out of the way—preferring the
/// same column/row and favoring the vacated `drag_origin` slot so elements
/// in a stack cleanly swap places like a sortable drag-and-drop list.
fn reflow_free(want: Rect, placed: &[Rect], drag_rect: Rect, drag_origin: Rect, vw: f32, vh: f32) -> Rect {
    let start = clamp_on_screen(want, vw, vh);
    if !placed.iter().any(|o| o.overlaps(&start)) {
        return start;
    }
    let (w, h) = (want.w, want.h);
    let mut xs = vec![want.x, 0.0, vw - w, drag_origin.x, drag_origin.right() - w];
    let mut ys = vec![want.y, 0.0, vh - h, drag_origin.y, drag_origin.bottom() - h];
    for o in placed {
        xs.extend([o.right() + GAP, o.x - w - GAP]);
        ys.extend([o.bottom() + GAP, o.y - h - GAP]);
    }
    let fits = |v: f32, len: f32, size: f32| v >= 0.0 && v + size <= len.max(size);
    xs.retain(|&x| fits(x, vw, w));
    ys.retain(|&y| fits(y, vh, h));

    let hits_drag = start.overlaps(&drag_rect);
    let (wcx, wcy) = (want.x + w * 0.5, want.y + h * 0.5);
    let (to_ox, to_oy) = (drag_origin.x + drag_origin.w * 0.5 - wcx, drag_origin.y + drag_origin.h * 0.5 - wcy);

    let mut best: Option<(f32, Rect)> = None;
    for &x in &xs {
        for &y in &ys {
            let r = Rect { x, y, w, h };
            if placed.iter().any(|o| o.overlaps(&r)) {
                continue;
            }
            let mut score = (x - want.x).powi(2) + (y - want.y).powi(2);
            if (x - want.x).abs() < 1.0 || (y - want.y).abs() < 1.0 {
                score *= 0.85;
            }
            if hits_drag {
                let (dx, dy) = (x + w * 0.5 - wcx, y + h * 0.5 - wcy);
                if dx * to_ox + dy * to_oy > 0.0 {
                    score *= 0.5;
                }
            }
            if best.is_none_or(|(bs, _)| score < bs) {
                best = Some((score, r));
            }
        }
    }
    best.map_or(start, |(_, r)| r)
}

/// Website-style drag-and-drop placement: `dragged` takes priority where
/// the user drags it, and other elements reflow out of the way (swapping
/// toward `drag_origin` when dragged onto a sibling).
pub fn place_drag(elems: &[Elem], sizes: &[(f32, f32)], vw: f32, vh: f32, dragged: usize, drag_origin: Rect) -> Vec<Rect> {
    if dragged >= elems.len() || sizes.len() < elems.len() {
        return place_all(elems, sizes, vw, vh, None);
    }
    let mut out = vec![Rect::default(); elems.len()];
    let mut placed: Vec<Rect> = Vec::with_capacity(elems.len());
    let drag_rect = clamp_on_screen(wanted(&elems[dragged], sizes[dragged], vw, vh), vw, vh);
    placed.push(drag_rect);
    out[dragged] = drag_rect;
    for i in 0..elems.len() {
        if i == dragged {
            continue;
        }
        let want = wanted(&elems[i], sizes[i], vw, vh);
        let r = reflow_free(want, &placed, drag_rect, drag_origin, vw, vh);
        placed.push(r);
        out[i] = r;
    }
    out
}

/// Offsets a newly added element `e` so it stacks below existing elements
/// at the same anchor (or above them at bottom anchors so it stays on screen).
pub fn stack_new(e: &mut Elem, existing: &[Elem], rects: &[Rect], vw: f32, vh: f32) {
    let same: Vec<usize> = existing.iter().enumerate().filter(|(_, ex)| ex.anchor == e.anchor).map(|(i, _)| i).collect();
    if same.is_empty() {
        return;
    }
    if rects.len() == existing.len() && vw > 0.0 && vh > 0.0 {
        let matched: Vec<Rect> = same.iter().map(|&i| rects[i]).filter(|r| r.w > 0.0 && r.h > 0.0).collect();
        if !matched.is_empty() {
            let (_, ay) = anchor_point(e.anchor, vw, vh);
            e.y = match e.anchor.row() {
                0 => {
                    let max_b = matched.iter().map(Rect::bottom).fold(f32::NEG_INFINITY, f32::max);
                    (max_b + GAP * 2.0 - ay).round()
                }
                2 => {
                    let min_t = matched.iter().map(|r| r.y).fold(f32::INFINITY, f32::min);
                    (min_t - GAP * 2.0 - ay).round()
                }
                _ => {
                    let max_b = matched.iter().map(Rect::bottom).fold(f32::NEG_INFINITY, f32::max);
                    (max_b + GAP * 2.0 - ay + 12.0).round()
                }
            };
            return;
        }
    }
    if e.anchor.row() == 2 {
        let min_y = same.iter().map(|&i| existing[i].y).fold(f32::INFINITY, f32::min);
        e.y = (min_y - 28.0).round();
    } else {
        let max_y = same.iter().map(|&i| existing[i].y).fold(f32::NEG_INFINITY, f32::max);
        e.y = (max_y + 28.0).round();
    }
}

/// Result of snapping a dragged rectangle to screen and sibling alignment lines.
#[derive(Clone, Debug, Default)]
pub struct SnapResult {
    pub rect: Rect,
    /// Vertical guide lines (HUD x coordinates).
    pub guides_x: Vec<f32>,
    /// Horizontal guide lines (HUD y coordinates).
    pub guides_y: Vec<f32>,
}

/// Snaps `want` within `threshold` HUD units to screen center, safe margins,
/// or other elements' edges/centers, returning the snapped rect and active guide lines.
pub fn snap_rect(want: Rect, others: &[Rect], vw: f32, vh: f32, threshold: f32) -> SnapResult {
    let mut vx = vec![MARGIN, (vw / 2.0).floor(), (vw - MARGIN).max(0.0)];
    let mut vy = vec![MARGIN, (vh / 2.0).floor(), (vh - MARGIN).max(0.0)];
    for o in others {
        vx.extend([o.x, (o.x + o.w * 0.5).round(), o.right()]);
        vy.extend([o.y, (o.y + o.h * 0.5).round(), o.bottom()]);
    }
    let snap_axis = |pos: f32, size: f32, lines: &[f32]| -> f32 {
        if threshold <= 0.0 {
            return pos;
        }
        let mut best = (threshold + 0.001, pos);
        for &line in lines {
            for target in [line, line - size * 0.5, line - size] {
                let d = (pos - target).abs();
                if d < best.0 {
                    best = (d, target.round());
                }
            }
        }
        best.1
    };
    let mut r = Rect {
        x: snap_axis(want.x, want.w, &vx),
        y: snap_axis(want.y, want.h, &vy),
        w: want.w,
        h: want.h,
    };
    r = clamp_on_screen(r, vw, vh);
    let mut guides_x = Vec::new();
    for &line in &vx {
        let hit = (r.x - line).abs() <= 0.75 || (r.x + r.w * 0.5 - line).abs() <= 0.75 || (r.right() - line).abs() <= 0.75;
        if hit && !guides_x.iter().any(|&g: &f32| (g - line).abs() < 0.5) {
            guides_x.push(line);
        }
    }
    let mut guides_y = Vec::new();
    for &line in &vy {
        let hit = (r.y - line).abs() <= 0.75 || (r.y + r.h * 0.5 - line).abs() <= 0.75 || (r.bottom() - line).abs() <= 0.75;
        if hit && !guides_y.iter().any(|&g: &f32| (g - line).abs() < 0.5) {
            guides_y.push(line);
        }
    }
    SnapResult { rect: r, guides_x, guides_y }
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

    #[test]
    fn place_drag_swaps_siblings_and_springs_back_when_passed() {
        let mut a = Elem::new(Kind::Health, Anchor::BottomLeft);
        let mut b = Elem::new(Kind::Armor, Anchor::BottomLeft);
        let sizes = [(72.0, 24.0), (100.0, 24.0)];
        // a at bottom (y=328), b directly above it (y=300) on 640x360
        b.y = -28.0;
        let orig_a = wanted(&a, sizes[0], 640.0, 360.0);
        let orig_b = wanted(&b, sizes[1], 640.0, 360.0);
        assert_eq!((orig_a.y, orig_b.y), (328.0, 300.0));

        // Drag a upward onto b's position: a gets priority at y=300, b swaps down toward orig_a!
        a.y = -28.0;
        let swapped = place_drag(&[a.clone(), b.clone()], &sizes, 640.0, 360.0, 0, orig_a);
        assert_eq!(swapped[0].y, 300.0);
        assert!(swapped[1].y > swapped[0].y, "b should slide down into a's vacated slot: {swapped:?}");
        assert!(!swapped[0].overlaps(&swapped[1]));

        // Drag a far away to top-right: b springs right back to its original position!
        a.x = 400.0;
        a.y = -250.0;
        let passed = place_drag(&[a, b], &sizes, 640.0, 360.0, 0, orig_a);
        assert_eq!(passed[1], orig_b);
    }

    #[test]
    fn stack_new_places_below_on_top_anchors_and_above_on_bottom_anchors() {
        let fps = Elem::new(Kind::Fps, Anchor::TopRight);
        let fps_r = wanted(&fps, (40.0, 12.0), 640.0, 360.0);
        let mut clock = Elem::new(Kind::Clock, Anchor::TopRight);
        stack_new(&mut clock, &[fps], &[fps_r], 640.0, 360.0);
        let clock_r = wanted(&clock, (40.0, 12.0), 640.0, 360.0);
        assert!(clock_r.y >= fps_r.bottom() + GAP);

        let health = Elem::new(Kind::Health, Anchor::BottomLeft);
        let health_r = wanted(&health, (72.0, 24.0), 640.0, 360.0);
        let mut armor = Elem::new(Kind::Armor, Anchor::BottomLeft);
        stack_new(&mut armor, &[health], &[health_r], 640.0, 360.0);
        let armor_r = wanted(&armor, (72.0, 24.0), 640.0, 360.0);
        assert!(armor_r.bottom() <= health_r.y - GAP);
    }

    #[test]
    fn snap_rect_snaps_to_center_and_reports_guides() {
        let want = r(309.0, 171.0, 20.0, 20.0); // center (319, 181), within 2 units of (320, 180)
        let res = snap_rect(want, &[], 640.0, 360.0, 3.0);
        assert_eq!((res.rect.x, res.rect.y), (310.0, 170.0));
        assert!(res.guides_x.contains(&320.0));
        assert!(res.guides_y.contains(&180.0));
    }
}

