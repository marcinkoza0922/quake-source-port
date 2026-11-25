//! Texture slots, samplers, pictures and material bind groups.

use super::{NO_TEX, Renderer};
use crate::convert::{self, Palette, TexUse};
use crate::{Error, PicId, Result};
use qformats::image::Pic;

/// How a texture is sampled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Sampling {
    /// World/model surfaces: nearest or linear by the texture filter setting, repeat.
    World,
    /// Always nearest, repeat (UI pictures, colormap index textures).
    Nearest,
    /// Nearest, clamp to edge (sprites, skybox faces).
    Clamp,
    /// Bilinear, clamp to edge.
    Linear,
}

pub(super) struct Tex {
    pub(super) tex: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    pub(super) sampling: Sampling,
    /// Freed with the level.
    pub(super) level: bool,
}

pub(super) struct PicInfo {
    pub(super) slot: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}

pub(super) struct Samplers {
    pub(super) world: wgpu::Sampler,
    pub(super) nearest: wgpu::Sampler,
    pub(super) clamp: wgpu::Sampler,
    pub(super) linear: wgpu::Sampler,
}

fn sampler(dev: &wgpu::Device, linear: bool, repeat: bool, mips: bool) -> wgpu::Sampler {
    let filter = if linear { wgpu::FilterMode::Linear } else { wgpu::FilterMode::Nearest };
    let mode = if repeat { wgpu::AddressMode::Repeat } else { wgpu::AddressMode::ClampToEdge };
    dev.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: mode,
        address_mode_v: mode,
        address_mode_w: mode,
        mag_filter: filter,
        min_filter: filter,
        mipmap_filter: if mips { wgpu::MipmapFilterMode::Linear } else { wgpu::MipmapFilterMode::Nearest },
        ..Default::default()
    })
}

impl Samplers {
    pub(super) fn new(dev: &wgpu::Device, world_linear: bool) -> Self {
        Self {
            world: sampler(dev, world_linear, true, true),
            nearest: sampler(dev, false, true, false),
            clamp: sampler(dev, false, false, false),
            linear: sampler(dev, true, false, false),
        }
    }

    pub(super) fn get(&self, s: Sampling) -> &wgpu::Sampler {
        match s {
            Sampling::World => &self.world,
            Sampling::Nearest => &self.nearest,
            Sampling::Clamp => &self.clamp,
            Sampling::Linear => &self.linear,
        }
    }
}

/// Creates an RGBA8 texture and uploads `mips` (mip levels whose size does
/// not match the chain are dropped).
pub(super) fn make_texture(
    dev: &wgpu::Device,
    queue: &wgpu::Queue,
    w: u32,
    h: u32,
    mips: &[&[u8]],
) -> (wgpu::Texture, wgpu::TextureView) {
    let (w, h) = (w.max(1), h.max(1));
    let mut levels = 0;
    for (i, m) in mips.iter().enumerate() {
        let (mw, mh) = ((w >> i).max(1), (h >> i).max(1));
        if m.len() < (mw * mh * 4) as usize || (i > 0 && (w >> i == 0 || h >> i == 0)) {
            break;
        }
        levels = i as u32 + 1;
    }
    let levels = levels.max(1);
    let tex = dev.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        mip_level_count: levels,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    for (i, m) in mips.iter().take(levels as usize).enumerate() {
        let (mw, mh) = ((w >> i).max(1), (h >> i).max(1));
        write_rows(queue, &tex, i as u32, [0, 0, 0], mw, mh, &m[..(mw * mh * 4) as usize]);
    }
    let view = tex.create_view(&Default::default());
    (tex, view)
}

/// Writes a `w`×`h` RGBA8 region at `origin` (x, y, layer).
pub(super) fn write_rows(queue: &wgpu::Queue, tex: &wgpu::Texture, mip: u32, origin: [u32; 3], w: u32, h: u32, data: &[u8]) {
    if w == 0 || h == 0 {
        return;
    }
    queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: tex,
            mip_level: mip,
            origin: wgpu::Origin3d { x: origin[0], y: origin[1], z: origin[2] },
            aspect: wgpu::TextureAspect::All,
        },
        data,
        wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * 4), rows_per_image: Some(h) },
        wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
}

pub(super) fn solid(dev: &wgpu::Device, queue: &wgpu::Queue, c: [u8; 4]) -> wgpu::TextureView {
    make_texture(dev, queue, 1, 1, &[&c]).1
}

