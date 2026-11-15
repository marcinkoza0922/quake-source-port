use super::*;

pub(super) struct FrameRes {
    pub(super) cmd: vk::CommandBuffer,
    pub(super) fence: vk::Fence,
    pub(super) acquire: vk::Semaphore,
    pub(super) dynbuf: Buffer,
    pub(super) set: vk::DescriptorSet,
    pub(super) comp_set: vk::DescriptorSet,
    pub(super) cull_set: vk::DescriptorSet,
}

pub(super) struct Swapchain {
    pub(super) sc: vk::SwapchainKHR,
    pub(super) images: Vec<vk::Image>,
    pub(super) views: Vec<vk::ImageView>,
    pub(super) render_sems: Vec<vk::Semaphore>,
    pub(super) vsync: bool,
    pub(super) format: vk::Format,
    /// TRANSFER_SRC usage is supported (screenshots).
    pub(super) readable: bool,
}

pub(super) struct Targets {
    pub(super) portals: Vec<(Image, u32)>,
    /// Single-sampled scene colour (MSAA resolve target), sampled by post.
    pub(super) color: Image,
    /// Multisampled scene colour when MSAA is on.
    pub(super) color_ms: Option<Image>,
    pub(super) depth: Image,
    /// Scene size in pixels (output size × render scale).
    pub(super) scene_w: u32,
    pub(super) scene_h: u32,
    pub(super) color_slot: u32,
    /// Exact sample-0 depth/alpha snapshot, allocated at AO target size for MSAA SSAO.
    pub(super) ao_source: Option<(Image, u32)>,
    /// Single-sampled visibility/view depth for AO application (output-capped, half-res on low/medium).
    pub(super) ao: Image,
    pub(super) ao_w: u32,
    pub(super) ao_h: u32,
    pub(super) ao_slot: u32,
    /// Single-sampled, scene-sized per-object velocity for post-processing.
    pub(super) vel: Image,
    pub(super) vel_slot: u32,
    /// Headless output image + readback buffer.
    pub(super) out: Option<(Image, Buffer)>,
}

pub(super) struct Tex {
    /// None: slot reserved for a render target owned elsewhere.
    pub(super) image: Option<Image>,
    pub(super) level: bool,
    pub(super) sampling: Sampling,
}

/// How a texture is sampled.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Sampling {
    /// World/model surfaces: follows the texture filter + anisotropy setting.
    World,
    /// Always nearest, repeat (UI pictures, colormap index textures).
    Nearest,
    /// Nearest, clamp to edge (sprites, skybox faces).
    Clamp,
    /// Bilinear, clamp to edge (smooth UI shapes).
    Linear,
}

pub(super) struct WorldGpu {
    pub(super) portals: Vec<crate::portals::Portal>,
    pub(super) cpu: world::World,
    /// Compact immutable metadata for CPU draw construction, including brushes.
    pub(super) draw_surfaces: Vec<DrawSurface>,
    pub(super) vbuf: Buffer,
    pub(super) samples: Buffer,
    pub(super) surfs: Buffer,
    pub(super) materials: Buffer,
    /// Per BSP texture: kind and material flags.
    pub(super) tex_info: Vec<(TextureKind, u32)>,
    pub(super) submodel_ids: Vec<ModelId>,
    pub(super) cull: WorldCull,
    /// Per-frame scratch reused by `gpu_world_draw`.
    pub(super) scratch_bits: Vec<u32>,
    pub(super) scratch_cmds: Vec<vk::DrawIndexedIndirectCommand>,
}

/// Draw construction needs only these four fields of the larger CPU surface.
pub(super) struct DrawSurface {
    pub(super) first_index: u32,
    pub(super) num_indices: u32,
    pub(super) texture: u32,
    pub(super) lm: u32,
}

impl From<&world::Surface> for DrawSurface {
    fn from(s: &world::Surface) -> Self {
        Self { first_index: s.first_index, num_indices: s.num_indices, texture: s.texture, lm: s.lm }
    }
}

/// GPU world culling (cull.comp) resources.
pub(super) struct WorldCull {
    pub(super) marks: Buffer,
    pub(super) surfs: Buffer,
    pub(super) src: Buffer,
    pub(super) surf_frame: Buffer,
    /// Output index buffer per frame in flight.
    pub(super) out: Vec<Buffer>,
    /// Draw commands + lightmap dispatch args per frame in flight
    /// (device-local: cull.comp updates them with atomics).
    pub(super) cmdbuf: Vec<Buffer>,
    pub(super) nmarks: u32,
    /// Per draw command: batch texture and first index of its region.
    pub(super) cmd_tex: Vec<u32>,
    pub(super) cmd_first: Vec<u32>,
    /// Command index ranges: opaque, sky, then liquids of type 1..=4.
    pub(super) cats: [std::ops::Range<u32>; 6],
    pub(super) framecount: u32,
}

pub(super) struct AliasSurface {
    pub(super) first_index: u32,
    pub(super) num_indices: u32,
    pub(super) verts_base: u32,
    pub(super) num_verts: u32,
    /// Per skin group: images as [base, fb, cmap] slots + intervals.
    pub(super) skins: Vec<(Vec<[u32; 3]>, Vec<f32>)>,
    pub(super) holey: bool,
}

pub(super) struct AliasModel {
    pub(super) vbuf: Buffer,
    pub(super) ibuf: Buffer,
    pub(super) surfaces: Vec<AliasSurface>,
    /// (first pose, pose count, intervals)
    pub(super) frames: Vec<convert::FrameInfo>,
    pub(super) scale: [f32; 3],
    pub(super) scale_origin: [f32; 3],
    pub(super) radius: f32,
    pub(super) maxs_z: f32,
    pub(super) md3: bool,
}

pub(super) struct SpriteFrameGpu {
    pub(super) images: Vec<(u32, [f32; 4])>, // slot, (left, right, down, up)
    pub(super) intervals: Vec<f32>,
}

pub(super) struct SpriteModel {
    pub(super) kind: i32,
    pub(super) frames: Vec<SpriteFrameGpu>,
    pub(super) radius: f32,
}

pub(super) enum Model {
    Brush(u32),
    Alias(Box<AliasModel>),
    Sprite(SpriteModel),
}

pub(super) struct PicInfo {
    pub(super) slot: u32,
    pub(super) width: u32,
    pub(super) height: u32,
}
