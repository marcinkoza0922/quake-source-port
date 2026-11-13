//! RetroArch slang shader presets (`.slangp`) on the final image, run by
//! librashader. The post pass renders into [`Slang::src`] instead of the
//! output, the filter chain draws `src` onto the output, and the UI goes on
//! top (scope 0) or into `src` before the chain (scope 1).

use crate::gpu::{Gpu, Image};
use crate::{Error, FRAMES, Renderer, Result};
use ash::vk;
use librashader::presets::{ShaderFeatures, ShaderPreset, ShaderPresetPack};
use librashader::runtime::vk::{FilterChain, FilterChainOptions, FrameOptions, VulkanImage};
use librashader::runtime::{FilterChainParameters, Size, Viewport};
use std::path::{Path, PathBuf};

/// A `#pragma parameter` of the loaded preset (for menus and configs).
#[derive(Clone, Debug, PartialEq)]
pub struct ShaderParam {
    pub id: String,
    pub description: String,
    /// The shader's default, after the preset's own overrides.
    pub initial: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

pub(crate) struct Slang {
    pub(crate) chain: FilterChain,
    pub(crate) path: PathBuf,
    pub(crate) params: Vec<ShaderParam>,
    /// Chain input: the post pass output at scene size, in the post format.
    pub(crate) src: Option<Image>,
    pub(crate) src_fmt: vk::Format,
    /// Clear history/feedback images on the next frame (new chain or input).
    pub(crate) fresh: bool,
}

impl Slang {
    /// (Re)creates `src` at `w`×`h`; the caller has idled the device if an
    /// old image is replaced.
    pub(crate) fn ensure_src(&mut self, gpu: &mut Gpu, w: u32, h: u32, fmt: vk::Format) -> Result<()> {
        if self.src.as_ref().is_some_and(|s| s.width == w && s.height == h) {
            return Ok(());
        }
        if let Some(old) = self.src.take() {
            gpu.free_image(old);
        }
        self.src_fmt = fmt;
        self.fresh = true;
        self.src = Some(gpu.image(
            w,
            h,
            1,
            1,
            fmt,
            vk::ImageUsageFlags::COLOR_ATTACHMENT | vk::ImageUsageFlags::SAMPLED | vk::ImageUsageFlags::TRANSFER_SRC,
            vk::ImageAspectFlags::COLOR,
        )?);
        Ok(())
    }

    /// Records the chain from `src` (SHADER_READ_ONLY) to `out` (left in
    /// COLOR_ATTACHMENT_OPTIMAL).
    pub(crate) fn record(
        &mut self,
        cmd: vk::CommandBuffer,
        out: vk::Image,
        out_fmt: vk::Format,
        (w, h): (u32, u32),
        frame: u64,
    ) -> Result<()> {
        let Some(src) = &self.src else { return Ok(()) };
        let input = VulkanImage {
            image: src.image,
            size: Size::new(src.width, src.height),
            format: self.src_fmt,
        };
        let vp = Viewport {
            x: 0.0,
            y: 0.0,
            mvp: None,
            output: VulkanImage { image: out, size: Size::new(w, h), format: out_fmt },
            size: Size::new(w, h),
        };
        // SAFETY: src is in SHADER_READ_ONLY_OPTIMAL and out in
        // COLOR_ATTACHMENT_OPTIMAL (the caller's barriers); both outlive the
        // command buffer's execution (freed only after a device idle).
        unsafe { self.chain.frame(&input, &vp, cmd, frame as usize, Some(&FrameOptions { clear_history: std::mem::take(&mut self.fresh), ..Default::default() })) }
            .map_err(|e| Error::new(format!("slang: {e}")))
    }
}

/// Parses a preset and collects its parameters.
fn load_pack(path: &Path) -> std::result::Result<(ShaderPresetPack, Vec<ShaderParam>), String> {
    let preset = ShaderPreset::try_parse(path, ShaderFeatures::empty()).map_err(|e| e.to_string())?;
    let pack = ShaderPresetPack::load_from_preset::<librashader::runtime::vk::error::FilterChainError>(preset)
        .map_err(|e| e.to_string())?;
    let mut params: Vec<ShaderParam> = Vec::new();
    for pass in &pack.passes {
        let mut ps: Vec<_> = pass.data.parameters.values().collect();
        // the hash map loses `#pragma parameter` order; ids sort well enough
        ps.sort_by(|a, b| a.id.cmp(&b.id));
        for p in ps {
            if params.iter().any(|q| q.id == p.id.as_str()) {
                continue;
            }
            let initial = pack
                .parameters
                .iter()
                .find(|m| m.name == p.id)
                .map_or(p.initial, |m| m.value);
            params.push(ShaderParam {
                id: p.id.to_string(),
                description: p.description.clone(),
                initial,
                min: p.minimum,
                max: p.maximum,
                step: p.step,
            });
        }
    }
    Ok((pack, params))
}

impl Renderer {
    /// Loads a `.slangp` preset onto the final image (None = off). Returns
    /// the preset's parameters, or the load/compile error (the old chain is
    /// dropped either way).
    pub fn set_shader_preset(&mut self, path: Option<&Path>) -> std::result::Result<Vec<ShaderParam>, String> {
        self.drop_shader();
        let Some(path) = path else { return Ok(Vec::new()) };
        let (pack, params) = load_pack(path)?;
        let g = &self.gpu;
        let objs = (g.pdev, g.instance.clone(), g.device.clone(), g.queue);
        let opts = FilterChainOptions {
            frames_in_flight: FRAMES as u32,
            force_no_mipmaps: false,
            use_dynamic_rendering: true,
            disable_cache: false,
        };
        self.idle();
        // SAFETY: the device is idle and this thread owns the queue, which
        // the loader submits its uploads to and waits on.
        let chain = unsafe { FilterChain::load_from_pack(pack, objs, Some(&opts)) }.map_err(|e| e.to_string())?;
        self.slang = Some(Slang { chain, path: path.to_path_buf(), params: params.clone(), src: None, src_fmt: vk::Format::UNDEFINED, fresh: true });
        Ok(params)
    }

    /// Path and parameters of the loaded preset.
    pub fn shader_preset(&self) -> Option<(&Path, &[ShaderParam])> {
        self.slang.as_ref().map(|s| (s.path.as_path(), s.params.as_slice()))
    }

    /// Sets a preset parameter; false if the preset has no such parameter.
    pub fn set_shader_param(&mut self, id: &str, value: f32) -> bool {
        let Some(s) = &self.slang else { return false };
        let Some(p) = s.params.iter().find(|p| p.id == id) else { return false };
        s.chain.parameters().set_parameter_value(id, value.clamp(p.min, p.max));
        true
    }

    pub(crate) fn drop_shader(&mut self) {
        if let Some(mut s) = self.slang.take() {
            self.idle();
            if let Some(src) = s.src.take() {
                self.gpu.free_image(src);
            }
        }
    }
}

impl Renderer {
    /// Sizes the chain input to the scene before a frame is recorded.
    pub(crate) fn prepare_shader(&mut self) -> Result<()> {
        let Some(t) = &self.targets else { return Ok(()) };
        let (w, h, fmt) = (t.scene_w, t.scene_h, self.pipes.post_format);
        let Some(s) = self.slang.as_mut() else { return Ok(()) };
        if s.src.as_ref().is_some_and(|i| i.width == w && i.height == h) {
            return Ok(());
        }
        // SAFETY: valid device; the old input may still be in flight.
        let _ = unsafe { self.gpu.device.device_wait_idle() };
        s.ensure_src(&mut self.gpu, w, h, fmt)
    }
}
