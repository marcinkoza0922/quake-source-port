//! Post pass inputs: the scene and motion targets, the palette and the
//! r_softemu palette lookup table (built on the GPU by `shaders/lut.wgsl`),
//! and the post parameters (softemu, colour-blind filter, add-on goggles).

use super::Renderer;
use super::pipes::DrawU;
use crate::Settings;

const LUT_SRC: &str = include_str!("shaders/lut.wgsl");
/// 128³ palette indices, 4 per u32.
const LUT_WORDS: u32 = 128 * 128 * 128 / 4;

#[derive(Default)]
pub(super) struct PostRes {
    /// Softemu lookup table (created on first use) and the metric it holds.
    lut: Option<(wgpu::Buffer, i32)>,
    /// Stand-in for the table while softemu has never been on.
    dummy: Option<wgpu::Buffer>,
    /// LUT compute pipeline and its metric uniform (created on first use).
    compute: Option<(wgpu::ComputePipeline, wgpu::Buffer)>,
}

impl Renderer {
    /// Rebuilds the softemu lookup table when its metric changes (on the GPU,
    /// so the browser doesn't stall on the 2M-cell nearest-colour search).
    pub(super) fn update_lut(&mut self, st: &Settings) {
        let lit = self.world.as_ref().is_some_and(|w| w.cpu.lit);
        let Some(metric) = crate::softemu::lut_metric(st, lit) else { return };
        if self.post.lut.as_ref().is_some_and(|l| l.1 == metric) {
            return;
        }
        let dev = &self.dev;
        let (pipe, ubuf) = self.post.compute.get_or_insert_with(|| {
            let module = dev.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("lut"),
                source: wgpu::ShaderSource::Wgsl(LUT_SRC.into()),
            });
            let pipe = dev.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("lut"),
                layout: None,
                module: &module,
                entry_point: Some("cs_lut"),
                compilation_options: Default::default(),
                cache: None,
            });
            let ubuf = dev.create_buffer(&wgpu::BufferDescriptor {
                label: Some("lut metric"),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            (pipe, ubuf)
        });
        let buf = match self.post.lut.take() {
            Some((b, _)) => b,
            None => {
                // the post bind group points at the stand-in until now
                self.bufs.post_group = None;
                dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("softemu lut"),
                    size: LUT_WORDS as u64 * 4,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
            }
        };
        self.queue.write_buffer(ubuf, 0, crate::bytes_of(&[metric as u32, 0, 0, 0]));
        let group = dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipe.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&self.palette_view) },
                wgpu::BindGroupEntry { binding: 1, resource: buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: ubuf.as_entire_binding() },
            ],
        });
        let mut enc = dev.create_command_encoder(&Default::default());
        {
            let mut pass = enc.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipe);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(LUT_WORDS / 64, 1, 1);
        }
        self.queue.submit([enc.finish()]);
        self.post.lut = Some((buf, metric));
    }

    /// Bind group 1 of the post pass (rebuilt with the scene targets).
    pub(super) fn ensure_post_group(&mut self) {
        let Some(t) = &self.targets else { return };
        if self.bufs.post_group.is_some() {
            return;
        }
        let lut = match &self.post.lut {
            Some((b, _)) => b,
            None => self.post.dummy.get_or_insert_with(|| {
                self.dev.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("softemu lut (unused)"),
                    size: 16,
                    usage: wgpu::BufferUsages::STORAGE,
                    mapped_at_creation: false,
                })
            }),
        };
        // a scaled scene is filtered up/down to the output (vkQuake r_scale)
        let sampler = if t.scene == (self.width, self.height) { &self.samplers.clamp } else { &self.samplers.linear };
        let vel = t.vel.as_ref().map_or(&self.black, |v| v.view());
        self.bufs.post_group = Some(self.dev.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipes.post_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&t.color_view) },
                // motion velocity (black = none)
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(vel) },
                wgpu::BindGroupEntry { binding: 3, resource: wgpu::BindingResource::Sampler(sampler) },
                wgpu::BindGroupEntry { binding: 4, resource: wgpu::BindingResource::TextureView(&self.palette_view) },
                wgpu::BindGroupEntry { binding: 5, resource: lut.as_entire_binding() },
            ],
        }));
    }
}

/// Post pass `DrawU` of one view (see `fs_post` in `shaders/screen.wgsl`).
pub(super) struct PostView {
    pub(super) blend: [f32; 4],
    pub(super) warp: f32,
    /// Scene uv rectangle (u0, v0, u1, v1).
    pub(super) uv: [f32; 4],
    /// Motion blur scale in target uv (x, y); 0 = off.
    pub(super) mb: [f32; 2],
    pub(super) speed_blur: f32,
}

pub(super) fn post_draw(st: &Settings, v: &PostView, out: [u32; 2], render_scale: f32, time: f64) -> DrawU {
    let [w, h] = out.map(|x| x.max(1));
    let g = &st.goggles;
    let mut model = [0.0; 16];
    model[..4].copy_from_slice(&[v.mb[0], v.mb[1], v.speed_blur, st.motion_blur_quality.min(1) as f32]);
    model[4..8].copy_from_slice(&[w as f32, h as f32, crate::softemu::post_dither(st), render_scale]);
    model[8..12].copy_from_slice(&[g.intensity.clamp(0.25, 3.0), g.noise.clamp(0.0, 2.0), (time % 1000.0) as f32, 0.0]);
    DrawU {
        model,
        color: v.blend.map(|c| c.clamp(0.0, 1.0)),
        // z: underwater warp clock (< 0 off); w: output aspect
        params: [st.gamma.clamp(0.1, 4.0), st.contrast.max(0.0), v.warp, w as f32 / h as f32],
        extra: v.uv,
        ix: [crate::softemu::post_mode(st), st.colorblind.min(3), g.mode.min(2), g.quality.min(2)],
        ..Default::default()
    }
}
