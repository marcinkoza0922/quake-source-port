//! Host glue (host.c / host_cmd.c / cl_main.c): command buffer, cvars, the
//! loopback server + client, per-frame input/render/sound.

use std::path::PathBuf;

use qclient::{ClientEvent, ClientState, InputState, MoveSpeeds, UserCmd};
use qcore::cmd::{Args, Cbuf, CmdContext, Commands};
use qcore::cvar::{CVAR_ARCHIVE, CVAR_SERVERINFO, Cvars};
use qcore::math::{Vec3, angle_vectors};
use qinput::aim::{AimContext, AimTarget, TargetKind, aim_assist_allowed};
use qinput::keys::{K_ESCAPE, KeyDest};
use qinput::{Device, Input, InputEvent, InputHost, Routed};
use qnet::protocol::{CLC_STRINGCMD, STAT_ACTIVEWEAPON, STAT_AMMO, STAT_HEALTH, STAT_WEAPONFRAME};
use qrender::{Renderer, Settings};
use qserver::{ClientMove, Server};
use qsound::{Listener, SoundEvent, SoundSystem};

use crate::assets::Level;
use crate::console::Console;
use crate::draw::{Canvas, Pics};
use crate::menu::{Action, Menu, Screen};
use crate::platform::HostEvent;
use crate::view::{ViewCvars, ViewState};
mod commands;
use commands::*;
mod net;
mod demo;
mod mapswitch;
mod render;
mod split;
mod split_input;
#[cfg(test)]
mod split_tests;
mod shader;
mod saves;
mod death;
mod console;
mod input;
mod frame;
mod ctx;
mod cvars;
use cvars::*;
pub(crate) mod feedback;
mod hitmarker;
mod crosshair;
use crosshair::*;
mod video;
mod touch;

/// Server tic rate when rendering faster (ironwail-style decoupling).
const SERVER_TIC: f64 = 1.0 / 72.0;

/// Virtual rows taken by the classic status bar + inventory bar.
const CLASSIC_SBAR_LINES: f32 = 48.0;

/// Quakespasm's default.cfg for games without one (the 2021 re-release).
const DEFAULT_CFG: &str = include_str!("default.cfg");
/// Gamepad bindings applied after any default.cfg (id1's has none), and
/// after a config.cfg that binds no gamepad key at all.
const PAD_DEFAULTS: &str = include_str!("pad_defaults.cfg");

/// (key, command) pairs of [`PAD_DEFAULTS`].
fn pad_default_binds() -> Vec<(i32, String)> {
    PAD_DEFAULTS
        .lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("bind ")?.trim();
            let (k, cmd) = rest.split_once(char::is_whitespace)?;
            let k = qinput::keys::string_to_keynum(k);
            (k >= 0).then(|| (k, cmd.trim().trim_matches('"').to_string()))
        })
        .collect()
}

mod aim_vis;
use aim_vis::WorldVis;

