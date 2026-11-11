//! PVS queries on the world BSP (Mod_PointInLeaf, Mod_LeafPVS, SV_FatPVS).

use std::sync::Arc;

use qworld::{CONTENTS_SOLID, CollisionModel, Vec3};

#[derive(Debug, Default)]
pub struct Vis {
    pub cm: Arc<CollisionModel>,
    pub visdata: Vec<u8>,
    /// Per leaf index (leaf 0 included).
    pub visofs: Vec<i32>,
    /// worldmodel->numleafs (visleafs of submodel 0).
    pub numleafs: usize,
}

impl Vis {
    pub fn new(cm: Arc<CollisionModel>, bsp: &qformats::Bsp) -> Self {
        Vis {
            cm,
            visdata: bsp.visdata.clone(),
            visofs: bsp.leafs.iter().map(|l| l.visofs).collect(),
            numleafs: bsp.models.first().map_or(0, |m| m.visleafs.max(0) as usize),
        }
    }

    pub fn bytes(&self) -> usize {
        self.numleafs.div_ceil(8)
    }

    fn plane_diff(&self, node: usize, p: Vec3) -> f32 {
        let pl = self.cm.hull0.get(node).and_then(|n| self.cm.planes.get(n.plane as usize));
        pl.map_or(0.0, |pl| p.dot(pl.normal) - pl.dist)
    }

    /// Mod_PointInLeaf: leaf index.
    pub fn point_in_leaf(&self, p: Vec3) -> usize {
        let mut n: i32 = 0;
        if self.cm.node_children.is_empty() {
            return 0;
        }
        for _ in 0..100_000 {
            if n < 0 {
                return (-(n + 1)) as usize;
            }
            let Some(ch) = self.cm.node_children.get(n as usize) else { return 0 };
            n = if self.plane_diff(n as usize, p) > 0.0 { ch[0] } else { ch[1] };
        }
        0
    }

    /// Mod_LeafPVS (leaf 0 / no vis: everything visible).
    pub fn leaf_pvs(&self, leaf: usize) -> Vec<u8> {
        if leaf == 0 {
            return vec![0xff; self.bytes()];
        }
        let ofs = self.visofs.get(leaf).copied().unwrap_or(-1);
        qformats::bsp::decompress_vis(&self.visdata, ofs, self.numleafs)
    }

    /// SV_FatPVS: union of the PVS of all leafs within 8 units of `org`.
    pub fn fat_pvs(&self, org: Vec3, out: &mut Vec<u8>) {
        out.clear();
        out.resize(self.bytes(), 0);
        if !self.cm.node_children.is_empty() {
            self.add_fat(org, 0, out, 0);
        }
    }

    fn add_fat(&self, org: Vec3, mut n: i32, out: &mut [u8], depth: u32) {
        if depth > 512 {
            return;
        }
        loop {
            if n < 0 {
                let li = (-(n + 1)) as usize;
                if self.cm.leaf_contents.get(li).is_some_and(|&c| c != CONTENTS_SOLID) {
                    for (o, b) in out.iter_mut().zip(self.leaf_pvs(li)) {
                        *o |= b;
                    }
                }
                return;
            }
            let Some(ch) = self.cm.node_children.get(n as usize).copied() else { return };
            let d = self.plane_diff(n as usize, org);
            if d > 8.0 {
                n = ch[0];
            } else if d < -8.0 {
                n = ch[1];
            } else {
                self.add_fat(org, ch[0], out, depth + 1);
                n = ch[1];
            }
        }
    }
}

/// SV_EdictInPVS on a leaf list.
pub fn leafs_in_pvs(leafs: &[u32], pvs: &[u8]) -> bool {
    leafs.iter().any(|&l| pvs.get(l as usize >> 3).is_some_and(|b| b & (1 << (l & 7)) != 0))
}
