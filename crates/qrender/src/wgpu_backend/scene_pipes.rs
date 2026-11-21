//! The scene pipelines (world, liquids, sky, alias models, polygons and the
//! screen-space effects that read the scene), rebuilt per MSAA sample count
//! and bloom glow attachment.

use super::SCENE_FORMAT;
use super::pipes::{ALIAS_ATTRS, AliasV, Desc, POLY_ATTRS, WORLD_ATTRS, blend_of, pipeline, pipeline_with, poly_index, vb};
use crate::FteBlend;

/// Pipelines drawing into the (possibly multisampled) scene target.
pub(super) struct ScenePipes {
    pub(super) samples: u32,
    /// Built with the bloom glow attachment (`*_glow` fragment entry points).
    pub(super) glow: bool,
    pub(super) world: wgpu::RenderPipeline,
    pub(super) world_alpha: wgpu::RenderPipeline,
    pub(super) water: wgpu::RenderPipeline,
    pub(super) water_alpha: wgpu::RenderPipeline,
    pub(super) sky: wgpu::RenderPipeline,
    pub(super) alias: wgpu::RenderPipeline,
    pub(super) alias_alpha: wgpu::RenderPipeline,
    /// Thermal goggles: alias models behind the scene depth, additive.
    pub(super) alias_xray: wgpu::RenderPipeline,
    /// FTE blend modes × (triangles, lines).
    pub(super) poly: Vec<wgpu::RenderPipeline>,
    /// SSAO and motion-velocity pipelines (read the scene targets).
    pub(super) fx: Option<super::fx_pipes::FxPipes>,
}

impl ScenePipes {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        dev: &wgpu::Device,
        frame_bgl: &wgpu::BindGroupLayout,
        mat_bgl: &wgpu::BindGroupLayout,
        draw_bgl: &wgpu::BindGroupLayout,
        scene_layout: &wgpu::PipelineLayout,
        sky_layout: &wgpu::PipelineLayout,
        scene: &wgpu::ShaderModule,
        sky_m: &wgpu::ShaderModule,
        samples: u32,
        glow: bool,
        scene_depth_sampling: bool,
    ) -> ScenePipes {
        let fs = |name: &'static str| -> &'static str {
            if !glow {
                return name;
            }
            match name {
                "fs_world" => "fs_world_glow",
                "fs_water" => "fs_water_glow",
                "fs_alias" => "fs_alias_glow",
                "fs_sky" => "fs_sky_glow",
                _ => "fs_poly_glow",
            }
        };
        let alpha = Some(wgpu::BlendState::ALPHA_BLENDING);
        let tri = wgpu::PrimitiveTopology::TriangleList;
        let world_vb = [vb::<crate::world::WorldVertex>(&WORLD_ATTRS)];
        let alias_vb = [vb::<AliasV>(&ALIAS_ATTRS)];
        let poly_vb = [vb::<crate::FteVertex>(&POLY_ATTRS)];
        let scene_pipe = |vs, fs_name, buffers, blend, write| {
            pipeline(dev, Desc {
                layout: scene_layout,
                module: scene,
                vs,
                fs: fs(fs_name),
                buffers,
                topology: tri,
                blend,
                depth: Some((write, true)),
                format: SCENE_FORMAT,
                samples,
                glow,
            })
        };
        let world = scene_pipe("vs_world", "fs_world", &world_vb, None, true);
        let world_alpha = scene_pipe("vs_world", "fs_world", &world_vb, alpha, false);
        let water = scene_pipe("vs_world", "fs_water", &world_vb, None, true);
        let water_alpha = scene_pipe("vs_world", "fs_water", &world_vb, alpha, false);
        let alias = scene_pipe("vs_alias", "fs_alias", &alias_vb, None, true);
        let alias_alpha = scene_pipe("vs_alias", "fs_alias", &alias_vb, alpha, false);
        let add = wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::One, operation: wgpu::BlendOperation::Add };
        let alias_xray = pipeline_with(
            dev,
            Desc {
                layout: scene_layout,
                module: scene,
                vs: "vs_alias",
                fs: fs("fs_alias"),
                buffers: &alias_vb,
                topology: tri,
                blend: Some(wgpu::BlendState { color: add, alpha: add }),
                depth: Some((false, true)),
                format: SCENE_FORMAT,
                samples,
                glow,
            },
            wgpu::CompareFunction::Greater,
            // alias triangles wind clockwise here (Vulkan flips y): keep front faces
            Some(wgpu::Face::Front),
        );
        let sky = pipeline(dev, Desc {
            layout: sky_layout,
            module: sky_m,
            vs: "vs_sky",
            fs: fs("fs_sky"),
            buffers: &world_vb,
            topology: tri,
            blend: None,
            depth: Some((true, true)),
            format: SCENE_FORMAT,
            samples,
            glow,
        });
        let mut poly = Vec::new();
        // Overlay (debug lines) last: drawn without depth testing, as in Vulkan
        for b in FteBlend::ALL.into_iter().chain([FteBlend::Overlay]) {
            for lines in [false, true] {
                debug_assert_eq!(poly.len(), poly_index(b, lines));
                poly.push(pipeline(dev, Desc {
                    layout: scene_layout,
                    module: scene,
                    vs: "vs_poly",
                    fs: fs("fs_poly"),
                    buffers: &poly_vb,
                    topology: if lines { wgpu::PrimitiveTopology::LineList } else { tri },
                    blend: Some(blend_of(b)),
                    depth: Some((false, b != FteBlend::Overlay)),
                    format: SCENE_FORMAT,
                    samples,
                    glow,
                }));
            }
        }
        let fx = scene_depth_sampling.then(|| {
            super::fx_pipes::FxPipes::new(dev, [frame_bgl, mat_bgl, draw_bgl], samples, glow)
        });
        ScenePipes {
            samples,
            glow,
            world,
            world_alpha,
            water,
            water_alpha,
            sky,
            alias,
            alias_alpha,
            alias_xray,
            poly,
            fx,
        }
    }
}
