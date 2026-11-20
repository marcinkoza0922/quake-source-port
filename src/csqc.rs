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
use qrender::{Renderer, View};
use qsound::SoundSystem;
use qvm::{Progs, Vm, ofs_parm};

use crate::assets::Level;
use crate::console::Console;
use crate::draw::Canvas;

pub mod builtins;
pub mod draw;
pub mod markup;
pub mod net;
pub mod scene;
#[cfg(test)]
mod tests;

const CSQC_MAX_EDICTS: usize = 2048;

/// Entry-point function indices discovered in `csprogs.dat`.
#[derive(Default, Clone, Copy)]
pub struct Entries {
    pub init: Option<usize>,
    pub shutdown: Option<usize>,
    pub draw_hud: Option<usize>,
    pub draw_scores: Option<usize>,
    pub update_view: Option<usize>,
    pub parse_event: Option<usize>,
    pub parse_print: Option<usize>,
    pub parse_centerprint: Option<usize>,
    pub parse_stuffcmd: Option<usize>,
    pub console_command: Option<usize>,
}

/// Optional standard CSQC globals (0 = absent in this `csprogs.dat`).
#[derive(Default, Clone, Copy)]
pub struct CsqcGlobals {
    pub time: usize,
    pub frametime: usize,
    pub cltime: usize,
    pub player_localentnum: usize,
    pub player_localnum: usize,
    pub maxclients: usize,
    pub intermission: usize,
    pub intermission_time: usize,
    pub sb_showscores: usize,
    pub v_forward: usize,
    pub v_right: usize,
    pub v_up: usize,
    pub view_angles: usize,
    pub pmove_org: usize,
    pub pmove_vel: usize,
    pub input_angles: usize,
    pub trace_allsolid: usize,
    pub trace_startsolid: usize,
    pub trace_fraction: usize,
    pub trace_endpos: usize,
    pub trace_plane_normal: usize,
    pub trace_plane_dist: usize,
    pub trace_ent: usize,
    pub trace_inopen: usize,
    pub trace_inwater: usize,
}

/// Optional standard CSQC entity fields (`usize::MAX` = absent).
#[derive(Clone, Copy)]
pub struct CsqcFields {
    pub origin: usize,
    pub angles: usize,
    pub model: usize,
    pub modelindex: usize,
    pub frame: usize,
    pub frame2: usize,
    pub lerpfrac: usize,
    pub skin: usize,
    pub colormap: usize,
    pub alpha: usize,
    pub effects: usize,
    pub renderflags: usize,
    pub drawmask: usize,
    pub mins: usize,
    pub maxs: usize,
}

impl Default for CsqcFields {
    fn default() -> Self {
        Self {
            origin: usize::MAX,
            angles: usize::MAX,
            model: usize::MAX,
            modelindex: usize::MAX,
            frame: usize::MAX,
            frame2: usize::MAX,
            lerpfrac: usize::MAX,
            skin: usize::MAX,
            colormap: usize::MAX,
            alpha: usize::MAX,
            effects: usize::MAX,
            renderflags: usize::MAX,
            drawmask: usize::MAX,
            mins: usize::MAX,
            maxs: usize::MAX,
        }
    }
}

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

fn find_fn(p: &Progs, name: &str) -> Option<usize> {
    p.find_function(name).filter(|&i| p.functions[i].first_statement > 0)
}

fn resolve_entries(p: &Progs) -> Entries {
    Entries {
        init: find_fn(p, "CSQC_Init"),
        shutdown: find_fn(p, "CSQC_Shutdown"),
        draw_hud: find_fn(p, "CSQC_DrawHud"),
        draw_scores: find_fn(p, "CSQC_DrawScores"),
        update_view: find_fn(p, "CSQC_UpdateView"),
        parse_event: find_fn(p, "CSQC_Parse_Event"),
        parse_print: find_fn(p, "CSQC_Parse_Print"),
        parse_centerprint: find_fn(p, "CSQC_Parse_CenterPrint"),
        parse_stuffcmd: find_fn(p, "CSQC_Parse_StuffCmd"),
        console_command: find_fn(p, "CSQC_ConsoleCommand"),
    }
}

