//! Server state, level spawning (SV_SpawnServer), the frame loop
//! (Host_ServerFrame / SV_Physics / SV_RunClients) and client connection.

use std::collections::HashMap;
use std::sync::Arc;

use qcore::cmd::{Cbuf, CmdContext, Commands};
use qcore::cvar::{CVAR_ARCHIVE, CVAR_NOTIFY, CVAR_SERVERINFO, Cvars};
use qcore::fs::Fs;
use qvm::{SpawnFilter, Vm, VmError, ofs_parm};
use qworld::{CollisionModel, PhysParams, SOLID_BSP, MOVETYPE_PUSH, MOVETYPE_STEP, MOVETYPE_WALK, Vec3, World};

use crate::client::{Client, ClientMove, Signon};
use crate::ents::Ents;
use crate::fields::{Fields, Globals};
use crate::host::Ctx;
use crate::msg::*;
use crate::vis::Vis;

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
}

pub struct Server {
    pub core: Core,
    pub level: Option<Level>,
    pub(crate) cmds: Commands<Server>,
}

const MAX_LOG: usize = 4096;

impl Core {
    pub fn print(&mut self, s: &str) {
        if self.log.len() >= MAX_LOG {
            self.log.drain(..MAX_LOG / 2);
        }
        self.log.push(s.to_string());
    }

    pub fn maxclients(&self) -> usize {
        self.clients.len()
    }

    /// Particle extensions on: `sv_fteparticles` and a single-player
    /// server (our svcdp_* messages would break other clients; vkQuake
    /// gates them on the client's protocol extensions).
    pub fn fte_particles(&self) -> bool {
        self.cvars.value_of("sv_fteparticles") != 0.0 && self.clients.len() == 1
    }

    /// Record a Host_Error (first one wins).
    pub fn host_error(&mut self, e: VmError) {
        if self.sv.error.is_none() {
            self.print(&format!("Host_Error: {e}\n"));
            self.sv.error = Some(e);
        }
    }

    /// SV_ModelIndex (0 for "" or unknown).
    pub fn model_index(&self, name: &[u8]) -> i32 {
        if name.is_empty() {
            return 0;
        }
        self.sv.model_precache.iter().position(|m| m.as_bytes() == name).unwrap_or(0) as i32
    }

    pub(crate) fn ext(&mut self, e: u32) -> &mut EdictExt {
        let i = e as usize;
        if self.sv.ext.len() <= i {
            self.sv.ext.resize(i + 1, EdictExt::default());
        }
        &mut self.sv.ext[i]
    }

    /// SV_ReserveSignonSpace + current signon buffer.
    pub(crate) fn signon(&mut self, need: usize) -> &mut SizeBuf {
        let full = self.sv.signon.last().is_none_or(|s| s.len() + need > s.maxsize);
        if full && self.sv.signon.len() < MAX_SIGNON_BUFFERS {
            self.sv.signon.push(SizeBuf::new(SIGNON_SIZE));
        }
        self.sv.signon.last_mut().unwrap()
    }
}

/// Run QC function `f` (no-op for 0 or after a Host_Error).
pub(crate) fn exec(vm: &mut Vm, c: &mut Core, w: &mut World, f: i32) {
    if f <= 0 || c.sv.error.is_some() {
        return;
    }
    let r = {
        let mut h = Ctx { c: &mut *c, w };
        vm.execute(&mut h, f as usize)
    };
    if let Err(e) = r {
        c.host_error(e);
    }
}

/// ED_Free with the unlink the VM requires.
pub(crate) fn ed_free(vm: &mut Vm, c: &mut Core, w: &mut World, e: u32) {
    w.unlink(e);
    vm.ed_free(e);
    let x = c.ext(e);
    x.alpha = ENTALPHA_DEFAULT;
    x.leafs.clear();
}

impl CmdContext for Server {
    fn cbuf(&mut self) -> &mut Cbuf {
        &mut self.core.cbuf
    }
    fn cvars(&mut self) -> &mut Cvars {
        &mut self.core.cvars
    }
    fn commands(&mut self) -> &mut Commands<Self> {
        &mut self.cmds
    }
    fn print(&mut self, text: &str) {
        self.core.print(text);
    }
}

