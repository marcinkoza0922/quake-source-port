//! Client-Side QuakeC (`csprogs.dat`) runtime — supports both Simple CSQC HUD
//! (`CSQC_Init`, `CSQC_Shutdown`, `CSQC_DrawHud`, `CSQC_DrawScores`, ironwail/vkQuake)
//! and Full CSQC (`CSQC_UpdateView`, `CSQC_Parse_Event`, `CSQC_Parse_Print`,
//! `CSQC_Parse_CenterPrint`, `CSQC_Parse_StuffCmd`, `CSQC_ConsoleCommand`,
//! `CSQC_InputEvent`, 3D scene building, `read*`, `draw*`).

use std::collections::HashSet;

use qclient::ClientState;
use qcore::cmd::Cbuf;
use qcore::cvar::Cvars;
use qcore::fs::Fs;
use qcore::math::{Vec3, angle_vectors};
use crate::gfx::Renderer;
use qrender::View;
use qsound::SoundSystem;
use qvm::{Progs, Vm, ofs_parm};

use crate::assets::Level;
use crate::console::Console;
use crate::draw::Canvas;

pub mod builtins;
mod defs;
pub mod draw;
pub mod markup;
pub mod net;
pub mod scene;
#[cfg(test)]
mod tests;

pub use defs::{CsqcFields, CsqcGlobals, Entries};

const CSQC_MAX_EDICTS: usize = 2048;

/// Non-VM CSQC state kept separate from `Vm` so `CsqcHost` can borrow it mutably
/// while `Vm::execute` runs.
#[derive(Default)]
pub struct CsqcState {
    pub entries: Entries,
    pub globals: CsqcGlobals,
    pub fields: CsqcFields,
    pub pics: draw::CsqcPics,
    pub commands: HashSet<String>,
    pub scene: scene::SceneState,
    pub packet: net::PacketReader,
    pub cursor_mode: bool,
    /// Dynamic precached models loaded by CSQC (`precache_model`).
    pub extra_models: Vec<String>,
    cached_mapname: (String, i32),
    cached_levelname: (String, i32),
    cached_worldmodel: (String, i32),
}

/// Mutable host references supplied to CSQC builtins during one VM invocation.
pub struct CsqcEnv<'a> {
    pub fs: &'a Fs,
    pub cvars: &'a mut Cvars,
    pub cl: &'a ClientState,
    pub level: &'a mut Level,
    pub r: &'a mut Renderer,
    pub snd: Option<&'a mut SoundSystem>,
    pub con: &'a mut Console,
    pub cbuf: &'a mut Cbuf,
    pub canvas: Option<&'a mut Canvas>,
    pub base_view: Option<View>,
    pub is_demo: bool,
    pub realtime: f64,
    pub frametime: f32,
    pub width: u32,
    pub height: u32,
    pub virt: [f32; 2],
    pub show_scores: bool,
    pub not_menu: bool,
}

/// Active CSQC instance (absent when no `csprogs.dat` is loaded).
#[derive(Default)]
pub struct Csqc {
    vm: Option<Vm>,
    pub state: CsqcState,
}

/// True when the active filesystem ships a `csprogs.dat` with `CSQC_DrawHud`.
pub fn fs_has_hud(fs: &Fs) -> bool {
    fs.open("csprogs.dat")
        .and_then(|b| Progs::parse_csqc(&b).ok())
        .is_some_and(|p| defs::find_fn(&p, "CSQC_DrawHud").is_some())
}

impl Csqc {
    pub fn active(&self) -> bool {
        self.vm.is_some()
    }

    /// True for Full-CSQC-only progs (when `CSQC_UpdateView` exists without Simple CSQC `CSQC_DrawHud`).
    pub fn handles_view(&self) -> bool {
        self.vm.is_some() && self.state.entries.update_view.is_some() && self.state.entries.draw_hud.is_none()
    }

    /// True when Simple CSQC (`CSQC_DrawHud`) is loaded.
    pub fn has_simple_hud(&self) -> bool {
        self.vm.is_some() && self.state.entries.draw_hud.is_some()
    }