fn resolve_globals(p: &Progs) -> CsqcGlobals {
    let g = |n: &str| p.find_global(n).map_or(0, |d| d.ofs as usize);
    CsqcGlobals {
        time: g("time"),
        frametime: g("frametime"),
        cltime: g("cltime"),
        player_localentnum: g("player_localentnum"),
        player_localnum: g("player_localnum"),
        maxclients: g("maxclients"),
        intermission: g("intermission"),
        intermission_time: g("intermission_time"),
        sb_showscores: g("sb_showscores"),
        v_forward: g("v_forward"),
        v_right: g("v_right"),
        v_up: g("v_up"),
        view_angles: g("view_angles"),
        pmove_org: g("pmove_org"),
        pmove_vel: g("pmove_vel"),
        input_angles: g("input_angles"),
        trace_allsolid: g("trace_allsolid"),
        trace_startsolid: g("trace_startsolid"),
        trace_fraction: g("trace_fraction"),
        trace_endpos: g("trace_endpos"),
        trace_plane_normal: g("trace_plane_normal"),
        trace_plane_dist: g("trace_plane_dist"),
        trace_ent: g("trace_ent"),
        trace_inopen: g("trace_inopen"),
        trace_inwater: g("trace_inwater"),
    }
}

fn resolve_fields(p: &Progs) -> CsqcFields {
    let f = |n: &str| p.find_field(n).map_or(usize::MAX, |d| d.ofs as usize);
    CsqcFields {
        origin: f("origin"),
        angles: f("angles"),
        model: f("model"),
        modelindex: f("modelindex"),
        frame: f("frame"),
        frame2: f("frame2"),
        lerpfrac: f("lerpfrac"),
        skin: f("skin"),
        colormap: f("colormap"),
        alpha: f("alpha"),
        effects: f("effects"),
        renderflags: f("renderflags"),
        drawmask: f("drawmask"),
        mins: f("mins"),
        maxs: f("maxs"),
    }
}

impl Csqc {
    pub fn active(&self) -> bool {
        self.vm.is_some()
    }

    pub fn handles_view(&self) -> bool {
        self.vm.is_some() && self.state.entries.update_view.is_some()
    }

    pub fn handles_hud(&self) -> bool {
        self.vm.is_some() && (self.state.entries.draw_hud.is_some() || self.state.entries.update_view.is_some())
    }

    pub fn has_command(&self, name: &str) -> bool {
        self.vm.is_some() && self.state.commands.contains(&name.to_ascii_lowercase())
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
        let entries = resolve_entries(&progs);
        let globals = resolve_globals(&progs);
        let fields = resolve_fields(&progs);
        let vm = match Vm::new_csqc(progs, CSQC_MAX_EDICTS) {
            Ok(v) => v,
            Err(e) => {
                env.con.print(&format!("CSQC: init error: {e}\n"));
                return false;
            }
        };
        self.state = CsqcState {
            entries,
            globals,
            fields,
            ..Default::default()
        };
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

    /// Calls `CSQC_Shutdown` and tears down the CSQC VM and caches.
    pub fn shutdown(&mut self, env: &mut CsqcEnv<'_>) {
        if let Some(f) = self.state.entries.shutdown
            && self.vm.is_some()
        {
            self.call(f, env);
        }
        self.vm = None;
        self.state = CsqcState::default();
    }

    /// Unconditionally drops the CSQC VM (used on disconnect without needing a full env).
    pub fn clear(&mut self) {
        self.vm = None;
        self.state = CsqcState::default();
    }

    fn sync_globals(&mut self, env: &CsqcEnv<'_>) {
        let Some(vm) = &mut self.vm else { return };
        let g = self.state.globals;
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
        set_f(vm, g.time, cl.time as f32);
        set_f(vm, g.frametime, env.frametime);
        set_f(vm, g.cltime, env.realtime as f32);
        set_f(vm, g.player_localentnum, cl.viewentity as f32);
        set_f(vm, g.player_localnum, cl.viewentity.saturating_sub(1) as f32);
        set_f(vm, g.maxclients, cl.maxclients as f32);
        set_f(vm, g.intermission, cl.intermission as f32);
        set_f(vm, g.intermission_time, cl.completed_time as f32);
        set_f(vm, g.sb_showscores, if env.show_scores { 1.0 } else { 0.0 });
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

    /// Simple CSQC HUD: calls `CSQC_DrawHud(virsize, showscores)` and `CSQC_DrawScores(virsize, showscores)`.
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
        if let Some(f) = self.state.entries.draw_scores {
            if let Some(vm) = &mut self.vm {
                vm.set_g_v(ofs_parm(0), sz);
                vm.set_g_f(ofs_parm(1), scores);
            }
            self.call(f, env);
        }
    }
}