fn register_cvars(cv: &mut Cvars) {
    for (n, d, f) in [
        ("skill", "1", 0),
        ("deathmatch", "0", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("coop", "0", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("teamplay", "0", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("fraglimit", "0", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("timelimit", "0", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("samelevel", "0", 0),
        ("noexit", "0", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("nomonsters", "0", 0),
        ("temp1", "0", 0),
        ("hostname", "UNNAMED", 0),
        ("developer", "0", 0),
        ("sv_gravity", "800", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("sv_friction", "4", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("sv_stopspeed", "100", 0),
        ("sv_maxvelocity", "2000", 0),
        ("sv_nostep", "0", 0),
        ("edgefriction", "2", 0),
        ("sv_accelerate", "10", 0),
        ("sv_maxspeed", "320", CVAR_NOTIFY | CVAR_SERVERINFO),
        ("sv_idealpitchscale", "0.8", 0),
        ("sv_aim", "1", 0),
        ("sv_altnoclip", "1", CVAR_ARCHIVE),
        ("sv_gameplayfix_elevators", "2", 0),
        ("sv_freezenonclients", "0", 0),
        ("sv_protocol", "999", 0),
        ("max_edicts", "16384", 0),
        ("cl_rollangle", "2.0", 0),
        ("cl_rollspeed", "200", 0),
        ("external_ents", "1", CVAR_ARCHIVE),
        ("registered", "1", 0),
        ("pr_checkextension", "1", 0),
        // FTE/DP particle builtins + svcdp_* messages (single player only).
        ("sv_fteparticles", "1", 0),
    ] {
        cv.register(n, d, f);
    }
    for &(n, d, f) in crate::assists::CVARS {
        cv.register(n, d, f);
    }
}

/// MAX_SCOREBOARD: most client slots.
pub const MAX_SCOREBOARD: usize = 16;

impl Server {
    /// A server with `maxclients` slots (1 for single player).
    pub fn new(fs: Fs, mut cvars: Cvars, maxclients: usize) -> Self {
        register_cvars(&mut cvars);
        let game = fs.game_dir().unwrap_or("").to_ascii_lowercase();
        let standard_quake = !matches!(game.as_str(), "hipnotic" | "rogue" | "quoth");
        let loc = crate::loc::Localization::load(&fs);
        let mut s = Server {
            core: Core {
                loc,
                fs,
                cvars,
                cbuf: Cbuf::new(),
                clients: (0..maxclients.max(1)).map(|_| Client::default()).collect(),
                serverflags: 0.0,
                changelevel_issued: false,
                skip_entity_updates: false,
                standard_quake,
                log: Vec::new(),
                sv: Sv::default(),
                cmd_client: None,
                cmd_names: Vec::new(),
                queued_client_cmds: Vec::new(),
            },
            level: None,
            cmds: Commands::with_builtins(),
        };
        crate::commands::register(&mut s);
        s
    }

    /// Resize the client slots (no server may be running).
    pub fn set_maxclients(&mut self, n: usize) {
        self.core.clients = (0..n.clamp(1, MAX_SCOREBOARD)).map(|_| Client::default()).collect();
    }

    pub(crate) fn add_cmd(&mut self, name: &str, f: qcore::cmd::CmdFn<Server>) {
        self.cmds.add(name, f);
        self.core.cmd_names.push(name.to_ascii_lowercase());
    }

    /// True if `name` is a server console command.
    pub fn has_command(&self, name: &str) -> bool {
        self.cmds.exists(name)
    }

    /// Server command names starting with `partial` (console completion).
    pub fn complete_command(&self, partial: &str) -> Vec<&str> {
        self.cmds.complete(partial)
    }

    /// Host_Error message of the current level, if any.
    pub fn error(&self) -> Option<&VmError> {
        self.core.sv.error.as_ref()
    }

    pub fn active(&self) -> bool {
        self.level.is_some() && self.core.sv.active
    }

    /// Server time (`qcvm->time`).
    pub fn time(&self) -> f64 {
        self.level.as_ref().map_or(0.0, |l| l.vm.time)
    }

    pub fn stats(&self) -> LevelStats {
        let Some(l) = &self.level else { return LevelStats::default() };
        let g = &self.core.sv.globals;
        LevelStats {
            total_secrets: l.vm.g_f(g.total_secrets) as i32,
            found_secrets: l.vm.g_f(g.found_secrets) as i32,
            total_monsters: l.vm.g_f(g.total_monsters) as i32,
            killed_monsters: l.vm.g_f(g.killed_monsters) as i32,
        }
    }

    /// Take queued console text (QC `localcmd`, changelevel) so a host with
    /// a single shared command buffer can run it.
    pub fn take_text(&mut self) -> String {
        self.core.cbuf.take_text()
    }

    /// Queue console text (Cbuf_AddText).
    pub fn add_text(&mut self, text: &str) {
        self.core.cbuf.add_text(text);
    }

    /// Cbuf_Execute.
    pub fn execute(&mut self) {
        qcore::cmd::execute(self);
    }

    /// Execute one console command immediately.
    pub fn command(&mut self, text: &str) {
        qcore::cmd::execute_string(self, text);
    }

    /// SV_SpawnServer. Errors (missing map/progs, QC errors while spawning) are
    /// also recorded as the level error.
    pub fn spawn_server(&mut self, mapname: &str) -> Result<(), String> {
        let r = self.spawn_inner(mapname);
        if let Err(e) = &r {
            self.core.print(&format!("Couldn't spawn server {mapname}: {e}\n"));
            self.core.sv.active = false;
            if self.core.sv.error.is_none() {
                self.core.sv.error = Some(VmError::new(e.clone()));
            }
        }
        r
    }

    fn spawn_inner(&mut self, mapname: &str) -> Result<(), String> {
        let c = &mut self.core;
        c.changelevel_issued = false;
        if c.cvars.value_of("coop") != 0.0 {
            c.cvars.set("deathmatch", "0");
        }
        let skill = ((c.cvars.value_of("skill") + 0.5) as i32).clamp(0, 3);
        c.cvars.set_value("skill", skill as f32);

        self.level = None;
        let maxclients = c.maxclients();
        let protocol = match c.cvars.value_of("sv_protocol") as u32 {
            PROTOCOL_NETQUAKE => PROTOCOL_NETQUAKE,
            PROTOCOL_RMQ => PROTOCOL_RMQ,
            _ => PROTOCOL_FITZQUAKE,
        };
        let progs = c.fs.open("progs.dat").ok_or("couldn't load progs.dat")?;
        let max_edicts = (c.cvars.value_of("max_edicts") as usize).clamp(256, 32000);
        let mut vm = Vm::load(&progs, max_edicts, maxclients).map_err(|e| e.to_string())?;
        let fields = Fields::new(&vm).map_err(|e| e.to_string())?;
        let globals = Globals::new(&vm).map_err(|e| e.to_string())?;

        let modelname = format!("maps/{mapname}.bsp");
        let data = c.fs.open(&modelname).ok_or_else(|| format!("{modelname} not found"))?;
        let bsp = qformats::Bsp::parse(&data).map_err(|e| format!("{modelname}: {e:?}"))?;
        let cm = Arc::new(CollisionModel::from_bsp(&bsp));
        let mut world = World::new(cm.clone());

        let has_global = |vm: &Vm, n: &str, v: f32| vm.global_ofs(n).is_some_and(|o| vm.g_f(o) == v);
        let isqex = has_global(&vm, "EF_QUADLIGHT", 16.0)
            && (has_global(&vm, "EF_PENTLIGHT", 32.0) || has_global(&vm, "EF_PENTALIGHT", 32.0));
        let parse_client_command = vm.progs.find_function("SV_ParseClientCommand");

        c.sv = Sv {
            name: mapname.to_string(),
            modelname: modelname.clone(),
            loading: true,
            protocol,
            protocolflags: if protocol == PROTOCOL_RMQ { PRFL_INT32COORD | PRFL_SHORTANGLE } else { 0 },
            model_precache: vec![String::new(), modelname.clone()],
            models: vec![None, None],
            sound_precache: vec![String::new()],
            particle_precache: vec![String::new()],
            lightstyles: vec![Vec::new(); MAX_LIGHTSTYLES],
            datagram: SizeBuf::new(MAX_DATAGRAM),
            reliable_datagram: SizeBuf::new(MAX_DATAGRAM),
            signon: vec![SizeBuf::new(SIGNON_SIZE)],
            fields,
            globals,
            ext: vec![EdictExt::default(); vm.max_edicts()],
            vis: Vis::new(cm.clone(), &bsp),
            effects_mask: if isqex { -1 } else { !(16 | 32 | 64) },
            parse_client_command,
            ..Sv::default()
        };
        let sub0 = cm.submodels.first();
        c.sv.models[1] = Some(ModelInfo {
            name: modelname.clone(),
            brush: true,
            mins: sub0.map_or(Vec3::ZERO, |m| m.mins),
            maxs: sub0.map_or(Vec3::ZERO, |m| m.maxs),
        });
        for (i, m) in cm.submodels.iter().enumerate().skip(1) {
            let name = format!("*{i}");
            c.sv.model_precache.push(name.clone());
            c.sv.models.push(Some(ModelInfo { name, brush: true, mins: m.mins, maxs: m.maxs }));
            world.set_model(1 + i, Some((cm.clone(), i)));
        }
        for cl in &mut c.clients {
            cl.spawned = false;
        }

        vm.time = 1.0;
        vm.seed_random(0x1234_5678);
        // world edict
        vm.edict_mut(0).fill(0);
        let f = &c.sv.fields;
        let h = vm.strings.engine(modelname.as_bytes());
        vm.set_ent_i(0, f.model, h);
        vm.set_ent_f(0, f.modelindex, 1.0);
        vm.set_ent_f(0, f.solid, SOLID_BSP as f32);
        vm.set_ent_f(0, f.movetype, MOVETYPE_PUSH as f32);
        let g = &c.sv.globals;
        let coop = c.cvars.value_of("coop");
        if coop != 0.0 {
            vm.set_g_f(g.coop, coop);
        } else {
            vm.set_g_f(g.deathmatch, c.cvars.value_of("deathmatch"));
        }
        let h = vm.strings.engine(mapname.as_bytes());
        vm.set_g_i(g.mapname, h);
        vm.set_g_f(g.serverflags, c.serverflags);

        let mut ents = bsp.entities.clone();
        if c.cvars.value_of("external_ents") != 0.0
            && let Some(e) = c.fs.open_companion(&format!("maps/{mapname}.bsp"), &format!("maps/{mapname}.ent"))
        {
            ents = String::from_utf8_lossy(&e).into_owned();
        }
        let filter = SpawnFilter {
            deathmatch: c.cvars.value_of("deathmatch") != 0.0,
            skill,
            nomonsters: c.cvars.value_of("nomonsters") != 0.0,
        };
        let r = {
            let mut h = Ctx { c: &mut *c, w: &mut world };
            vm.load_from_file(&mut h, &ents, filter)
        };
        if let Err(e) = r {
            c.host_error(e);
        }
        if let Some(e) = &c.sv.error {
            return Err(e.to_string());
        }
        c.sv.active = true;
        c.sv.loading = false;
        vm.protect_world = true;
        self.level = Some(Level { vm, world, assist: Vec::new() });

        // run two frames to allow everything to settle
        self.physics(0.1);
        self.physics(0.1);
        if let Some(e) = &self.core.sv.error {
            return Err(e.to_string());
        }
        self.create_baseline();
        for i in 0..self.core.maxclients() {
            if self.core.clients[i].active {
                self.send_serverinfo(i);
            }
        }
        for i in 0..self.core.maxclients() {
            if self.core.clients[i].active && self.core.clients[i].local {
                self.local_signon(i);
            }
        }
        Ok(())
    }

    /// Client-side half of the signon handshake for an in-process client.
    fn local_signon(&mut self, i: usize) {
        self.flush_client(i);
        self.client_command(i, "prespawn");
        self.flush_client(i);
        self.client_command(i, "spawn");
        self.flush_client(i);
        self.client_command(i, "begin");
    }

    /// SV_SendServerinfo.
    pub(crate) fn send_serverinfo(&mut self, i: usize) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let sv = &c.sv;
        let mut m = std::mem::take(&mut c.clients[i].message);
        m.byte(SVC_PRINT);
        m.string(format!("\x02\nFITZQUAKE 0.85 SERVER ({} CRC)\n", l.vm.progs.crc).as_bytes());
        m.byte(SVC_SERVERINFO);
        m.long(sv.protocol as i32);
        if sv.protocol == PROTOCOL_RMQ {
            m.long(sv.protocolflags as i32);
        }
        m.byte(c.clients.len() as i32);
        let dm = c.cvars.value_of("coop") == 0.0 && c.cvars.value_of("deathmatch") != 0.0;
        m.byte(if dm { GAME_DEATHMATCH } else { GAME_COOP });
        let msg_ofs = l.vm.field_ofs("message");
        let message = msg_ofs.and_then(|o| l.vm.ent_str(0, o).ok()).unwrap_or(b"").to_vec();
        m.string(&message);
        for (k, s) in sv.model_precache.iter().enumerate().skip(1) {
            if sv.protocol != PROTOCOL_NETQUAKE || k < 256 {
                m.string(s.as_bytes());
            }
        }
        m.byte(0);
        for (k, s) in sv.sound_precache.iter().enumerate().skip(1) {
            if sv.protocol != PROTOCOL_NETQUAKE || k < 256 {
                m.string(s.as_bytes());
            }
        }
        m.byte(0);
        if c.fte_particles() {
            for (k, name) in sv.particle_precache.iter().enumerate().skip(1) {
                crate::particles::write_precache(&mut m, k, name);
            }
        }
        let sounds = l.vm.field_ofs("sounds").map_or(0, |o| l.vm.ent_f(0, o) as i32);
        m.byte(SVC_CDTRACK);
        m.byte(sounds);
        m.byte(sounds);
        m.byte(SVC_SETVIEW);
        m.short(i as i32 + 1);
        m.byte(SVC_SIGNONNUM);
        m.byte(1);
        let cl = &mut c.clients[i];
        cl.message = m;
        cl.sendsignon = Signon::Flush;
        cl.spawned = false;
    }

    /// SV_ConnectClient for slot `i`.
    pub fn connect_client(&mut self, i: usize) {
        if i >= self.core.maxclients() || self.level.is_none() {
            return;
        }
        let cl = &mut self.core.clients[i];
        *cl = Client { active: true, name: "unconnected".into(), ..Client::default() };
        let l = self.level.as_mut().unwrap();
        let c = &mut self.core;
        let g = c.sv.globals.clone();
        if c.sv.loadgame {
            c.clients[i].spawn_parms = c.sv.loadgame_parms;
        } else {
            { let fnum = l.vm.g_i(g.f_set_new_parms); exec(&mut l.vm, c, &mut l.world, fnum); }
            for k in 0..NUM_SPAWN_PARMS {
                c.clients[i].spawn_parms[k] = l.vm.g_f(g.parm1 + k);
            }
        }
        self.send_serverinfo(i);
    }

    /// Connect an in-process client and run the whole signon sequence
    /// (prespawn/spawn/begin). Returns the slot.
    pub fn connect_local(&mut self) -> Option<usize> {
        let i = self.core.clients.iter().position(|c| !c.active)?;
        self.connect_client(i);
        if !self.core.clients[i].active {
            return None;
        }
        self.core.clients[i].local = true;
        self.core.clients[i].name = "player".into();
        self.local_signon(i);
        Some(i)
    }

    /// Set the input applied each frame (clc_move).
    pub fn set_move(&mut self, i: usize, mv: ClientMove) {
        if let Some(c) = self.core.clients.get_mut(i) {
            c.cmd = mv;
            c.new_move = true;
        }
    }

    /// SV_ReadClientMove's ping: `client_time` is the server time the client
    /// had last received (cl.mtime[0]) when it sent the move.
    pub fn record_ping(&mut self, i: usize, client_time: f32) {
        let now = self.level.as_ref().map_or(0.0, |l| l.vm.time) as f32;
        if let Some(c) = self.core.clients.get_mut(i) {
            let k = c.num_pings % crate::client::NUM_PING_TIMES;
            c.ping_times[k] = (now - client_time).max(0.0);
            c.num_pings += 1;
        }
    }

    /// Queue a clc_stringcmd from client `i` (processed next frame, or now via [`Server::client_command`]).
    pub fn queue_client_command(&mut self, i: usize, s: &str) {
        if let Some(c) = self.core.clients.get_mut(i) {
            c.stringcmds.push_back(s.to_string());
        }
    }

    /// Drain the delivered reliable messages of client `i`, in order. Each is
    /// one network message (never merged, so demos stay under MAX_MSGLEN).
    pub fn take_reliable(&mut self, i: usize) -> Vec<Vec<u8>> {
        self.core.clients.get_mut(i).map(|c| std::mem::take(&mut c.reliable_out)).unwrap_or_default()
    }

    /// Drain the delivered unreliable datagrams of client `i`.
    pub fn take_datagrams(&mut self, i: usize) -> Vec<Vec<u8>> {
        self.core.clients.get_mut(i).map(|c| c.datagrams.drain(..).collect()).unwrap_or_default()
    }

    /// Edict of client `i` (i + 1).
    pub fn client_edict(i: usize) -> u32 {
        i as u32 + 1
    }

    /// SV_DropClient.
    pub fn drop_client(&mut self, i: usize, crash: bool) {
        if !self.core.clients.get(i).is_some_and(|c| c.active) {
            return;
        }
        if !crash {
            let cl = &mut self.core.clients[i];
            cl.message.byte(SVC_DISCONNECT);
            let m = std::mem::take(&mut cl.message.data);
            cl.reliable_out.push(m);
            if cl.spawned
                && let Some(l) = &mut self.level
            {
                let c = &mut self.core;
                let g = c.sv.globals.clone();
                let saveself = l.vm.self_ent();
                l.vm.set_self(i as u32 + 1);
                { let fnum = l.vm.g_i(g.f_client_disconnect); exec(&mut l.vm, c, &mut l.world, fnum); }
                l.vm.set_self(saveself);
            }
        }
        let cl = &mut self.core.clients[i];
        cl.active = false;
        cl.spawned = false;
        cl.name.clear();
        cl.old_frags = -999999;
    }

    /// Host_ServerFrame for a frame of `dt` seconds: console buffer, client
    /// input, physics, then client messages.
    pub fn frame(&mut self, dt: f64) {
        self.frame_held(dt, false);
    }

    /// Host_ServerFrame: `hold` freezes physics without toggling `sv.paused`
    /// (single player with the menu open; no pause plaque, no svc_setpause).
    pub fn frame_held(&mut self, dt: f64, hold: bool) {
        self.execute();
        if self.level.is_none() || self.core.sv.error.is_some() || !self.core.sv.active {
            return;
        }
        let ft = dt.clamp(0.001, 0.1);
        {
            let l = self.level.as_mut().unwrap();
            l.vm.set_g_f(self.core.sv.globals.frametime, ft as f32);
        }
        self.core.sv.datagram.clear();
        self.run_clients(ft);
        if !self.core.sv.paused && !hold {
            self.physics(ft);
        }
        if self.core.sv.error.is_some() || self.level.is_none() {
            return;
        }
        self.send_client_messages();
    }

    fn phys_params(&self, ft: f64) -> PhysParams {
        let cv = &self.core.cvars;
        PhysParams {
            time: self.level.as_ref().map_or(0.0, |l| l.vm.time),
            frametime: ft,
            gravity: cv.value_of("sv_gravity"),
            friction: cv.value_of("sv_friction"),
            stopspeed: cv.value_of("sv_stopspeed"),
            maxvelocity: cv.value_of("sv_maxvelocity"),
            nostep: cv.value_of("sv_nostep") != 0.0,
            edgefriction: cv.value_of("edgefriction"),
            accelerate: cv.value_of("sv_accelerate"),
            maxspeed: cv.value_of("sv_maxspeed"),
            gameplayfix_elevators: cv.value_of("sv_gameplayfix_elevators"),
            maxclients: self.core.maxclients() as u32,
            altnoclip: cv.value_of("sv_altnoclip") != 0.0,
            rollangle: cv.value_of("cl_rollangle"),
            rollspeed: cv.value_of("cl_rollspeed"),
            idealpitchscale: cv.value_of("sv_idealpitchscale"),
        }
    }

    /// SV_RunClients.
    fn run_clients(&mut self, ft: f64) {
        for i in 0..self.core.maxclients() {
            if !self.core.clients[i].active {
                continue;
            }
            while let Some(s) = self.core.clients[i].stringcmds.pop_front() {
                self.client_command(i, &s);
            }
            if self.level.is_none() || !self.core.clients[i].active {
                continue;
            }
            let p = self.phys_params(ft);
            let Some(l) = &mut self.level else { return };
            let c = &mut self.core;
            let e = i as u32 + 1;
            let f = c.sv.fields.clone();
            let cl = &mut c.clients[i];
            let mv = cl.cmd;
            if cl.new_move {
                cl.new_move = false;
                l.vm.set_ent_v(e, f.v_angle, mv.angles);
                l.vm.set_ent_f(e, f.button0, (mv.buttons & 1) as f32);
                l.vm.set_ent_f(e, f.button2, ((mv.buttons & 2) >> 1) as f32);
                if mv.impulse != 0 {
                    l.vm.set_ent_f(e, f.impulse, mv.impulse as f32);
                }
                cl.cmd.impulse = 0;
            }
            if !cl.spawned {
                cl.cmd = ClientMove::default();
                continue;
            }
            if c.sv.paused {
                continue;
            }
            let cmd = qworld::user::UserCmd { forwardmove: mv.forward, sidemove: mv.side, upmove: mv.up };
            let mut ents = Ents { vm: &mut l.vm, c };
            qworld::user::client_think(&l.world, &mut ents, &p, e, &cmd);
        }
    }

    /// SV_Physics for one frame of `ft` seconds.
    pub(crate) fn physics(&mut self, ft: f64) {
        let mut p = self.phys_params(ft);
        self.core.sv.phys = p;
        let maxclients = self.core.maxclients() as u32;
        let freeze = self.core.cvars.value_of("sv_freezenonclients") != 0.0;
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let g = c.sv.globals.clone();
        let f = c.sv.fields.clone();
        let vm = &mut l.vm;
        let w = &mut l.world;
        vm.set_g_f(g.frametime, ft as f32);
        vm.set_g_i(g.self_, 0);
        vm.set_g_i(g.other, 0);
        vm.set_g_f(g.time, vm.time as f32);
        exec(vm, c, w, vm.g_i(g.f_start_frame));
        let mut i = 0u32;
        loop {
            let cap = if freeze { maxclients + 1 } else { vm.num_edicts() as u32 };
            if i >= cap || c.sv.error.is_some() {
                break;
            }
            let e = i;
            i += 1;
            if vm.is_free(e) {
                continue;
            }
            p.time = vm.time;
            if vm.g_f(g.force_retouch) != 0.0 {
                w.link_edict(&mut Ents { vm: &mut *vm, c: &mut *c }, e, true);
            }
            if e > 0 && e <= maxclients {
                if !c.clients[e as usize - 1].active {
                    continue;
                }
                vm.set_g_f(g.time, vm.time as f32);
                vm.set_self(e);
                exec(vm, c, w, vm.g_i(g.f_player_pre_think));
                let ok = qworld::phys::physics_client_move(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &p, e);
                if ok {
                    vm.set_g_f(g.time, vm.time as f32);
                    vm.set_self(e);
                    exec(vm, c, w, vm.g_i(g.f_player_post_think));
                }
            } else {
                qworld::phys::physics_entity(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &p, e);
            }
            if c.sv.error.is_some() {
                break;
            }
            // johnfitz: lerp interval
            let x = c.ext(e);
            x.sendinterval = false;
            if !vm.is_free(e) {
                let nt = vm.ent_f(e, f.nextthink);
                let mt = vm.ent_f(e, f.movetype) as i32;
                let fr = vm.ent_f(e, f.frame);
                let x = c.ext(e);
                if nt as f64 > vm.time && (mt == MOVETYPE_STEP || mt == MOVETYPE_WALK || fr != x.oldframe) {
                    let j = q_rint((nt - x.oldthinktime) * 255.0);
                    if (0..256).contains(&j) && j != 25 && j != 26 {
                        x.sendinterval = true;
                    }
                }
            }
        }
        let fr = vm.g_f(g.force_retouch);
        if fr != 0.0 {
            vm.set_g_f(g.force_retouch, fr - 1.0);
        }
        if !freeze {
            vm.time += ft;
        }
        if c.sv.error.is_none() {
            crate::assists::frame(l, c);
        }
        self.process_queued_client_cmds();
    }

    pub(crate) fn process_queued_client_cmds(&mut self) {
        while !self.core.queued_client_cmds.is_empty() {
            let q = std::mem::take(&mut self.core.queued_client_cmds);
            for (i, s) in q {
                self.exec_client_string(i, &s);
            }
        }
    }

    /// SV_CreateBaseline.
    fn create_baseline(&mut self) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let vm = &l.vm;
        let f = c.sv.fields.clone();
        let maxclients = c.maxclients() as u32;
        let player = c.model_index(b"progs/player.mdl");
        for e in 0..vm.num_edicts() as u32 {
            if vm.is_free(e) {
                continue;
            }
            if e > maxclients && vm.ent_f(e, f.modelindex) == 0.0 {
                continue;
            }
            let protocol = c.sv.protocol;
            let mut b = Baseline {
                origin: vm.ent_v(e, f.origin),
                angles: vm.ent_v(e, f.angles),
                frame: vm.ent_f(e, f.frame) as i32,
                skin: vm.ent_f(e, f.skin) as i32,
                scale: ENTSCALE_DEFAULT,
                ..Baseline::default()
            };
            if e > 0 && e <= maxclients {
                b.colormap = e as i32;
                b.modelindex = player;
                b.alpha = ENTALPHA_DEFAULT;
            } else {
                b.modelindex = c.model_index(vm.ent_str(e, f.model).unwrap_or(b""));
                b.alpha = match f.alpha {
                    Some(o) => entalpha_encode(vm.ent_f(e, o)),
                    None => c.ext(e).alpha,
                };
                if protocol == PROTOCOL_RMQ
                    && let Some(o) = f.scale
                {
                    b.scale = entscale_encode(vm.ent_f(e, o));
                }
            }
            let mut bits = 0;
            if protocol == PROTOCOL_NETQUAKE {
                if b.modelindex & 0xff00 != 0 {
                    b.modelindex = 0;
                }
                if b.frame & 0xff00 != 0 {
                    b.frame = 0;
                }
                b.alpha = ENTALPHA_DEFAULT;
                b.scale = ENTSCALE_DEFAULT;
            } else {
                if b.modelindex & 0xff00 != 0 {
                    bits |= B_LARGEMODEL;
                }
                if b.frame & 0xff00 != 0 {
                    bits |= B_LARGEFRAME;
                }
                if b.alpha != ENTALPHA_DEFAULT {
                    bits |= B_ALPHA;
                }
                if b.scale != ENTSCALE_DEFAULT {
                    bits |= B_SCALE;
                }
            }
            c.ext(e).baseline = b;
            let pf = c.sv.protocolflags;
            let m = c.signon(35);
            m.byte(if bits != 0 { SVC_SPAWNBASELINE2 } else { SVC_SPAWNBASELINE });
            m.short(e as i32);
            if bits != 0 {
                m.byte(bits);
            }
            if bits & B_LARGEMODEL != 0 { m.short(b.modelindex) } else { m.byte(b.modelindex) }
            if bits & B_LARGEFRAME != 0 { m.short(b.frame) } else { m.byte(b.frame) }
            m.byte(b.colormap);
            m.byte(b.skin);
            for k in 0..3 {
                m.coord(b.origin[k], pf);
                m.angle(b.angles[k], pf);
            }
            if bits & B_ALPHA != 0 {
                m.byte(b.alpha as i32);
            }
            if bits & B_SCALE != 0 {
                m.byte(b.scale as i32);
            }
        }
    }

    /// SV_SaveSpawnparms.
    pub(crate) fn save_spawn_parms(&mut self) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let g = c.sv.globals.clone();
        c.serverflags = l.vm.g_f(g.serverflags);
        for i in 0..c.maxclients() {
            if !c.clients[i].active {
                continue;
            }
            l.vm.set_self(i as u32 + 1);
            { let fnum = l.vm.g_i(g.f_set_change_parms); exec(&mut l.vm, c, &mut l.world, fnum); }
            for k in 0..NUM_SPAWN_PARMS {
                c.clients[i].spawn_parms[k] = l.vm.g_f(g.parm1 + k);
            }
        }
    }

    /// Host_Spawn_f for client `i`.
    pub(crate) fn client_spawn(&mut self, i: usize) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        if c.clients[i].spawned {
            c.print("Spawn not valid -- already spawned\n");
            return;
        }
        let g = c.sv.globals.clone();
        let f = c.sv.fields.clone();
        let e = i as u32 + 1;
        let vm = &mut l.vm;
        if c.sv.loadgame {
            c.sv.paused = false;
        } else {
            vm.edict_mut(e).fill(0);
            vm.set_ent_f(e, f.colormap, e as f32);
            vm.set_ent_f(e, f.team, ((c.clients[i].colors & 15) + 1) as f32);
            let h = vm.strings.engine(c.clients[i].name.as_bytes());
            vm.set_ent_i(e, f.netname, h);
            for k in 0..NUM_SPAWN_PARMS {
                vm.set_g_f(g.parm1 + k, c.clients[i].spawn_parms[k]);
            }
            vm.set_g_f(g.time, vm.time as f32);
            vm.set_self(e);
            exec(vm, c, &mut l.world, vm.g_i(g.f_client_connect));
            exec(vm, c, &mut l.world, vm.g_i(g.f_put_client_in_server));
        }

        let pf = c.sv.protocolflags;
        let mut m = std::mem::take(&mut c.clients[i].message);
        m.clear();
        m.byte(SVC_TIME);
        m.float(vm.time as f32);
        for (k, cl) in c.clients.iter().enumerate() {
            m.byte(SVC_UPDATENAME);
            m.byte(k as i32);
            m.string(cl.name.as_bytes());
            m.byte(SVC_UPDATEFRAGS);
            m.byte(k as i32);
            m.short(cl.old_frags);
            m.byte(SVC_UPDATECOLORS);
            m.byte(k as i32);
            m.byte(cl.colors);
        }
        for (k, s) in c.sv.lightstyles.iter().enumerate() {
            m.byte(SVC_LIGHTSTYLE);
            m.byte(k as i32);
            m.string(s);
        }
        for (stat, ofs) in [
            (STAT_TOTALSECRETS, g.total_secrets),
            (STAT_TOTALMONSTERS, g.total_monsters),
            (STAT_SECRETS, g.found_secrets),
            (STAT_MONSTERS, g.killed_monsters),
        ] {
            m.byte(SVC_UPDATESTAT);
            m.byte(stat);
            m.long(vm.g_f(ofs) as i32);
        }
        m.byte(SVC_SETANGLE);
        let a = vm.ent_v(e, f.angles);
        m.angle(a[0], pf);
        m.angle(a[1], pf);
        m.angle(0.0, pf);
        crate::send::write_clientdata(vm, c, &mut l.world, e, &mut m);
        m.byte(SVC_SIGNONNUM);
        m.byte(3);
        let cl = &mut c.clients[i];
        cl.message = m;
        cl.sendsignon = Signon::Flush;
    }

    /// Process a client string command (SV_ReadClientMessage's clc_stringcmd).
    pub fn client_command(&mut self, i: usize, s: &str) {
        if !self.core.clients.get(i).is_some_and(|c| c.active) {
            return;
        }
        let low = s.trim_start().to_ascii_lowercase();
        let is_signon = low.starts_with("spawn") || low.starts_with("begin") || low.starts_with("prespawn");
        if !is_signon
            && let (Some(fnum), Some(l)) = (self.core.sv.parse_client_command, &mut self.level)
        {
            let c = &mut self.core;
            let h = l.vm.strings.temp(s.as_bytes());
            l.vm.set_g_i(ofs_parm(0), h);
            l.vm.set_g_f(c.sv.globals.time, l.vm.time as f32);
            l.vm.set_self(i as u32 + 1);
            exec(&mut l.vm, c, &mut l.world, fnum as i32);
            self.process_queued_client_cmds();
            return;
        }
        self.exec_client_string(i, s);
    }

    /// Cmd_ExecuteString(s, src_client) restricted to the commands clients may run.
    pub(crate) fn exec_client_string(&mut self, i: usize, s: &str) {
        const ALLOWED: &[&str] = &[
            "status", "god", "notarget", "fly", "name", "noclip", "setpos", "say", "say_team", "tell",
            "color", "kill", "pause", "spawn", "begin", "prespawn", "kick", "ping", "give", "ban",
        ];
        let low = s.trim_start().to_ascii_lowercase();
        if !ALLOWED.iter().any(|a| low.starts_with(a)) {
            self.core.print(&format!("client {i} tried to {s}\n"));
            return;
        }
        let old = self.core.cmd_client.replace(i);
        qcore::cmd::execute_string(self, s);
        self.core.cmd_client = old;
    }
}