pub struct Host {
    pub server: Server,
    pub cl: ClientState,
    pub local: Option<usize>,
    cmds: Commands<Host>,
    cbuf: Cbuf,
    pub input: Input,
    pub con: Console,
    pub menu: Menu,
    pub r: Renderer,
    pics: Pics,
    pub snd: Option<SoundSystem>,
    level: Level,
    view: ViewState,
    debris: crate::debris::Debris,
    blood: crate::blood::Blood,
    impacts: crate::impacts::Impacts,
    pub realtime: f64,
    accum: f64,
    pub quit: bool,
    pend_attack: bool,
    pend_attack2: bool,
    pend_jump: bool,
    pend_impulse: u8,
    wheel: crate::wheel::Wheel,
    /// Player 1's add-on toy slot.
    toy: crate::addons::ToySlot,
    captions: crate::captions::Captions,
    hud_fade: crate::sbar::HudFade,
    feedback: feedback::Feedback,
    feedback_sounds: feedback::CueCache,
    portal_fx: crate::addons::portal_fx::Presentation,
    rail_fx: crate::addons::rail_fx::Presentation,
    hook_fx: crate::addons::hook_fx::HookFx,
    /// Split-screen players 2..6 (`cl_splitscreen`); player 1 is `cl`.
    pub guests: Vec<crate::splitscreen::Guest>,
    /// Player 1's gamepad (the first pad used); other pads can join.
    p1_pad: Option<u32>,
    /// While a guest pad's event runs: binds act on that guest's input.
    input_target: Option<usize>,
    guest_cbuf: Cbuf,
    /// Label family of the last connected gamepad.
    pad_family: crate::glyphs::Family,
    /// Modern menu text/shape atlas (`crate::font`).
    ui_text: std::rc::Rc<std::cell::RefCell<crate::font::UiText>>,
    /// Rumble to play this frame (low, high, ms), taken by the main loop.
    rumble: Option<(u16, u16, u32)>,
    /// Last seen (weapon, ammo, weaponframe, punch) for fire rumble.
    fire_track: (i32, i32, i32, [f32; 3]),
    /// Held menu navigation key and when it repeats next (gamepad).
    nav_hold: Option<(i32, f64)>,
    /// Finger working the menu (tap/drag-scroll).
    menu_touch: Option<touch::MenuTouch>,
    last_cmd: UserCmd,
    screenshot: Option<String>,
    delayed: Vec<(f64, String)>,
    pub headless: bool,
    show_scores: bool,
    startup: u32,
    pub window: Option<qplatform::Window>,
    cmdline: Vec<String>,
    last_vol: (f32, f32),
    fps: (f64, u32, f32),
    shader: shader::ShaderState,
    fs_snapshot: (PathBuf, Vec<String>),
    stuffed: bool,
    /// The current level's exit was already written to qsp_stats.json.
    stats_recorded: bool,
    /// The demo being played (cls.demoplayback).
    demo: Option<crate::demo::Demo>,
    /// The startdemos loop (cls.demos) and the next index in it; -1 stops the loop.
    demos: Vec<String>,
    demonum: i32,
    /// cls.demorecording.
    recording: Option<crate::demo::DemoWriter>,
    /// `record <name> <map> [track]`: starts when the new map connects.
    pending_record: Option<(PathBuf, i32)>,
    /// Messages up to signon 2 (demo_head), so `record` works mid-game,
    /// and the signon they reach (loopback may batch several stages).
    demo_head: Vec<Vec<u8>>,
    demo_head_signon: u32,
    /// timedemo: frame count and wall-clock start at the second frame.
    timedemo: Option<(u32, std::time::Instant)>,
    /// UDP listen socket while maxplayers > 1 (listen or dedicated server).
    net_server: Option<crate::net::NetServer>,
    /// Connection to a remote server (`connect`).
    net_client: Option<crate::net::NetClient>,
    /// Connection to a QuakeWorld server (`connect qw://...`).
    qw_client: Option<crate::qw::QwClient>,
    /// Last server address, for `reconnect` (true: QuakeWorld).
    last_server: Option<(std::net::SocketAddr, bool)>,
    /// UDP port to listen on (`-port`, `port`).
    pub net_port: u16,
    /// `-dedicated`: no local client.
    pub dedicated: bool,
    /// Remote scoreboard pings: next `ping` request time, and the reply
    /// deadline while one is expected (cl.expectingpingtimes).
    ping_next: f64,
    ping_expect: Option<f64>,
    /// Running `slist`.
    lan_search: Option<crate::net::LanSearch>,
    autosave: crate::autosave::Tracker,
    death_handled: bool,
    /// FTE scripted particles.
    fte: crate::fte::Fte,
}

