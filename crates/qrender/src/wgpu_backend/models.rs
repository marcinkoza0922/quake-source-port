//! Alias (MDL/MD3) and sprite models, skyboxes.

use super::textures::Sampling;
use crate::convert::TexUse;
use super::{NO_TEX, Renderer};
use crate::convert::{self, AliasVertex, FrameInfo};
use crate::{Md3SkinImage, Md3SurfaceSkins, ModelId, Result};
use qformats::{Md3, Mdl, Spr};

pub(super) struct AliasSurface {
    pub(super) first_index: u32,
    pub(super) num_indices: u32,
    /// Range of this surface's mesh vertices in `AliasModel::verts`.
    pub(super) first_vert: u32,
    pub(super) vert_count: u32,
    /// Offset of pose 0 in `AliasModel::poses` and vertices per pose.
    pub(super) pose_base: u32,
    pub(super) num_verts: u32,
    /// Per skin group: images as [base, fb, cmap] slots + intervals.
    pub(super) skins: Vec<(Vec<[u32; 3]>, Vec<f32>)>,
    pub(super) holey: bool,
}

pub(super) struct AliasModel {
    pub(super) verts: Vec<AliasVertex>,
    pub(super) ibuf: wgpu::Buffer,
    pub(super) indices: Vec<u32>,
    pub(super) poses: Vec<u32>,
    pub(super) surfaces: Vec<AliasSurface>,
    pub(super) frames: Vec<FrameInfo>,
    pub(super) scale: [f32; 3],
    pub(super) scale_origin: [f32; 3],
    pub(super) radius: f32,
    pub(super) maxs_z: f32,
    pub(super) md3: bool,
}

pub(super) struct SpriteFrame {
    pub(super) images: Vec<(u32, [f32; 4])>, // slot, (left, right, down, up)
    pub(super) intervals: Vec<f32>,
}

pub(super) struct SpriteModel {
    pub(super) kind: i32,
    pub(super) frames: Vec<SpriteFrame>,
    pub(super) radius: f32,
}

pub(super) enum Model {
    Brush(u32),
    Alias(Box<AliasModel>),
    Sprite(SpriteModel),
}

