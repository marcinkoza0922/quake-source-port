use super::*;

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct FrameUbo {
    pub(super) portal_clip: [f32; 4],
    pub(super) portal_plane: [[f32; 4]; 4],
    pub(super) portal_min: [[f32; 4]; 4],
    pub(super) portal_max: [[f32; 4]; 4],
    pub(super) viewproj: Mat4,
    pub(super) eye: [f32; 4],
    pub(super) fog: [f32; 4],
    pub(super) vright: [f32; 4],
    pub(super) vup: [f32; 4],
    pub(super) vfwd: [f32; 4],
    /// x skyfog, y skybox flag, z sky layer alpha, w particle style (r_particles).
    pub(super) sky: [f32; 4],
    pub(super) skybox0: [u32; 4],
    pub(super) skybox1: [u32; 4],
    pub(super) alphas: [f32; 4],
    pub(super) soft: [f32; 4],
    /// x world dither level, y alias mode, z goggles mode, w caustic texture slot.
    pub(super) softf: [u32; 4],
    /// x ripple strength, y caustics strength, z ripple count, w water quality (0..=1).
    pub(super) water: [f32; 4],
    /// xy gl_FragCoord -> portal UV scale, zw valid UV clamp max.
    pub(super) portal_uv: [f32; 4],
    /// xy scene px per vel px, zw scene max texel coords.
    pub(super) vel_uv: [f32; 4],
    /// Skybox wind: xyz direction (world), w loop phase.
    pub(super) wind: [f32; 4],
    /// xy centre, z liquid surface height, w start time.
    pub(super) ripples: [[f32; 4]; crate::water::MAX_RIPPLES],
    pub(super) ripple_amp: [[f32; 4]; crate::water::MAX_RIPPLES / 4],
    /// x: planar reflection strength (r_water_reflections_amount), 0 = off.
    pub(super) refl: [f32; 4],
    /// Number of active teleporter impact ripples in x.
    pub(super) teleport_fx: [u32; 4],
    /// World xyz and the time of the impact.
    pub(super) teleport_ripples: [[f32; 4]; crate::MAX_TELEPORT_RIPPLES],
    /// x: deluxemap atlas layer offset (0 = off), y: HDR lightmap tonemap (0/1),
    /// z: deluxemap specular strength, w: deluxemap normal modulation strength.
    pub(super) hdr_deluxe: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct AliasPush {
    pub(super) model: Mat4,
    pub(super) light: [f32; 4],
    pub(super) shadevec: [f32; 4],
    pub(super) poses: [u32; 4],
    pub(super) texs: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct WorldPush {
    pub(super) model: Mat4,
    pub(super) params: [f32; 4],
}

/// Velocity pass push constants (velalias.vert / velworld.vert): current
/// model matrix, rows of last frame's model transform, and
/// [pose1, pose2, blend bits, MSAA samples] (world: only samples is used).
#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct VelPush {
    pub(super) model: Mat4,
    pub(super) prev: [[f32; 4]; 3],
    pub(super) misc: [u32; 4],
}

/// A moving entity drawn into the velocity target.
pub(super) enum VelDraw {
    Alias(usize, [[f32; 4]; 3]),
    World(usize, [[f32; 4]; 3]),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct AoSourcePush {
    pub(super) scene_per_ao: [f32; 2],
    pub(super) scene_max: [i32; 2],
    pub(super) msaa: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct AoDownPush {
    pub(super) src: u32,
    pub(super) src_mip: i32,
    pub(super) src_max: [i32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct AoPush {
    pub(super) proj: [f32; 4],
    pub(super) size: [f32; 4],
    pub(super) scene_per_ao: [f32; 2],
    pub(super) scene_max: [i32; 2],
    pub(super) quality: u32,
    pub(super) color_slot: u32,
    pub(super) max_mip: i32,
    pub(super) _pad: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct DropletsPush {
    pub(super) age: f32,
    pub(super) seed: u32,
    pub(super) high: u32,
    pub(super) aspect: f32,
    pub(super) px: f32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct AoApplyPush {
    pub(super) ao_tex: u32,
    pub(super) msaa: u32,
    pub(super) quality: u32,
    pub(super) _pad: u32,
    pub(super) ao_per_scene: [f32; 2],
    pub(super) ao_max: [i32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct PolyPush {
    pub(super) color: [f32; 4],
    pub(super) tex: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct FtePush {
    pub(super) fogmix: [f32; 4],
    pub(super) tex: u32,
    pub(super) depth_tex: u32,
    pub(super) depth_uv: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct PostPush {
    pub(super) blend: [f32; 4],
    pub(super) gamma: f32,
    pub(super) contrast: f32,
    pub(super) scene: u32,
    pub(super) dither: f32,
    pub(super) mode: u32,
    pub(super) scale: f32,
    pub(super) colorblind: u32,
    pub(super) vel_tex: u32,
    pub(super) mbx: f32,
    pub(super) mby: f32,
    pub(super) uvs: [f32; 2],
    /// Add-on goggles: mode (0 off, 1 night vision, 2 thermal), gain, noise, time.
    pub(super) goggles: u32,
    pub(super) g_gain: f32,
    pub(super) g_noise: f32,
    pub(super) g_time: f32,
    pub(super) speed_blur: f32,
    pub(super) mb_quality: u32,
    pub(super) mb_w: f32,
    pub(super) mb_h: f32,
    pub(super) g_quality: u32,
    /// Software-style underwater ripple clock in seconds; negative = off.
    pub(super) warp: f32,
    /// r_bloom: chain level 0 slot, strength (0 = off) and view UV scale.
    pub(super) bloom_tex: u32,
    pub(super) bloom: f32,
    pub(super) bloom_uv: [f32; 2],
    /// Enhanced Water screen effects (`WaterScreen::packed`) and droplet age
    /// (negative = none); the clock is `g_time`.
    pub(super) water: u32,
    pub(super) drops: f32,
}

// The graphics layout has a 128-byte push constant range.
const _: () = assert!(std::mem::size_of::<PostPush>() == 128);

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct UiVert {
    pub(super) pos: [f32; 2],
    pub(super) uv: [f32; 2],
    pub(super) color: u32,
    pub(super) tex: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct LmDyn {
    pub(super) styles: [f32; 256],
    pub(super) num_dlights: u32,
    pub(super) pad: [u32; 3],
    pub(super) dl: [[f32; 8]; 64],
}

/// Bump writer into the mapped per-frame buffer.
pub(super) struct Dyn {
    pub(super) ptr: *mut u8,
    pub(super) ofs: u64,
    pub(super) cap: u64,
}

impl Dyn {
    pub(super) fn push<T: Copy>(&mut self, data: &[T], align: u64) -> Option<u64> {
        let b = bytes_of(data);
        let o = self.ofs.next_multiple_of(align);
        if o + b.len() as u64 > self.cap {
            return None;
        }
        // SAFETY: in bounds of the mapped buffer; the frame's fence was waited on.
        unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), self.ptr.add(o as usize), b.len()) };
        self.ofs = o + b.len() as u64;
        Some(o)
    }
    pub(super) fn push_slices<T: Copy>(&mut self, slices: &[&[T]], align: u64) -> Option<u64> {
        let total: usize = slices.iter().map(|s| std::mem::size_of_val(*s)).sum();
        let o = self.ofs.next_multiple_of(align);
        if o + total as u64 > self.cap {
            return None;
        }
        let mut dst = o as usize;
        for s in slices {
            let b = bytes_of(s);
            if !b.is_empty() {
                // SAFETY: total bounds checked above; frame fence waited on.
                unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), self.ptr.add(dst), b.len()) };
                dst += b.len();
            }
        }
        self.ofs = o + total as u64;
        Some(o)
    }
    pub(super) fn alloc_uninit<T: Copy>(&mut self, count: usize, align: u64) -> Option<(u64, *mut T)> {
        let bytes = count * std::mem::size_of::<T>();
        let o = self.ofs.next_multiple_of(align.max(std::mem::align_of::<T>() as u64));
        if o + bytes as u64 > self.cap {
            return None;
        }
        self.ofs = o + bytes as u64;
        // SAFETY: aligned and within mapped dynbuf capacity; frame fence waited on.
        Some((o, unsafe { self.ptr.add(o as usize) as *mut T }))
    }
    pub(super) fn write_at<T: Copy>(&mut self, ofs: u64, data: &[T]) {
        let b = bytes_of(data);
        assert!(ofs + b.len() as u64 <= self.cap);
        // SAFETY: bounds asserted; frame fence waited on.
        unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), self.ptr.add(ofs as usize), b.len()) };
    }
}

/// FTE draws: vertex and index offsets, then (pipeline, texture slot,
/// batch, shader mode). Mode is the blend index, or [`GUN_PORTAL_MODE`].
pub(super) type FteDraw = (u64, u64, Vec<(usize, u32, crate::FteBatch, f32)>);
/// Instanced normal FTE billboards: quad buffer offset in dynbuf, then
/// (blend index 0..8, texture slot, batch, shader mode).
pub(super) type FteQuadDraw = (u64, Vec<(usize, u32, crate::FteBatch, f32)>);
/// `fte.frag` mode for add-on portal ovals (screen-space view texture).
pub(super) const GUN_PORTAL_MODE: f32 = 8.0;

#[derive(Clone, Copy, Default)]
pub(super) struct Range {
    pub(super) ofs: u64,
    pub(super) count: u32,
}

#[derive(Clone, Copy)]
pub(super) struct WorldDraw {
    pub(super) model: Mat4,
    /// Brush entity: drawn with a depth bias toward the viewer so it wins
    /// over coplanar world faces (vkQuake gl_zfix).
    pub(super) zfix: bool,
    /// True when visible in the main view (false for portal-only offscreen brushes).
    pub(super) main_visible: bool,
    pub(super) alpha: f32,
    pub(super) opaque: Range,
    /// Prefix of `opaque` whose texture (or animation) has a fullbright mask.
    pub(super) opaque_fb: Range,
    pub(super) sky: Range,
    /// Liquids, one range per type for the GPU-culled world (CPU path: [0]).
    pub(super) water: [Range; 4],
    pub(super) water_alpha: [Range; 4],
    /// Index buffer the commands refer to (dynbuf when None).
    pub(super) ibuf: Option<vk::Buffer>,
    /// Buffer holding the indirect commands (dynbuf when None).
    pub(super) cbuf: Option<vk::Buffer>,
    /// Brush entity bounding sphere (centre, radius) for shadow casting;
    /// radius 0 for the world.
    pub(super) bounds: [f32; 4],
    pub(super) polys: u32,
    pub(super) triangles: u32,
}

/// Per-frame inputs of cull.comp, filled while building the scene.
pub(super) struct CullFrame {
    pub(super) push: CullPush,
    pub(super) groups: u32,
    /// Commands, dispatch args and leaf bits staged in dynbuf, copied to `cmdbuf`.
    pub(super) stage_ofs: u64,
    pub(super) stage_len: u64,
    /// Lightmap dispatch args (x, y, z): offset in dynbuf's staging copy
    /// and in `cmdbuf`.
    pub(super) disp_ofs: u64,
    pub(super) cmdbuf: vk::Buffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct CullPush {
    pub(super) frustum: [[f32; 4]; 4],
    pub(super) eye: [f32; 4],
    pub(super) framecount: u32,
    pub(super) nmarks: u32,
    pub(super) ncmds: u32,
    pub(super) ofs_vis: u32,
    pub(super) ofs_lm: u32,
    pub(super) pad: u32,
    pub(super) max_lm: u32,
}

pub(super) struct AliasDraw {
    pub(super) model: usize,
    pub(super) first_index: u32,
    pub(super) num_indices: u32,
    pub(super) push: AliasPush,
    pub(super) origin: [f32; 3],
    pub(super) radius: f32,
    pub(super) main_visible: bool,
    pub(super) viewmodel: bool,
    /// Drawn only into portal views.
    pub(super) portal_only: bool,
    /// `RenderEntity::key`: no shadow from the dynamic light it emits.
    pub(super) key: u32,
}

pub(super) struct SpriteDraw {
    pub(super) vofs: u64,
    pub(super) tex: u32,
    pub(super) alpha: f32,
}

pub(super) enum Trans {
    World(usize),
    Alias(usize),
    Sprite(usize),
}

/// One view's draw lists for a frame (several with split-screen).
pub(super) struct ViewPass {
    pub(super) view: Option<ViewSetup>,
    /// Output rectangle (x, y, w, h) in output pixels.
    pub(super) rect: [u32; 4],
    pub(super) blend: [f32; 4],
    pub(super) speed_blur: f32,
    /// Screen ripple clock (WaterWarp::Screen); negative = off.
    pub(super) warp: f32,
    /// Enhanced Water screen effects.
    pub(super) water: crate::WaterScreen,
    /// Scene target region drawn (top-left), in target pixels.
    pub(super) scene_w: u32,
    pub(super) scene_h: u32,
    /// Dynamic UBO offset in dynbuf.
    pub(super) ubo: u32,
    pub(super) world_draws: Vec<WorldDraw>,
    pub(super) alias_draws: Vec<AliasDraw>,
    pub(super) sprite_draws: Vec<SpriteDraw>,
    pub(super) trans: Vec<(f32, Trans)>,
    pub(super) vel_draws: Vec<VelDraw>,
    pub(super) cull: Option<CullFrame>,
    pub(super) particles: Range,
    pub(super) fte: Option<FteDraw>,
    pub(super) fte_quads: Option<FteQuadDraw>,
    pub(super) portal_draws: Vec<super::portals::PortalDraw>,
}
