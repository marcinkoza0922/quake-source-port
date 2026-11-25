//! MD3 and IQM model uploading for the Vulkan renderer.

use super::*;

type SkinGroups = Vec<(Vec<[u32; 3]>, Vec<f32>)>;

impl Renderer {
    fn upload_surface_skins(
        &mut self,
        up: &mut gpu::Upload,
        spec: Option<&Md3SurfaceSkins>,
    ) -> Result<SkinGroups> {
        let mut skins = Vec::new();
        let Some(ss) = spec else {
            return Ok(skins);
        };
        for (frames, intervals) in &ss.skins {
            let mut imgs = Vec::with_capacity(frames.len());
            for img in frames {
                let bs = self.add_texture(up, img.width, img.height, &[&img.base], TexUse::Base, Sampling::World)?;
                let fs = match &img.fb {
                    Some((w, h, data)) => self.add_texture(up, *w, *h, &[data], TexUse::Glow, Sampling::World)?,
                    None => NO_TEX,
                };
                let cs = match &img.cmap {
                    Some((w, h, data)) => self.add_texture(up, *w, *h, &[data], TexUse::Index, Sampling::Nearest)?,
                    None => NO_TEX,
                };
                imgs.push([bs, fs, cs]);
            }
            skins.push((imgs, intervals.clone()));
        }
        Ok(skins)
    }

    /// Loads an MD3 model with pre-decoded per-surface skins.
    pub fn load_md3(&mut self, md3: &Md3, surface_skins: &[Md3SurfaceSkins]) -> Result<ModelId> {
        self.idle();
        let num_frames = md3.frames.len();
        let mut all_verts = Vec::new();
        let mut all_idx = Vec::new();
        let mut all_poses = Vec::new();
        struct PendingSurf {
            first_index: u32,
            num_indices: u32,
            pose_ofs: u32,
            num_verts: u32,
            skins: SkinGroups,
            holey: bool,
        }
        let mut up = self.gpu.begin_upload()?;
        let mut pending = Vec::with_capacity(md3.surfaces.len());
        for (si, s) in md3.surfaces.iter().enumerate() {
            let first_index = all_idx.len() as u32;
            let (sv, s_idx, sp, used) =
                convert::md3_surface(s, num_frames, all_verts.len() as u32);
            let pose_ofs = all_poses.len() as u32;
            all_verts.extend_from_slice(&sv);
            all_idx.extend_from_slice(&s_idx);
            all_poses.extend_from_slice(&sp);
            let spec = surface_skins.get(si);
            let skins = self.upload_surface_skins(&mut up, spec)?;
            pending.push(PendingSurf {
                first_index,
                num_indices: s_idx.len() as u32,
                pose_ofs,
                num_verts: used,
                skins,
                holey: spec.is_some_and(|ss| ss.holey),
            });
        }
        let (vbuf, ibuf, base) = self.upload_alias_buffers(up, &all_verts, &all_idx, &all_poses)?;
        let surfaces = pending
            .into_iter()
            .map(|p| AliasSurface {
                first_index: p.first_index,
                num_indices: p.num_indices,
                verts_base: base + p.pose_ofs,
                num_verts: p.num_verts,
                skins: p.skins,
                holey: p.holey,
            })
            .collect();
        let frames = (0..num_frames).map(|f| (f as u32, 1, Vec::new())).collect();
        let mut maxs_z: f32 = 0.0;
        let mut radius: f32 = 0.0;
        for f in &md3.frames {
            maxs_z = maxs_z.max(f.bounds[1][2]);
            radius = radius
                .max(f.radius)
                .max(world::dot(f.bounds[0], f.bounds[0]).sqrt())
                .max(world::dot(f.bounds[1], f.bounds[1]).sqrt());
        }
        self.models.push(Model::Alias(Box::new(AliasModel {
            vbuf,
            ibuf,
            surfaces,
            frames,
            scale: [qformats::md3::XYZ_SCALE; 3],
            scale_origin: [0.0; 3],
            radius,
            maxs_z,
            md3: true,
            iqm: false,
            num_bones: 0,
            bone_base: 0,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }

    /// Loads an IQM v2 skeletal model with GPU matrix-palette skinning.
    pub fn load_iqm(&mut self, iqm: &qformats::Iqm, surface_skins: &[Md3SurfaceSkins]) -> Result<ModelId> {
        self.idle();
        let num_frames = iqm.num_frames_effective();
        let num_bones = iqm.num_bones();
        let matrices = iqm.compute_frame_matrices();
        let vert_base = self.alias_cpu.len() as u32;
        let nv = iqm.positions.len();
        let mut packed = Vec::with_capacity(nv * 8 + matrices.len() * 12);
        let mut verts = Vec::with_capacity(nv);
        for i in 0..nv {
            let p = iqm.positions[i];
            let n = iqm.normals.get(i).copied().unwrap_or([0.0, 0.0, 1.0]);
            let bi = iqm.blend_indexes.get(i).copied().unwrap_or([0; 4]);
            let bw = iqm.blend_weights.get(i).copied().unwrap_or([255, 0, 0, 0]);
            let uv = iqm.texcoords.get(i).copied().unwrap_or([0.0; 2]);
            let clamped = bi.map(|b| (b as usize).min(num_bones - 1) as u8);
            verts.push(convert::AliasVertex { vert: vert_base + (i as u32) * 8, st: uv });
            packed.extend_from_slice(&[
                p[0].to_bits(), p[1].to_bits(), p[2].to_bits(),
                n[0].to_bits(), n[1].to_bits(), n[2].to_bits(),
                u32::from_le_bytes(clamped),
                u32::from_le_bytes(bw),
            ]);
        }
        let bone_base = vert_base + (packed.len() as u32);
        for m in &matrices {
            for f in m {
                packed.push(f.to_bits());
            }
        }
        let mut all_idx = Vec::with_capacity(iqm.triangles.len() * 3);
        let mut up = self.gpu.begin_upload()?;
        let mut surfaces = Vec::with_capacity(iqm.meshes.len());
        for (mi, mesh) in iqm.meshes.iter().enumerate() {
            let first_index = all_idx.len() as u32;
            let start = mesh.first_triangle as usize;
            let end = (start + mesh.num_triangles as usize).min(iqm.triangles.len());
            for tri in &iqm.triangles[start..end] {
                all_idx.extend_from_slice(tri);
            }
            let spec = surface_skins.get(mi);
            let skins = self.upload_surface_skins(&mut up, spec)?;
            surfaces.push(AliasSurface {
                first_index,
                num_indices: all_idx.len() as u32 - first_index,
                verts_base: vert_base,
                num_verts: nv as u32,
                skins,
                holey: spec.is_some_and(|ss| ss.holey),
            });
        }
        let (vbuf, ibuf, _) = self.upload_alias_buffers(up, &verts, &all_idx, &packed)?;
        let frames = (0..num_frames).map(|f| (f as u32, 1, Vec::new())).collect();
        let (_lo, hi, radius) = iqm.compute_bounds();
        self.models.push(Model::Alias(Box::new(AliasModel {
            vbuf,
            ibuf,
            surfaces,
            frames,
            scale: [1.0; 3],
            scale_origin: [0.0; 3],
            radius,
            maxs_z: hi[2],
            md3: false,
            iqm: true,
            num_bones: num_bones as u32,
            bone_base,
        })));
        Ok(ModelId(self.models.len() as u32 - 1))
    }
}
