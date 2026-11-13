use super::*;

/// Per-precache model info (the server-side parts of qmodel_t).
#[derive(Debug, Clone)]
pub struct ModelInfo {
    pub name: String,
    pub brush: bool,
    pub mins: Vec3,
    pub maxs: Vec3,
}

/// entity_state_t baseline.
#[derive(Debug, Clone, Copy, Default)]
pub struct Baseline {
    pub origin: [f32; 3],
    pub angles: [f32; 3],
    pub modelindex: i32,
    pub frame: i32,
    pub colormap: i32,
    pub skin: i32,
    pub effects: i32,
    pub alpha: u8,
    pub scale: u8,
}

/// Engine-side per-edict data (the non-QC parts of edict_t).
#[derive(Debug, Clone, Default)]
pub struct EdictExt {
    pub baseline: Baseline,
    pub alpha: u8,
    pub leafs: Vec<u32>,
    pub sendinterval: bool,
    pub oldframe: f32,
    pub oldthinktime: f32,
}

/// Monster/secret totals from the QC globals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelStats {
    pub total_secrets: i32,
    pub found_secrets: i32,
    pub total_monsters: i32,
    pub killed_monsters: i32,
}

/// Per-level server state (`server_t` minus the VM and world).
#[derive(Debug, Default)]
pub struct Sv {
    pub name: String,
    pub modelname: String,
    pub active: bool,
    pub loading: bool,
    pub paused: bool,
    pub protocol: u32,
    pub protocolflags: u32,
    pub model_precache: Vec<String>,
    pub models: Vec<Option<ModelInfo>>,
    pub sound_precache: Vec<String>,
    /// particleeffectnum names (index 0 unused).
    pub particle_precache: Vec<String>,
    pub lightstyles: Vec<Vec<u8>>,
    pub datagram: SizeBuf,
    pub reliable_datagram: SizeBuf,
    pub signon: Vec<SizeBuf>,
    pub fields: Fields,
    pub globals: Globals,
    pub ext: Vec<EdictExt>,
    pub vis: Vis,
    pub lastcheck: u32,
    pub lastchecktime: f64,
    pub checkpvs: Vec<u8>,
    pub effects_mask: i32,
    pub phys: PhysParams,
    /// Host_Error: the level stops running.
    pub error: Option<VmError>,
    /// Level restored from a savegame: clients keep their saved edicts.
    pub loadgame: bool,
    pub(crate) loadgame_parms: [f32; 16],
    pub(crate) brush_cache: HashMap<String, Arc<CollisionModel>>,
    pub(crate) parse_client_command: Option<usize>,
}

/// Everything QC builtins can reach besides the VM and the world.
pub struct Core {
    pub fs: Fs,
    pub cvars: Cvars,
    pub cbuf: Cbuf,
    pub clients: Vec<Client>,
    pub serverflags: f32,
    pub changelevel_issued: bool,
    /// This game was started with add-ons (fixed until the next new game or load).
    pub addons_on: bool,
    /// Skip the per-frame entity updates in client datagrams (tools that
    /// drive a local client and throw its datagrams away, like qbot).
    pub skip_entity_updates: bool,
    /// False for hipnotic/rogue (weapon stat is sent as a bit index).
    pub standard_quake: bool,
    /// Console output.
    pub log: Vec<String>,
    /// Re-release localization (`$key` strings, LOC_Format).
    pub loc: crate::loc::Localization,
    pub sv: Sv,
    /// Client whose command is executing (`host_client`); None = console.
    pub cmd_client: Option<usize>,
    pub(crate) cmd_names: Vec<String>,
    pub(crate) queued_client_cmds: Vec<(usize, String)>,
}

/// The QC VM and collision world of the running level.
pub struct Level {
    pub vm: Vm,
    pub world: World,
    pub(crate) assist: Vec<crate::assists::ClientAssist>,
    /// Portal gun state (empty unless add-ons are in use).
    pub portals: crate::addons::portal::Portals,
    /// Monster fall damage while the portal gun is on.
    pub(crate) falls: crate::addons::fall::Falls,
    /// Hookshot state and the map's surface materials.
    pub hooks: crate::addons::hookshot::Hooks,
}

pub struct Server {
    pub core: Core,
    pub level: Option<Level>,
    pub(crate) cmds: Commands<Server>,
}
