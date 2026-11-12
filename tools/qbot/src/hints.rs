//! Hand-written navigation hints, one file per map:
//! `tools/qbot/hints/<game>/<map>.txt`. They patch gaps in the automatic
//! graph (running jumps over gaps, ledges, crates, areas the flood fill
//! never reaches) and are applied every time the graph is built.
//!
//! Format, one directive per line, `#` starts a comment, coordinates are
//! player origins (as `-v` prints them, e.g. the from/to of a NOCLIP line):
//!
//! ```text
//! seed  x y z                  # extra flood-fill start
//! walk  x y z  ->  x y z       # one-way walk edge
//! walk  x y z  <-> x y z       # both ways
//! jump  x y z  ->  x y z       # hold jump while moving from a to b
//! block x y z  ->  x y z       # no way through here (a closed secret door,
//!                              # a gap the flood fill jumps that a player can't)
//! ```
//!
//! `block` removes every edge crossing the segment a..b (XY, within 40 units
//! of its height), both ways (`->` and `<->` mean the same). A block that cuts
//! no edge is reported, as the graph moved under it.
//!
//! Each end snaps to the floor below it (up to 128 units) and joins the
//! graph at the nearest node within 48 units, or becomes a new node linked
//! to the nodes around it, after a flood fill from there.

