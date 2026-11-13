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
mod types;
pub use types::*;
mod spawn;
mod physics;
mod connect;
mod clients;

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
        // Set by 2021-rerelease progs (hipnotic/rogue cvar_set it every frame).
        ("campaign", "0", 0),
        ("pr_checkextension", "1", 0),
        // FTE/DP particle builtins + svcdp_* messages (single player only).
        ("sv_fteparticles", "1", 0),
        // FTE protocol extensions offered to clients (PEXT2_* mask; 8 =
        // replacement deltas). 0 keeps the plain 15/666/999 protocol.
        ("sv_protocol_pext2", "8", 0),
    ] {
        cv.register(n, d, f);
    }
    for &(n, d, f) in crate::assists::CVARS.iter().chain(crate::addons::CVARS) {
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
                addons_on: false,
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

    pub(crate) fn process_queued_client_cmds(&mut self) {
        while !self.core.queued_client_cmds.is_empty() {
            let q = std::mem::take(&mut self.core.queued_client_cmds);
            for (i, s) in q {
                self.exec_client_string(i, &s);
            }
        }
    }

}