impl Host {
    pub fn new(fs: qcore::fs::Fs, mut r: Renderer, snd: Option<SoundSystem>, cmdline: Vec<String>) -> Self {
        let mut cvars = Cvars::new();
        for &(n, d, f) in HOST_CVARS.iter().chain(COMPAT_CVARS) {
            cvars.register(n, d, f);
        }
        let input = Input::new(&mut cvars);
        // everything the options menu edits persists in config.cfg
        for n in crate::menu::option_cvars() {
            if let Some(d) = cvars.by_name(n).map(|c| c.default.clone()) {
                cvars.register(n, &d, CVAR_ARCHIVE);
            }
        }
        let fs_snapshot = (fs.basedir().to_path_buf(), fs.game_dirs().to_vec());
        let pics = Pics::new(&fs, &mut r);
        let server = Server::new(fs, cvars, 1);
        let mut h = Host {
            server,
            autosave: Default::default(),
            death_handled: false,
            fte: crate::fte::Fte::new(),
            cl: ClientState::new(),
            local: None,
            cmds: Commands::with_builtins(),
            cbuf: Cbuf::new(),
            input,
            con: Console::new(),
            menu: Menu::new(),
            r,
            pics,
            snd,
            level: Level::default(),
            view: ViewState::default(),
            debris: crate::debris::Debris::default(),
            blood: crate::blood::Blood::default(),
            impacts: crate::impacts::Impacts::default(),
            realtime: 0.0,
            accum: 0.0,
            quit: false,
            pend_attack: false,
            pend_attack2: false,
            pend_jump: false,
            pend_impulse: 0,
            wheel: Default::default(),
            toy: Default::default(),
            captions: Default::default(),
            hud_fade: Default::default(),
            feedback: Default::default(),
            feedback_sounds: Default::default(),
            portal_fx: Default::default(),
            rail_fx: Default::default(),
            hook_fx: Default::default(),
            guests: Vec::new(),
            p1_pad: None,
            input_target: None,
            guest_cbuf: Cbuf::new(),
            pad_family: Default::default(),
            ui_text: Default::default(),
            rumble: None,
            fire_track: (0, 0, 0, [0.0; 3]),
            nav_hold: None,
            menu_touch: None,
            last_cmd: UserCmd::default(),
            screenshot: None,
            delayed: Vec::new(),
            headless: false,
            show_scores: false,
            startup: 0,
            window: None,
            cmdline,
            last_vol: (-1.0, -1.0),
            fps: (0.0, 0, 0.0),
            shader: Default::default(),
            fs_snapshot,
            stuffed: false,
            stats_recorded: false,
            demo: None,
            demos: Vec::new(),
            demonum: 0,
            recording: None,
            pending_record: None,
            demo_head: Vec::new(),
            demo_head_signon: 0,
            timedemo: None,
            net_server: None,
            net_client: None,
            qw_client: None,
            last_server: None,
            net_port: crate::net::DEFAULT_PORT,
            dedicated: false,
            lan_search: None,
            ping_next: 0.0,
            ping_expect: None,
        };
        qinput::register_commands(&mut h.cmds);
        register_commands(&mut h.cmds);
        shader::register_shader_commands(&mut h.cmds);
        h.con.forced = true;
        h.con.frac = 1.0;
        // 2026 update compat (ironwail host.c): word-wrap centerprints for mg3.
        let mg3 = h.server.core.fs.game_dirs().iter().any(|d| d.eq_ignore_ascii_case("mg3"));
        h.server.core.cvars.set("scr_usekfont", if mg3 { "1" } else { "0" });
        h.cbuf.add_text("exec quake.rc\n");
        h
    }

    pub fn fs(&self) -> &qcore::fs::Fs {
        &self.server.core.fs
    }

    /// Send a name/color change to the server for the local client.
    fn send_userinfo(&mut self, cmd: &str) {
        if let Some(q) = &mut self.qw_client {
            qw_userinfo(q, cmd);
            return;
        }
        if self.net_client.is_some() {
            if self.cl.signon > 0 {
                self.cl.send_string_cmd(cmd);
            }
            return;
        }
        if let Some(i) = self.local.filter(|_| self.server.active()) {
            self.server.client_command(i, cmd);
            self.drain_server_log();
        }
    }

    fn cv(&self, name: &str) -> f32 {
        self.server.core.cvars.value_of(name)
    }

    fn cvs(&self, name: &str) -> &str {
        self.server.core.cvars.by_name(name).map_or("", |c| c.string.as_str())
    }

    pub fn toggle_console(&mut self) {
        if self.con.forced {
            if !self.menu.active() {
                self.menu.open(Screen::Main);
            }
            return;
        }
        self.con.open = !self.con.open;
        self.con.input.clear();
        self.con.cursor = 0;
        self.menu.screen = Screen::None;
    }