use crate::nav::{self, EdgeKind, Geo, Nav};
use qcore::math::Vec3;
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub enum Hint {
    Seed(Vec3),
    Edge { kind: EdgeKind, a: Vec3, b: Vec3, both: bool },
    Block { a: Vec3, b: Vec3 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Line {
    pub line: usize,
    pub hint: Hint,
}

/// Hints for `map` from `dir` (no file: none).
pub fn load(dir: &Path, map: &str) -> Result<Vec<Line>, String> {
    let path = dir.join(format!("{map}.txt"));
    match std::fs::read_to_string(&path) {
        Ok(s) => parse(&s).map_err(|e| format!("{}: {e}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(format!("{}: {e}", path.display())),
    }
}

pub fn parse(s: &str) -> Result<Vec<Line>, String> {
    let mut out = Vec::new();
    for (i, raw) in s.lines().enumerate() {
        let line = i + 1;
        let text = raw.split('#').next().unwrap_or("");
        let w: Vec<&str> = text.split_whitespace().collect();
        if w.is_empty() {
            continue;
        }
        let num = |k: usize| -> Result<f32, String> {
            w.get(k).ok_or_else(|| format!("line {line}: too few fields"))?.parse::<f32>().map_err(|_| format!("line {line}: bad number {:?}", w[k]))
        };
        let vec = |k: usize| -> Result<Vec3, String> { Ok(Vec3::new(num(k)?, num(k + 1)?, num(k + 2)?)) };
        let hint = match w[0] {
            "seed" if w.len() == 4 => Hint::Seed(vec(1)?),
            "walk" | "jump" | "block" if w.len() == 8 => {
                let both = match w[4] {
                    "->" => false,
                    "<->" => true,
                    a => return Err(format!("line {line}: expected -> or <->, got {a:?}")),
                };
                match w[0] {
                    "block" => Hint::Block { a: vec(1)?, b: vec(5)? },
                    "walk" => Hint::Edge { kind: EdgeKind::Walk, a: vec(1)?, b: vec(5)?, both },
                    _ => Hint::Edge { kind: EdgeKind::Jump, a: vec(1)?, b: vec(5)?, both },
                }
            }
            d => return Err(format!("line {line}: can't read {d:?} directive ({} fields)", w.len())),
        };
        out.push(Line { line, hint });
    }
    Ok(out)
}

/// A graph node at `p`: an existing one close by, else a new one.
fn anchor(nav: &mut Nav, geo: &Geo, p: Vec3) -> Option<u32> {
    let g = nav::ground(geo, p + Vec3::new(0.0, 0.0, 1.0), 128.0)?;
    nav.extend(geo, g);
    if let Some(n) = nav.nearest(g, 48.0).filter(|&n| (nav.pos[n as usize] - g).length() < 48.0) {
        return Some(n);
    }
    let n = nav.add(g);
    for m in nav.around(g, g, 48.0, g.z, 24.0) {
        if m != n {
            let d = (nav.pos[m as usize] - g).length();
            nav.link(m, n, EdgeKind::Walk, d);
            nav.link(n, m, EdgeKind::Walk, d);
        }
    }
    Some(n)
}

/// Whether segments p..q and a..b cross in XY.
fn crosses(p: Vec3, q: Vec3, a: Vec3, b: Vec3) -> bool {
    let side = |o: Vec3, s: Vec3, t: Vec3| (s.x - o.x) * (t.y - o.y) - (s.y - o.y) * (t.x - o.x);
    let (d1, d2) = (side(a, b, p), side(a, b, q));
    let (d3, d4) = (side(p, q, a), side(p, q, b));
    // Fine nodes can lie exactly on the barrier. Include endpoint contact
    // so splitting a crossing into two edges cannot bypass a block hint.
    d1 * d2 <= 0.0 && d3 * d4 <= 0.0
        && p.x.min(q.x) <= a.x.max(b.x) && a.x.min(b.x) <= p.x.max(q.x)
        && p.y.min(q.y) <= a.y.max(b.y) && a.y.min(b.y) <= p.y.max(q.y)
}

/// Remove the edges crossing the wall a..b; returns how many.
fn block(nav: &mut Nav, a: Vec3, b: Vec3) -> usize {
    let (zlo, zhi) = (a.z.min(b.z) - 40.0, a.z.max(b.z) + 40.0);
    let (lo, hi) = (a.min(b) - Vec3::splat(64.0), a.max(b) + Vec3::splat(64.0));
    let mut cut = 0;
    for n in nav.around(lo, hi, 0.0, (zlo + zhi) * 0.5, (zhi - zlo) * 0.5) {
        let p = nav.pos[n as usize];
        let before = nav.edges[n as usize].len();
        let pos = &nav.pos;
        let keep: Vec<_> = nav.edges[n as usize].iter().copied().filter(|e| !crosses(p, pos[e.to as usize], a, b)).collect();
        cut += before - keep.len();
        nav.edges[n as usize] = keep;
    }
    cut
}

/// Preserve explicit map barriers when runtime refinement adds new edges.
pub fn reapply_blocks(nav: &mut Nav, hints: &[Line]) {
    for h in hints {
        if let Hint::Block { a, b } = h.hint { block(nav, a, b); }
    }
}

pub fn allows_motion(positions: &[Vec3], hints: &[Line]) -> bool {
    !hints.iter().any(|h| {
        let Hint::Block { a, b } = h.hint else { return false; };
        positions.windows(2).any(|p| (p[0].z - (a.z + b.z) * 0.5).abs() <= (a.z - b.z).abs() * 0.5 + 40.0
            && crosses(p[0], p[1], a, b))
    })
}

/// Apply hints to a freshly built graph; returns the lines that failed.
pub fn apply(nav: &mut Nav, geo: &Geo, hints: &[Line]) -> Vec<String> {
    let mut bad = Vec::new();
    for h in hints {
        match h.hint {
            Hint::Seed(p) => {
                if nav::ground(geo, p + Vec3::new(0.0, 0.0, 1.0), 128.0).is_none() {
                    bad.push(format!("hint line {}: no floor under seed {p:?}", h.line));
                } else {
                    let first = nav.pos.len() as u32;
                    nav.extend(geo, p);
                    // The flood only links new nodes outwards: where it met the
                    // old graph on a walkable step, link back in too.
                    for n in first..nav.pos.len() as u32 {
                        let back: Vec<(u32, f32)> = nav.edges[n as usize]
                            .iter()
                            .filter(|e| e.to < first && e.kind == EdgeKind::Walk && (nav.pos[e.to as usize].z - nav.pos[n as usize].z).abs() <= 18.5)
                            .map(|e| (e.to, e.cost))
                            .collect();
                        for (m, c) in back {
                            nav.link(m, n, EdgeKind::Walk, c);
                        }
                    }
                }
            }
            Hint::Block { a, b } => {
                let cut = block(nav, a, b);
                if cut == 0 {
                    bad.push(format!("hint line {}: block {a:?} -> {b:?} cuts no edge", h.line));
                }
            }
            Hint::Edge { kind, a, b, both } => {
                let (Some(na), Some(nb)) = (anchor(nav, geo, a), anchor(nav, geo, b)) else {
                    bad.push(format!("hint line {}: no floor under {a:?} or {b:?}", h.line));
                    continue;
                };
                let cost = (nav.pos[nb as usize] - nav.pos[na as usize]).length() + if kind == EdgeKind::Jump { 32.0 } else { 0.0 };
                // A hint overrides whatever the flood fill found between the two.
                nav.unlink(na, nb);
                nav.link(na, nb, kind, cost);
                if both {
                    nav.unlink(nb, na);
                    nav.link(nb, na, kind, cost);
                }
            }
        }
    }
    bad
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_directives() {
        let h = parse("# e1m1\nseed 1 2 3\nwalk 0 0 0 -> 10 0 0  # ledge\n\njump 1 1 1 <-> 2 2 2\n").unwrap();
        assert_eq!(h.len(), 3);
        assert_eq!(h[0], Line { line: 2, hint: Hint::Seed(Vec3::new(1.0, 2.0, 3.0)) });
        assert!(matches!(h[1].hint, Hint::Edge { kind: EdgeKind::Walk, both: false, .. }));
        assert!(matches!(h[2].hint, Hint::Edge { kind: EdgeKind::Jump, both: true, .. }));
        assert_eq!(h[2].line, 5);
    }

    #[test]
    fn crossing() {
        let v = |x, y| Vec3::new(x, y, 0.0);
        assert!(crosses(v(0.0, -1.0), v(0.0, 1.0), v(-1.0, 0.0), v(1.0, 0.0)));
        assert!(!crosses(v(0.0, 1.0), v(0.0, 2.0), v(-1.0, 0.0), v(1.0, 0.0)));
    }

    #[test]
    fn runtime_edges_and_probes_respect_hint_barriers() {
        let hints = parse("block 0 -64 24 -> 0 64 24").unwrap();
        let from = Vec3::new(-16.0, 0.0, 24.0);
        let to = Vec3::new(16.0, 0.0, 24.0);
        let mut nav = Nav::empty(from, vec![]);
        let a = nav.add(from);
        let b = nav.add(to);
        nav.link(a, b, EdgeKind::Walk, 32.0);
        reapply_blocks(&mut nav, &hints);
        assert!(nav.edge(a, b).is_none());
        assert!(!allows_motion(&[from, to], &hints));
        assert!(!allows_motion(&[from, Vec3::new(0.0, 0.0, 24.0), to], &hints));
        assert!(allows_motion(&[from + Vec3::new(0.0, 0.0, 128.0), to + Vec3::new(0.0, 0.0, 128.0)], &hints));
    }

    #[test]
    fn rejects_bad_lines() {
        assert!(parse("walk 0 0 0 => 1 1 1").unwrap_err().contains("line 1"));
        assert!(parse("seed 1 x 3").is_err());
        assert!(parse("fly 1 2 3").is_err());
        assert!(matches!(parse("block 0 0 0 -> 1 1 1").unwrap()[0].hint, Hint::Block { .. }));
    }
}
