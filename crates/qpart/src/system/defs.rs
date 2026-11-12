use super::*;

#[derive(Clone, Debug, Default)]
pub struct Particle {
    pub(super) next: u32,
    pub die: f32,
    pub org: Vec3,
    pub rgba: [f32; 4],
    pub scale: f32,
    pub s1: f32,
    pub t1: f32,
    pub s2: f32,
    pub t2: f32,
    /// Previous traced position (throttles traces).
    pub oldorg: Vec3,
    pub vel: Vec3,
    pub angle: f32,
    pub rotationspeed: f32,
    /// Next emit time (types with `emitinterval` >= 0).
    pub nextemit: f32,
    /// Trail state (types with `emitinterval` < 0).
    pub trail: Option<TrailHandle>,
}

#[derive(Clone, Debug, Default)]
pub struct BeamSeg {
    pub(super) next: u32,
    /// Particle index.
    pub p: u32,
    pub flags: u8,
    pub dir: Vec3,
    pub texture_s: f32,
}

#[derive(Clone, Debug, Default)]
pub struct Decal {
    pub(super) next: u32,
    pub die: f32,
    /// 0: world, >0: entity the vertices are relative to.
    pub entity: i32,
    pub vertex: [Vec3; 3],
    pub texcoords: [[f32; 2]; 3],
    pub valpha: [f32; 3],
    pub rgba: [f32; 4],
}

/// Owner-side reference to a trailstate (the C `trailstate_t **` key).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrailHandle {
    pub(super) idx: u16,
    pub(super) serial: u32,
}

#[derive(Clone, Debug, Default)]
pub(super) struct TrailState {
    pub(super) serial: u32,
    pub(super) assoc: Option<TrailHandle>,
    pub(super) lastbeam: u32,
    /// lastdist / statetime.
    pub(super) state1: f32,
    /// laststop / emittime.
    pub(super) state2: f32,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct TypeRt {
    pub(super) particles: u32,
    pub(super) beams: u32,
    pub(super) decals: u32,
    pub(super) in_run: bool,
    pub(super) countoverflow: f32,
}

impl Default for TypeRt {
    fn default() -> Self {
        TypeRt { particles: NIL, beams: NIL, decals: NIL, in_run: false, countoverflow: 0.0 }
    }
}

/// Cvar-backed settings.
#[derive(Clone, Debug)]
pub struct Settings {
    /// r_part_density
    pub density: f32,
    /// r_part_contentswitch
    pub contentswitch: bool,
    /// r_bouncysparks
    pub bouncysparks: bool,
    /// r_particle_tracelimit (per frame)
    pub trace_limit: u32,
    /// r_part_rain
    pub rain: bool,
    /// r_part_rain_quantity
    pub rain_quantity: f32,
    /// r_decal_noperpendicular
    pub decal_noperpendicular: bool,
    /// r_lightflicker
    pub lightflicker: bool,
    /// r_part_maxparticles
    pub max_particles: usize,
    /// r_part_maxdecals
    pub max_decals: usize,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            density: 1.0,
            contentswitch: true,
            bouncysparks: true,
            trace_limit: 16_777_216,
            rain: true,
            rain_quantity: 1.0,
            decal_noperpendicular: true,
            lightflicker: true,
            max_particles: 65536,
            max_decals: 8192,
        }
    }
}

/// Result of a world trace (CL_TraceLine).
#[derive(Clone, Copy, Debug)]
pub struct Trace {
    /// 1 when nothing was hit.
    pub frac: f32,
    pub endpos: Vec3,
    pub normal: Vec3,
    /// Entity hit (0 = world).
    pub entity: i32,
}

/// What the particle system needs from the client world.
pub trait World {
    /// Whether a world model is loaded (`cl.worldmodel`).
    fn has_world(&self) -> bool {
        true
    }
    fn trace_line(&self, start: Vec3, end: Vec3) -> Trace;
    /// FTECONTENTS_* mask at `p`.
    fn point_contents(&self, p: Vec3) -> u32;
    /// Origin of a brush entity with a model, None if it has none.
    fn entity_origin(&self, _entity: i32) -> Option<Vec3> {
        None
    }
    /// Calls `out` with each surface polygon (in the model's space) of
    /// `entity`'s model (0 = world) near `center` within `radius`, skipping
    /// sky/water, and with `noperp` only surfaces facing against `normal`
    /// (dot(surface normal, normal) <= -0.5). Q1BSP_ClipDecalToNodes.
    fn decal_polys(&self, _entity: i32, _center: Vec3, _radius: f32, _normal: Vec3, _noperp: bool, _out: &mut dyn FnMut(&[Vec3])) {}
}