    pub fn has_draw_scores(&self) -> bool {
        self.vm.is_some() && self.state.entries.draw_scores.is_some()
    }

    pub fn has_command(&self, name: &str) -> bool {
        let low = name.to_ascii_lowercase();
        // Simple CSQC receives `showscores` from the engine; let `+showscores`/`-showscores`
        // update `Host::show_scores` instead of letting Full-CSQC handlers (e.g. AD) swallow them.
        if self.has_simple_hud() && matches!(low.as_str(), "+showscores" | "-showscores") {
            return false;
        }
        self.vm.is_some() && self.state.commands.contains(&low)
    }

    /// Loads `csprogs.dat` from the active filesystem (unless `cl_nocsqc` is set)
    /// and invokes `CSQC_Init`.
    pub fn load(&mut self, env: &mut CsqcEnv<'_>) {
        self.shutdown(env);
        if env.cvars.value_of("cl_nocsqc") != 0.0 || env.cvars.value_of("pr_checkextension") == 0.0 {
            return;
        }
        let Some(bytes) = env.fs.open("csprogs.dat") else {
            return;
        };
        let _ = self.load_bytes(&bytes, env);
    }

    /// Loads CSQC from raw `csprogs.dat` bytes (used by [`Self::load`] and tests).
    pub fn load_bytes(&mut self, bytes: &[u8], env: &mut CsqcEnv<'_>) -> bool {
        self.shutdown(env);
        let progs = match Progs::parse_csqc(bytes) {
            Ok(p) => p,
            Err(e) => {
                env.con.print(&format!("CSQC: csprogs.dat parse error: {e}\n"));
                return false;
            }
        };
        let entries = defs::resolve_entries(&progs);
        let globals = defs::resolve_globals(&progs);
        let fields = defs::resolve_fields(&progs);
        let vm = match Vm::new_csqc(progs, CSQC_MAX_EDICTS) {
            Ok(v) => v,
            Err(e) => {
                env.con.print(&format!("CSQC: init error: {e}\n"));
                return false;
            }
        };
        self.state = CsqcState { entries, globals, fields, ..self.empty_state() };
        self.vm = Some(vm);
        self.sync_globals(env);
        if let Some(f) = self.state.entries.init {
            if let Some(vm) = &mut self.vm {
                let eng = vm.strings.engine(b"quake-source-port");
                vm.set_g_f(ofs_parm(0), 0.0); // apilevel (0 = standard)
                vm.set_g_i(ofs_parm(1), eng);
                vm.set_g_f(ofs_parm(2), 1.0); // engineversion
            }
            self.call(f, env);
        }
        true
    }

    /// State without a VM; the registered pictures stay (`CsqcPics`).
    fn empty_state(&mut self) -> CsqcState {
        let mut pics = std::mem::take(&mut self.state.pics);
        pics.new_vm();
        CsqcState { pics, ..Default::default() }
    }

    /// Calls `CSQC_Shutdown` and tears down the CSQC VM and caches.
    pub fn shutdown(&mut self, env: &mut CsqcEnv<'_>) {
        if let Some(f) = self.state.entries.shutdown
            && self.vm.is_some()
        {
            self.call(f, env);
        }
        self.vm = None;
        self.state = self.empty_state();
    }

    /// Unconditionally drops the CSQC VM (used on disconnect without needing a full env).
    pub fn clear(&mut self) {
        self.vm = None;
        self.state = self.empty_state();
    }

