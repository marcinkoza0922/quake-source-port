//! What a hookshot hit is made of. BSP has no material data, so faces are
//! classified by texture name, following the naming of id1 and common mods
//! (`wood1_1`, `wizwood1_2`, `metal5_2`, `wizmet1_1`, `tech04_3`,
//! `comp1_1`, `cop1_1`, `stone1_3`, `bricka2_4`, `rock3_8`, `city4_6`, ...).
//! Anything unrecognised counts as stone.

use qcore::math::Vec3;
use qformats::Bsp;

/// Hookable material classes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Wood,
    Metal,
    Stone,
    /// Sky, liquids, clip and trigger brushes: never hookable.
    None,
}

/// Name fragments per class, checked wood first (so `wizwood` is wood and
/// `wizmet` is metal). Order inside a list does not matter.
const WOOD: &[&str] = &["wood", "crate", "plank", "timber", "tree", "trunk", "bark", "barrel", "door03", "door04"];
const METAL: &[&str] = &[
    "metal", "met", "tech", "comp", "cop", "slip", "grate", "grill", "pipe", "iron", "steel", "rivet", "bolt", "plat_", "plate", "chain", "mmetal",
];

/// Name fragments of electronics textures (computer panels, tech walls and
/// lights, buttons and switches, slipgates): where the cable add-on plugs in.
const ELECTRONIC: &[&str] = &[
    "comp", "tech", "twall", "tlight", "htek", "basebtn", "basebutn", "butn", "button", "mtlsw", "swtch", "floorsw", "slip", "screen",
    "monitor", "console", "panel", "circuit", "terminal", "keypad", "elec", "generat", "reactor", "fuse", "socket", "outlet",
];

/// The texture shows electronics (case-insensitive, like [`classify`]).
pub fn electronic(name: &str) -> bool {
    if classify(name) == Material::None {
        return false;
    }
    let n = name.to_ascii_lowercase();
    let n = n.strip_prefix('+').map(|s| s.get(1..).unwrap_or("")).unwrap_or(&n);
    ELECTRONIC.iter().any(|e| n.contains(e))
}

/// Classifies a texture name (case-insensitive; animation `+0`/`+a` and
/// toggle prefixes are ignored).
pub fn classify(name: &str) -> Material {
    let n = name.to_ascii_lowercase();
    if n.starts_with('*') || n.starts_with('!') || n.starts_with("sky") || n.contains("clip") || n.starts_with("trigger") || n == "skip" {
        return Material::None;
    }
    let n = n.strip_prefix('+').map(|s| s.get(1..).unwrap_or("")).unwrap_or(&n);
    if WOOD.iter().any(|w| n.contains(w)) {
        Material::Wood
    } else if METAL.iter().any(|m| n.contains(m)) {
        Material::Metal
    } else {
        Material::Stone
    }
}

pub struct Face {
    pub normal: Vec3,
    pub dist: f32,
    /// Polygon in submodel space.
    pub verts: Vec<Vec3>,
    pub material: Material,
    pub electronic: bool,
}

/// Per brush submodel, its faces with their material (built once per map).
#[derive(Default)]
pub struct Surfaces {
    models: Vec<Vec<Face>>,
}

impl Surfaces {
    pub fn new(bsp: &Bsp) -> Self {
        let face = |f: &qformats::bsp::Face| -> Option<Face> {
            let pl = bsp.planes.get(f.plane as usize)?;
            let sign = if f.side != 0 { -1.0 } else { 1.0 };
            let name = bsp.texinfo.get(f.texinfo as usize).and_then(|t| bsp.textures.get(t.miptex.max(0) as usize)).and_then(|t| t.as_ref());
            let verts = (f.first_edge..f.first_edge + f.num_edges)
                .filter_map(|i| {
                    let se = *bsp.surfedges.get(i as usize)?;
                    let e = bsp.edges.get(se.unsigned_abs() as usize)?;
                    bsp.vertexes.get(e[usize::from(se < 0)] as usize).map(|&v| Vec3::from(v))
                })
                .collect();
            Some(Face {
                normal: Vec3::from(pl.normal) * sign,
                dist: pl.dist * sign,
                verts,
                material: name.map_or(Material::Stone, |t| classify(&t.name)),
                electronic: name.is_some_and(|t| electronic(&t.name)),
            })
        };
        let models = bsp
            .models
            .iter()
            .map(|m| {
                let r = m.first_face.max(0) as usize..(m.first_face.max(0) + m.num_faces.max(0)) as usize;
                bsp.faces.get(r).unwrap_or(&[]).iter().filter_map(face).collect()
            })
            .collect();
        Surfaces { models }
    }