/// A world with nothing in it.
pub struct EmptyWorld;

impl World for EmptyWorld {
    fn has_world(&self) -> bool {
        false
    }
    fn trace_line(&self, _start: Vec3, end: Vec3) -> Trace {
        Trace { frac: 1.0, endpos: end, normal: Vec3::ZERO, entity: 0 }
    }
    fn point_contents(&self, _p: Vec3) -> u32 {
        FTECONTENTS_EMPTY
    }
}

/// Receives renderable state during [`System::frame`].
pub trait Sink {
    fn particle(&mut self, _ty: TypeId, _p: &Particle) {}
    /// Segment from `p` (of `seg`) to `q` (of `next`).
    fn beam(&mut self, _ty: TypeId, _seg: &BeamSeg, _next: &BeamSeg, _p: &Particle, _q: &Particle) {}
    fn decal(&mut self, _ty: TypeId, _d: &Decal) {}
}

pub struct NullSink;
impl Sink for NullSink {}

#[derive(Clone, Debug, PartialEq)]
pub struct DlightSpawn {
    pub key: i32,
    pub org: Vec3,
    pub radius: f32,
    /// Absolute client time.
    pub die: f64,
    pub decay: f32,
    pub rgb: [f32; 3],
}

#[derive(Clone, Debug, PartialEq)]
pub struct SoundSpawn {
    pub name: String,
    pub org: Vec3,
    pub vol: f32,
    pub atten: f32,
}

/// One rain-emitting triangle (skytris_t).
#[derive(Clone, Debug)]
pub struct SkyTri {
    pub org: Vec3,
    pub x: Vec3,
    pub y: Vec3,
    pub area: f32,
    pub nexttime: f64,
    pub ptype: TypeId,
    /// Outward surface normal (already flipped for SURF_PLANEBACK).
    pub normal: Vec3,
}

/// Fans a surface polygon into sky triangles (R_Part_SkyTri).
pub fn sky_tris_for_polygon(verts: &[Vec3], normal: Vec3, ptype: TypeId) -> Vec<SkyTri> {
    let mut out = Vec::new();
    if verts.len() >= 64 {
        return out;
    }
    let v1 = 0;
    let mut v2 = 1;
    for v3 in 2..verts.len() {
        let org = verts[v1];
        let x = verts[v2] - org;
        let y = verts[v3] - org;
        let (xm, ym) = (x.length(), y.length());
        let theta = (x.dot(y) / (xm * ym)).acos();
        let area = psin(theta) * xm * ym;
        if area > 0.0 {
            out.push(SkyTri { org, x, y, area, nexttime: 0.0, ptype, normal });
        }
        v2 = v3;
    }
    out
}

#[derive(Debug, Default)]
pub(super) struct SkyModel {
    pub(super) tris: Vec<SkyTri>,
    pub(super) skytime: f64,
}

/// A brush entity whose sky triangles emit rain this frame.
#[derive(Clone, Copy, Debug)]
pub struct RainEmitter {
    pub model: usize,
    /// forward, right, up (AngleVectors).
    pub axis: [Vec3; 3],
    pub origin: Vec3,
}

/// Per-frame inputs to [`System::frame`].
#[derive(Clone, Debug, Default)]
pub struct FrameInput {
    pub cl_time: f64,
    pub vieworg: Vec3,
    pub rain: Vec<RainEmitter>,
}

/// Classic effect fallbacks (PE_DEFAULT etc.).
#[derive(Clone, Copy, Debug, Default)]
pub struct PeTypes {
    pub default: Option<TypeId>,
    pub size2: Option<TypeId>,
    pub size3: Option<TypeId>,
    pub defaulttrail: Option<TypeId>,
}

pub(super) struct DeferredTrail {
    pub(super) start: Vec3,
    pub(super) end: Vec3,
    pub(super) ty: TypeId,
    /// Particle owning the trailstate.
    pub(super) owner: Option<u32>,
}

pub(super) struct DeferredDecal {
    pub(super) ty: TypeId,
    pub(super) entity: i32,
    pub(super) center: Vec3,
    pub(super) normal: Vec3,
    pub(super) scale: f32,
}

pub(super) struct DecalCtx {
    pub(super) ty: TypeId,
    pub(super) entity: i32,
    pub(super) center: Vec3,
    pub(super) normal: Vec3,
    pub(super) tangent1: Vec3,
    pub(super) tangent2: Vec3,
    pub(super) scale0: f32,
    pub(super) scale1: f32,
    pub(super) scale2: f32,
    pub(super) bias1: f32,
    pub(super) bias2: f32,
}