    fn sync_globals(&mut self, env: &CsqcEnv<'_>) {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        let Some(vm) = &mut self.vm else { return };
        let g = self.state.globals;
        let f = self.state.fields;
        let cl = env.cl;
        vm.time = cl.time;
        let set_f = |vm: &mut Vm, ofs: usize, val: f32| {
            if ofs != 0 {
                vm.set_g_f(ofs, val);
            }
        };
        let set_v = |vm: &mut Vm, ofs: usize, val: [f32; 3]| {
            if ofs != 0 {
                vm.set_g_v(ofs, val);
            }
        };
        let cached_str = |vm: &mut Vm, slot: &mut (String, i32), val: &str| -> i32 {
            if slot.1 == 0 || slot.0 != val {
                slot.0.clear();
                slot.0.push_str(val);
                slot.1 = vm.strings.engine(val.as_bytes());
            }
            slot.1
        };
        set_f(vm, g.time, cl.time as f32);
        set_f(vm, g.frametime, env.frametime);
        set_f(vm, g.cltime, env.realtime as f32);
        set_f(vm, g.clframetime, env.frametime);
        set_f(vm, g.deathmatch, cl.gametype as f32);
        set_f(vm, g.coop, if cl.gametype == 0 && cl.maxclients != 1 { 1.0 } else { 0.0 });
        set_f(vm, g.total_monsters, cl.statsf[STAT_TOTALMONSTERS]);
        set_f(vm, g.total_secrets, cl.statsf[STAT_TOTALSECRETS]);
        set_f(vm, g.killed_monsters, cl.statsf[STAT_MONSTERS]);
        set_f(vm, g.found_secrets, cl.statsf[STAT_SECRETS]);
        set_f(vm, g.player_localentnum, cl.viewentity as f32);
        set_f(vm, g.player_localnum, cl.viewentity.saturating_sub(1) as f32);
        set_f(vm, g.maxclients, cl.maxclients as f32);
        set_f(vm, g.intermission, cl.intermission as f32);
        set_f(vm, g.intermission_time, cl.completed_time as f32);
        set_f(vm, g.sb_showscores, if env.show_scores { 1.0 } else { 0.0 });
        if g.world != 0 {
            vm.set_g_i(g.world, 0);
        }
        if g.mapname != 0 {
            let h = cached_str(vm, &mut self.state.cached_mapname, &cl.mapname);
            vm.set_g_i(g.mapname, h);
        }
        // Worldspawn entity (edict 0) fields expected by Simple CSQC HUDs (ironwail/vkQuake).
        if f.solid != usize::MAX {
            vm.set_ent_f(0, f.solid, 4.0);
        }
        if f.modelindex != usize::MAX {
            vm.set_ent_f(0, f.modelindex, 1.0);
        }
        if f.model != usize::MAX {
            let wm = cl.model_precache.get(1).map_or("", String::as_str);
            let h = cached_str(vm, &mut self.state.cached_worldmodel, wm);
            vm.set_ent_i(0, f.model, h);
        }
        if f.message != usize::MAX {
            let h = cached_str(vm, &mut self.state.cached_levelname, &cl.levelname);
            vm.set_ent_i(0, f.message, h);
        }
        set_v(vm, g.view_angles, cl.viewangles);
        set_v(vm, g.input_angles, cl.viewangles);
        let (fwd, right, up) = angle_vectors(Vec3::from_array(cl.viewangles));
        set_v(vm, g.v_forward, fwd.to_array());
        set_v(vm, g.v_right, right.to_array());
        set_v(vm, g.v_up, up.to_array());
        if let Some(e) = cl.entities.get(cl.viewentity) {
            set_v(vm, g.pmove_org, e.origin);
        }
        set_v(vm, g.pmove_vel, cl.velocity);
    }

    fn call(&mut self, fn_idx: usize, env: &mut CsqcEnv<'_>) -> Option<f32> {
        let vm = self.vm.as_mut()?;
        let mut host = builtins::CsqcHost { state: &mut self.state, env };
        match vm.execute(&mut host, fn_idx) {
            Ok(()) => Some(vm.g_f(qvm::OFS_RETURN)),
            Err(e) => {
                host.env.con.print(&format!("CSQC error: {e}\n"));
                None
            }
        }
    }

    pub fn parse_event(&mut self, payload: &[u8], env: &mut CsqcEnv<'_>) {
        let Some(f) = self.state.entries.parse_event else { return };
        self.sync_globals(env);
        self.state.packet.reset(payload, env.cl.protocolflags);
        self.call(f, env);
        self.state.packet.clear();
    }

