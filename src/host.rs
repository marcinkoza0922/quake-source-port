//! Host glue (host.c / host_cmd.c / cl_main.c): command buffer, cvars, the
//! loopback server + client, per-frame input/render/sound.

use std::path::PathBuf;

use qclient::{ClientEvent, ClientState, InputState, MoveSpeeds, UserCmd};
use qcore::cmd::{Args, Cbuf, CmdContext, Commands, Origin};
use qcore::cvar::{CVAR_ARCHIVE, CVAR_SERVERINFO, CVAR_USER, Cvars};
use qcore::math::{Vec3, angle_vectors};
use qinput::aim::{AimContext, AimTarget, TargetKind, aim_assist_allowed};
use qinput::keys::{K_ESCAPE, KeyDest};
use qinput::{Device, Input, InputEvent, InputHost, Routed};
use qnet::protocol::{CLC_STRINGCMD, STAT_ACTIVEWEAPON, STAT_AMMO, STAT_HEALTH, STAT_WEAPONFRAME};
use crate::gfx::Renderer;
use qrender::Settings;
use qserver::{ClientMove, Server};
use qsound::{Listener, SoundEvent, SoundSystem};

use crate::assets::Level;
use crate::console::Console;
use crate::draw::{Canvas, Pics};
use crate::menu::{Action, Menu, Screen};
use crate::platform::HostEvent;
use crate::view::{ViewCvars, ViewState};
mod commands;
mod render_commands;
mod compat_cmds;
mod quaddicted_cli;
mod ref_cmds;
mod bugreport;
mod foreign_cvars;
mod global_cfg;
use commands::*;
mod net;
mod demo;
mod demo_bar;
mod demo_play;
mod demo_mouse;
mod export_ui;
mod autodemo;
mod ghost;
mod capture;
mod mapswitch;
mod render;
mod speed;
mod level_time;
mod metrics;
mod hud_frame;
mod split;
mod split_input;
#[cfg(test)]
mod split_tests;
mod shader;
mod saves;
mod death;
mod console;
mod language;
mod input;
mod frame;
mod underwater;
mod prof;
mod goggles;
mod presets;
mod bullet_time;
mod freecam;
mod hitstop;
mod pad_gestures;
mod toy_fx;
mod ctx;
mod csqc;
mod cvars;
use cvars::*;
pub(crate) mod feedback;
use crate::reticle::{self, draw_crosshair, hitmarker};
mod video;
mod touch;
mod training_hud;

/// Server tic rate when rendering faster (ironwail-style decoupling).
const SERVER_TIC: f64 = 1.0 / 72.0;


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
mod grenade_arc;
mod arc_learn;
use aim_vis::WorldVis;

