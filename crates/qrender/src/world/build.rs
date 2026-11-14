use super::*;

impl World {
    pub fn build(bsp: &Bsp, lit: Option<&[u8]>) -> World {
        let numleafs = bsp.models.first().map_or(0, |m| m.visleafs.max(0) as usize);
        let has_light = !bsp.lighting.is_empty();
        let mut w = World {
            transparent_liquids: false,
            hidden_surfaces: Default::default(),
            texture_names: bsp.textures.iter().map(|t| t.as_ref().map_or_else(String::new, |t| t.name.clone())).collect(),
            light_cache: Default::default(),
            vertices: Vec::new(),
            indices: Vec::new(),
            surfaces: Vec::with_capacity(bsp.faces.len()),
            lm_surfs: Vec::new(),
            samples: Vec::new(),
            atlas_layers: 1,
            lit: lit.is_some_and(|l| !l.is_empty() && l.len() == bsp.lighting.len() * 3),
            anims: Vec::new(),
            submodels: Vec::new(),
            bsp_nodes: bsp.nodes.clone(),
            planes: bsp.planes.clone(),
            leafs: bsp.leafs.clone(),
            marksurfaces: bsp.marksurfaces.clone(),
            visdata: bsp.visdata.clone(),
            numleafs,
            vis_leaves: Vec::new(),
            vis_leaf: usize::MAX,
            has_light,
            packer: Packer {
                allocated: vec![0; ATLAS as usize],
                layer: 0,
            },
        };
        w.append(bsp, lit, 0);
        if let Some(m) = bsp.models.first() {
            w.bake_liquid(m.first_face.max(0) as usize, m.num_faces.max(0) as usize);
        }
        w
    }

    /// Appends an external brush model (`maps/b_*.bsp`) whose textures start
    /// at `tex_base` in the material table. Its faces share the world's
    /// vertex buffer and lightmap atlas. Returns the index of its model 0 in
    /// `submodels`.
    pub fn add_brush(&mut self, bsp: &Bsp, lit: Option<&[u8]>, tex_base: u32) -> u32 {
        let first = self.submodels.len() as u32;
        self.texture_names.resize(tex_base as usize + bsp.textures.len(), String::new());
        for (i, texture) in bsp.textures.iter().enumerate() {
            self.texture_names[tex_base as usize + i] =
                texture.as_ref().map_or_else(String::new, |t| t.name.clone());
        }
        self.append(bsp, lit, tex_base);
        first
    }