    pub fn console_command(&mut self, line: &str, env: &mut CsqcEnv<'_>) -> bool {
        let Some(f) = self.state.entries.console_command else { return false };
        self.sync_globals(env);
        if let Some(vm) = &mut self.vm {
            let s = vm.strings.temp(line.as_bytes());
            vm.set_g_i(ofs_parm(0), s);
        }
        self.call(f, env).is_some_and(|v| v != 0.0)
    }

    pub fn parse_print(&mut self, msg: &str, env: &mut CsqcEnv<'_>) -> bool {
        let Some(f) = self.state.entries.parse_print else { return false };
        self.sync_globals(env);
        if let Some(vm) = &mut self.vm {
            let s = vm.strings.temp(msg.as_bytes());
            vm.set_g_i(ofs_parm(0), s);
            vm.set_g_f(ofs_parm(1), 2.0); // PRINT_HIGH
        }
        self.call(f, env);
        true
    }

    pub fn parse_centerprint(&mut self, msg: &str, env: &mut CsqcEnv<'_>) -> bool {
        let Some(f) = self.state.entries.parse_centerprint else { return false };
        self.sync_globals(env);
        if let Some(vm) = &mut self.vm {
            let s = vm.strings.temp(msg.as_bytes());
            vm.set_g_i(ofs_parm(0), s);
        }
        self.call(f, env);
        true
    }

    pub fn parse_stuffcmd(&mut self, msg: &str, env: &mut CsqcEnv<'_>) -> bool {
        let Some(f) = self.state.entries.parse_stuffcmd else { return false };
        self.sync_globals(env);
        if let Some(vm) = &mut self.vm {
            let s = vm.strings.temp(msg.as_bytes());
            vm.set_g_i(ofs_parm(0), s);
        }
        self.call(f, env);
        true
    }

    /// Full CSQC frame: calls `CSQC_UpdateView(vwidth, vheight, notmenu)`.
    pub fn update_view(&mut self, env: &mut CsqcEnv<'_>) {
        let Some(f) = self.state.entries.update_view else { return };
        self.sync_globals(env);
        self.state.scene.clear(env.base_view, env.virt[0], env.virt[1]);
        if let Some(vm) = &mut self.vm {
            vm.set_g_f(ofs_parm(0), env.virt[0]);
            vm.set_g_f(ofs_parm(1), env.virt[1]);
            vm.set_g_f(ofs_parm(2), if env.not_menu { 1.0 } else { 0.0 });
        }
        self.call(f, env);
    }

    /// Simple CSQC HUD: calls `CSQC_DrawHud(virsize, showscores)` and (when not in a menu) `CSQC_DrawScores(virsize, showscores)`.
    pub fn draw_hud(&mut self, env: &mut CsqcEnv<'_>) {
        self.sync_globals(env);
        let sz = [env.virt[0], env.virt[1], 0.0];
        let scores = if env.show_scores { 1.0 } else { 0.0 };
        if let Some(f) = self.state.entries.draw_hud {
            if let Some(vm) = &mut self.vm {
                vm.set_g_v(ofs_parm(0), sz);
                vm.set_g_f(ofs_parm(1), scores);
            }
            self.call(f, env);
        }
        if env.not_menu
            && let Some(f) = self.state.entries.draw_scores
        {
            if let Some(vm) = &mut self.vm {
                vm.set_g_v(ofs_parm(0), sz);
                vm.set_g_f(ofs_parm(1), scores);
            }
            self.call(f, env);
        }
    }

    /// Intermission CSQC scores: calls `CSQC_DrawScores(virsize, showscores)`.
    pub fn draw_scores(&mut self, env: &mut CsqcEnv<'_>) {
        let Some(f) = self.state.entries.draw_scores else { return };
        self.sync_globals(env);
        let sz = [env.virt[0], env.virt[1], 0.0];
        let scores = if env.show_scores { 1.0 } else { 0.0 };
        if let Some(vm) = &mut self.vm {
            vm.set_g_v(ofs_parm(0), sz);
            vm.set_g_f(ofs_parm(1), scores);
        }
        self.call(f, env);
    }
}