type SettingsCacheKey = (u64, bool, u32, bool, bool, bool, [u32; 4]);
type SettingsCache = (SettingsCacheKey, qrender::Settings, f32, bool);

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
    /// The mixer shared with the audio device, for video capture.
    pub mixer: Option<std::sync::Arc<qsound::SharedMixer>>,
    /// Enhanced audio state (reverb probe, occlusion, footsteps, soundtracks).
    audio_fx: crate::audio::AudioFx,
    /// Last rendered camera view (used for immediate occlusion checks on sound start).
    last_audio_view: Option<qrender::View>,
    /// Video recording or clip export in progress.
    capture: Option<capture::CaptureState>,
    /// Export dialog open: what it would export.
    pending_export: Option<capture::ExportReq>,
    /// Export screen: the still under the panel, and when it was last drawn.
    export_pic: Option<(qrender::PicId, u32, u32)>,
    export_ui_at: Option<qcore::time::Instant>,
    /// The last frame's renderer settings (the export panel reuses them).
    last_settings: Option<qrender::Settings>,
    /// Cached cvar-derived portion of `qrender::Settings`, invalidated on cvar/level changes.
    settings_cache: Option<SettingsCache>,
    /// Cached per-frame host cvars, invalidated on cvar/zoom/freecam/mapper changes.
    render_cvars_cache: Option<(render::RenderCvarsKey, render::RenderCvars)>,
    /// Reusable per-frame render buffers (zero steady-state heap reallocs).
    frame_ents: Vec<qrender::RenderEntity>,
    frame_dyn_verts: Vec<u32>,
    frame_parts: Vec<qrender::Particle>,
    frame_dls: Vec<qrender::DLight>,
    demo_msg_buf: Vec<u8>,
    /// Last cvar generation seen by `sync_language`.
    lang_cvar_gen: Option<u64>,
    /// Reusable per-frame brush entity list shared between sound and render.
    frame_brushes: Option<Vec<crate::fte::world::BrushEnt>>,
    /// `cl_autodemo`: this level's recording, and the last finished one.
    autodemo: Option<autodemo::AutoDemo>,
    last_autodemo: Option<PathBuf>,
    /// Headless export done: quit once background jobs have finished.
    quit_after_jobs: bool,
    level: Level,
    csqc: crate::csqc::Csqc,
    view: ViewState,
    debris: crate::debris::Debris,
    blood: crate::blood::Blood,
    impacts: crate::impacts::Impacts,
    hitfx: crate::hitfx::HitFx,
    tracers: crate::tracers::Tracers,
    bboxes: crate::bboxes::BBoxes,
    automap: crate::automap::Automap,
    /// `r_testlight`: fixed dynamic lights for checking real-time lighting.
    test_lights: Vec<qrender::DLight>,
    /// scr_showspeed: (peak this interval, shown value, interval start).
    speed: (f32, f32, f64),
    pub realtime: f64,
    accum: f64,
    pub quit: bool,
    pend_attack: bool,
    pend_attack2: bool,
    pend_reload: bool,
    pend_jump: bool,
    pend_impulse: u8,
    wheel: crate::wheel::Wheel,
    /// Player 1's weapon selection.
    toy: crate::addons::WeaponSelection,
    captions: crate::captions::Captions,
    hud: crate::hud::Hud,
    training: crate::hud::Training,
    feedback: feedback::Feedback,
    feedback_sounds: feedback::CueCache,
    portal_fx: crate::addons::portal_fx::Presentation,
    rail_fx: crate::addons::rail_fx::Presentation,
    akimbo_fx: crate::addons::akimbo_fx::Presentation,
    rifle_fx: crate::addons::rifle_fx::Presentation,
    /// Player 1's rifle sight.
    ads: crate::addons::rifle_ads::Ads,
    hook_fx: crate::addons::hook_fx::HookFx,
    cable_fx: crate::addons::cable_fx::CableFx,
    grenade_arc: grenade_arc::GrenadeArc,
    arc_learn: arc_learn::ArcLearn,
    gel_fx: crate::addons::gel_fx::GelFx,
    water_fx: crate::addons::water_fx::WaterFx,
    boomerang_fx: crate::addons::boomerang_fx::Presentation,
    /// Player 1's add-on equipment (goggles) and equipment wheel.
    goggles: goggles::Goggles,
    pend_dodge: bool,
    bullet_time: bullet_time::BulletTime,
    freecam: freecam::FreeCam,
    hitstop: hitstop::HitStop,
    /// Split-screen players 2..6 (`cl_splitscreen`); player 1 is `cl`.
    pub guests: Vec<crate::splitscreen::Guest>,
    /// Player 1's gamepad (the first pad used); other pads can join.
    p1_pad: Option<u32>,
    /// While a guest pad's event runs: binds act on that guest's input.
    input_target: Option<usize>,
    guest_cbuf: Cbuf,
    /// Label family of the last connected gamepad.
    pad_family: crate::glyphs::Family,
    pad_families: std::collections::HashMap<u32, crate::glyphs::Family>,
    /// Modern menu text/shape atlas (`crate::font`).
    ui_text: std::rc::Rc<std::cell::RefCell<crate::font::UiText>>,
    /// Rumble to play this frame (low, high, ms), taken by the main loop.
    rumble: Option<(u16, u16, u32)>,
    touch_haptic: Option<(f32, u32)>,
    touch_quickload_probe: f64,
    /// Last seen (weapon, ammo, weaponframe, punch) for fire rumble.
    fire_track: (i32, i32, i32, [f32; 3]),
    /// Held menu navigation key and when it repeats next (gamepad).
    nav_hold: Option<(i32, f64)>,
    /// Controller double-tap / long-press gestures on stats and menu buttons.
    pad_gestures: pad_gestures::PadGestures,
    /// Finger working the menu (tap/drag-scroll).
    menu_touch: Option<touch::MenuTouch>,
    menu_fling: Option<touch::MenuFling>,
    pub(crate) backgrounded: bool,
    last_cmd: UserCmd,
    screenshot: Option<String>,
    /// Save thumbnail to write from the next frame, and uploaded thumbnails
    /// by save name (picture, file time).
    thumb: Option<PathBuf>,
    save_thumbs: std::collections::HashMap<String, (qrender::PicId, qcore::time::SystemTime)>,
    /// The pending screenshot is drawn without the console (bug reports).
    clean_shot: bool,
    /// Frame profiler while `prof_start` is recording (tools/qprof).
    prof: Option<prof::Profiler>,
    delayed: Vec<(f64, String, Origin)>,
    /// Origin of the command being executed; the shell is for `User` only.
    cmd_origin: Origin,
    pub headless: bool,
    show_scores: bool,
    startup: u32,
    pub window: Option<qplatform::Window>,
    /// SDL text input state last set (`None`: not yet), see `sync_key_dest`.
    pub(super) text_input: Option<bool>,
    touch_console_keyboard: bool,
    cmdline: Vec<String>,
    last_vol: (f32, f32),
    metrics: metrics::Metrics,
    shader: shader::ShaderState,
    fs_snapshot: (PathBuf, Vec<String>),
    stuffed: bool,
    /// The current level's exit was already written to qsp_stats.json.
    stats_recorded: bool,
    /// Personal-best state for the HUD timer and PB demos (`level_time`).
    pb: level_time::LevelPb,
    /// Personal-best demo ghost (`cl_demo_ghost`).
    ghost: ghost::GhostState,
    /// The main menu theme is playing (`menu_music`); a CD track replaces it.
    menu_music: bool,
    /// The demo being played (cls.demoplayback).
    demo: Option<crate::demo::Demo>,
    /// The startdemos loop (cls.demos) and the next index in it; -1 stops the loop.
    demos: Vec<String>,
    demonum: i32,
    /// cls.demorecording.
    recording: Option<crate::demo::DemoWriter>,
    /// Background conversions of finished demos (`demo::convert`).
    demo_jobs: Vec<std::sync::mpsc::Receiver<String>>,
    /// `record <name> <map> [track]`: starts when the new map connects.
    pending_record: Option<(PathBuf, i32)>,
    /// Messages up to signon 2 (demo_head), so `record` works mid-game,
    /// and the signon they reach (loopback may batch several stages).
    demo_head: Vec<Vec<u8>>,
    demo_head_signon: u32,
    /// Demo seek in progress: sounds and one-shot effects are skipped.
    demo_seeking: bool,
    /// Mouse on the demo playback bar, and the relative-mouse state applied.
    demo_ui: demo_mouse::Pointer,
    mouse_relative: Option<bool>,
    /// timedemo: frame count and wall-clock start at the second frame.
    timedemo: Option<(u32, qcore::time::Instant)>,
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
    /// Running `slist` or `slist_internet`.
    server_searches: [Option<crate::net::browser::ServerSearch>; 2],
    /// Saved Favorites and Recent servers (`<userdata>/servers.txt`).
    pub saved_servers: crate::net::browser::SavedServers,
    autosave: crate::autosave::Tracker,
    death_handled: bool,
    /// FTE scripted particles.
    fte: crate::fte::Fte,
    /// `play`/`playvol` sound counter (S_Play's `hash`).
    console_sound_id: u32,
    /// `quaddicted_list` / `quaddicted_install` in progress.
    qd_cli: Option<quaddicted_cli::QdCli>,
    /// `togglezoom` / `+zoom` / `-zoom`: (zoom 0..=1, direction -1/0/1).
    zoom: (f32, f32),
    /// `cd` console command state (enabled, paused, remap, current track).
    cd: crate::audio::soundtrack::CdState,
}

