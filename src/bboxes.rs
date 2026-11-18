//! `r_showbboxes`: wireframe server-side bounding boxes for mappers, a port
//! of ironwail's R_ShowBoundingBoxes. Boxes are drawn on top of the scene
//! (no depth test). The box under the crosshair is white and, with
//! `r_showbboxes_links`/`_targets`, its links to other edicts are drawn as
//! arrows (outgoing red, incoming blue, both purple). Single player only.
//!
//! - `r_showbboxes` 1 all edicts, 2 only those in the PVS; negative values
//!   draw unfocused boxes plain white instead of colouring by model type.
//! - `r_showbboxes_think` 1 thinkers only, -1 non-thinkers only.
//! - `r_showbboxes_health` 1 healthy only, -1 non-healthy only.
//! - `r_showbboxes_filter` space-separated: `#42` edict number, `=name`
//!   exact classname, anything else a classname substring.
//!
//! `pointfile` (R_ReadPointFile_f) draws a map's leak path from
//! `maps/<map>.pts` with the same line geometry, depth-tested.
use crate::blood::Geometry;
use qcore::cvar::Cvars;
use qcore::math::Vec3;
use qrender::{FteBatch, FteBlend, FteVertex, PicId, Renderer};
use qserver::{BoxModel, EdictBox};

const LINK_OUT: u8 = 1;
const LINK_IN: u8 = 2;

pub struct Settings {
    pub mode: i32,
    pub think: f32,
    pub health: f32,
    pub links: u8,
    pub targets: bool,
    pub filter: String,
}

impl Settings {
    pub fn read(cv: &Cvars) -> Settings {
        let v = |n: &str| cv.value_of(n);
        Settings {
            mode: v("r_showbboxes") as i32,
            think: v("r_showbboxes_think"),
            health: v("r_showbboxes_health"),
            links: (v("r_showbboxes_links") as i32).clamp(0, 3) as u8,
            targets: v("r_showbboxes_targets") != 0.0,
            filter: cv.by_name("r_showbboxes_filter").map_or_else(String::new, |c| c.string.clone()),
        }
    }
}

/// True when `b` passes `r_showbboxes_filter` (empty matches everything).
pub fn filter_matches(filter: &str, b: &EdictBox) -> bool {
    let mut words = filter.split_whitespace().peekable();
    if words.peek().is_none() {
        return true;
    }
    words.any(|w| {
        if let Some(n) = w.strip_prefix('#') {
            n.parse::<u32>().is_ok_and(|n| n == b.num)
        } else if b.classname.is_empty() {
            false
        } else if let Some(name) = w.strip_prefix('=') {
            b.classname == name
        } else {
            b.classname.contains(w)
        }
    })
}

/// Distance along the ray `origin + t * dir` to where it enters the box.
fn ray_box(origin: Vec3, dir: Vec3, mins: Vec3, maxs: Vec3) -> Option<f32> {
    let (mut near, mut far) = (f32::NEG_INFINITY, f32::INFINITY);
    for i in 0..3 {
        if dir[i].abs() < 1e-6 {
            if origin[i] < mins[i] || origin[i] > maxs[i] {
                return None;
            }
            continue;
        }
        let (a, b) = ((mins[i] - origin[i]) / dir[i], (maxs[i] - origin[i]) / dir[i]);
        near = near.max(a.min(b));
        far = far.min(a.max(b));
    }
    (near <= far && far > 0.0).then_some(near)
}

/// Draw rectangle extents: point entities get a 16-unit box.
fn extent(b: &EdictBox) -> (Vec3, Vec3) {
    let pad = if b.point { Vec3::splat(8.0) } else { Vec3::ZERO };
    (b.mins - pad, b.maxs + pad)
}

fn center(b: &EdictBox) -> Vec3 {
    (b.mins + b.maxs) * 0.5
}

#[derive(Default)]
pub struct BBoxes {
    pic: Option<PicId>,
    /// `pointfile`: the leak path from `maps/<map>.pts`, collinear points
    /// merged; cleared on map load.
    pub pointfile: Vec<Vec3>,
}

/// R_ReadPointFile_f's parse: `x y z` lines until one doesn't parse, with
/// runs of collinear points merged. Returns (points read, path).
pub fn parse_pointfile(text: &str) -> (usize, Vec<Vec3>) {
    let collinear = |a: Vec3, b: Vec3, c: Vec3| (b - a).length() + (c - b).length() < (c - a).length() * 1.00001;
    let mut path: Vec<Vec3> = Vec::new();
    let mut read = 0;
    for l in text.lines() {
        let v: Vec<f32> = l.split_whitespace().map_while(|w| w.parse().ok()).take(3).collect();
        if v.len() < 3 {
            break;
        }
        read += 1;
        path.push(Vec3::new(v[0], v[1], v[2]));
        let n = path.len();
        if n >= 3 && collinear(path[n - 3], path[n - 2], path[n - 1]) {
            path[n - 2] = path[n - 1];
            path.pop();
        }
    }
    (read, path)
}