    /// Time scale for the world while the weapon wheel is open (single player only).
    /// Also applies the accessibility game speed (`a11y_gamespeed`).
    fn wheel_timescale(&self) -> f32 {
        let sp = self.local.is_some() && self.server.core.maxclients() == 1 && self.demo.is_none();
        let speed = if sp { self.cv("a11y_gamespeed").clamp(0.25, 1.0) } else { 1.0 };
        if self.wheel.open && sp { self.cv("cl_wheel_timescale").clamp(0.05, 1.0) * speed } else { speed }
    }

    /// Level exit (intermission or finale): merge kills/secrets/time into
    /// the mod's qsp_stats.json. Not for deathmatch or headless tool runs.
    fn record_level_stats(&mut self) {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        if self.headless || self.demo.is_some() || self.cv("deathmatch") != 0.0 || self.level.mapname.is_empty() {
            return;
        }
        let st = |i: usize| self.cl.stats[i].max(0) as u32;
        let run = qui::stats::MapStats {
            kills: st(STAT_MONSTERS),
            total_kills: st(STAT_TOTALMONSTERS),
            secrets: st(STAT_SECRETS),
            total_secrets: st(STAT_TOTALSECRETS),
            best_time: self.cl.completed_time as f32,
            completions: 0,
        };
        let dir = self.user_dir();
        let mut stats = qui::stats::Stats::load(&dir);
        stats.record(&self.level.mapname, run);
        if let Err(e) = stats.save(&dir) {
            self.con.print(&format!("couldn't write {}: {e}\n", qui::stats::FILE_NAME));
        }
    }

    /// CL_Disconnect: shut down the local server and the client.
    fn stop_game(&mut self) {
        if let Some(n) = &mut self.net_server {
            n.shutdown(&mut self.server, self.realtime);
        }
        if let Some(i) = self.local {
            self.server.drop_client(i, false);
        }
        self.server.level = None;
        self.server.core.sv.active = false;
        self.server.core.set_addons(false);
        self.disconnect_client();
    }

    /// Registers the modern text atlas and follows `ui_menufont`/`ui_fontfile`.
    fn sync_ui_text(&mut self) {
        let want = (self.cv("ui_menufont") as i32, self.cvs("ui_fontfile").to_string());
        let mut ui = self.ui_text.borrow_mut();
        if ui.pic.is_none() {
            let n = crate::font::UiText::atlas_size();
            ui.pic = self.r.register_pic_rgba(n, n, &ui.px).ok();
        }
        let warn = if ui.source != want { ui.set_font(&self.server.core.fs, want.0, &want.1) } else { None };
        drop(ui);
        if let Some(w) = warn {
            self.con.print(&w);
        }
    }

    /// Loads FTE effects for `r_particledesc` and the current map
    /// (R_ParticleDesc_Callback); `r_fteparticles 0` turns them off.
    fn sync_fte(&mut self) {
        let desc = if self.cv("r_fteparticles") != 0.0 { self.cvs("r_particledesc").to_string() } else { String::new() };
        let map = self.level.mapname.clone();
        let opts = qpart::RenderOptions {
            sparks: self.cv("r_part_sparks"),
            sparks_textured: self.cv("r_part_sparks_textured") != 0.0,
            sparks_trifan: self.cv("r_part_sparks_trifan") != 0.0,
            beams: self.cv("r_part_beams"),
        };
        let settings = qpart::Settings {
            density: self.cv("r_part_density"),
            contentswitch: self.cv("r_part_contentswitch") != 0.0,
            bouncysparks: self.cv("r_bouncysparks") != 0.0,
            trace_limit: self.cv("r_particle_tracelimit").max(0.0) as u32,
            rain: self.cv("r_part_rain") != 0.0,
            rain_quantity: self.cv("r_part_rain_quantity"),
            decal_noperpendicular: self.cv("r_decal_noperpendicular") != 0.0,
            lightflicker: self.cv("r_lightflicker") != 0.0,
            max_particles: self.cv("r_part_maxparticles").max(1.0) as usize,
            max_decals: self.cv("r_part_maxdecals").max(1.0) as usize,
        };
        let con = &mut self.con;
        self.fte.sync(&desc, &map, opts, settings, &self.server.core.fs, &mut self.cl, &mut |s| con.print(s));
    }

}