/// The palette as a 256x1 texture (colormap ramps in the alias shader).
pub(super) fn palette_texture(dev: &wgpu::Device, queue: &wgpu::Queue, pal: &Palette) -> wgpu::TextureView {
    let px: Vec<u8> = pal.iter().flat_map(|c| [c[0], c[1], c[2], 255]).collect();
    make_texture(dev, queue, 256, 1, &[&px]).1
}

/// 8x8 soft round dot for classic particles.
pub(super) fn particle_dot() -> Vec<u8> {
    let mut px = Vec::with_capacity(256);
    for y in 0..8 {
        for x in 0..8 {
            let (dx, dy) = (x as f32 - 3.5, y as f32 - 3.5);
            let a = if dx * dx + dy * dy <= 13.0 { 255 } else { 0 };
            px.extend_from_slice(&[255, 255, 255, a]);
        }
    }
    px
}

impl Renderer {
    fn alloc_slot(&mut self) -> u32 {
        if let Some(i) = self.textures.iter().position(|t| t.is_none()) {
            return i as u32;
        }
        self.textures.push(None);
        self.textures.len() as u32 - 1
    }

    /// Creates an RGBA8 texture from a mip chain in a new slot, shrunk when
    /// it's over the size limit.
    pub(super) fn add_texture(&mut self, w: u32, h: u32, mips: &[&[u8]], usage: TexUse, sampling: Sampling) -> Result<u32> {
        let limit = usage.limit(self.max_texture_size, self.dev.limits().max_texture_dimension_2d);
        let fitted = convert::fit(w, h, mips, limit, usage);
        let refs: Vec<&[u8]>;
        let (w, h, mips) = match &fitted {
            Some((w, h, m)) => {
                refs = m.iter().map(|m| &**m).collect();
                (*w, *h, &refs[..])
            }
            None => (w, h, mips),
        };
        let slot = self.alloc_slot();
        let (tex, view) = make_texture(&self.dev, &self.queue, w, h, mips);
        self.textures[slot as usize] = Some(Tex { tex, view, sampling, level: usage.level() });
        Ok(slot)
    }

    /// Frees a slot and drops every bind group that may reference it.
    pub(super) fn free_texture(&mut self, slot: u32) {
        if let Some(t) = self.textures.get_mut(slot as usize).and_then(|t| t.take()) {
            t.tex.destroy();
            self.bind_groups.clear();
            self.sky_group = None;
        }
    }

    fn view_or<'a>(&'a self, slot: u32, fallback: &'a wgpu::TextureView) -> &'a wgpu::TextureView {
        self.textures
            .get(slot as usize)
            .and_then(|t| t.as_ref())
            .map_or(fallback, |t| &t.view)
    }

