use super::*;

pub(super) struct Ofs {
    pub(super) origin: usize,
    pub(super) absmin: usize,
    pub(super) absmax: usize,
    pub(super) health: usize,
    pub(super) flags: usize,
    pub(super) solid: usize,
    pub(super) takedamage: usize,
    pub(super) movetype: usize,
    pub(super) touch: usize,
    pub(super) state: Option<usize>,
    pub(super) waterlevel: usize,
    pub(super) view_ofs: usize,
    pub(super) target: Option<usize>,
    pub(super) targetname: Option<usize>,
    pub(super) pos1: Option<usize>,
    pub(super) pos2: Option<usize>,
    pub(super) maxs: usize,
    pub(super) owner: Option<usize>,
    pub(super) items: Option<usize>,
}

impl Ofs {
    pub(super) fn new(vm: &Vm) -> Option<Ofs> {
        let f = |n| vm.field_ofs(n);
        Some(Ofs {
            origin: f("origin")?,
            absmin: f("absmin")?,
            absmax: f("absmax")?,
            health: f("health")?,
            flags: f("flags")?,
            solid: f("solid")?,
            takedamage: f("takedamage")?,
            movetype: f("movetype")?,
            touch: f("touch")?,
            state: f("state"),
            waterlevel: f("waterlevel")?,
            view_ofs: f("view_ofs")?,
            target: f("target"),
            targetname: f("targetname"),
            pos1: f("pos1"),
            pos2: f("pos2"),
            maxs: f("maxs")?,
            owner: f("owner"),
            items: f("items"),
        })
    }
}

/// `QBOT_DEBUG=1`: lift, entry and node dumps while building the graph.
pub(super) fn debug() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("QBOT_DEBUG").is_some())
}

/// `QBOT_TRACE=1`: the bot's position and move twice a second.
pub(super) fn trace() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("QBOT_TRACE").is_some())
}

pub(super) fn v(a: [f32; 3]) -> Vec3 {
    Vec3::from_array(a)
}

/// Yaw/pitch (Quake view convention: positive pitch looks down) from `d`.
pub(super) fn look_angles(d: Vec3) -> [f32; 3] {
    let yaw = d.y.atan2(d.x).to_degrees();
    let pitch = -(d.z.atan2((d.x * d.x + d.y * d.y).sqrt())).to_degrees();
    [pitch, yaw, 0.0]
}
