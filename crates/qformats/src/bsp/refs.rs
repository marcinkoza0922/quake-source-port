//! Cross-lump reference checks. Client code indexes the lumps directly, so a
//! parsed map must not point outside them. Only references the engine follows
//! count: an unused face edge or an empty range may hold anything.

use super::{Face, FMT, Leaf, Model, Node, TexInfo};
use crate::{Error, Result, Vec3};

pub(super) struct Lumps<'a> {
    pub planes: usize,
    pub nodes: &'a [Node],
    pub leafs: &'a [Leaf],
    pub marksurfaces: &'a [u32],
    pub faces: &'a [Face],
    pub texinfo: &'a [TexInfo],
    pub surfedges: &'a [i32],
    pub edges: &'a [[u32; 2]],
    pub models: &'a [Model],
}

fn bad(what: &str, i: usize) -> Error {
    Error::new(FMT, format!("{what} {i} out of range"))
}

/// The elements of a range, which must be inside the lump unless empty.
fn slice<'a, T>(lump: &'a [T], first: u32, n: u32, what: &str, i: usize) -> Result<&'a [T]> {
    if n == 0 {
        return Ok(&[]);
    }
    let first = first as usize;
    lump.get(first..first.saturating_add(n as usize)).ok_or_else(|| bad(what, i))
}

pub(super) fn check(l: &Lumps) -> Result<()> {
    for (i, n) in l.nodes.iter().enumerate() {
        slice(l.faces, n.first_face, n.num_faces, "faces of node", i)?;
        for c in n.children {
            let ok = if c < 0 { (!c as usize) < l.leafs.len() } else { (c as usize) < l.nodes.len() };
            if !ok {
                return Err(bad("child of node", i));
            }
        }
    }
    for (i, m) in l.models.iter().enumerate() {
        if m.num_faces > 0 {
            let first = u32::try_from(m.first_face).map_err(|_| bad("faces of model", i))?;
            slice(l.faces, first, m.num_faces as u32, "faces of model", i)?;
        }
    }
    for (i, f) in l.leafs.iter().enumerate() {
        let marks = slice(l.marksurfaces, f.first_marksurface, f.num_marksurfaces, "marksurfaces of leaf", i)?;
        if marks.iter().any(|&m| m as usize >= l.faces.len()) {
            return Err(bad("face of leaf", i));
        }
    }
    for (i, f) in l.faces.iter().enumerate() {
        if f.plane as usize >= l.planes {
            return Err(bad("plane of face", i));
        }
        if f.texinfo as usize >= l.texinfo.len() {
            return Err(bad("texinfo of face", i));
        }
        let se = slice(l.surfedges, f.first_edge, f.num_edges, "edges of face", i)?;
        if se.iter().any(|s| s.unsigned_abs() as usize >= l.edges.len()) {
            return Err(bad("edge of face", i));
        }
    }
    Ok(())
}

/// Released maps have the odd bad edge (`zer1m2`, `zerend`), so these are
/// pointed at vertex 0 instead of failing the map.
pub(super) fn repair_edges(edges: &mut [[u32; 2]], vertexes: &[Vec3]) -> Result<()> {
    for v in edges.iter_mut().flatten().filter(|v| **v as usize >= vertexes.len()) {
        *v = 0;
    }
    match edges.is_empty() || !vertexes.is_empty() {
        true => Ok(()),
        false => Err(Error::new(FMT, "edges without vertexes")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FACE: Face = Face { plane: 0, side: 0, first_edge: 0, num_edges: 3, texinfo: 0, styles: [0; 4], lightofs: -1 };
    const NODE: Node = Node { plane: 0, children: [-1, -2], mins: [0.0; 3], maxs: [0.0; 3], first_face: 0, num_faces: 1 };
    const LEAF: Leaf =
        Leaf { contents: -1, visofs: -1, mins: [0.0; 3], maxs: [0.0; 3], first_marksurface: 0, num_marksurfaces: 1, ambient_level: [0; 4] };
    const MODEL: Model =
        Model { mins: [0.0; 3], maxs: [0.0; 3], origin: [0.0; 3], headnode: [0; 4], visleafs: 1, first_face: 0, num_faces: 1 };

    /// Checks a one-face map after `f` changed its records.
    fn map(f: impl Fn(&mut Node, &mut Leaf, &mut Face, &mut Model, &mut Vec<u32>, &mut Vec<i32>)) -> bool {
        let (mut node, mut leaf, mut face, mut model) = (NODE, LEAF, FACE, MODEL);
        let (mut marks, mut surfedges) = (vec![0], vec![1, -2, 2]);
        f(&mut node, &mut leaf, &mut face, &mut model, &mut marks, &mut surfedges);
        check(&Lumps {
            planes: 1,
            nodes: &[node],
            leafs: &[Leaf { num_marksurfaces: 0, ..LEAF }, leaf],
            marksurfaces: &marks,
            faces: &[face],
            texinfo: &[TexInfo { vecs: [[0.0; 4]; 2], miptex: 0, flags: 0 }],
            surfedges: &surfedges,
            edges: &[[0, 0]; 3],
            models: &[model],
        })
        .is_ok()
    }

    #[test]
    fn references_outside_their_lump_fail() {
        assert!(map(|_, _, _, _, _, _| {}));
        assert!(!map(|n, _, _, _, _, _| n.num_faces = 2));
        assert!(!map(|n, _, _, _, _, _| n.first_face = u32::MAX));
        assert!(!map(|n, _, _, _, _, _| n.children[0] = 1));
        assert!(!map(|n, _, _, _, _, _| n.children[0] = -3));
        assert!(!map(|n, _, _, _, _, _| n.children[1] = i32::MIN));
        assert!(!map(|_, l, _, _, _, _| l.num_marksurfaces = 2));
        assert!(!map(|_, _, _, _, m, _| m[0] = 1));
        assert!(!map(|_, _, f, _, _, _| f.plane = 1));
        assert!(!map(|_, _, f, _, _, _| f.texinfo = 1));
        assert!(!map(|_, _, f, _, _, _| f.num_edges = 4));
        assert!(!map(|_, _, _, _, _, s| s[0] = 3));
        assert!(!map(|_, _, _, _, _, s| s[0] = i32::MIN));
        assert!(!map(|_, _, _, m, _, _| m.first_face = -1));
        assert!(!map(|_, _, _, m, _, _| m.num_faces = 2));
    }

    #[test]
    fn unused_references_pass() {
        assert!(map(|n, _, _, _, _, _| (n.first_face, n.num_faces) = (99, 0)));
        assert!(map(|_, l, _, _, _, _| (l.first_marksurface, l.num_marksurfaces) = (99, 0)));
        assert!(map(|_, _, _, m, _, _| (m.first_face, m.num_faces) = (-1, 0)));
        assert!(map(|_, _, _, _, m, _| m.push(99)));
        assert!(map(|_, _, _, _, _, s| s.push(99)));
    }

    #[test]
    fn bad_edges_are_repaired() {
        let mut e = [[0, 1], [2, u32::MAX]];
        assert!(repair_edges(&mut e, &[[0.0; 3]; 2]).is_ok());
        assert_eq!(e, [[0, 1], [0, 0]]);
        assert!(repair_edges(&mut e, &[]).is_err());
        assert!(repair_edges(&mut [], &[]).is_ok());
    }
}