    /// Material of submodel `sub` at local point `p` on a surface facing
    /// `normal` (Stone when no face matches).
    pub fn at(&self, sub: usize, p: Vec3, normal: Vec3) -> Material {
        self.face(sub, p, normal).map_or(Material::Stone, |f| f.material)
    }

    /// The face of submodel `sub` at local point `p` facing `normal`.
    pub fn face(&self, sub: usize, p: Vec3, normal: Vec3) -> Option<&Face> {
        let faces = self.models.get(sub)?;
        let mut best: Option<(f32, &Face)> = None;
        for f in faces {
            let d = (p.dot(f.normal) - f.dist).abs();
            if d > 1.0 || f.normal.dot(normal) < 0.9 || f.verts.len() < 3 {
                continue;
            }
            if inside(&f.verts, f.normal, p) && best.is_none_or(|(bd, _)| d < bd) {
                best = Some((d, f));
            }
        }
        best.map(|(_, f)| f)
    }

    /// Every face of submodel `sub`.
    pub fn faces(&self, sub: usize) -> &[Face] {
        self.models.get(sub).map_or(&[], |v| v.as_slice())
    }
}

/// `p` lies within the convex polygon (either winding), with a little slack.
pub fn inside(verts: &[Vec3], n: Vec3, p: Vec3) -> bool {
    let mut sign = 0.0f32;
    for i in 0..verts.len() {
        let (a, b) = (verts[i], verts[(i + 1) % verts.len()]);
        let edge = b - a;
        let len = edge.length().max(1e-3);
        let s = edge.cross(p - a).dot(n) / len;
        if s.abs() < 0.5 {
            continue;
        }
        if sign == 0.0 {
            sign = s.signum();
        } else if s.signum() != sign {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use Material::*;

    #[test]
    fn texture_names_classify_like_id1_and_mods() {
        let cases = [
            ("wood1_1", Wood),
            ("WIZWOOD1_2", Wood),
            ("+0crate", Wood),
            ("door04_1", Wood),
            ("metal1_1", Metal),
            ("wizmet1_2", Metal),
            ("met5_1", Metal),
            ("tech04_3", Metal),
            ("comp1_1", Metal),
            ("cop1_1", Metal),
            ("+aslipbot", Metal),
            ("plat_top1", Metal),
            ("stone1_3", Stone),
            ("bricka2_4", Stone),
            ("rock3_8", Stone),
            ("city4_6", Stone),
            ("wall14_5", Stone),
            ("ground1_6", Stone),
            ("unknownthing", Stone),
            ("sky4", None),
            ("*water0", None),
            ("*lava1", None),
            ("clip", None),
            ("trigger", None),
        ];
        for (name, want) in cases {
            assert_eq!(classify(name), want, "{name}");
        }
    }

    #[test]
    fn electronics_are_tech_panels_buttons_and_slipgates() {
        for name in ["comp1_1", "TECH04_3", "twall2_1", "tlight01", "+0basebtn", "+abutn", "+1mtlsw", "+0slip", "htek04_2", "mswtch_2"] {
            assert!(electronic(name), "{name}");
        }
        for name in ["wood1_1", "metal1_1", "wizmet1_2", "stone1_3", "rune1_1", "wswitch1", "*slime0", "sky1", "trigger"] {
            assert!(!electronic(name), "{name}");
        }
    }

    #[test]
    fn point_in_polygon_respects_edges() {
        let sq = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 0.0), Vec3::new(0.0, 10.0, 0.0)];
        let n = Vec3::new(0.0, 0.0, 1.0);
        assert!(inside(&sq, n, Vec3::new(5.0, 5.0, 0.0)));
        assert!(!inside(&sq, n, Vec3::new(15.0, 5.0, 0.0)));
        let rev: Vec<Vec3> = sq.iter().rev().copied().collect();
        assert!(inside(&rev, n, Vec3::new(5.0, 5.0, 0.0)));
    }
}