impl BBoxes {
    /// R_ShowPointFile: the leak path as depth-tested arrows.
    pub fn pointfile_frame(&mut self, eye: Vec3, r: &mut Renderer, out: &mut Geometry) {
        if self.pointfile.len() < 2 {
            return;
        }
        let Some(pic) = self.pic.or_else(|| r.register_pic_rgba_linear(1, 1, &[255; 4]).ok()) else { return };
        self.pic = Some(pic);
        let first = out.1.len() as u32;
        for w in self.pointfile.windows(2) {
            arrow(out, w[0], w[1], eye, rgba(0xff3f3f7f));
        }
        let n = out.1.len() as u32 - first;
        out.2.push(FteBatch { pic, blend: FteBlend::Blend, lines: true, first_index: first, num_indices: n, soft: false, gel: false });
    }

    /// Append the wireframes for `boxes` seen from `eye` looking along `fwd`.
    pub fn frame(&mut self, s: &Settings, boxes: &[EdictBox], eye: Vec3, fwd: Vec3, r: &mut Renderer, out: &mut Geometry) {
        if s.mode == 0 || boxes.is_empty() {
            return;
        }
        let shown: Vec<&EdictBox> = boxes
            .iter()
            .filter(|b| s.mode.abs() < 2 || b.in_pvs)
            .filter(|b| s.think == 0.0 || (b.nextthink <= 0.0) != (s.think > 0.0))
            .filter(|b| s.health == 0.0 || (b.health <= 0.0) != (s.health > 0.0))
            .filter(|b| filter_matches(&s.filter, b))
            .collect();
        // The closest box under the crosshair (ignoring any we stand inside).
        let focused = shown
            .iter()
            .filter_map(|b| {
                let (lo, hi) = extent(b);
                ray_box(eye, fwd, lo, hi).filter(|&d| d > 0.0).map(|d| (d, b.num))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, n)| n);
        // Edicts linked to the focused one, with LINK_* flags.
        let mut linked: Vec<(u32, u8)> = Vec::new();
        if let Some(f) = focused.and_then(|n| boxes.iter().find(|b| b.num == n)) {
            let add = |n: u32, flag: u8, linked: &mut Vec<(u32, u8)>| {
                if s.links & flag == 0 && !s.targets {
                    return;
                }
                match linked.iter_mut().find(|l| l.0 == n) {
                    Some(l) => l.1 |= flag,
                    None => linked.push((n, flag)),
                }
            };
            if s.links & LINK_OUT != 0 {
                for &n in &f.links {
                    add(n, LINK_OUT, &mut linked);
                }
            }
            for b in boxes.iter().filter(|b| b.num != f.num) {
                if s.targets {
                    if !f.targetname.is_empty() && b.target == f.targetname {
                        add(b.num, LINK_IN, &mut linked);
                    }
                    if !f.target.is_empty() && b.targetname == f.target {
                        add(b.num, LINK_OUT, &mut linked);
                    }
                }
                if s.links & LINK_IN != 0 && b.links.contains(&f.num) {
                    add(b.num, LINK_IN, &mut linked);
                }
            }
        }
        let Some(pic) = self.pic.or_else(|| r.register_pic_rgba_linear(1, 1, &[255; 4]).ok()) else { return };
        self.pic = Some(pic);
        let first = out.1.len() as u32;
        if let Some(f) = focused.and_then(|n| boxes.iter().find(|b| b.num == n)) {
            for &(n, flags) in &linked {
                let Some(t) = boxes.iter().find(|b| b.num == n) else { continue };
                let (from, to) = (center(f), center(t));
                match flags {
                    LINK_OUT => arrow(out, from, to, eye, rgba(0x7f7f3f3f)),
                    LINK_IN => arrow(out, to, from, eye, rgba(0x7f3f3f7f)),
                    _ => line(out, from, to, rgba(0x7f7f3f7f)),
                }
            }
        }
        for b in &shown {
            let color = if Some(b.num) == focused {
                0xffffffff
            } else if linked.iter().any(|l| l.0 == b.num) {
                0xaaaaaaaa
            } else if s.mode > 0 {
                if b.health > 0.0 {
                    0x7f0000ff
                } else {
                    match b.model {
                        BoxModel::Brush => 0x7fff8080,
                        BoxModel::Alias => 0x7f408080,
                        BoxModel::Sprite => 0x7f4040ff,
                        BoxModel::None => 0x7f800080,
                    }
                }
            } else {
                0x7fffffff
            };
            let color = rgba(color);
            if b.point {
                point(out, b.mins, color);
            } else {
                wire_box(out, b.mins, b.maxs, color);
            }
        }
        let n = out.1.len() as u32 - first;
        if n > 0 {
            out.2.push(FteBatch { pic, blend: FteBlend::Overlay, lines: true, first_index: first, num_indices: n, soft: false, gel: false });
        }
    }
}