impl Renderer {
    fn index_buffer(&self, idx: &[u32]) -> wgpu::Buffer {
        let buf = self.dev.create_buffer(&wgpu::BufferDescriptor {
            label: Some("alias indices"),
            size: (idx.len() as u64 * 4).max(4).next_multiple_of(4),
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        if !idx.is_empty() {
            self.queue.write_buffer(&buf, 0, crate::bytes_of(idx));
        }
        buf
    }

    /// Id of world submodel `*n` (n >= 1).
    pub fn submodel(&self, n: usize) -> Option<ModelId> {
        self.world.as_ref()?.submodel_ids.get(n.checked_sub(1)?).copied()
    }

    /// Sets/clears a skybox (faces rt, bk, lf, ft, up, dn).
    pub fn set_skybox(&mut self, faces: Option<&[qformats::image::Rgba; 6]>) -> Result<()> {
        if let Some(old) = self.skybox.take() {
            for s in old {
                self.free_texture(s);
            }
        }
        self.sky_group = None;
        let Some(faces) = faces else { return Ok(()) };
        let mut slots = [NO_TEX; 6];
        for (i, f) in faces.iter().enumerate() {
            slots[i] = self.add_texture(f.width, f.height, &[&f.pixels], TexUse::Sky, Sampling::Clamp)?;
        }
        self.skybox = Some(slots);
        Ok(())
    }

    /// Loads an alias model.
    pub fn load_alias(&mut self, mdl: &Mdl) -> Result<ModelId> {
        self.load_alias_glow(mdl, &[])
    }

    /// Loads an alias model whose skin `k` gets the true-colour RGBA glow
    /// layer `glow[k]` instead of palette fullbrights.
    pub fn load_alias_glow(&mut self, mdl: &Mdl, glow: &[Vec<u8>]) -> Result<ModelId> {
        let (verts, idx) = convert::alias_mesh(mdl);
        let (poses, frames) = convert::alias_poses(mdl);
        let holey = mdl.flags & qformats::mdl::MF_HOLEY != 0;
        let pal = self.palette;
        let mut skins = Vec::new();
        for (k, s) in mdl.skins.iter().enumerate() {
            let mut imgs = Vec::new();
            for img in &s.images {
                let (mut b, mut fb, cm) = convert::alias_skin(&pal, img, holey);
                if let Some(g) = glow.get(k).filter(|g| g.len() == b.len()) {
                    for (t, gl) in b.as_chunks_mut::<4>().0.iter_mut().zip(g.as_chunks::<4>().0) {
                        if gl[3] > 0 {
                            *t = [0, 0, 0, 255];
                        }
                    }
                    fb = Some(g.clone());
                }
                let (w, h) = (mdl.skin_width, mdl.skin_height);
                let bs = self.add_texture(w, h, &[&b], TexUse::Base, Sampling::World)?;
                let fs = match fb {
                    Some(fb) => self.add_texture(w, h, &[&fb], TexUse::Glow, Sampling::World)?,
                    None => NO_TEX,
                };
                let cs = match cm {
                    Some(cm) => self.add_texture(w, h, &[&cm], TexUse::Index, Sampling::Nearest)?,
                    None => NO_TEX,
                };
                imgs.push([bs, fs, cs]);
            }
            skins.push((imgs, s.intervals.clone()));
        }
        let mut maxs_z: f32 = 0.0;
        let mut radius: f32 = mdl.bounding_radius;
        for f in &mdl.frames {
            let a = mdl.decode(f.bboxmin);
            let b = mdl.decode(f.bboxmax);
            maxs_z = maxs_z.max(b[2]);
            radius = radius.max(crate::world::dot(a, a).sqrt()).max(crate::world::dot(b, b).sqrt());
        }
        let ibuf = self.index_buffer(&idx);
        self.models.push(Model::Alias(Box::new(AliasModel {
            surfaces: vec![AliasSurface {
                first_index: 0,
                num_indices: idx.len() as u32,
                first_vert: 0,
                vert_count: verts.len() as u32,
                pose_base: 0,
                num_verts: mdl.num_verts as u32,
                skins,
                holey,
            }],
            verts,
            ibuf,
            indices: idx,
            poses,
            frames,
            scale: mdl.scale,
            scale_origin: mdl.scale_origin,
            radius,
            maxs_z,
            md3: false,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Converts a palette-indexed MD3 skin image into RGBA base, fullbright and
    /// colour-translation map buffers using the renderer palette.
    pub fn md3_indexed_skin(&self, width: u32, height: u32, pixels: &[u8], holey: bool) -> Md3SkinImage {
        let (base, fb, cmap) = convert::alias_skin(&self.palette, pixels, holey);
        Md3SkinImage {
            width,
            height,
            base,
            fb: fb.map(|d| (width, height, d)),
            cmap: cmap.map(|d| (width, height, d)),
        }
    }

    fn md3_skin(&mut self, img: &Md3SkinImage) -> Result<[u32; 3]> {
        let b = self.add_texture(img.width, img.height, &[&img.base], TexUse::Base, Sampling::World)?;
        let f = match &img.fb {
            Some((w, h, d)) => self.add_texture(*w, *h, &[d], TexUse::Glow, Sampling::World)?,
            None => NO_TEX,
        };
        let c = match &img.cmap {
            Some((w, h, d)) => self.add_texture(*w, *h, &[d], TexUse::Index, Sampling::Nearest)?,
            None => NO_TEX,
        };
        Ok([b, f, c])
    }

    /// Loads an MD3 model with pre-decoded per-surface skins.
    pub fn load_md3(&mut self, md3: &Md3, surface_skins: &[Md3SurfaceSkins]) -> Result<ModelId> {
        let num_frames = md3.frames.len();
        let (mut all_verts, mut all_idx, mut all_poses) = (Vec::new(), Vec::new(), Vec::new());
        let mut surfaces = Vec::with_capacity(md3.surfaces.len());
        for (si, s) in md3.surfaces.iter().enumerate() {
            let first_index = all_idx.len() as u32;
            let first_vert = all_verts.len() as u32;
            let (sv, s_idx, sp, used) = convert::md3_surface(s, num_frames, first_vert);
            let pose_base = all_poses.len() as u32;
            all_verts.extend_from_slice(&sv);
            all_idx.extend_from_slice(&s_idx);
            all_poses.extend_from_slice(&sp);
            let spec = surface_skins.get(si);
            let mut skins = Vec::new();
            for (frames, intervals) in spec.map_or(&[][..], |ss| &ss.skins[..]) {
                let mut imgs = Vec::with_capacity(frames.len());
                for img in frames {
                    imgs.push(self.md3_skin(img)?);
                }
                skins.push((imgs, intervals.clone()));
            }
            surfaces.push(AliasSurface {
                first_index,
                num_indices: s_idx.len() as u32,
                first_vert,
                vert_count: sv.len() as u32,
                pose_base,
                num_verts: used,
                skins,
                holey: spec.is_some_and(|ss| ss.holey),
            });
        }
        let frames = (0..num_frames).map(|f| (f as u32, 1, Vec::new())).collect();
        let mut maxs_z: f32 = 0.0;
        let mut radius: f32 = 0.0;
        for f in &md3.frames {
            maxs_z = maxs_z.max(f.bounds[1][2]);
            radius = radius
                .max(f.radius)
                .max(crate::world::dot(f.bounds[0], f.bounds[0]).sqrt())
                .max(crate::world::dot(f.bounds[1], f.bounds[1]).sqrt());
        }
        let ibuf = self.index_buffer(&all_idx);
        self.models.push(Model::Alias(Box::new(AliasModel {
            verts: all_verts,
            ibuf,
            indices: all_idx,
            poses: all_poses,
            surfaces,
            frames,
            scale: [qformats::md3::XYZ_SCALE; 3],
            scale_origin: [0.0; 3],
            radius,
            maxs_z,
            md3: true,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Loads an IQM v2 skeletal model (CPU-skinned into MD3-format poses for wgpu).
    pub fn load_iqm(&mut self, iqm: &qformats::Iqm, surface_skins: &[Md3SurfaceSkins]) -> Result<ModelId> {
        let num_frames = iqm.num_frames_effective();
        let num_bones = iqm.num_bones();
        let matrices = iqm.compute_frame_matrices();
        let nv = iqm.positions.len();
        let verts: Vec<AliasVertex> = (0..nv)
            .map(|i| AliasVertex { vert: i as u32, st: iqm.texcoords.get(i).copied().unwrap_or([0.0; 2]) })
            .collect();
        let mut all_poses = Vec::with_capacity(num_frames * nv * 2);
        let inv_scale = 1.0 / qformats::md3::XYZ_SCALE;
        for f in 0..num_frames {
            let fm = &matrices[f * num_bones..(f + 1) * num_bones];
            for i in 0..nv {
                let p = iqm.positions[i];
                let n = iqm.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
                let bi = iqm.blend_indexes.get(i).copied().unwrap_or([0; 4]);
                let bw = iqm.blend_weights.get(i).copied().unwrap_or([255, 0, 0, 0]);
                let wsum: f32 = bw.iter().map(|&w| w as f32).sum();
                let (sp, sn) = if wsum > 0.0 {
                    let mut m = [0.0f32; 12];
                    for k in 0..4 {
                        let w = bw[k] as f32 / wsum;
                        if w <= 0.0 { continue; }
                        let bm = fm[(bi[k] as usize).min(num_bones - 1)];
                        for c in 0..12 { m[c] += w * bm[c]; }
                    }
                    let op = [
                        m[0] * p[0] + m[1] * p[1] + m[2] * p[2] + m[3],
                        m[4] * p[0] + m[5] * p[1] + m[6] * p[2] + m[7],
                        m[8] * p[0] + m[9] * p[1] + m[10] * p[2] + m[11],
                    ];
                    let on = [
                        m[0] * n[0] + m[1] * n[1] + m[2] * n[2],
                        m[4] * n[0] + m[5] * n[1] + m[6] * n[2],
                        m[8] * n[0] + m[9] * n[1] + m[10] * n[2],
                    ];
                    let nl = (on[0] * on[0] + on[1] * on[1] + on[2] * on[2]).sqrt();
                    (op, if nl > 1e-6 { on.map(|c| c / nl) } else { n })
                } else {
                    (p, n)
                };
                let q = |v: f32| ((v * inv_scale).clamp(-32768.0, 32767.0) as i32 + 32768) as u32;
                let lat = (sn[2].clamp(-1.0, 1.0).acos() * (255.0 / std::f32::consts::TAU)).rem_euclid(256.0) as u32;
                let lng = (sn[1].atan2(sn[0]) * (255.0 / std::f32::consts::TAU)).rem_euclid(256.0) as u32;
                all_poses.push((q(sp[0]) & 0xffff) | ((q(sp[1]) & 0xffff) << 16));
                all_poses.push((q(sp[2]) & 0xffff) | ((lat & 0xff) << 16) | ((lng & 0xff) << 24));
            }
        }
        let mut all_idx = Vec::with_capacity(iqm.triangles.len() * 3);
        let mut surfaces = Vec::with_capacity(iqm.meshes.len());
        for (mi, mesh) in iqm.meshes.iter().enumerate() {
            let first_index = all_idx.len() as u32;
            let start = mesh.first_triangle as usize;
            let end = (start + mesh.num_triangles as usize).min(iqm.triangles.len());
            for tri in &iqm.triangles[start..end] {
                all_idx.extend_from_slice(tri);
            }
            let spec = surface_skins.get(mi);
            let mut skins = Vec::new();
            for (frames, intervals) in spec.map_or(&[][..], |ss| &ss.skins[..]) {
                let mut imgs = Vec::with_capacity(frames.len());
                for img in frames { imgs.push(self.md3_skin(img)?); }
                skins.push((imgs, intervals.clone()));
            }
            surfaces.push(AliasSurface {
                first_index,
                num_indices: all_idx.len() as u32 - first_index,
                first_vert: 0,
                vert_count: nv as u32,
                pose_base: 0,
                num_verts: nv as u32,
                skins,
                holey: spec.is_some_and(|ss| ss.holey),
            });
        }
        let frames = (0..num_frames).map(|f| (f as u32, 1, Vec::new())).collect();
        let (_lo, hi, radius) = iqm.compute_bounds();
        let ibuf = self.index_buffer(&all_idx);
        self.models.push(Model::Alias(Box::new(AliasModel {
            verts,
            ibuf,
            indices: all_idx,
            poses: all_poses,
            surfaces,
            frames,
            scale: [qformats::md3::XYZ_SCALE; 3],
            scale_origin: [0.0; 3],
            radius,
            maxs_z: hi[2],
            md3: true,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Loads a sprite (SPR v1 palette or SPR32).
    pub fn load_sprite(&mut self, spr: &Spr) -> Result<ModelId> {
        let pal = self.palette;
        let mut frames = Vec::new();
        for f in &spr.frames {
            let mut images = Vec::new();
            for im in &f.images {
                let rgba = if spr.is_rgba() { im.pixels.clone() } else { convert::to_rgba(&pal, &im.pixels, Some(255)) };
                let slot = self.add_texture(im.width, im.height, &[&rgba], TexUse::Sprite, Sampling::Clamp)?;
                let left = im.origin[0] as f32;
                let up = im.origin[1] as f32;
                images.push((slot, [left, left + im.width as f32, up - im.height as f32, up]));
            }
            frames.push(SpriteFrame { images, intervals: f.intervals.clone() });
        }
        self.models.push(Model::Sprite(SpriteModel { kind: spr.kind, frames, radius: spr.bounding_radius }));
        Ok(ModelId(self.models.len() as u32 - 1))
    }
}