/// Fog_FogCommand_f. Fades (the time argument) apply instantly.
fn cmd_fog(h: &mut Host, a: &Args) {
    let f = h.cl.fog;
    let n = |i: usize| qcore::cvar::q_atof(a.argv(i));
    let (d, [r, g, b]) = match a.argc() {
        2 | 3 => (n(1), f.color),
        4 => (f.density, [n(1), n(2), n(3)]),
        5 | 6 => (n(1), [n(2), n(3), n(4)]),
        _ => {
            h.print(&format!(
                "usage:\n   fog <density>\n   fog <red> <green> <blue>\n   fog <density> <red> <green> <blue>\ncurrent values:\n   \"density\" is \"{}\"\n   \"red\"     is \"{}\"\n   \"green\"   is \"{}\"\n   \"blue\"    is \"{}\"\n",
                f.density, f.color[0], f.color[1], f.color[2]
            ));
            return;
        }
    };
    h.cl.fog = qclient::Fog { density: d.max(0.0), color: [r, g, b].map(|c| c.clamp(0.0, 1.0)), time: 0.0 };
}

/// The original command line without the mod selection (`-game x`,
/// `-hipnotic`/`-rogue`/`-quoth`) and without `+command` sequences, which
/// belong to the first launch.
fn relaunch_args(cmdline: &[String]) -> Vec<String> {
    let mut out = Vec::new();
    let mut it = cmdline.iter();
    let mut in_cmd = false;
    while let Some(a) = it.next() {
        let l = a.to_ascii_lowercase();
        if l == "-game" {
            it.next();
            in_cmd = false;
        } else if matches!(l.as_str(), "-hipnotic" | "-rogue" | "-quoth") {
            in_cmd = false;
        } else if a.starts_with('+') {
            in_cmd = true;
        } else if a.starts_with('-') && a.len() > 1 && a.parse::<f64>().is_err() {
            in_cmd = false;
            out.push(a.clone());
        } else if !in_cmd {
            out.push(a.clone());
        }
    }
    out
}

/// The strings of the clc_stringcmd messages in `out`.
pub(crate) fn string_cmds(out: &[u8]) -> Vec<String> {
    let mut v = Vec::new();
    let mut p = 0;
    while p < out.len() && out[p] == CLC_STRINGCMD {
        let end = out[p + 1..].iter().position(|&b| b == 0).map_or(out.len(), |e| p + 1 + e);
        v.push(String::from_utf8_lossy(&out[p + 1..end]).into_owned());
        p = end + 1;
    }
    v
}

/// NQ `name`/`color` commands as QW userinfo changes.
fn qw_userinfo(q: &mut crate::qw::QwClient, cmd: &str) {
    let a = Args::tokenize(cmd);
    match a.argv(0).to_ascii_lowercase().as_str() {
        "name" => q.setinfo("name", a.argv(1)),
        "color" => {
            let top = a.argv(1).to_string();
            let bottom = if a.argc() > 2 { a.argv(2).to_string() } else { top.clone() };
            q.setinfo("topcolor", &top);
            q.setinfo("bottomcolor", &bottom);
        }
        _ => q.stringcmd(cmd.trim_end()),
    }
}

/// Mod_PointInLeaf on the client's copy of the world.
fn bsp_leaf(bsp: &qformats::bsp::Bsp, p: [f32; 3]) -> usize {
    let mut n = 0i32;
    while n >= 0 {
        let Some(node) = bsp.nodes.get(n as usize) else { return 0 };
        let Some(pl) = bsp.planes.get(node.plane as usize) else { return 0 };
        let d = Vec3::from(pl.normal).dot(Vec3::from_array(p)) - pl.dist;
        n = node.children[(d <= 0.0) as usize];
    }
    (-(n + 1)) as usize
}

#[cfg(test)]
mod tests;