/// ironwail's ABGR colour constant as RGBA bytes.
fn rgba(c: u32) -> [u8; 4] {
    c.to_le_bytes()
}

fn line(g: &mut Geometry, a: Vec3, b: Vec3, color: [u8; 4]) {
    let n = g.0.len() as u32;
    for p in [a, b] {
        g.0.push(FteVertex { pos: p.to_array(), uv: [0.5, 0.5], color });
    }
    g.1.extend_from_slice(&[n, n + 1]);
}

fn point(g: &mut Geometry, o: Vec3, color: [u8; 4]) {
    for axis in [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)] {
        line(g, o - axis * 8.0, o + axis * 8.0, color);
    }
}

fn wire_box(g: &mut Geometry, mins: Vec3, maxs: Vec3, color: [u8; 4]) {
    let n = g.0.len() as u32;
    for i in 0..8 {
        let p = Vec3::new(
            if i & 1 != 0 { mins.x } else { maxs.x },
            if i & 2 != 0 { mins.y } else { maxs.y },
            if i & 4 != 0 { mins.z } else { maxs.z },
        );
        g.0.push(FteVertex { pos: p.to_array(), uv: [0.5, 0.5], color });
    }
    const EDGES: [u32; 24] = [0, 1, 0, 2, 0, 4, 1, 3, 1, 5, 2, 3, 2, 6, 3, 7, 4, 5, 4, 6, 5, 7, 6, 7];
    g.1.extend(EDGES.iter().map(|e| n + e));
}

/// A line from `from` to `to` with an arrowhead halfway, facing the viewer.
fn arrow(g: &mut Geometry, from: Vec3, to: Vec3, eye: Vec3, color: [u8; 4]) {
    line(g, from, to, color);
    let len = (to - from).length();
    if len < 1.0 {
        return;
    }
    let dir = (to - from) / len;
    let mid = from + dir * (len * 0.5);
    let side = dir.cross(mid - eye);
    if side.length() < 1e-3 {
        return;
    }
    let side = side.normalize();
    let size = (len * 0.1).clamp(4.0, 16.0);
    let back = mid - dir * size;
    line(g, mid, back + side * size * 0.5, color);
    line(g, mid, back - side * size * 0.5, color);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edict(num: u32, classname: &str) -> EdictBox {
        EdictBox {
            num,
            mins: Vec3::ZERO,
            maxs: Vec3::splat(1.0),
            point: false,
            model: BoxModel::None,
            health: 0.0,
            nextthink: 0.0,
            classname: classname.into(),
            target: String::new(),
            targetname: String::new(),
            in_pvs: true,
            links: Vec::new(),
        }
    }

    #[test]
    fn filter_syntax() {
        let b = edict(42, "trigger_secret");
        assert!(filter_matches("", &b));
        assert!(filter_matches("#42", &b));
        assert!(!filter_matches("#4", &b));
        assert!(filter_matches("=trigger_secret", &b));
        assert!(!filter_matches("=trigger", &b));
        assert!(filter_matches("artifact secret", &b));
        assert!(!filter_matches("monster_", &b));
    }

    #[test]
    fn pointfile_merges_collinear_points() {
        let (n, path) = parse_pointfile("0 0 0\n1 0 0\n2 0 0\n2 5 0\n2 6 0\njunk\n9 9 9\n");
        assert_eq!(n, 5);
        assert_eq!(path, [Vec3::ZERO, Vec3::new(2.0, 0.0, 0.0), Vec3::new(2.0, 6.0, 0.0)]);
    }

    #[test]
    fn ray_hits_nearest_face() {
        let hit = ray_box(Vec3::new(-10.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::splat(1.0));
        assert_eq!(hit, Some(10.0));
        assert_eq!(ray_box(Vec3::new(-10.0, 5.0, 0.5), Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::splat(1.0)), None);
        assert_eq!(ray_box(Vec3::new(10.0, 0.5, 0.5), Vec3::new(1.0, 0.0, 0.0), Vec3::ZERO, Vec3::splat(1.0)), None);
    }
}