    /// Ensures the material bind group for (base, extra, extra) slots exists.
    pub(super) fn ensure_material(&mut self, key: [u32; 3]) {
        if self.bind_groups.contains_key(&key) {
            return;
        }
        let sampling = self
            .textures
            .get(key[0] as usize)
            .and_then(|t| t.as_ref())
            .map_or(Sampling::Nearest, |t| t.sampling);
        let views = [
            self.view_or(key[0], &self.white),
            self.view_or(key[1], &self.black),
            self.view_or(key[2], &self.black),
        ];
        let bg = self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipes.mat_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(views[0]) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(views[1]) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::TextureView(views[2]) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(self.samplers.get(sampling)) },
            ],
        });
        self.bind_groups.insert(key, bg);
    }

    /// Ensures the sky bind group (skybox faces, or the two layers in slots 4, 5).
    pub(super) fn ensure_sky(&mut self) {
        if self.sky_group.is_some() {
            return;
        }
        let slots = match self.skybox {
            Some(s) => s,
            None => [NO_TEX, NO_TEX, NO_TEX, NO_TEX, self.sky_layers[0], self.sky_layers[1]],
        };
        let views: Vec<&wgpu::TextureView> = slots.iter().map(|&s| self.view_or(s, &self.black)).collect();
        let mut entries: Vec<wgpu::BindGroupEntry> = views
            .iter()
            .enumerate()
            .map(|(i, v)| wgpu::BindGroupEntry { binding: i as u32, resource: wgpu::BindingResource::TextureView(v) })
            .collect();
        entries.push(wgpu::BindGroupEntry { binding: 6, resource: wgpu::BindingResource::Sampler(&self.samplers.world) });
        self.sky_group = Some(self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipes.sky_bgl,
            entries: &entries,
        }));
    }

    /// Rebuilds samplers when world texture filtering changes.
    pub(super) fn set_world_filter(&mut self, linear: bool) {
        if linear != self.world_linear {
            self.world_linear = linear;
            self.samplers = Samplers::new(&self.dev, linear);
            self.bind_groups.clear();
            self.sky_group = None;
        }
    }

    /// Registers a 2D picture (index 255 transparent). Persists across maps.
    pub fn register_pic(&mut self, pic: &Pic) -> Result<PicId> {
        let rgba = convert::to_rgba(&self.palette, &pic.pixels, Some(255));
        self.register_pic_rgba(pic.width, pic.height, &rgba)
    }

    pub fn register_pic_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> {
        self.register_pic_sampled(width, height, rgba, Sampling::Nearest)
    }

    /// Like [`register_pic_rgba`](Self::register_pic_rgba) but bilinear filtered.
    pub fn register_pic_rgba_linear(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<PicId> {
        self.register_pic_sampled(width, height, rgba, Sampling::Linear)
    }

    fn register_pic_sampled(&mut self, width: u32, height: u32, rgba: &[u8], sampling: Sampling) -> Result<PicId> {
        let slot = self.add_texture(width, height, &[rgba], TexUse::Pic, sampling)?;
        self.pics.push(PicInfo { slot, width, height });
        Ok(PicId(self.pics.len() as u32 - 1))
    }

    /// Sets conchars (128×128 indices, 0 transparent) used by [`crate::Draw2D::Char`].
    pub fn set_conchars(&mut self, pixels: &[u8]) -> Result<PicId> {
        let rgba = convert::to_rgba(&self.palette, pixels, Some(0));
        let id = self.register_pic_rgba(128, 128, &rgba)?;
        self.conchars = Some(id);
        Ok(id)
    }

    /// Updates the stored dimensions of a picture; the next
    /// [`Self::update_pic_rows`] recreates its texture at the new size.
    pub fn resize_pic(&mut self, pic: PicId, width: u32, height: u32) -> Result<()> {
        let info = self.pics.get_mut(pic.0 as usize).ok_or_else(|| Error::new("no such picture"))?;
        info.width = width;
        info.height = height;
        Ok(())
    }

    /// Replaces a picture's texels (same size it was registered with).
    pub fn update_pic(&mut self, pic: PicId, rgba: &[u8]) -> Result<()> {
        self.update_pic_rows(pic, rgba, u32::MAX)
    }

    /// Like [`Self::update_pic`], but only the first `rows` rows of `rgba`
    /// are uploaded and the rest of the picture becomes transparent black.
    pub fn update_pic_rows(&mut self, pic: PicId, rgba: &[u8], rows: u32) -> Result<()> {
        let &PicInfo { slot, width, height } = self.pics.get(pic.0 as usize).ok_or_else(|| Error::new("no such picture"))?;
        if width.max(height) > self.dev.limits().max_texture_dimension_2d {
            return Err(Error::new("picture larger than the GPU supports"));
        }
        let rows = rows.min(height);
        let mut data = rgba[..((width * rows * 4) as usize).min(rgba.len())].to_vec();
        data.resize((width * height * 4) as usize, 0);
        let same = self.textures.get(slot as usize).and_then(|t| t.as_ref()).is_some_and(|t| {
            let s = t.tex.size();
            (s.width, s.height) == (width.max(1), height.max(1)) && t.tex.mip_level_count() == 1
        });
        if same {
            let t = self.textures[slot as usize].as_ref().map(|t| t.tex.clone());
            if let Some(t) = t {
                write_rows(&self.queue, &t, 0, [0, 0, 0], width, height, &data);
            }
            return Ok(());
        }
        let sampling = self.textures.get(slot as usize).and_then(|t| t.as_ref()).map_or(Sampling::Nearest, |t| t.sampling);
        self.free_texture(slot);
        let (tex, view) = make_texture(&self.dev, &self.queue, width, height, &[&data]);
        if (slot as usize) >= self.textures.len() {
            self.textures.resize_with(slot as usize + 1, || None);
        }
        self.textures[slot as usize] = Some(Tex { tex, view, sampling, level: false });
        Ok(())
    }
}
