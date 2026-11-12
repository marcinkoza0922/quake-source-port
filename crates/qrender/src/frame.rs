//! Per-frame scene building and command recording.

use crate::convert::group_index;
use crate::world::{NO_LM, box_in_frustum, dot};
use crate::*;

type Mat4 = [f32; 16];

#[repr(C)]
#[derive(Clone, Copy)]
struct FrameUbo {
    viewproj: Mat4,
    eye: [f32; 4],
    fog: [f32; 4],
    vright: [f32; 4],
    vup: [f32; 4],
    vfwd: [f32; 4],
    sky: [f32; 4],
    skybox0: [u32; 4],
    skybox1: [u32; 4],
    alphas: [f32; 4],
    soft: [f32; 4],
    softf: [u32; 4],
}

/// ironwail NOISESCALE.
const NOISESCALE: f32 = 9.0 / 255.0;

/// R_SetupView / R_ChooseBModelProgram / alias mode selection for r_softemu:
/// (screen dither, texture dither, world dither level, alias mode).
fn softemu_params(st: &Settings) -> (f32, f32, u32, u32) {
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
struct AliasPush {
    model: Mat4,
    light: [f32; 4],
    shadevec: [f32; 4],
    poses: [u32; 4],
    texs: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct WorldPush {
    model: Mat4,
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PolyPush {
    color: [f32; 4],
    tex: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct PostPush {
    blend: [f32; 4],
    gamma: f32,
    contrast: f32,
    scene: u32,
    dither: f32,
    mode: u32,
    scale: f32,
    colorblind: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct UiVert {
    pos: [f32; 2],
    uv: [f32; 2],
    color: u32,
    tex: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct LmDyn {
    styles: [f32; 256],
    num_dlights: u32,
    pad: [u32; 3],
    dl: [[f32; 8]; 64],
}

/// Bump writer into the mapped per-frame buffer.
struct Dyn {
    ptr: *mut u8,
    ofs: u64,
    cap: u64,
}

impl Dyn {
    fn push<T: Copy>(&mut self, data: &[T], align: u64) -> Option<u64> {
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
    fn write_at<T: Copy>(&mut self, ofs: u64, data: &[T]) {
        let b = bytes_of(data);
        assert!(ofs + b.len() as u64 <= self.cap);
        // SAFETY: bounds asserted; frame fence waited on.
        unsafe { std::ptr::copy_nonoverlapping(b.as_ptr(), self.ptr.add(ofs as usize), b.len()) };
    }
}

/// FTE particles: vertex offset, index offset, (pipeline, texture slot, batch).
type FteDraw = (u64, u64, Vec<(usize, u32, crate::FteBatch)>);

#[derive(Clone, Copy, Default)]
struct Range {
    ofs: u64,
    count: u32,
}

struct WorldDraw {
    model: Mat4,
    /// Brush entity: drawn with a depth bias toward the viewer so it wins
    /// over coplanar world faces (vkQuake gl_zfix).
    zfix: bool,
    alpha: f32,
    opaque: Range,
    sky: Range,
    /// Liquids, one range per type for the GPU-culled world (CPU path: [0]).
    water: [Range; 4],
    water_alpha: [Range; 4],
    /// Index buffer the commands refer to (dynbuf when None).
    ibuf: Option<vk::Buffer>,
    /// Buffer holding the indirect commands (dynbuf when None).
    cbuf: Option<vk::Buffer>,
}

/// Per-frame inputs of cull.comp, filled while building the scene.
struct CullFrame {
    push: CullPush,
    groups: u32,
    /// Commands, dispatch args and leaf bits staged in dynbuf, copied to `cmdbuf`.
    stage_ofs: u64,
    stage_len: u64,
    /// Lightmap dispatch args (x, y, z): offset in dynbuf's staging copy
    /// and in `cmdbuf`.
    disp_ofs: u64,
    cmdbuf: vk::Buffer,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct CullPush {
    frustum: [[f32; 4]; 4],
    eye: [f32; 4],
    framecount: u32,
    nmarks: u32,
    ncmds: u32,
    ofs_vis: u32,
    ofs_lm: u32,
    pad: u32,
    max_lm: u32,
}

struct AliasDraw {
    model: usize,
    push: AliasPush,
    viewmodel: bool,
}

struct SpriteDraw {
    vofs: u64,
    tex: u32,
    alpha: f32,
}

enum Trans {
    World(usize),
    Alias(usize),
    Sprite(usize),
}

fn ident() -> Mat4 {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut c = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            c[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    c
}

/// From a row-major 3x3 rotation and translation.
fn affine(r: [[f32; 3]; 3], t: [f32; 3]) -> Mat4 {
    let mut m = ident();
    for (row, rr) in r.iter().enumerate() {
        for (col, v) in rr.iter().enumerate() {
            m[col * 4 + row] = *v;
        }
    }
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m
}

fn mul3(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut c = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    c
}

/// R_RotateForEntity: Rz(yaw) * Ry(-pitch) * Rx(roll).
fn entity_rotation(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sp, cp) = (-angles[0]).to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    let rz = [[cy, -sy, 0.0], [sy, cy, 0.0], [0.0, 0.0, 1.0]];
    let ry = [[cp, 0.0, sp], [0.0, 1.0, 0.0], [-sp, 0.0, cp]];
    let rx = [[1.0, 0.0, 0.0], [0.0, cr, -sr], [0.0, sr, cr]];
    mul3(mul3(rz, ry), rx)
}

/// AngleVectors: (forward, right, up).
pub fn angle_vectors(a: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (sy, cy) = a[1].to_radians().sin_cos();
    let (sp, cp) = a[0].to_radians().sin_cos();
    let (sr, cr) = a[2].to_radians().sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}

fn pack_color(c: [f32; 4]) -> u32 {
    let b = c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32);
    b[0] | b[1] << 8 | b[2] << 16 | b[3] << 24
}

struct ViewSetup {
    vp: Mat4,
    eye: [f32; 3],
    fwd: [f32; 3],
    right: [f32; 3],
    up: [f32; 3],
    frustum: [[f32; 4]; 4],
}

fn setup_view(v: &View, w: u32, h: u32) -> ViewSetup {
    let (fwd, right, up) = angle_vectors(v.angles);
    // fov_adapt: fov_x is for 4:3, widened for the actual aspect
    let aspect = w.max(1) as f32 / h.max(1) as f32;
    let tx43 = (v.fov_x.clamp(1.0, 179.0).to_radians() * 0.5).tan();
    let ty = tx43 * 0.75;
    let tx = ty * aspect;
    let (n, f) = (4.0f32, 65536.0f32);
    // view: rows right, -up, forward
    let rows = [right, [-up[0], -up[1], -up[2]], fwd];
    let e = v.origin;
    let mut view = ident();
    for (r, row) in rows.iter().enumerate() {
        for c in 0..3 {
            view[c * 4 + r] = row[c];
        }
        view[12 + r] = -dot(*row, e);
    }
    let mut proj = [0.0; 16];
    proj[0] = 1.0 / tx;
    proj[5] = 1.0 / ty;
    proj[10] = f / (f - n);
    proj[11] = 1.0;
    proj[14] = -f * n / (f - n);
    let vp = mul(&proj, &view);
    let row = |i: usize| [vp[i], vp[4 + i], vp[8 + i], vp[12 + i]];
    let (r0, r1, r3) = (row(0), row(1), row(3));
    let mut frustum = [[0.0; 4]; 4];
    for (k, (a, s)) in [(r0, 1.0), (r0, -1.0), (r1, 1.0), (r1, -1.0)]
        .iter()
        .enumerate()
    {
        let p: [f32; 4] = std::array::from_fn(|i| r3[i] + s * a[i]);
        let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
        frustum[k] = p.map(|x| x / l);
    }
    ViewSetup {
        vp,
        eye: e,
        fwd,
        right,
        up,
        frustum,
    }
}

fn sphere_in_frustum(f: &[[f32; 4]; 4], c: [f32; 3], r: f32) -> bool {
    f.iter()
        .all(|p| p[0] * c[0] + p[1] * c[1] + p[2] * c[2] + p[3] >= -r)
}

#[allow(clippy::too_many_arguments)]
fn build_world_draw(
    w: &WorldGpu,
    buckets: &mut [Vec<u32>],
    dynw: &mut Dyn,
    surfs: &[u32],
    alt: bool,
    time: f64,
    alphas: [f32; 4],
    ent_alpha: f32,
    model: Mat4,
    lm_vis: &mut Vec<u32>,
) -> WorldDraw {
    for &si in surfs {
        let s = &w.cpu.surfaces[si as usize];
        let t = (w.cpu.resolve_texture(s.texture, alt, time, si) as usize).min(buckets.len() - 1);
        buckets[t].push(si);
        if s.lm != NO_LM {
            lm_vis.push(s.lm);
        }
    }
    let mut cats: [Vec<vk::DrawIndexedIndirectCommand>; 4] = Default::default();
    for (t, b) in buckets.iter_mut().enumerate() {
        if b.is_empty() {
            continue;
        }
        let mut first = None;
        let mut count = 0u32;
        for &si in b.iter() {
            let s = &w.cpu.surfaces[si as usize];
            let idx =
                &w.cpu.indices[s.first_index as usize..(s.first_index + s.num_indices) as usize];
            let Some(o) = dynw.push(idx, 4) else { break };
            first.get_or_insert(o);
            count += s.num_indices;
        }
        b.clear();
        let Some(first) = first else { continue };
        let (kind, flags) = w.tex_info[t];
        let cat = match kind {
            TextureKind::Sky => 1,
            TextureKind::Liquid => {
                let ty = (flags & 0xf).clamp(1, 4) as usize;
                if alphas[ty - 1] * ent_alpha < 1.0 {
                    3
                } else {
                    2
                }
            }
            _ => 0,
        };
        cats[cat].push(vk::DrawIndexedIndirectCommand {
            index_count: count,
            instance_count: 1,
            first_index: (first / 4) as u32,
            vertex_offset: 0,
            first_instance: t as u32,
        });
    }
    let mut put = |c: &Vec<vk::DrawIndexedIndirectCommand>| match dynw.push(c, 16) {
        Some(ofs) if !c.is_empty() => Range {
            ofs,
            count: c.len() as u32,
        },
        _ => Range::default(),
    };
    WorldDraw {
        model,
        zfix: false,
        alpha: ent_alpha,
        opaque: put(&cats[0]),
        sky: put(&cats[1]),
        water: [put(&cats[2]), Range::default(), Range::default(), Range::default()],
        water_alpha: [put(&cats[3]), Range::default(), Range::default(), Range::default()],
        ibuf: None,
        cbuf: None,
    }
}

/// Per-frame setup of the GPU-culled world: leaf bits and zeroed draw
/// commands in dynbuf, the cull push constants, and the world's WorldDraw
/// (commands in dynbuf, indices in this frame's cull output buffer).
fn gpu_world_draw(
    wg: &mut WorldGpu,
    dynw: &mut Dyn,
    vs: &ViewSetup,
    time: f64,
    alphas: [f32; 4],
    fi: usize,
) -> Option<(WorldDraw, CullFrame)> {
    let mut bits = std::mem::take(&mut wg.scratch_bits);
    wg.cpu.leaf_bits(vs.eye, &vs.frustum, &mut bits);
    let ntex = wg.tex_info.len().max(1) as u32;
    let c = &mut wg.cull;
    c.framecount = c.framecount.wrapping_add(1).max(1);
    let mut cmds = std::mem::take(&mut wg.scratch_cmds);
    cmds.clear();
    cmds.extend(c.cmd_tex.iter().zip(&c.cmd_first).map(|(&t, &first)| {
        let tex = match wg.cpu.anims.get(t as usize) {
            Some(a) if !a.2 && !a.0.is_empty() => wg.cpu.resolve_texture(t, false, time, 0),
            _ => t,
        };
        vk::DrawIndexedIndirectCommand {
            index_count: 0,
            instance_count: 1,
            first_index: first,
            vertex_offset: 0,
            first_instance: tex.min(ntex - 1),
        }
    }));
    // Staged as commands then dispatch args, copied to cmdbuf on the GPU.
    let disp_ofs = cmds.len() as u64 * 20;
    let stage_ofs = dynw.push(&cmds, 16)?;
    dynw.push(&[0u32; 4], 4)?;
    dynw.push(&bits, 4)?;
    let range = |r: &std::ops::Range<u32>| Range {
        ofs: r.start as u64 * 20,
        count: r.end - r.start,
    };
    let mut water = [Range::default(); 4];
    let mut water_alpha = [Range::default(); 4];
    for ty in 0..4 {
        let r = range(&c.cats[2 + ty]);
        if alphas[ty] < 1.0 {
            water_alpha[ty] = r;
        } else {
            water[ty] = r;
        }
    }
    let wd = WorldDraw {
        model: ident(),
        zfix: false,
        alpha: 1.0,
        opaque: range(&c.cats[0]),
        sky: range(&c.cats[1]),
        water,
        water_alpha,
        ibuf: Some(c.out[fi].buf),
        cbuf: Some(c.cmdbuf[fi].buf),
    };
    let push = CullPush {
        frustum: vs.frustum,
        eye: [vs.eye[0], vs.eye[1], vs.eye[2], 0.0],
        framecount: c.framecount,
        nmarks: c.nmarks,
        ncmds: cmds.len() as u32,
        ofs_vis: (disp_ofs / 4 + 4) as u32,
        ofs_lm: (OFS_VIS / 4) as u32,
        pad: 0,
        max_lm: MAX_VIS as u32,
    };
    let cf = CullFrame {
        push,
        groups: c.nmarks.div_ceil(64),
        stage_ofs,
        stage_len: disp_ofs + 16 + bits.len() as u64 * 4,
        disp_ofs,
        cmdbuf: c.cmdbuf[fi].buf,
    };
    wg.scratch_bits = bits;
    wg.scratch_cmds = cmds;
    Some((wd, cf))
}

impl Renderer {
    /// Draws and presents one frame.
    pub fn render(&mut self, fr: &RenderFrame) -> Result<()> {
        if let Some(s) = &self.swap
            && (self.needs_resize || s.vsync != fr.settings.vsync)
        {
            self.create_swapchain(fr.settings.vsync)?;
        }
        self.apply_settings(&fr.settings)?;
        if self.targets.is_none() && (self.swap.is_some() || self.width > 0) {
            self.create_targets()?;
        }
        if self.width == 0 || self.height == 0 {
            return Ok(());
        }
        let dev = self.gpu.device.clone();
        let fi = self.cur;
        let (fence, acquire, cmd) = (
            self.frames[fi].fence,
            self.frames[fi].acquire,
            self.frames[fi].cmd,
        );
        // SAFETY: fence owned by this frame.
        unsafe { dev.wait_for_fences(&[fence], true, u64::MAX)? };
        self.init_gpu_times();
        self.collect_gpu_times(fi);
        let mut img_idx = 0u32;
        if let (Some(s), Some(f)) = (&self.swap, &self.gpu.swapchain_fn) {
            // SAFETY: valid swapchain/semaphore.
            match unsafe { f.acquire_next_image(s.sc, u64::MAX, acquire, vk::Fence::null()) } {
                Ok((i, _)) => img_idx = i,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    self.needs_resize = true;
                    return Ok(());
                }
                Err(e) => return Err(e.into()),
            }
        }
        // SAFETY: fence not in use (waited).
        unsafe { dev.reset_fences(&[fence])? };

        let (w, h) = (self.width, self.height);
        let mut dynw = Dyn {
            ptr: self.frames[fi].dynbuf.mem.ptr,
            ofs: OFS_BUMP,
            cap: self.frames[fi].dynbuf.size,
        };
        let st = &fr.settings;
        let alphas = [
            st.wateralpha,
            st.lavaalpha.unwrap_or(st.wateralpha),
            st.slimealpha.unwrap_or(st.wateralpha),
            st.telealpha.unwrap_or(st.wateralpha),
        ]
        .map(|a| a.clamp(0.0, 1.0));
        let time = fr.time;
        let mut styles = [256.0f32; 256];
        for (i, v) in fr.lightstyles.iter().take(256).enumerate() {
            styles[i] = *v as f32;
        }
        let style_ints: Vec<i32> = styles.iter().map(|&v| v as i32).collect();

        let mut world_draws: Vec<WorldDraw> = Vec::new();
        let mut alias_draws: Vec<AliasDraw> = Vec::new();
        let mut sprite_draws: Vec<SpriteDraw> = Vec::new();
        let mut trans: Vec<(f32, Trans)> = Vec::new();
        let mut lm_vis: Vec<u32> = Vec::new();
        let mut cull: Option<CullFrame> = None;
        let mut particles = Range::default();
        // FTE particles: (vertex offset, index offset, batches with texture slots)
        let mut fte: Option<FteDraw> = None;
        // scene target size (render scale) and the 3D view height in it
        let (sw, sh) = self
            .targets
            .as_ref()
            .map_or((w, h), |t| (t.scene_w, t.scene_h));
        let scene_h = fr.view.map_or(sh, |v| {
            let px = h.saturating_sub(v.reserve_bottom) as f32 * sh as f32 / h.max(1) as f32;
            (px.round() as u32).clamp(1, sh)
        });
        let view = fr.view.map(|v| setup_view(&v, sw, scene_h));
        let fog = fr.fog;

        if let Some(vs) = &view {
            if let Some(wg) = self.world.as_mut()
                && let Some((wd, cf)) = gpu_world_draw(wg, &mut dynw, vs, time, alphas, fi)
            {
                world_draws.push(wd);
                cull = Some(cf);
            }
            for e in fr.entities {
                let Some(m) = self.models.get(e.model.0 as usize) else {
                    continue;
                };
                let rot = entity_rotation(e.angles);
                let dist = dot(sub(e.origin, vs.eye), vs.fwd);
                match m {
                    Model::Brush(sm) => {
                        let Some(wg) = self.world.as_ref() else {
                            continue;
                        };
                        let Some(sub_) = wg.cpu.submodels.get(*sm as usize) else {
                            continue;
                        };
                        let rotated = e.angles.iter().any(|&a| a != 0.0);
                        if rotated {
                            let r = [0, 1, 2]
                                .map(|i| sub_.mins[i].abs().max(sub_.maxs[i].abs()))
                                .iter()
                                .map(|x| x * x)
                                .sum::<f32>()
                                .sqrt();
                            if !sphere_in_frustum(&vs.frustum, e.origin, r) {
                                continue;
                            }
                        } else {
                            let mn = [0, 1, 2].map(|i| sub_.mins[i] + e.origin[i]);
                            let mx = [0, 1, 2].map(|i| sub_.maxs[i] + e.origin[i]);
                            if !box_in_frustum(&vs.frustum, mn, mx) {
                                continue;
                            }
                        }
                        // eye in model space (R^T * (eye - origin))
                        let d = sub(vs.eye, e.origin);
                        let le = [0, 1, 2]
                            .map(|c| rot[0][c] * d[0] + rot[1][c] * d[1] + rot[2][c] * d[2]);
                        let mut surfs = Vec::new();
                        for si in sub_.first_surface..sub_.first_surface + sub_.num_surfaces {
                            let Some(s) = wg.cpu.surfaces.get(si as usize) else {
                                break;
                            };
                            if s.num_indices == 0 {
                                continue;
                            }
                            let dd = dot(le, s.normal) - s.dist;
                            if (dd < 0.0) != s.flipped {
                                continue;
                            }
                            surfs.push(si);
                        }
                        let alpha = e.alpha.clamp(0.0, 1.0);
                        let mut wd = build_world_draw(
                            wg,
                            &mut self.buckets,
                            &mut dynw,
                            &surfs,
                            e.frame != 0,
                            time,
                            alphas,
                            alpha,
                            affine(rot, e.origin),
                            &mut lm_vis,
                        );
                        wd.zfix = true;
                        if alpha < 1.0 {
                            trans.push((dist, Trans::World(world_draws.len())));
                        }
                        world_draws.push(wd);
                    }
                    Model::Alias(am) => {
                        if am.frames.is_empty() || am.num_indices == 0 {
                            continue;
                        }
                        if !e.viewmodel && !sphere_in_frustum(&vs.frustum, e.origin, am.radius) {
                            continue;
                        }
                        let t = time + e.syncbase as f64;
                        let pose = |f: u32| {
                            let (first, n, iv) = &am.frames[(f as usize).min(am.frames.len() - 1)];
                            first + group_index(iv, *n as usize, t) as u32
                        };
                        let p1 = am.verts_base + pose(e.prev_frame) * am.num_verts;
                        let p2 = am.verts_base + pose(e.frame) * am.num_verts;
                        let skin = if am.skins.is_empty() {
                            [NO_TEX; 3]
                        } else {
                            let (imgs, iv) = &am.skins[e.skin as usize % am.skins.len()];
                            imgs[group_index(iv, imgs.len(), t)]
                        };
                        let model = mul(
                            &affine(rot, e.origin),
                            &affine(
                                [
                                    [am.scale[0], 0.0, 0.0],
                                    [0.0, am.scale[1], 0.0],
                                    [0.0, 0.0, am.scale[2]],
                                ],
                                am.scale_origin,
                            ),
                        );
                        // R_SetupAliasLighting
                        let wl = self.world.as_ref().map(|w| &w.cpu);
                        let mut lc = wl
                            .and_then(|w| {
                                w.light_point(e.origin, &style_ints).or_else(|| {
                                    w.light_point(
                                        [e.origin[0], e.origin[1], e.origin[2] + am.maxs_z * 0.5],
                                        &style_ints,
                                    )
                                })
                            })
                            .unwrap_or([128.0; 3]);
                        if st.dynamic {
                            for dl in fr.dlights {
                                let d = sub(e.origin, dl.origin);
                                let add = dl.radius - dot(d, d).sqrt();
                                if add > 0.0 {
                                    for (c, dc) in lc.iter_mut().zip(dl.color) {
                                        *c += add * dc;
                                    }
                                }
                            }
                        }
                        let minimum = if e.viewmodel {
                            72.0
                        } else if e.player {
                            24.0
                        } else {
                            0.0
                        };
                        let sum = lc[0] + lc[1] + lc[2];
                        if sum < minimum {
                            lc = lc.map(|c| c + (minimum - sum) / 3.0);
                        }
                        let sum = lc[0] + lc[1] + lc[2];
                        if sum > 288.0 {
                            lc = lc.map(|c| c * 288.0 / sum);
                        }
                        let q = ((e.angles[1] * (16.0 / 360.0)) as i32 & 15) as f32;
                        let ang = q / 16.0 * std::f32::consts::TAU;
                        let sv = norm([(-ang).cos(), (-ang).sin(), 1.0]);
                        let alpha = e.alpha.clamp(0.0, 1.0);
                        let mut flags = 0u32;
                        if am.holey {
                            flags |= 1;
                        }
                        if e.fullbright {
                            flags |= 2;
                        }
                        let [top, bot] = e.colormap.unwrap_or([1, 6]);
                        if e.colormap.is_some() {
                            flags |= 4;
                        }
                        let push = AliasPush {
                            model,
                            light: [lc[0] / 200.0, lc[1] / 200.0, lc[2] / 200.0, alpha],
                            shadevec: [sv[0], sv[1], sv[2], e.lerp.clamp(0.0, 1.0)],
                            poses: [p1, p2, top.min(13) as u32, bot.min(13) as u32],
                            texs: [skin[0], skin[1], skin[2], flags],
                        };
                        if alpha < 1.0 && !e.viewmodel {
                            trans.push((dist, Trans::Alias(alias_draws.len())));
                        }
                        alias_draws.push(AliasDraw {
                            model: e.model.0 as usize,
                            push,
                            viewmodel: e.viewmodel,
                        });
                    }
                    Model::Sprite(sm) => {
                        if sm.frames.is_empty()
                            || !sphere_in_frustum(&vs.frustum, e.origin, sm.radius.max(16.0))
                        {
                            continue;
                        }
                        let f = &sm.frames[e.frame as usize % sm.frames.len()];
                        if f.images.is_empty() {
                            continue;
                        }
                        let (slot, [l, r, dn, upv]) = f.images
                            [group_index(&f.intervals, f.images.len(), time + e.syncbase as f64)];
                        let (sr, su) = match sm.kind {
                            qformats::spr::SPR_VP_PARALLEL_UPRIGHT => {
                                let fw = norm([vs.fwd[0], vs.fwd[1], 0.0]);
                                ([fw[1], -fw[0], 0.0], [0.0, 0.0, 1.0])
                            }
                            qformats::spr::SPR_FACING_UPRIGHT => {
                                let fw =
                                    norm([e.origin[0] - vs.eye[0], e.origin[1] - vs.eye[1], 0.0]);
                                ([fw[1], -fw[0], 0.0], [0.0, 0.0, 1.0])
                            }
                            qformats::spr::SPR_ORIENTED => {
                                let (_, r, u) = angle_vectors(e.angles);
                                (r, u)
                            }
                            qformats::spr::SPR_VP_PARALLEL_ORIENTED => {
                                let (s, c) = e.angles[2].to_radians().sin_cos();
                                (
                                    [0, 1, 2].map(|i| vs.right[i] * c + vs.up[i] * s),
                                    [0, 1, 2].map(|i| vs.right[i] * -s + vs.up[i] * c),
                                )
                            }
                            _ => (vs.right, vs.up),
                        };
                        let p =
                            |x: f32, y: f32| [0, 1, 2].map(|i| e.origin[i] + sr[i] * x + su[i] * y);
                        let v = |pos: [f32; 3], u: f32, vv: f32| [pos[0], pos[1], pos[2], u, vv];
                        let quad = [
                            v(p(l, dn), 0.0, 1.0),
                            v(p(l, upv), 0.0, 0.0),
                            v(p(r, upv), 1.0, 0.0),
                            v(p(l, dn), 0.0, 1.0),
                            v(p(r, upv), 1.0, 0.0),
                            v(p(r, dn), 1.0, 1.0),
                        ];
                        let Some(vofs) = dynw.push(&quad, 16) else {
                            continue;
                        };
                        let alpha = e.alpha.clamp(0.0, 1.0);
                        trans.push((dist, Trans::Sprite(sprite_draws.len())));
                        sprite_draws.push(SpriteDraw {
                            vofs,
                            tex: slot,
                            alpha,
                        });
                    }
                }
            }
            if !fr.particles.is_empty() {
                let data: Vec<([f32; 3], u32)> = fr
                    .particles
                    .iter()
                    .map(|p| {
                        let c = self.palette[p.color as usize];
                        let a = (p.alpha.clamp(0.0, 1.0) * 255.0) as u32;
                        (
                            p.origin,
                            c[0] as u32 | (c[1] as u32) << 8 | (c[2] as u32) << 16 | a << 24,
                        )
                    })
                    .collect();
                if let Some(ofs) = dynw.push(&data, 16) {
                    particles = Range {
                        ofs,
                        count: data.len() as u32,
                    };
                }
            }
            if !fr.fte_batches.is_empty() && !fr.fte_indices.is_empty() {
                let batches: Vec<_> = fr
                    .fte_batches
                    .iter()
                    .filter_map(|b| {
                        let slot = self.pics.get(b.pic.0 as usize)?.slot;
                        Some((crate::pipelines::fte_index(b.blend, b.lines), slot, *b))
                    })
                    .collect();
                if let (Some(v), Some(i)) = (dynw.push(fr.fte_vertices, 16), dynw.push(fr.fte_indices, 4)) {
                    fte = Some((v, i, batches));
                }
            }
            trans.sort_by(|a, b| b.0.total_cmp(&a.0));
        }

        // UI vertices
        let mut ui: Vec<UiVert> = Vec::with_capacity(fr.draw2d.len() * 6);
        let quad = |ui: &mut Vec<UiVert>,
                    x: f32,
                    y: f32,
                    w: f32,
                    h: f32,
                    uv: [f32; 4],
                    color: [f32; 4],
                    tex: u32| {
            let c = pack_color(color);
            let v = |px: f32, py: f32, u: f32, vv: f32| UiVert {
                pos: [px, py],
                uv: [u, vv],
                color: c,
                tex,
            };
            ui.extend_from_slice(&[
                v(x, y, uv[0], uv[1]),
                v(x + w, y, uv[2], uv[1]),
                v(x + w, y + h, uv[2], uv[3]),
                v(x, y, uv[0], uv[1]),
                v(x + w, y + h, uv[2], uv[3]),
                v(x, y + h, uv[0], uv[3]),
            ]);
        };
        for d in fr.draw2d {
            match *d {
                Draw2D::Pic {
                    x,
                    y,
                    w,
                    h,
                    pic,
                    uv,
                    color,
                } => {
                    if let Some(p) = self.pics.get(pic.0 as usize) {
                        quad(&mut ui, x, y, w, h, uv, color, p.slot);
                    }
                }
                Draw2D::Char {
                    x,
                    y,
                    size,
                    ch,
                    color,
                } => {
                    if ch == b' ' {
                        continue;
                    }
                    if let Some(p) = self.conchars.and_then(|c| self.pics.get(c.0 as usize)) {
                        let (cx, cy) = ((ch & 15) as f32 / 16.0, (ch >> 4) as f32 / 16.0);
                        let e = 1.0 / 2048.0;
                        let uv = [cx + e, cy + e, cx + 1.0 / 16.0 - e, cy + 1.0 / 16.0 - e];
                        quad(&mut ui, x, y, size, size, uv, color, p.slot);
                    }
                }
                Draw2D::Fill { x, y, w, h, color } => {
                    quad(&mut ui, x, y, w, h, [0.0; 4], color, NO_TEX)
                }
                Draw2D::Tile {
                    x,
                    y,
                    w,
                    h,
                    pic,
                    scale,
                } => {
                    if let Some(p) = self.pics.get(pic.0 as usize) {
                        let (pw, ph) = (
                            p.width as f32 * scale.max(1e-3),
                            p.height as f32 * scale.max(1e-3),
                        );
                        let uv = [x / pw, y / ph, (x + w) / pw, (y + h) / ph];
                        quad(&mut ui, x, y, w, h, uv, [1.0; 4], p.slot);
                    }
                }
            }
        }
        let ui_ofs = if ui.is_empty() {
            None
        } else {
            dynw.push(&ui, 16)
        };

        // UBO + lightmap dynamic data
        if let Some(vs) = &view {
            let sky = self.skybox;
            let soft = softemu_params(st);
            let ubo = FrameUbo {
                viewproj: vs.vp,
                eye: [vs.eye[0], vs.eye[1], vs.eye[2], time as f32],
                fog: [
                    fog.color[0],
                    fog.color[1],
                    fog.color[2],
                    fog.density.max(0.0) / 64.0,
                ],
                vright: [vs.right[0], vs.right[1], vs.right[2], 0.0],
                vup: [vs.up[0], vs.up[1], vs.up[2], 0.0],
                vfwd: [vs.fwd[0], vs.fwd[1], vs.fwd[2], 0.0],
                sky: [
                    st.skyfog,
                    if sky.is_some() { 1.0 } else { 0.0 },
                    st.skyalpha,
                    0.0,
                ],
                skybox0: sky.map_or([NO_TEX; 4], |s| [s[0], s[1], s[2], s[3]]),
                skybox1: [
                    sky.map_or(NO_TEX, |s| s[4]),
                    sky.map_or(NO_TEX, |s| s[5]),
                    self.sky_layers[0],
                    self.sky_layers[1],
                ],
                alphas,
                soft: [soft.0, soft.1, 0.0, 0.0],
                softf: [soft.2, soft.3, 0, 0],
            };
            dynw.write_at(0, &[ubo]);
            let mut lmd = LmDyn {
                styles,
                num_dlights: 0,
                pad: [0; 3],
                dl: [[0.0; 8]; 64],
            };
            if st.dynamic {
                for (i, d) in fr.dlights.iter().take(64).enumerate() {
                    lmd.dl[i] = [
                        d.origin[0],
                        d.origin[1],
                        d.origin[2],
                        d.radius,
                        d.color[0],
                        d.color[1],
                        d.color[2],
                        d.minlight,
                    ];
                    lmd.num_dlights = i as u32 + 1;
                }
            }
            dynw.write_at(OFS_LMDYN, &[lmd]);
            lm_vis.truncate(MAX_VIS as usize);
            dynw.write_at(OFS_VIS, &lm_vis);
            if let Some(c) = &cull {
                // The cull shader appends the world's lightmaps after these.
                dynw.write_at(c.stage_ofs + c.disp_ofs, &[lm_vis.len() as u32, 1, 1, 0]);
            }
        }

        // ---- record ----
        let tsq = self.gpu_times.as_ref().map(|t| t.pool);
        if let Some(t) = &mut self.gpu_times {
            t.written[fi] = true;
        }
        let ts_base = fi as u32 * GPU_TS;
        // SAFETY: recording into `cmd`; query index within the pool.
        let ts = |k: u32| unsafe {
            if let Some(q) = tsq {
                dev.cmd_write_timestamp(cmd, vk::PipelineStageFlags::BOTTOM_OF_PIPE, q, ts_base + k);
            }
        };
        let f = &self.frames[fi];
        let dynbuf = f.dynbuf.buf;
        let set = [f.set];
        let comp_set = [f.comp_set];
        let p = &self.pipes;
        let Some(t) = &self.targets else {
            return Ok(());
        };
        let Some(lightmap) = &self.lightmap else {
            return Ok(());
        };
        // SAFETY: all recorded handles are alive until this frame's fence signals;
        // the command buffer is not in use (fence waited).
        unsafe {
            dev.reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;
            dev.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;
            if let Some(q) = tsq {
                dev.cmd_reset_query_pool(cmd, q, ts_base, GPU_TS);
            }
            ts(0);
            let lm_rng = gpu::range(vk::ImageAspectFlags::COLOR, 1, lightmap.layers);
            if let Some(c) = &cull {
                let cs = vk::PipelineBindPoint::COMPUTE;
                dev.cmd_copy_buffer(
                    cmd,
                    dynbuf,
                    c.cmdbuf,
                    &[vk::BufferCopy { src_offset: c.stage_ofs, dst_offset: 0, size: c.stage_len }],
                );
                let tb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::SHADER_READ | vk::AccessFlags::SHADER_WRITE)];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &tb,
                    &[],
                    &[],
                );
                dev.cmd_bind_pipeline(cmd, cs, p.cull);
                dev.cmd_bind_descriptor_sets(cmd, cs, p.cull_layout, 0, &[f.cull_set], &[]);
                dev.cmd_push_constants(cmd, p.cull_layout, vk::ShaderStageFlags::COMPUTE, 0, bytes_of(&[c.push]));
                dev.cmd_dispatch(cmd, c.groups, 1, 1);
                let mb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::SHADER_WRITE)
                    .dst_access_mask(
                        vk::AccessFlags::INDIRECT_COMMAND_READ
                            | vk::AccessFlags::INDEX_READ
                            | vk::AccessFlags::SHADER_READ,
                    )];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::PipelineStageFlags::DRAW_INDIRECT
                        | vk::PipelineStageFlags::VERTEX_INPUT
                        | vk::PipelineStageFlags::COMPUTE_SHADER,
                    vk::DependencyFlags::empty(),
                    &mb,
                    &[],
                    &[],
                );
            }
            ts(1);
            if (!lm_vis.is_empty() || cull.is_some()) && self.world.is_some() {
                gpu::barrier(
                    &dev,
                    cmd,
                    lightmap.image,
                    lm_rng,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::GENERAL,
                );
                dev.cmd_bind_pipeline(cmd, vk::PipelineBindPoint::COMPUTE, p.lightmap);
                dev.cmd_bind_descriptor_sets(
                    cmd,
                    vk::PipelineBindPoint::COMPUTE,
                    p.comp_layout,
                    0,
                    &comp_set,
                    &[],
                );
                match &cull {
                    Some(c) => dev.cmd_dispatch_indirect(cmd, c.cmdbuf, c.disp_ofs),
                    None => dev.cmd_dispatch(cmd, lm_vis.len() as u32, 1, 1),
                }
                gpu::barrier(
                    &dev,
                    cmd,
                    lightmap.image,
                    lm_rng,
                    vk::ImageLayout::GENERAL,
                    vk::ImageLayout::GENERAL,
                );
            }
            ts(2);
            let crng = gpu::range(vk::ImageAspectFlags::COLOR, 1, 1);
            let drng = gpu::range(vk::ImageAspectFlags::DEPTH, 1, 1);
            gpu::barrier(
                &dev,
                cmd,
                t.color.image,
                crng,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            gpu::barrier(
                &dev,
                cmd,
                t.depth.image,
                drng,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL,
            );
            if let Some(ms) = &t.color_ms {
                gpu::barrier(
                    &dev,
                    cmd,
                    ms.image,
                    crng,
                    vk::ImageLayout::UNDEFINED,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                );
            }
            let area = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: w,
                    height: h,
                },
            };
            let area_target = vk::Rect2D {
                offset: vk::Offset2D::default(),
                extent: vk::Extent2D {
                    width: sw,
                    height: sh,
                },
            };
            let clear = vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [0.0, 0.0, 0.0, 1.0],
                },
            };
            let catt = [match &t.color_ms {
                Some(ms) => vk::RenderingAttachmentInfo::default()
                    .image_view(ms.view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .resolve_mode(vk::ResolveModeFlags::AVERAGE)
                    .resolve_image_view(t.color.view)
                    .resolve_image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::DONT_CARE)
                    .clear_value(clear),
                None => vk::RenderingAttachmentInfo::default()
                    .image_view(t.color.view)
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(clear),
            }];
            let datt = vk::RenderingAttachmentInfo::default()
                .image_view(t.depth.view)
                .image_layout(vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::CLEAR)
                .store_op(vk::AttachmentStoreOp::DONT_CARE)
                .clear_value(vk::ClearValue {
                    depth_stencil: vk::ClearDepthStencilValue {
                        depth: 1.0,
                        stencil: 0,
                    },
                });
            let ri = vk::RenderingInfo::default()
                .render_area(area_target)
                .layer_count(1)
                .color_attachments(&catt)
                .depth_attachment(&datt);
            dev.cmd_begin_rendering(cmd, &ri);
            ts(3);
            let vp_full = vk::Viewport {
                x: 0.0,
                y: 0.0,
                width: w as f32,
                height: h as f32,
                min_depth: 0.0,
                max_depth: 1.0,
            };
            let vp_scene = vk::Viewport {
                width: sw as f32,
                height: scene_h as f32,
                ..vp_full
            };
            let area_scene = vk::Rect2D {
                extent: vk::Extent2D {
                    width: sw,
                    height: scene_h,
                },
                ..area
            };
            dev.cmd_set_viewport(cmd, 0, &[vp_scene]);
            dev.cmd_set_scissor(cmd, 0, &[area_scene]);
            let gfx = vk::PipelineBindPoint::GRAPHICS;
            let all = vk::ShaderStageFlags::VERTEX | vk::ShaderStageFlags::FRAGMENT;
            let push = |data: &[u8]| dev.cmd_push_constants(cmd, p.layout, all, 0, data);
            if view.is_some() {
                dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[]);
                dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                let draw_world = |wd: &WorldDraw, cats: &[(Range, vk::Pipeline)]| {
                    dev.cmd_bind_index_buffer(cmd, wd.ibuf.unwrap_or(dynbuf), 0, vk::IndexType::UINT32);
                    push(bytes_of(&[WorldPush {
                        model: wd.model,
                        params: [wd.alpha, 0.0, 0.0, 0.0],
                    }]));
                    // vkQuake gl_zfix values for a D32F depth buffer
                    if wd.zfix {
                        dev.cmd_set_depth_bias(cmd, -4.0, 0.0, -0.125);
                    }
                    for (r, pipe) in cats {
                        if r.count > 0 {
                            dev.cmd_bind_pipeline(cmd, gfx, *pipe);
                            dev.cmd_draw_indexed_indirect(cmd, wd.cbuf.unwrap_or(dynbuf), r.ofs, r.count, 20);
                        }
                    }
                    if wd.zfix {
                        dev.cmd_set_depth_bias(cmd, 0.0, 0.0, 0.0);
                    }
                };
                if let Some(wg) = &self.world {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                    dev.cmd_bind_index_buffer(cmd, dynbuf, 0, vk::IndexType::UINT32);
                    for wd in world_draws.iter().filter(|wd| wd.alpha >= 1.0) {
                        draw_world(
                            wd,
                            &[
                                (wd.opaque, p.world),
                                (wd.sky, p.sky),
                                (wd.water[0], p.water),
                                (wd.water[1], p.water),
                                (wd.water[2], p.water),
                                (wd.water[3], p.water),
                            ],
                        );
                    }
                }
                ts(4);
                let draw_alias = |a: &AliasDraw, pipe: vk::Pipeline| {
                    let Some(Model::Alias(am)) = self.models.get(a.model) else {
                        return;
                    };
                    dev.cmd_bind_pipeline(cmd, gfx, pipe);
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[am.vbuf.buf], &[0]);
                    dev.cmd_bind_index_buffer(cmd, am.ibuf.buf, 0, vk::IndexType::UINT32);
                    push(bytes_of(&[a.push]));
                    dev.cmd_draw_indexed(cmd, am.num_indices, 1, 0, 0, 0);
                };
                for a in alias_draws
                    .iter()
                    .filter(|a| a.push.light[3] >= 1.0 && !a.viewmodel)
                {
                    draw_alias(a, p.alias);
                }
                ts(5);
                // translucent: world liquids first, then sorted entities
                if let Some(wg) = &self.world {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                    dev.cmd_bind_index_buffer(cmd, dynbuf, 0, vk::IndexType::UINT32);
                    for wd in world_draws.iter().filter(|wd| wd.alpha >= 1.0) {
                        let wa = wd.water_alpha.map(|r| (r, p.water_alpha));
                        draw_world(wd, &wa);
                    }
                }
                for (_, tr) in &trans {
                    match *tr {
                        Trans::World(i) => {
                            if let Some(wg) = &self.world {
                                let wd = &world_draws[i];
                                dev.cmd_bind_vertex_buffers(cmd, 0, &[wg.vbuf.buf], &[0]);
                                dev.cmd_bind_index_buffer(cmd, dynbuf, 0, vk::IndexType::UINT32);
                                draw_world(
                                    wd,
                                    &[
                                        (wd.opaque, p.world_alpha),
                                        (wd.sky, p.sky),
                                        (wd.water[0], p.water_alpha),
                                        (wd.water_alpha[0], p.water_alpha),
                                    ],
                                );
                            }
                        }
                        Trans::Alias(i) => draw_alias(&alias_draws[i], p.alias_alpha),
                        Trans::Sprite(i) => {
                            let s = &sprite_draws[i];
                            dev.cmd_bind_pipeline(cmd, gfx, p.poly);
                            dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[s.vofs]);
                            push(bytes_of(&[PolyPush {
                                color: [1.0, 1.0, 1.0, s.alpha],
                                tex: s.tex,
                            }]));
                            dev.cmd_draw(cmd, 6, 1, 0, 0);
                        }
                    }
                }
                ts(6);
                if particles.count > 0 {
                    dev.cmd_bind_pipeline(cmd, gfx, p.particle);
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[particles.ofs]);
                    dev.cmd_draw(cmd, 6, particles.count, 0, 0);
                }
                if let Some((vofs, iofs, batches)) = &fte {
                    dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[*vofs]);
                    dev.cmd_bind_index_buffer(cmd, dynbuf, *iofs, vk::IndexType::UINT32);
                    for (pipe, slot, b) in batches {
                        dev.cmd_bind_pipeline(cmd, gfx, p.fte[*pipe]);
                        let fogmix = if matches!(b.blend, crate::FteBlend::Blend | crate::FteBlend::BlendColour) { 1.0 } else { 0.0 };
                        push(bytes_of(&[PolyPush { color: [fogmix, 0.0, 0.0, 0.0], tex: *slot }]));
                        dev.cmd_draw_indexed(cmd, b.num_indices, 1, b.first_index, 0, 0);
                    }
                }
                let vm: Vec<&AliasDraw> = alias_draws.iter().filter(|a| a.viewmodel).collect();
                if !vm.is_empty() {
                    dev.cmd_set_viewport(
                        cmd,
                        0,
                        &[vk::Viewport {
                            max_depth: 0.3,
                            ..vp_scene
                        }],
                    );
                    for a in vm {
                        draw_alias(
                            a,
                            if a.push.light[3] < 1.0 {
                                p.alias_alpha
                            } else {
                                p.alias
                            },
                        );
                    }
                }
            }
            dev.cmd_end_rendering(cmd);
            ts(7);
            gpu::barrier(
                &dev,
                cmd,
                t.color.image,
                crng,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            );

            // post: gamma/contrast/polyblend to the output
            let (out_img, out_view) = match (&self.swap, &t.out) {
                (Some(s), _) => (s.images[img_idx as usize], s.views[img_idx as usize]),
                (None, Some((o, _))) => (o.image, o.view),
                _ => return Ok(()),
            };
            gpu::barrier(
                &dev,
                cmd,
                out_img,
                crng,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            );
            let oatt = [vk::RenderingAttachmentInfo::default()
                .image_view(out_view)
                .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                .load_op(vk::AttachmentLoadOp::DONT_CARE)
                .store_op(vk::AttachmentStoreOp::STORE)];
            let ri = vk::RenderingInfo::default()
                .render_area(area)
                .layer_count(1)
                .color_attachments(&oatt);
            dev.cmd_begin_rendering(cmd, &ri);
            dev.cmd_set_viewport(cmd, 0, &[vp_full]);
            dev.cmd_set_scissor(cmd, 0, &[area]);
            dev.cmd_bind_descriptor_sets(cmd, gfx, p.layout, 0, &set, &[]);
            dev.cmd_bind_pipeline(cmd, gfx, p.post);
            push(bytes_of(&[PostPush {
                blend: fr.blend,
                gamma: st.gamma.clamp(0.1, 4.0),
                contrast: st.contrast.clamp(0.1, 4.0),
                scene: t.color_slot,
                dither: match st.softemu {
                    0 => st.dither / 255.0,
                    1 => NOISESCALE * st.dither * st.dither_screen,
                    _ => 0.0,
                },
                mode: st.softemu.min(2),
                scale: self.render_scale,
                colorblind: st.colorblind.min(3),
            }]));
            dev.cmd_draw(cmd, 3, 1, 0, 0);
            ts(8);
            // 2D on top at output resolution, unaffected by render scale
            if let Some(ofs) = ui_ofs {
                dev.cmd_bind_pipeline(cmd, gfx, p.ui);
                dev.cmd_bind_vertex_buffers(cmd, 0, &[dynbuf], &[ofs]);
                push(bytes_of(&[[1.0 / w as f32, 1.0 / h as f32]]));
                dev.cmd_draw(cmd, ui.len() as u32, 1, 0, 0);
            }
            dev.cmd_end_rendering(cmd);
            if let Some(sw) = &self.swap {
                let mut last = vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL;
                if std::mem::take(&mut self.capture_requested) && sw.readable {
                    if let Some((b, _, _)) = self.capture.take() {
                        self.gpu.free_buffer(b);
                    }
                    let buf = self.gpu.buffer(
                        (w * h * 4) as u64,
                        vk::BufferUsageFlags::TRANSFER_DST,
                        true,
                    )?;
                    gpu::barrier(
                        &dev,
                        cmd,
                        out_img,
                        crng,
                        last,
                        vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    );
                    last = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
                    let region = [vk::BufferImageCopy {
                        image_subresource: vk::ImageSubresourceLayers {
                            aspect_mask: vk::ImageAspectFlags::COLOR,
                            mip_level: 0,
                            base_array_layer: 0,
                            layer_count: 1,
                        },
                        image_extent: vk::Extent3D {
                            width: w,
                            height: h,
                            depth: 1,
                        },
                        ..Default::default()
                    }];
                    dev.cmd_copy_image_to_buffer(cmd, out_img, last, buf.buf, &region);
                    self.capture = Some((buf, w, h));
                }
                gpu::barrier(
                    &dev,
                    cmd,
                    out_img,
                    crng,
                    last,
                    vk::ImageLayout::PRESENT_SRC_KHR,
                );
            } else if let Some((_, buf)) = &t.out {
                gpu::barrier(
                    &dev,
                    cmd,
                    out_img,
                    crng,
                    vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                );
                let region = [vk::BufferImageCopy {
                    image_subresource: vk::ImageSubresourceLayers {
                        aspect_mask: vk::ImageAspectFlags::COLOR,
                        mip_level: 0,
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    image_extent: vk::Extent3D {
                        width: w,
                        height: h,
                        depth: 1,
                    },
                    ..Default::default()
                }];
                // The readback buffer is shared by frames in flight: order
                // this copy after the previous frame's copy and host read.
                let rb = [vk::MemoryBarrier::default()
                    .src_access_mask(vk::AccessFlags::TRANSFER_WRITE)
                    .dst_access_mask(vk::AccessFlags::TRANSFER_WRITE)];
                dev.cmd_pipeline_barrier(
                    cmd,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::PipelineStageFlags::TRANSFER,
                    vk::DependencyFlags::empty(),
                    &rb,
                    &[],
                    &[],
                );
                dev.cmd_copy_image_to_buffer(
                    cmd,
                    out_img,
                    vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                    buf.buf,
                    &region,
                );
            }
            ts(9);
            dev.end_command_buffer(cmd)?;

            let cmds = [cmd];
            let wait_stage = [vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT];
            let wait = [acquire];
            if let (Some(s), Some(scf)) = (&self.swap, &self.gpu.swapchain_fn) {
                let sig = [s.render_sems[img_idx as usize]];
                let si = [vk::SubmitInfo::default()
                    .wait_semaphores(&wait)
                    .wait_dst_stage_mask(&wait_stage)
                    .command_buffers(&cmds)
                    .signal_semaphores(&sig)];
                dev.queue_submit(self.gpu.queue, &si, fence)?;
                let scs = [s.sc];
                let idx = [img_idx];
                let pi = vk::PresentInfoKHR::default()
                    .wait_semaphores(&sig)
                    .swapchains(&scs)
                    .image_indices(&idx);
                match scf.queue_present(self.gpu.queue, &pi) {
                    Ok(false) => {}
                    Ok(true) | Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => self.needs_resize = true,
                    Err(e) => return Err(e.into()),
                }
            } else {
                let si = [vk::SubmitInfo::default().command_buffers(&cmds)];
                dev.queue_submit(self.gpu.queue, &si, fence)?;
            }
        }
        self.cur = (self.cur + 1) % FRAMES;
        self.frame_count += 1;
        Ok(())
    }
}