impl Host {
    pub fn new(fs: qcore::fs::Fs, mut r: Renderer, snd: Option<SoundSystem>, cmdline: Vec<String>) -> Self {
        let mut cvars = Cvars::new();
        for &(n, d, f) in HOST_CVARS.iter().chain(COMPAT_CVARS).chain(foreign_cvars::FOREIGN_CVARS).chain(goggles::CVARS) {
            cvars.register(n, d, f);
        }
        global_cfg::load(&mut cvars);
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
            mixer: None,
            audio_fx: Default::default(),
            last_audio_view: None,
            level: Level::default(),
            csqc: Default::default(),
            view: ViewState::default(),
            debris: crate::debris::Debris::default(),
            blood: crate::blood::Blood::default(),
            impacts: crate::impacts::Impacts::default(),
            hitfx: crate::hitfx::HitFx::default(),
            tracers: Default::default(),
            bboxes: Default::default(),
            automap: Default::default(),
            test_lights: Vec::new(),
            speed: (0.0, -1.0, 0.0),
            realtime: 0.0,
            accum: 0.0,
            quit: false,
            pend_attack: false,
            pend_attack2: false,
            pend_reload: false,
            pend_jump: false,
            pend_impulse: 0,
            wheel: Default::default(),
            toy: Default::default(),
            captions: Default::default(),
            hud: Default::default(),
            training: Default::default(),
            feedback: Default::default(),
            feedback_sounds: Default::default(),
            portal_fx: Default::default(),
            rail_fx: Default::default(),
            akimbo_fx: Default::default(),
            rifle_fx: Default::default(),
            ads: Default::default(),
            hook_fx: Default::default(),
            cable_fx: Default::default(),
            grenade_arc: Default::default(),
            arc_learn: Default::default(),
            gel_fx: Default::default(),
            water_fx: Default::default(),
            boomerang_fx: Default::default(),
            goggles: Default::default(),
            pend_dodge: false,
            bullet_time: Default::default(),
            freecam: Default::default(),
            hitstop: Default::default(),
            guests: Vec::new(),
            p1_pad: None,
            input_target: None,
            guest_cbuf: Cbuf::new(),
            pad_family: Default::default(),
            pad_families: Default::default(),
            ui_text: Default::default(),
            rumble: None,
            touch_haptic: None,
            touch_quickload_probe: 0.0,
            fire_track: (0, 0, 0, [0.0; 3]),
            nav_hold: None,
            pad_gestures: Default::default(),
            menu_touch: None,
            menu_fling: None,
            backgrounded: false,
            last_cmd: UserCmd::default(),
            screenshot: None,
            thumb: None,
            save_thumbs: Default::default(),
            clean_shot: false,
            prof: None,
            delayed: Vec::new(),
            cmd_origin: Origin::Script,
            headless: false,
            show_scores: false,
            startup: 0,
            window: None,
            text_input: None,
            touch_console_keyboard: false,
            cmdline,
            last_vol: (-1.0, -1.0),
            metrics: Default::default(),
            shader: Default::default(),
            fs_snapshot,
            stuffed: false,
            stats_recorded: false,
            pb: Default::default(),
            ghost: Default::default(),
            menu_music: false,
            console_sound_id: 0,
            qd_cli: None,
            zoom: (0.0, 0.0),
            cd: crate::audio::soundtrack::CdState::new(),
            demo: None,
            demos: Vec::new(),
            demonum: 0,
            recording: None,
            demo_jobs: Vec::new(),
            pending_record: None,
            demo_head: Vec::new(),
            demo_head_signon: 0,
            capture: None,
            pending_export: None,
            export_pic: None,
            export_ui_at: None,
            last_settings: None,
            settings_cache: None,
            render_cvars_cache: None,
            frame_ents: Vec::new(),
            frame_dyn_verts: Vec::new(),
            frame_parts: Vec::new(),
            frame_dls: Vec::new(),
            demo_msg_buf: Vec::new(),
            lang_cvar_gen: None,
            frame_brushes: None,
            autodemo: None,
            last_autodemo: None,
            quit_after_jobs: false,
            demo_seeking: false,
            demo_ui: Default::default(),
            mouse_relative: None,
            timedemo: None,
            net_server: None,
            net_client: None,
            qw_client: None,
            last_server: None,
            net_port: crate::net::DEFAULT_PORT,
            dedicated: false,
            server_searches: [None, None],
            saved_servers: crate::net::browser::SavedServers::load(&crate::userdata::root().join("servers.txt")),
            ping_next: 0.0,
            ping_expect: None,
        };
        qinput::register_commands(&mut h.cmds);
        register_commands(&mut h.cmds);
        goggles::register_goggle_commands(&mut h.cmds);
        shader::register_shader_commands(&mut h.cmds);
        presets::register_preset_commands(&mut h.cmds);
        h.con.forced = true;
        h.con.frac = 1.0;
        h.con.load_history(&crate::user_dir("id1"));
        h.sync_shell_env();
        h.sync_console_catalog();
        h.refresh_soundtracks();
        h.menu.hud_edit.reload_presets();
        h.hud.has_mod_hud = crate::csqc::fs_has_hud(&h.server.core.fs);
        h.menu.hud_edit.has_mod_hud = h.hud.has_mod_hud;
        h.menu.update_saved_servers(&h.saved_servers);
        // 2026 update compat (ironwail host.c): word-wrap centerprints for mg3.
        let mg3 = h.server.core.fs.game_dirs().iter().any(|d| d.eq_ignore_ascii_case("mg3"));
        h.server.core.cvars.set("scr_usekfont", if mg3 { "1" } else { "0" });
        // First run of this game (no user config, none to inherit): ask for
        // a graphics preset when the main menu first shows (`sync_menu_ui`).
        h.menu.preset_pending = h.inherited_config().is_none() && !qcore::vfs::is_file(&h.user_dir().join("config.cfg"));
        h.cbuf.add_text("exec quake.rc\n");
        h
    }

    /// `-dedicated`: no local client. Like QuakeSpasm/ironwail/vkQuake
    /// Host_Init, a dedicated server runs only autoexec.cfg and the command
    /// line, never the game's quake.rc (which may set cvars, e.g. AD's
    /// `temp1 1024`).
    /// Player 1's current world origin (from the client's view entity).
    pub(super) fn player_origin(&self) -> Vec3 {
        let o = self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| e.origin);
        Vec3::from_array(o)
    }

    pub fn make_dedicated(&mut self) {
        self.dedicated = true;
        let _ = self.cbuf.take_text();
        self.cbuf.add_text("exec autoexec.cfg\nstuffcmds\n");
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

    /// Time scale for the world while the weapon wheel is open (single player only).
    /// Also applies the accessibility game speed (`a11y_gamespeed`).
    fn wheel_timescale(&self) -> f32 {
        let sp = self.local.is_some() && self.server.core.maxclients() == 1 && self.demo.is_none();
        let speed = if sp { self.cv("a11y_gamespeed").clamp(0.25, 1.0) } else { 1.0 };
        if self.wheel.open && sp { self.cv("cl_wheel_timescale").clamp(0.05, 1.0) * speed } else { speed }
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
        self.server.core.set_custom(false);
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
    /// `r_enhancedparticles` adds the built-in set either way.
    fn sync_fte(&mut self) {
        let desc = if self.cv("r_fteparticles") != 0.0 { self.cvs("r_particledesc").to_string() } else { String::new() };
        let enhanced = self.cv("r_enhancedfx") != 0.0 && self.cv("r_enhancedparticles") != 0.0;
        let map = self.level.mapname.clone();
        let opts = qpart::RenderOptions {
            sparks: self.cv("r_part_sparks"),
            sparks_textured: self.cv("r_part_sparks_textured") != 0.0,
            sparks_trifan: self.cv("r_part_sparks_trifan") != 0.0,
            beams: self.cv("r_part_beams"),
        };
        let settings = qpart::Settings {
            density: self.cv("r_part_density") * render::particle_density(self.cv("r_particle_quality")),
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
        self.fte.sync(&desc, &map, enhanced, opts, settings, &self.server.core.fs, &mut self.cl, &mut |s| con.print(s));
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
        _ => return h.print(&format!("usage:\n   fog <density>\n   fog <red> <green> <blue>\n   fog <density> <red> <green> <blue>\ncurrent values:\n   \"density\" is \"{}\"\n   \"red\"     is \"{}\"\n   \"green\"   is \"{}\"\n   \"blue\"    is \"{}\"\n", f.density, f.color[0], f.color[1], f.color[2])),
    };
    h.cl.fog = qclient::Fog { density: d.max(0.0), color: [r, g, b].map(|c| c.clamp(0.0, 1.0)), time: 0.0 };
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
