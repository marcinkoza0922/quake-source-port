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
    pub(super) sky: [f32; 4],
    pub(super) skybox0: [u32; 4],
    pub(super) skybox1: [u32; 4],
    pub(super) alphas: [f32; 4],
    pub(super) soft: [f32; 4],
    pub(super) softf: [u32; 4],
    /// x ripple strength, y caustics strength, z ripple count.
    pub(super) water: [f32; 4],
    /// xy centre, z liquid surface height, w start time.
    pub(super) ripples: [[f32; 4]; crate::water::MAX_RIPPLES],
    pub(super) ripple_amp: [[f32; 4]; crate::water::MAX_RIPPLES / 4],
}

/// ironwail NOISESCALE.
pub(super) const NOISESCALE: f32 = 9.0 / 255.0;

/// R_SetupView / R_ChooseBModelProgram / alias mode selection for r_softemu:
/// (screen dither, texture dither, world dither level, alias mode).
pub(super) fn softemu_params(st: &Settings) -> (f32, f32, u32, u32) {
    let (screen, texture) = match st.softemu {
        0 => (st.dither / 255.0, 0.0),
        2 => (NOISESCALE * st.dither * st.dither_screen, NOISESCALE * st.dither * st.dither_texture),
        _ => (0.0, 0.0), // fine dithers in post; banded doesn't dither
    };
    let banding = if st.softemu == 3 { st.lightmap_banding != 0.0 } else { st.lightmap_banding > 0.0 };
    let world = if banding { 2 } else if st.softemu >= 2 { 1 } else { 0 };
    let warp = if st.softemu == 3 { st.mdl_warp != 0.0 } else { st.mdl_warp > 0.0 };
    let alias = if warp { 2 } else if st.softemu == 2 { 1 } else { 0 };
    (screen, texture, world, alias)
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
pub(super) struct AoPush {
    pub(super) proj: [f32; 4],
    pub(super) size: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct PolyPush {
    pub(super) color: [f32; 4],
    pub(super) tex: u32,
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
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct UiVert {
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
    pub(super) fn write_at<T: Copy>(&mut self, ofs: u64, data: &[T]) {
        let b = bytes_of(data);
        assert!(ofs + b.len() as u64 <= self.cap);
        // SAFETY: bounds asserted; frame fence waited on.
        unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), self.ptr.add(ofs as usize), b.len()) };
    }
}

/// FTE particles: vertex offset, index offset, (pipeline, texture slot, batch).
/// FTE draws: vertex and index offsets, then (pipeline, texture slot,
/// batch, shader mode). Mode is the blend index, or [`GUN_PORTAL_MODE`].
pub(super) type FteDraw = (u64, u64, Vec<(usize, u32, crate::FteBatch, f32)>);
/// `fte.frag` mode for add-on portal ovals (screen-space view texture).
pub(super) const GUN_PORTAL_MODE: f32 = 8.0;

#[derive(Clone, Copy, Default)]
pub(super) struct Range {
    pub(super) ofs: u64,
    pub(super) count: u32,
}

pub(super) struct WorldDraw {
    pub(super) model: Mat4,
    /// Brush entity: drawn with a depth bias toward the viewer so it wins
    /// over coplanar world faces (vkQuake gl_zfix).
    pub(super) zfix: bool,
    pub(super) alpha: f32,
    pub(super) opaque: Range,
    pub(super) sky: Range,
    /// Liquids, one range per type for the GPU-culled world (CPU path: [0]).
    pub(super) water: [Range; 4],
    pub(super) water_alpha: [Range; 4],
    /// Index buffer the commands refer to (dynbuf when None).
    pub(super) ibuf: Option<vk::Buffer>,
    /// Buffer holding the indirect commands (dynbuf when None).
    pub(super) cbuf: Option<vk::Buffer>,
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
    pub(super) push: AliasPush,
    pub(super) viewmodel: bool,
    /// Drawn only into portal views.
    pub(super) portal_only: bool,
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
    pub(super) portal_draws: Vec<super::portals::PortalDraw>,
}