    pub(super) fn append(&mut self, bsp: &Bsp, lit: Option<&[u8]>, tex_base: u32) {
        let w = self;
        let surf_base = w.surfaces.len() as u32;
        let has_light = !bsp.lighting.is_empty();
        let packer = &mut w.packer;
        let lit = lit.filter(|l| l.len() == bsp.lighting.len() * 3);
        let ntex = bsp.textures.len().max(1);
        for f in &bsp.faces {
            let mut s = Surface {
                first_index: w.indices.len() as u32,
                num_indices: 0,
                texture: 0,
                kind: TextureKind::Normal,
                normal: [0.0, 0.0, 1.0],
                dist: 0.0,
                flipped: f.side != 0,
                lm: NO_LM,
                vecs: [[0.0; 4]; 2],
                texturemins: [0; 2],
                extents: [0; 2],
                samples: NO_LM,
                styles: f.styles,
                mins: [f32::MAX; 3],
                maxs: [f32::MIN; 3],
            };
            let (Some(ti), Some(pl)) = (
                bsp.texinfo.get(f.texinfo as usize),
                bsp.planes.get(f.plane as usize),
            ) else {
                w.surfaces.push(s);
                continue;
            };
            s.normal = pl.normal;
            s.dist = pl.dist;
            s.vecs = ti.vecs;
            let ltex = (ti.miptex.max(0) as usize).min(ntex - 1);
            s.texture = ltex as u32 + tex_base;
            let tex = bsp.textures.get(ltex).and_then(|t| t.as_ref());
            s.kind = tex.map_or(TextureKind::Normal, |t| t.kind);
            let mut verts = Vec::with_capacity(f.num_edges as usize);
            for e in 0..f.num_edges {
                let Some(&se) = bsp.surfedges.get((f.first_edge + e) as usize) else {
                    break;
                };
                let edge = bsp
                    .edges
                    .get(se.unsigned_abs() as usize)
                    .copied()
                    .unwrap_or([0, 0]);
                let vi = if se >= 0 { edge[0] } else { edge[1] };
                verts.push(bsp.vertexes.get(vi as usize).copied().unwrap_or([0.0; 3]));
            }
            if verts.len() < 3 {
                w.surfaces.push(s);
                continue;
            }
            // CalcSurfaceExtents (double precision as in ironwail)
            let mut mins = [f64::MAX; 2];
            let mut maxs = [f64::MIN; 2];
            for v in &verts {
                for j in 0..2 {
                    let t = &ti.vecs[j];
                    let val = v[0] as f64 * t[0] as f64
                        + v[1] as f64 * t[1] as f64
                        + v[2] as f64 * t[2] as f64
                        + t[3] as f64;
                    mins[j] = mins[j].min(val);
                    maxs[j] = maxs[j].max(val);
                }
                for (k, &c) in v.iter().enumerate() {
                    s.mins[k] = s.mins[k].min(c);
                    s.maxs[k] = s.maxs[k].max(c);
                }
            }
            let mut lmsize = [0u32; 2];
            for j in 0..2 {
                let bmin = (mins[j] / 16.0).floor() as i32;
                let bmax = (maxs[j] / 16.0).ceil() as i32;
                s.texturemins[j] = bmin * 16;
                s.extents[j] = (bmax - bmin) * 16;
                lmsize[j] = ((bmax - bmin) + 1).max(1) as u32;
            }
            let drawtiled = matches!(s.kind, TextureKind::Sky | TextureKind::Liquid)
                || ti.flags & TEX_SPECIAL != 0;
            let mut atlas_pos = (0u32, 0u32, 0u32);
            if !drawtiled && let Some(p) = packer.alloc(lmsize[0], lmsize[1]) {
                atlas_pos = p;
                let nstyles = f.styles.iter().take_while(|&&st| st != 255).count();
                let n = (lmsize[0] * lmsize[1]) as usize;
                let mut sample_ofs = NO_LM;
                if has_light && f.lightofs >= 0 && nstyles > 0 {
                    let base = f.lightofs as usize;
                    let ok = base + n * nstyles <= bsp.lighting.len();
                    if ok {
                        sample_ofs = w.samples.len() as u32;
                        for i in 0..n * nstyles {
                            let (r, g, b) = match lit {
                                Some(l) => (
                                    l[(base + i) * 3],
                                    l[(base + i) * 3 + 1],
                                    l[(base + i) * 3 + 2],
                                ),
                                None => {
                                    let v = bsp.lighting[base + i];
                                    (v, v, v)
                                }
                            };
                            w.samples
                                .push(r as u32 | (g as u32) << 8 | (b as u32) << 16);
                        }
                    }
                }
                s.samples = sample_ofs;
                s.lm = w.lm_surfs.len() as u32;
                w.lm_surfs.push(LmSurf {
                    sample_ofs,
                    styles: u32::from_le_bytes(f.styles),
                    size: lmsize[0] | lmsize[1] << 16,
                    atlas: p.0 | p.1 << 16,
                    vecs0: ti.vecs[0],
                    vecs1: ti.vecs[1],
                    plane: [pl.normal[0], pl.normal[1], pl.normal[2], pl.dist],
                    tmins: [
                        s.texturemins[0] as f32,
                        s.texturemins[1] as f32,
                        p.2 as f32,
                        0.0,
                    ],
                });
            }
            let base = w.vertices.len() as u32;
            // Quake64 re-release texture shift (not for sky/liquids).
            let tex_div = match bsp.textures.get(ti.miptex.max(0) as usize) {
                Some(Some(t)) if t.shift > 0 && matches!(t.kind, TextureKind::Normal | TextureKind::Fence) => {
                    2.0 * t.shift as f32
                }
                _ => 1.0,
            };
            for v in &verts {
                let st = [0, 1].map(|j| {
                    let t = &ti.vecs[j];
                    v[0] * t[0] + v[1] * t[1] + v[2] * t[2] + t[3]
                });
                let lm = if s.lm != NO_LM {
                    [
                        ((st[0] - s.texturemins[0] as f32 + 8.0) / 16.0 + atlas_pos.0 as f32)
                            / ATLAS as f32,
                        ((st[1] - s.texturemins[1] as f32 + 8.0) / 16.0 + atlas_pos.1 as f32)
                            / ATLAS as f32,
                        atlas_pos.2 as f32,
                    ]
                } else {
                    [0.0; 3]
                };
                let st = st.map(|c| c / tex_div);
                w.vertices.push(WorldVertex { pos: *v, st, lm, liquid: [0.0; 2] });
            }
            for i in 1..verts.len() as u32 - 1 {
                w.indices.extend_from_slice(&[base, base + i, base + i + 1]);
            }
            s.num_indices = (verts.len() as u32 - 2) * 3;
            w.surfaces.push(s);
        }
        w.atlas_layers = packer.layer + 1;
        let anims = build_anims(bsp);
        let tex_count = bsp.textures.len().max(1);
        w.anims.resize(tex_base as usize, (Vec::new(), Vec::new(), false));
        for i in 0..tex_count {
            let (p, a, r) = anims.get(i).cloned().unwrap_or_default();
            let off = |v: Vec<u32>| v.into_iter().map(|x| x + tex_base).collect();
            w.anims.push((off(p), off(a), r));
        }
        for m in &bsp.models {
            w.submodels.push(SubModel {
                first_surface: m.first_face.max(0) as u32 + surf_base,
                num_surfaces: m.num_faces.max(0) as u32,
                mins: m.mins,
                maxs: m.maxs,
            });
        }
    }
}
