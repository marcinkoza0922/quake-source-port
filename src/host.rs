//! Host glue (host.c / host_cmd.c / cl_main.c): command buffer, cvars, the
//! loopback server + client, per-frame input/render/sound.

use std::path::PathBuf;

use qclient::{ClientEvent, ClientState, InputState, MoveSpeeds, UserCmd};
use qcore::cmd::{Args, Cbuf, CmdContext, Commands};
use qcore::cvar::{CVAR_ARCHIVE, CVAR_SERVERINFO, Cvars};
use qcore::math::{Vec3, angle_vectors};
use qinput::aim::{AimContext, AimTarget, TargetKind, Visibility, aim_assist_allowed};
use qinput::keys::{K_ESCAPE, KeyDest};
use qinput::{Input, InputEvent, InputHost, Routed};
use qnet::protocol::{CLC_STRINGCMD, STAT_HEALTH};
use qrender::{Renderer, Settings};
use qserver::{ClientMove, Server};
use qsound::{Listener, SoundEvent, SoundSystem};

use crate::assets::Level;
use crate::console::Console;
use crate::draw::{Canvas, Pics};
use crate::menu::{Action, Menu, Screen};
use crate::platform::HostEvent;
use crate::view::{ViewCvars, ViewState};

/// Server tic rate when rendering faster (ironwail-style decoupling).
const SERVER_TIC: f64 = 1.0 / 72.0;

/// Virtual rows taken by the classic status bar + inventory bar.
const CLASSIC_SBAR_LINES: f32 = 48.0;

const HOST_CVARS: &[(&str, &str, u32)] = &[
    ("host_maxfps", "250", CVAR_ARCHIVE),
    ("fov", "90", CVAR_ARCHIVE),
    ("cl_bob", "0.02", 0),
    ("cl_bobcycle", "0.6", 0),
    ("cl_bobup", "0.5", 0),
    ("v_kicktime", "0.5", 0),
    ("v_kickroll", "0.6", 0),
    ("v_kickpitch", "0.6", 0),
    ("v_gunkick", "1", CVAR_ARCHIVE),
    ("r_drawviewmodel", "1", 0),
    ("r_lerpmodels", "1", CVAR_ARCHIVE),
    ("r_lerpmove", "1", CVAR_ARCHIVE),
    ("scr_hudstyle", "1", CVAR_ARCHIVE),
    ("crosshair", "0", CVAR_ARCHIVE),
    ("scr_centertime", "2", 0),
    ("scr_printspeed", "8", 0),
    ("volume", "0.7", CVAR_ARCHIVE),
    ("bgmvolume", "1", CVAR_ARCHIVE),
    ("gamma", "1", CVAR_ARCHIVE),
    ("contrast", "1", CVAR_ARCHIVE),
    ("r_wateralpha", "1", CVAR_ARCHIVE),
    ("r_skyfog", "0.5", 0),
    ("r_drawentities", "1", 0),
    ("r_lavaalpha", "0", 0),
    ("r_slimealpha", "0", 0),
    ("r_telealpha", "0", 0),
    ("r_dynamic", "1", 0),
    ("vid_vsync", "0", CVAR_ARCHIVE),
    ("vid_fullscreen", "0", CVAR_ARCHIVE),
    ("vid_width", "1280", CVAR_ARCHIVE),
    ("vid_height", "720", CVAR_ARCHIVE),
    ("r_msaa", "4", CVAR_ARCHIVE),
    ("r_renderscale", "1", CVAR_ARCHIVE),
    ("r_texfilter", "0", CVAR_ARCHIVE),
    ("r_anisotropy", "16", CVAR_ARCHIVE),
    ("gl_lodbias", "auto", CVAR_ARCHIVE),
    ("vid_fsaamode", "0", CVAR_ARCHIVE),
    ("r_dither", "1", CVAR_ARCHIVE),
    ("r_softemu", "0", CVAR_ARCHIVE),
    ("r_softemu_metric", "-1", CVAR_ARCHIVE),
    ("r_softemu_lightmap_banding", "-1", CVAR_ARCHIVE),
    ("r_softemu_mdl_warp", "-1", CVAR_ARCHIVE),
    ("r_softemu_dither_screen", "1", CVAR_ARCHIVE),
    ("r_softemu_dither_texture", "1", CVAR_ARCHIVE),
    ("cl_forwardspeed", "200", CVAR_ARCHIVE),
    ("cl_backspeed", "200", CVAR_ARCHIVE),
    ("cl_sidespeed", "350", 0),
    ("cl_upspeed", "200", 0),
    ("cl_movespeedkey", "2.0", 0),
    ("cl_yawspeed", "140", 0),
    ("cl_pitchspeed", "150", 0),
    ("cl_anglespeedkey", "1.5", 0),
    ("cl_alwaysrun", "1", CVAR_ARCHIVE),
    ("cl_maxpitch", "90", CVAR_ARCHIVE),
    ("cl_minpitch", "-90", CVAR_ARCHIVE),
    ("sv_aimassist", "0", CVAR_SERVERINFO),
    ("_cl_name", "player", CVAR_ARCHIVE),
    ("_cl_color", "0", CVAR_ARCHIVE),
    ("scr_showfps", "0", CVAR_ARCHIVE),
    ("viewsize", "100", CVAR_ARCHIVE),
    ("campaign", "0", 0),
    ("horde", "0", 0),
    ("sv_cheats", "0", 0),
    ("scratch1", "0", 0),
    ("scratch2", "0", 0),
    ("scratch3", "0", 0),
    ("scratch4", "0", 0),
    ("saved1", "0", CVAR_ARCHIVE),
    ("saved2", "0", CVAR_ARCHIVE),
    ("saved3", "0", CVAR_ARCHIVE),
    ("saved4", "0", CVAR_ARCHIVE),
    ("con_notifytime", "3", 0),
    ("scr_conspeed", "500", CVAR_ARCHIVE),
    ("scr_sbaralpha", "0.75", CVAR_ARCHIVE),
];

/// Cvars other engines register that mod configs (vkQuake.cfg, AD/KEX
/// configs) set. Registered without effect so loading those configs is
/// quiet; `gamecfg`/`savedgamecfg` are read and written by QC (pr_edict.c).
const COMPAT_CVARS: &[(&str, &str, u32)] = &[
    ("gamecfg", "0", 0),
    ("savedgamecfg", "0", CVAR_ARCHIVE),
    ("scr_usekfont", "0", 0),
    ("cl_beams_polygons", "1", 0),
    ("r_noshadow_list", "", 0),
    ("r_noEntityCastShadowList", "", 0),
    ("r_useportalculling", "1", 0),
    ("r_nolerp_list", "", 0),
    ("_snd_mixahead", "0.1", 0),
    ("bgm_extmusic", "1", 0),
    ("chase_active", "0", 0),
    ("fov_adapt", "1", 0),
    ("gl_clear", "0", 0),
    ("gl_farclip", "16384", 0),
    ("gl_fullbrights", "1", 0),
    ("gl_overbright", "1", 0),
    ("gl_overbright_models", "1", 0),
    ("gl_subdivide_size", "128", 0),
    ("gl_texture_anisotropy", "1", 0),
    ("gl_texturemode", "", CVAR_ARCHIVE),
    ("gl_triplebuffer", "1", 0),
    ("joy_deadzone", "0.175", 0),
    ("lookspring", "0", 0),
    ("r_clearcolor", "2", 0),
    ("r_novis", "0", 0),
    ("r_part_density", "1", 0),
    ("r_particles", "1", 0),
    ("r_quadparticles", "1", 0),
    ("r_scale", "1", 0),
    ("r_shadows", "0", 0),
    ("r_viewmodel_quake", "0", 0),
    ("scr_conscale", "1", 0),
    ("scr_menuscale", "1", 0),
    ("scr_sbarscale", "1", 0),
    ("sys_throttle", "0.02", 0),
    ("vid_minsize", "0", 0),
];

/// Quakespasm's default.cfg for games without one (the 2021 re-release).
const DEFAULT_CFG: &str = include_str!("default.cfg");

/// Line-of-sight through the map's hull 0.
struct WorldVis<'a>(&'a qworld::hull::CollisionModel);
impl Visibility for WorldVis<'_> {
    fn visible(&self, from: [f32; 3], to: [f32; 3]) -> bool {
        self.0.hull(0, 0).trace(Vec3::from_array(from), Vec3::from_array(to)).fraction >= 1.0
    }
}

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
    pub realtime: f64,
    accum: f64,
    pub quit: bool,
    pend_attack: bool,
    pend_jump: bool,
    pend_impulse: u8,
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
    fs_snapshot: (PathBuf, Vec<String>),
    stuffed: bool,
    /// The current level's exit was already written to qsp_stats.json.
    stats_recorded: bool,
}

impl CmdContext for Host {
    fn cbuf(&mut self) -> &mut Cbuf {
        &mut self.cbuf
    }
    fn cvars(&mut self) -> &mut Cvars {
        &mut self.server.core.cvars
    }
    fn commands(&mut self) -> &mut Commands<Self> {
        &mut self.cmds
    }
    fn print(&mut self, text: &str) {
        self.con.print(text);
    }
}

impl InputHost for Host {
    fn input(&mut self) -> &mut Input {
        &mut self.input
    }
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
            realtime: 0.0,
            accum: 0.0,
            quit: false,
            pend_attack: false,
            pend_jump: false,
            pend_impulse: 0,
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
            fs_snapshot,
            stuffed: false,
            stats_recorded: false,
        };
        qinput::register_commands(&mut h.cmds);
        register_commands(&mut h.cmds);
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

    /// Cmd_ExecuteString with server-command forwarding.
    pub fn exec_line(&mut self, line: &str) {
        let args = Args::tokenize(line);
        if args.argc() == 0 {
            return;
        }
        let name = args.argv(0).to_ascii_lowercase();
        let known = self.cmds.exists(&name) || self.cmds.alias(&name).is_some() || self.server.core.cvars.find(&name).is_some();
        if known {
            qcore::cmd::execute_string(self, line);
        } else if self.server.has_command(&name) {
            self.server.command(line);
        } else if let Some(i) = self.local.filter(|_| self.server.active()) {
            self.server.client_command(i, line);
        } else {
            self.con.print(&format!("Unknown command \"{}\"\n", args.argv(0)));
        }
        self.drain_server_log();
    }

    fn exec_cbuf(&mut self) {
        loop {
            // One shared Cbuf as in Quake: QC localcmd text runs here, so it
            // reaches host commands too.
            let t = self.server.take_text();
            if !t.is_empty() {
                self.cbuf.add_text(&t);
            }
            if self.cbuf.take_wait() {
                break;
            }
            let Some(line) = self.cbuf.next_line() else { break };
            self.exec_line(&line);
            if self.quit {
                break;
            }
        }
    }

    fn drain_server_log(&mut self) {
        // Most server log lines are also sent to the client (bprint, sprint);
        // echo only diagnostics unless `developer` is set.
        let log = std::mem::take(&mut self.server.core.log);
        let dev = self.cv("developer") != 0.0;
        for s in log {
            if dev || ["rror", "ouldn't", "not ", "nknown", "valid", "usage", "<"].iter().any(|k| s.contains(k)) {
                let s = self.server.core.loc.translate(&s);
                self.con.print(&s);
            }
        }
    }

    /// Commands from the command line (`+map e1m1 +skill 2`).
    fn stuff_cmds(&mut self) {
        if std::mem::replace(&mut self.stuffed, true) {
            return;
        }
        let mut out = String::new();
        for a in &self.cmdline {
            if let Some(rest) = a.strip_prefix('+') {
                out.push('\n');
                out.push_str(rest);
            } else if !out.is_empty() && !a.starts_with('-') {
                out.push(' ');
                if a.contains(' ') {
                    out.push_str(&format!("\"{a}\""));
                } else {
                    out.push_str(a);
                }
            }
        }
        out.push('\n');
        self.cbuf.insert_text(out.trim_start());
    }

    fn key_dest(&self) -> KeyDest {
        if self.menu.active() {
            KeyDest::Menu
        } else if self.con.open || self.con.forced {
            KeyDest::Console
        } else {
            KeyDest::Game
        }
    }

    fn sync_key_dest(&mut self) {
        let d = self.key_dest();
        if self.input.dest != d {
            qinput::set_key_dest(self, d);
            if let Some(w) = &self.window {
                let _ = w.set_relative_mouse(d == KeyDest::Game);
                let _ = w.set_text_input(d == KeyDest::Console);
            }
        }
    }

    fn local_sound(&mut self, name: &str) {
        if let Some(s) = &mut self.snd
            && let Some(sfx) = s.precache(name)
        {
            s.event(SoundEvent::Local(sfx));
        }
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

    fn handle_event(&mut self, ev: HostEvent) {
        match ev {
            HostEvent::Quit => self.quit = true,
            HostEvent::Resize => {
                if let Some(win) = &self.window {
                    let (w, h) = win.drawable_size();
                    self.r.resize(w, h);
                    // remember user-resized window size
                    if self.cv("vid_fullscreen") == 0.0 {
                        let (ww, wh) = win.window_size();
                        let cv = &mut self.server.core.cvars;
                        cv.set("vid_width", &ww.to_string());
                        cv.set("vid_height", &wh.to_string());
                    }
                }
            }
            HostEvent::Focus => {}
            HostEvent::MousePos { x, y } => {
                if !self.menu.active() {
                    return;
                }
                let Some(win) = &self.window else { return };
                let (pw, ph) = win.drawable_size();
                let (ww, wh) = win.window_size();
                let (px, py) = (x * pw as f32 / ww.max(1) as f32, y * ph as f32 / wh.max(1) as f32);
                let mut c = Canvas::new(pw, ph);
                c.menu_layout();
                let pos = (px / c.scale - c.ox, py / c.scale - c.oy);
                if let Some(snd) = self.menu.mouse_move(pos) {
                    self.local_sound(snd);
                }
            }
            HostEvent::Input(ie) => {
                if let InputEvent::Key { key, down: true } = ie
                    && key == b'`' as i32
                {
                    self.toggle_console();
                    self.sync_key_dest();
                    return;
                }
                if let InputEvent::Text(c) = ie
                    && (c == '`' || c == '~')
                {
                    return;
                }
                let routed = qinput::handle_event(self, ie);
                match routed {
                    Some(Routed::Escape) => self.menu.open(Screen::Main),
                    Some(Routed::Key { key, dest: KeyDest::Console }) => {
                        if key == K_ESCAPE {
                            if self.con.forced {
                                self.menu.open(Screen::Main);
                            } else {
                                self.con.open = false;
                            }
                        } else {
                            let cmds = &self.cmds;
                            let cv = &self.server.core.cvars;
                            let sv = &self.server;
                            let complete = |p: &str| -> Vec<String> {
                                let mut v: Vec<String> = cmds.complete(p).into_iter().map(str::to_string).collect();
                                v.extend(cv.complete(p).into_iter().map(str::to_string));
                                v.extend(sv.complete_command(p).into_iter().map(str::to_string));
                                v
                            };
                            if let Some(line) = self.con.key(key, &complete) {
                                self.cbuf.add_text(&line);
                                self.cbuf.add_text("\n");
                            }
                        }
                    }
                    Some(Routed::Text { ch, dest: KeyDest::Console }) => self.con.text(ch),
                    Some(Routed::Key { key, dest: KeyDest::Menu }) => {
                        let active = self.server.active();
                        let acts = self.menu.key(key, &mut self.server.core.cvars, active);
                        for a in acts {
                            match a {
                                Action::Command(c) => self.cbuf.add_text(&c),
                                Action::Sound(s) => self.local_sound(s),
                                Action::Close => {}
                            }
                        }
                    }
                    _ => {}
                }
                self.sync_key_dest();
            }
        }
    }

    /// Connects the local client when a server is running without one.
    fn check_local_client(&mut self) {
        if !self.server.active() {
            if self.local.take().is_some() || self.cl.signon != 0 {
                self.disconnect_client();
            }
            return;
        }
        let ok = self.local.is_some_and(|i| self.server.core.clients.get(i).is_some_and(|c| c.active));
        if !ok {
            self.cl = ClientState::new();
            self.cl.name = self.server.core.cvars.by_name("_cl_name").map_or("player".into(), |c| c.string.clone());
            self.local = self.server.connect_local();
            self.drain_server_log();
            self.feed_client();
        }
    }

    fn disconnect_client(&mut self) {
        self.cl = ClientState::new();
        self.local = None;
        self.level = Level::default();
        if let Some(s) = &self.snd {
            s.event(SoundEvent::StopAll);
            s.event(SoundEvent::ClearStatic);
            s.stop_music();
        }
        self.con.forced = true;
    }

    /// Delivers the server's reliable stream and datagrams to the client.
    fn feed_client(&mut self) {
        let Some(i) = self.local else { return };
        let rel = self.server.take_reliable(i);
        let mut msgs = Vec::new();
        if !rel.is_empty() {
            msgs.push(rel);
        }
        msgs.extend(self.server.take_datagrams(i));
        for m in msgs {
            if let Err(e) = self.cl.parse_server_message(&m) {
                self.con.print(&format!("Host_Error: {e:?}\n"));
                self.server.level = None;
                self.disconnect_client();
                return;
            }
        }
        self.process_client_events();
    }

    /// Client string commands (clc_stringcmd) back to the server.
    fn forward_client_cmds(&mut self) {
        let out = self.cl.take_reliable_out();
        let Some(i) = self.local else { return };
        let mut p = 0;
        while p < out.len() {
            if out[p] as i32 != CLC_STRINGCMD as i32 {
                break;
            }
            let end = out[p + 1..].iter().position(|&b| b == 0).map_or(out.len(), |e| p + 1 + e);
            let s = String::from_utf8_lossy(&out[p + 1..end]).into_owned();
            p = end + 1;
            let low = s.trim_start().to_ascii_lowercase();
            if ["prespawn", "spawn", "begin", "name", "color"].iter().any(|p| low.starts_with(p)) {
                continue; // connect_local runs the signon itself
            }
            self.server.client_command(i, s.trim_end());
        }
    }

    fn process_client_events(&mut self) {
        let evs = self.cl.take_events();
        for e in evs {
            match e {
                ClientEvent::Print(s) => {
                    let s = self.server.core.loc.translate(&s);
                    self.con.print(&s)
                }
                ClientEvent::CenterPrint(s) => {
                    let t = self.cv("scr_centertime") as f64;
                    let s = self.server.core.loc.translate(&s);
                    let cols = if self.cv("scr_usekfont") != 0.0 {
                        let (w, h) = self.r.size();
                        crate::console::center_wrap_cols(Canvas::new(w, h).vw())
                    } else {
                        0
                    };
                    self.con.center_print(&s, t, cols);
                }
                ClientEvent::StuffText(s) => self.cbuf.add_text(&s),
                ClientEvent::Sound { ent, channel, sound, origin, volume, attenuation } => {
                    if let (Some(snd), Some(Some(sfx))) = (&self.snd, self.level.sounds.get(sound)) {
                        snd.event(SoundEvent::Start { entity: ent, channel, sfx: sfx.clone(), origin, volume, attenuation });
                    }
                }
                ClientEvent::TempSound { name, origin } => {
                    if let Some(snd) = &mut self.snd
                        && let Some(sfx) = snd.precache(name)
                    {
                        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin, volume: 1.0, attenuation: 1.0 });
                    }
                }
                ClientEvent::LocalSound { sound } => {
                    if let (Some(snd), Some(Some(sfx))) = (&self.snd, self.level.sounds.get(sound)) {
                        snd.event(SoundEvent::Local(sfx.clone()));
                    }
                }
                ClientEvent::StopSound { ent, channel } => {
                    if let Some(snd) = &self.snd {
                        snd.event(SoundEvent::Stop { entity: ent, channel });
                    }
                }
                ClientEvent::StaticSound(s) => {
                    if let (Some(snd), Some(Some(sfx))) = (&self.snd, self.level.sounds.get(s.sound)) {
                        snd.event(SoundEvent::Static {
                            sfx: sfx.clone(),
                            origin: s.origin,
                            volume: s.volume as f32 / 255.0,
                            attenuation: s.attenuation as f32 / 64.0,
                        });
                    }
                }
                ClientEvent::CdTrack { track, .. } => {
                    if let Some(snd) = &mut self.snd {
                        snd.play_cd_track(track, true);
                    }
                }
                ClientEvent::Damage { armor, blood, from } => {
                    let cv = self.view_cvars();
                    self.view.parse_damage(&self.cl, &cv, armor, blood, from);
                }
                ClientEvent::Pause(p) => {
                    if let Some(snd) = &self.snd {
                        snd.pause_music(p);
                    }
                }
                ClientEvent::Skybox(name) => {
                    let fs = &self.server.core.fs;
                    self.level.set_sky(fs, &mut self.r, &name);
                }
                ClientEvent::BonusFlash => self.view.bonus_flash(),
                ClientEvent::NewMap => {
                    self.stats_recorded = false;
                    self.new_map()
                }
                ClientEvent::Signon(4) => {
                    self.con.forced = false;
                    self.con.open = false;
                    self.con.clear_notify();
                    self.menu.screen = Screen::None;
                    self.sync_key_dest();
                }
                ClientEvent::Disconnect => self.con.print("Server disconnected\n"),
                _ => {}
            }
        }
        if self.cl.intermission != 0 && !self.stats_recorded {
            self.stats_recorded = true;
            self.record_level_stats();
        }
    }

    /// Level exit (intermission or finale): merge kills/secrets/time into
    /// the mod's qsp_stats.json. Not for deathmatch or headless tool runs.
    fn record_level_stats(&mut self) {
        use qnet::protocol::{STAT_MONSTERS, STAT_SECRETS, STAT_TOTALMONSTERS, STAT_TOTALSECRETS};
        if self.headless || self.cv("deathmatch") != 0.0 || self.level.mapname.is_empty() {
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

    /// The base game's user config text, when the running mod has none of
    /// its own yet: (game dir it came from, text).
    fn inherited_config(&self) -> Option<(String, String)> {
        if self.user_dir().join("config.cfg").is_file() {
            return None;
        }
        let dirs = &self.fs_snapshot.1;
        dirs.iter().rev().skip(1).find_map(|g| {
            let t = std::fs::read(crate::user_dir(g).join("config.cfg")).ok()?;
            Some((g.clone(), String::from_utf8_lossy(&t).into_owned()))
        })
    }

    /// Name of the top gamedir ("id1" when no mod is mounted).
    pub fn game_name(&self) -> &str {
        self.fs_snapshot.1.last().map_or("id1", String::as_str)
    }

    /// `game <dir> [map]`: switch to another mod. The renderer, sound and
    /// file system are built for one gamedir at startup, so this saves the
    /// config and re-executes the engine with the mod's launch flags (and
    /// `+map`); the current mod only needs the `map` command.
    fn switch_game(&mut self, dir: &str, map: Option<&str>) {
        let basedir = self.fs_snapshot.0.clone();
        if dir.eq_ignore_ascii_case(self.game_name()) {
            if let Some(m) = map {
                self.cbuf.add_text(&format!("maxplayers 1\ndeathmatch 0\ncoop 0\nmap {m}\n"));
            }
            return;
        }
        let Some(info) = qui::mods::scan(&basedir).into_iter().find(|m| m.dir.eq_ignore_ascii_case(dir)) else {
            self.con.print(&format!("game: no game dir \"{dir}\" in {}\n", basedir.display()));
            return;
        };
        let mut args = relaunch_args(&self.cmdline);
        args.extend(info.launch_args());
        if let Some(m) = map {
            args.extend(["+map".to_string(), m.to_string()]);
        }
        self.write_config();
        let Ok(exe) = std::env::current_exe() else {
            self.con.print("game: can't find the engine executable\n");
            return;
        };
        self.con.print(&format!("Switching to {}...\n", info.title));
        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args);
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            // drop the window and audio device before the new image opens its own
            self.window = None;
            self.snd = None;
            let e = cmd.exec();
            self.con.print(&format!("game: exec failed: {e}\n"));
        }
        #[cfg(not(unix))]
        match cmd.spawn() {
            Ok(_) => self.quit = true,
            Err(e) => self.con.print(&format!("game: launch failed: {e}\n")),
        }
    }

    /// Fills the mod menu's list (`_mods`).
    fn scan_mods(&mut self) {
        let mods = qui::mods::scan(&self.fs_snapshot.0);
        let cur = self.game_name().to_string();
        self.menu.set_mods(mods, &cur);
    }

    /// Fills the mod menu's map page for one mod (`_modmaps <dir>`).
    fn scan_mod_maps(&mut self, dir: &str) {
        let maps = qui::mods::maps(&self.fs_snapshot.0, dir);
        let stats = qui::stats::Stats::load(&crate::user_dir(dir));
        self.menu.set_mod_maps(maps, stats);
    }

    fn new_map(&mut self) {
        if let Some(s) = &self.snd {
            s.event(SoundEvent::StopAll);
            s.event(SoundEvent::ClearStatic);
        }
        let t0 = std::time::Instant::now();
        let fs = &self.server.core.fs;
        let con = &mut self.con;
        self.level = Level::load(fs, &mut self.r, self.snd.as_mut(), &mut self.cl, &mut |s| con.print(s));
        // Fog_ParseWorldspawn; an svc_fog parsed in the same batch wins.
        if self.cl.fog == qclient::Fog::default() {
            let w = self.level.world_fog;
            self.cl.fog = qclient::Fog { density: w.density, color: w.color, time: 0.0 };
        }
        self.view.reset();
        self.con.print(&format!("loaded {} in {:.2}s\n", self.level.mapname, t0.elapsed().as_secs_f32()));
    }

    /// After config.cfg: command-line -width/-height/-fullscreen/-window
    /// override the vid_* cvars, then the display is applied.
    fn startup_video(&mut self) {
        let arg = |n: &str| self.cmdline.iter().position(|a| a == n).and_then(|i| self.cmdline.get(i + 1)).cloned();
        let has = |n: &str| self.cmdline.iter().any(|a| a == n);
        let cv = &mut self.server.core.cvars;
        if let Some(w) = arg("-width") {
            cv.set("vid_width", &w);
        }
        if let Some(h) = arg("-height") {
            cv.set("vid_height", &h);
        }
        if has("-window") {
            cv.set("vid_fullscreen", "0");
        } else if has("-fullscreen") {
            cv.set("vid_fullscreen", "1");
        }
        self.vid_apply();
    }

    /// Applies vid_fullscreen / vid_width / vid_height to the window.
    pub fn vid_apply(&mut self) {
        let Some(win) = &self.window else { return };
        let mode = match self.cv("vid_fullscreen") as i32 {
            1 => qplatform::DisplayMode::Fullscreen,
            2 => qplatform::DisplayMode::Borderless,
            _ => qplatform::DisplayMode::Windowed,
        };
        let size = (self.cv("vid_width").max(320.0) as u32, self.cv("vid_height").max(200.0) as u32);
        if let Err(e) = win.set_display(mode, size) {
            self.con.print(&format!("vid_apply: {e:?}\n"));
            return;
        }
        let (w, h) = win.drawable_size();
        self.r.resize(w, h);
    }

    /// Fills the Graphics page's pending display state from the cvars.
    fn vid_menu(&mut self) {
        let mut res = self.window.as_ref().map(|w| w.display_modes()).unwrap_or_default();
        let cur = (self.cv("vid_width") as u32, self.cv("vid_height") as u32);
        if !res.contains(&cur) {
            res.push(cur);
            res.sort_by_key(|r| std::cmp::Reverse(r.0 * r.1));
        }
        let m = &mut self.menu;
        m.resolutions = res;
        m.pending_res = cur;
        m.pending_mode = (self.server.core.cvars.value_of("vid_fullscreen") as usize).min(2);
        m.max_msaa = self.r.max_msaa();
    }

    /// scr_hudstyle 0: classic full-width status bar below the 3D view.
    fn classic_hud(&self) -> bool {
        self.cv("scr_hudstyle") == 0.0
    }

    fn view_cvars(&self) -> ViewCvars {
        let v = |n: &str| self.cv(n);
        ViewCvars {
            bob: v("cl_bob"),
            bobcycle: v("cl_bobcycle"),
            bobup: v("cl_bobup"),
            rollangle: v("cl_rollangle"),
            rollspeed: v("cl_rollspeed").max(1.0),
            kicktime: v("v_kicktime").max(0.01),
            kickroll: v("v_kickroll"),
            kickpitch: v("v_kickpitch"),
            fov: v("fov").clamp(10.0, 170.0),
            gunkick: v("v_gunkick") != 0.0,
            drawviewmodel: v("r_drawviewmodel") != 0.0,
            lerp_models: v("r_lerpmodels") != 0.0,
            lerp_move: v("r_lerpmove") != 0.0,
        }
    }

    fn move_speeds(&self) -> MoveSpeeds {
        let v = |n: &str| self.cv(n);
        MoveSpeeds {
            forward: v("cl_forwardspeed"),
            back: v("cl_backspeed"),
            side: v("cl_sidespeed"),
            up: v("cl_upspeed"),
            yaw: v("cl_yawspeed"),
            pitch: v("cl_pitchspeed"),
            movespeedkey: v("cl_movespeedkey"),
            anglespeedkey: v("cl_anglespeedkey"),
            maxpitch: v("cl_maxpitch"),
            minpitch: v("cl_minpitch"),
        }
    }

    /// Aim-assist targets from the loopback server's live monsters/players,
    /// positioned at the client's interpolated origins.
    fn aim_targets(&self) -> Vec<AimTarget> {
        let mut out = Vec::new();
        let Some(l) = &self.server.level else { return out };
        let vm = &l.vm;
        let (Some(fl), Some(hp), Some(org), Some(mn), Some(mx), Some(vel)) = (
            vm.field_ofs("flags"),
            vm.field_ofs("health"),
            vm.field_ofs("origin"),
            vm.field_ofs("mins"),
            vm.field_ofs("maxs"),
            vm.field_ofs("velocity"),
        ) else {
            return out;
        };
        let maxc = self.server.core.maxclients();
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || e as usize == self.cl.viewentity || vm.ent_f(e, hp) <= 0.0 {
                continue;
            }
            let flags = vm.ent_f(e, fl) as i32;
            let kind = if flags & 32 != 0 {
                TargetKind::Monster
            } else if (e as usize) <= maxc {
                TargetKind::Player
            } else {
                continue;
            };
            let origin = self.cl.entities.get(e as usize).filter(|c| c.model != 0).map_or(vm.ent_v(e, org), |c| c.origin);
            out.push(AimTarget { origin, mins: vm.ent_v(e, mn), maxs: vm.ent_v(e, mx), velocity: vm.ent_v(e, vel), kind });
        }
        out
    }

    /// One host frame of `dt` seconds.
    pub fn frame(&mut self, dt: f64, events: Vec<HostEvent>) {
        self.realtime += dt;
        self.con.realtime = self.realtime;
        for e in events {
            self.handle_event(e);
        }
        let now = self.realtime;
        let (due, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.delayed).into_iter().partition(|(t, _)| *t <= now);
        self.delayed = rest;
        for (_, c) in due {
            self.cbuf.add_text(&c);
            self.cbuf.add_text("\n");
        }
        self.exec_cbuf();
        if self.startup < 2 {
            self.startup += 1;
            if self.startup == 1 {
                self.startup_video();
                self.stuff_cmds();
                self.exec_cbuf();
            }
            if self.startup == 2 && !self.server.active() && self.cbuf.is_empty() && self.delayed.is_empty() {
                self.menu.open(Screen::Main);
            }
        }
        self.check_local_client();
        self.sync_key_dest();

        // input (every frame, for smooth view angles)
        let ft = dt as f32;
        let in_game = self.cl.signon == 4 && self.cl.intermission == 0;
        let targets = if in_game && self.input.gamepad_connected { self.aim_targets() } else { Vec::new() };
        let mut inp = {
            let eye = self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| {
                let mut o = e.origin;
                o[2] += self.cl.viewheight;
                o
            });
            let cm = self.server.level.as_ref().map(|l| l.world.worldmodel.clone());
            let vis = cm.as_deref().map(WorldVis);
            let allowed = aim_assist_allowed(self.cv("deathmatch") != 0.0, self.cv("sv_aimassist") != 0.0);
            let ctx = vis.as_ref().map(|v| AimContext { eye, player_velocity: self.cl.velocity, targets: &targets, vis: v, allowed });
            self.input.frame(&self.server.core.cvars, self.cl.viewangles, ft, ctx.as_ref())
        };
        if self.cv("cl_alwaysrun") != 0.0 {
            inp.speed = !inp.speed;
        }
        self.pend_attack |= inp.attack;
        self.pend_jump |= inp.jump;
        if inp.impulse != 0 {
            self.pend_impulse = inp.impulse;
        }
        if self.cl.signon == 4 {
            let sp = self.move_speeds();
            self.last_cmd = self.cl.base_move(&InputState { attack: false, jump: false, impulse: 0, ..inp }, &sp, ft);
        }

        // server tic
        self.accum += dt;
        let maxfps = self.cv("host_maxfps");
        let tic = if maxfps > 0.0 && maxfps <= 72.0 { 0.0 } else { SERVER_TIC };
        if self.accum >= tic {
            let sdt = self.accum.min(0.1);
            self.accum = 0.0;
            if let Some(i) = self.local {
                let mut cmd = self.last_cmd;
                cmd.buttons = self.pend_attack as u8 | (self.pend_jump as u8) << 1;
                cmd.impulse = self.pend_impulse;
                if self.cl.build_move(&cmd).is_some() {
                    self.server.set_move(i, ClientMove {
                        angles: cmd.viewangles,
                        forward: cmd.forwardmove,
                        side: cmd.sidemove,
                        up: cmd.upmove,
                        buttons: cmd.buttons,
                        impulse: cmd.impulse,
                    });
                    self.pend_attack = false;
                    self.pend_jump = false;
                    self.pend_impulse = 0;
                }
            }
            self.forward_client_cmds();
            // Keep QC localcmd text for the shared host Cbuf (see exec_cbuf).
            let t = self.server.take_text();
            self.cbuf.add_text(&t);
            self.server.frame(sdt);
            if let Some(e) = self.server.error() {
                let msg = format!("Host_Error: {e}\n");
                self.con.print(&msg);
                self.server.level = None;
                self.server.core.sv.active = false;
            }
            self.drain_server_log();
            self.feed_client();
        }
        self.check_local_client();
        if self.cl.signon > 0 {
            self.cl.frame(dt);
        }
        self.process_client_events();
        self.con.update(ft);

        let (view, gun) = if self.cl.signon == 4 && !self.level.mapname.is_empty() {
            let cv = self.view_cvars();
            let (mut v, g) = self.view.calc_refdef(&self.cl, &cv, &self.level, ft);
            self.cl.viewent.lerpflags &= !qclient::LERP_RESETANIM;
            if self.classic_hud() && self.cl.intermission == 0 {
                let (_, h) = self.r.size();
                v.reserve_bottom = (CLASSIC_SBAR_LINES * crate::draw::Canvas::new(1, h).scale) as u32;
            }
            (Some(v), g)
        } else {
            (None, None)
        };
        self.update_sound(view.as_ref(), ft);
        if self.screenshot.is_some() {
            self.r.request_capture();
        }
        self.render(view, gun, ft);
        if let Some(name) = self.screenshot.take() {
            self.write_screenshot(&name);
        }
    }

    fn update_sound(&mut self, view: Option<&qrender::View>, ft: f32) {
        let vol = (self.cv("volume"), self.cv("bgmvolume"));
        let Some(snd) = &self.snd else { return };
        if vol != self.last_vol {
            snd.set_volume(vol.0);
            snd.set_bgm_volume(vol.1);
            self.last_vol = vol;
        }
        let mut l = Listener { view_entity: self.cl.viewentity as i32, frametime: ft, ..Default::default() };
        if let Some(v) = view {
            l.origin = v.origin;
            let (_, r, _) = angle_vectors(Vec3::from_array(v.angles));
            l.right = r.to_array();
            if let (Some(sl), Some(bsp)) = (&self.server.level, &self.level.bsp) {
                let _ = sl;
                let leaf = self.server.core.sv.vis.point_in_leaf(Vec3::from_array(v.origin));
                l.ambient_levels = bsp.leafs.get(leaf).map(|lf| lf.ambient_level);
            }
            l.underwater = self.contents(v.origin) <= crate::view::CONTENTS_WATER;
        }
        snd.set_water_fx(l.underwater);
        snd.update(l);
    }

    fn contents(&self, p: [f32; 3]) -> i32 {
        self.server.level.as_ref().map_or(-1, |l| l.world.point_contents(Vec3::from_array(p)))
    }

    fn render(&mut self, view: Option<qrender::View>, gun: Option<qrender::RenderEntity>, ft: f32) {
        let (w, h) = self.r.size();
        let mut c = Canvas::new(w, h);
        let mut ents = Vec::new();
        let mut parts = Vec::new();
        let mut dls = Vec::new();
        let mut blend = [0.0; 4];
        if let Some(v) = &view {
            let fs = &self.server.core.fs;
            let cv = self.view_cvars();
            self.view.entities(&mut self.cl, &cv, &mut self.level, fs, &mut self.r, &mut ents);
            if self.cv("r_drawentities") == 0.0 {
                ents.clear();
            }
            if let Some(g) = gun {
                ents.push(g);
            }
            crate::view::particles(&self.cl, &mut parts);
            crate::view::dlights(&self.cl, &mut dls);
            let contents = self.contents(v.origin);
            blend = self.view.blend(&self.cl, contents, ft);
        }
        // 2D
        let crosshair = self.cv("crosshair") != 0.0;
        let showfps = self.cv("scr_showfps") != 0.0;
        let classic = self.classic_hud();
        {
            let fs = &self.server.core.fs;
            let pics = &mut self.pics;
            let r = &mut self.r;
            let mut pf = |n: &str| pics.get(fs, r, n);
            if view.is_some() {
                match self.cl.intermission {
                    1 => crate::sbar::intermission(&mut c, &mut pf, &self.cl),
                    2 => {
                        crate::sbar::finale(&mut c, &mut pf);
                        let n = ((self.cl.time - self.cl.completed_time) * self.server.core.cvars.value_of("scr_printspeed") as f64) as usize;
                        self.con.draw_center(&mut c, n);
                    }
                    3 => self.con.draw_center(&mut c, usize::MAX),
                    _ => {
                        if crosshair {
                            let scene_h = c.vh() - if classic { CLASSIC_SBAR_LINES } else { 0.0 };
                            let (x, y) = ((c.vw() / 2.0).floor() - 4.0, (scene_h / 2.0).floor() - 4.0);
                            c.ch(x, y, b'+');
                        }
                        let face = self.view.face_anim_time > self.cl.time;
                        let pack = crate::sbar::Pack::from_game_dirs(self.server.core.fs.game_dirs().iter().map(String::as_str));
                        let mut sb = crate::sbar::Sbar::new(&mut c, &mut pf, pack);
                        if classic {
                            sb.backtile(CLASSIC_SBAR_LINES);
                            sb.draw(&self.cl, self.show_scores, face);
                        } else {
                            sb.draw_modern(&self.cl, self.show_scores, face);
                        }
                    }
                }
            }
            let conback = pf("gfx/conback.lmp");
            let ingame = view.is_some() && self.cl.intermission == 0;
            self.con.draw(&mut c, conback, ingame, ingame);
            if self.server.core.sv.paused && view.is_some() {
                let p = pf("gfx/pause.lmp");
                if let Some((_, pw, _)) = p {
                    c.pic(((c.vw() - pw as f32) / 2.0).floor(), 48.0, p);
                }
            }
            self.menu.draw(&mut c, &mut pf, &self.server.core.cvars, self.realtime);
            if showfps {
                self.fps.0 += ft as f64;
                self.fps.1 += 1;
                if self.fps.0 >= 0.5 {
                    self.fps.2 = (self.fps.1 as f64 / self.fps.0) as f32;
                    self.fps = (0.0, 0, self.fps.2);
                }
                let s = format!("{:4.0} fps", self.fps.2);
                c.reset_origin();
                let x = c.vw() - s.len() as f32 * 8.0 - 8.0;
                c.string(x, 8.0, &s);
            }
        }
        let styles: Vec<i32> = (0..qnet::protocol::MAX_LIGHTSTYLES).map(|i| self.cl.lightstyle_value(i)).collect();
        let fog = qrender::Fog { density: self.cl.fog.density, color: self.cl.fog.color };
        let v = |n: &str| self.cv(n);
        let settings = Settings {
            gamma: v("gamma").clamp(0.3, 3.0),
            contrast: v("contrast").clamp(0.5, 3.0),
            // GL_WaterAlphaForTextureType: map keys win; per-type values of
            // 0 fall back to r_wateralpha.
            wateralpha: self.level.liquid_alpha[0].unwrap_or(v("r_wateralpha")).clamp(0.0, 1.0),
            lavaalpha: self.level.liquid_alpha[1].or(Some(v("r_lavaalpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            slimealpha: self.level.liquid_alpha[2].or(Some(v("r_slimealpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            telealpha: self.level.liquid_alpha[3].or(Some(v("r_telealpha"))).filter(|&a| a > 0.0).map(|a| a.min(1.0)),
            skyfog: v("r_skyfog"),
            dynamic: v("r_dynamic") != 0.0,
            vsync: v("vid_vsync") != 0.0,
            msaa: v("r_msaa").max(1.0) as u32,
            render_scale: v("r_renderscale").clamp(0.25, 2.0),
            texture_linear: v("r_texfilter") != 0.0,
            anisotropy: v("r_anisotropy").clamp(1.0, 16.0),
            texture_mode: qrender::parse_texture_mode(self.cvs("gl_texturemode")),
            lod_bias: {
                let s = self.cvs("gl_lodbias");
                if s.eq_ignore_ascii_case("auto") { None } else { s.trim().parse().ok() }
            },
            supersample: v("vid_fsaamode") != 0.0,
            dither: v("r_dither").max(0.0),
            softemu: v("r_softemu").clamp(0.0, 3.0) as u32,
            softemu_metric: v("r_softemu_metric") as i32,
            lightmap_banding: v("r_softemu_lightmap_banding"),
            mdl_warp: v("r_softemu_mdl_warp"),
            dither_screen: v("r_softemu_dither_screen"),
            dither_texture: v("r_softemu_dither_texture"),
            ..Settings::default()
        };
        let frame = qrender::RenderFrame {
            time: self.cl.time,
            view,
            entities: &ents,
            particles: &parts,
            dlights: &dls,
            lightstyles: &styles,
            fog,
            blend,
            draw2d: &c.list,
            settings,
        };
        if let Err(e) = self.r.render(&frame) {
            self.con.print(&format!("render error: {e:?}\n"));
        }
    }

    fn write_screenshot(&mut self, name: &str) {
        let (w, h) = self.r.size();
        match self.r.read_pixels() {
            Ok(px) => {
                let png = crate::png::encode_rgba(w, h, &px);
                let path = if name.is_empty() {
                    let dir = self.user_dir();
                    (0..10000).map(|i| dir.join(format!("spasm{i:04}.png"))).find(|p| !p.exists()).unwrap_or(dir.join("spasm.png"))
                } else {
                    let p = PathBuf::from(name);
                    if p.extension().is_none() { p.with_extension("png") } else { p }
                };
                if let Some(d) = path.parent() {
                    let _ = std::fs::create_dir_all(d);
                }
                match std::fs::write(&path, png) {
                    Ok(()) => self.con.print(&format!("Wrote {}\n", path.display())),
                    Err(e) => self.con.print(&format!("screenshot: {e}\n")),
                }
            }
            Err(e) => self.con.print(&format!("screenshot failed: {e:?}\n")),
        }
    }

    /// Writes config.cfg (bindings + archived cvars) in the active gamedir.
    pub fn write_config(&mut self) {
        // Headless runs are tests/tools; never clobber the user's config.
        if self.headless {
            return;
        }
        let dir = self.user_dir();
        let mut s = String::from("// generated by quake-source-port, do not modify\n");
        for l in self.input.archive_bindings() {
            s += &l;
            s.push('\n');
        }
        for l in self.server.core.cvars.archive_lines() {
            s += &l;
            s.push('\n');
        }
        let _ = std::fs::create_dir_all(&dir);
        if let Err(e) = std::fs::write(dir.join("config.cfg"), s) {
            eprintln!("couldn't write config.cfg: {e}");
        }
    }

    /// Per-game user data dir (config, saves, screenshots); never the game data dir.
    pub fn user_dir(&self) -> PathBuf {
        crate::user_dir(self.fs_snapshot.1.last().map_or("id1", String::as_str))
    }

    fn savegame_path(&self, name: &str) -> PathBuf {
        let mut n = name.to_string();
        if !n.ends_with(".sav") {
            n += ".sav";
        }
        self.user_dir().join(n)
    }

    fn scan_saves(&mut self) {
        for i in 0..crate::menu::MAX_SAVEGAMES {
            let p = self.savegame_path(&format!("s{i}"));
            self.menu.saves[i] = std::fs::read_to_string(p).ok().and_then(|t| t.lines().nth(1).map(|l| l.replace('_', " ").trim_end().to_string()));
        }
    }
}

fn register_commands(c: &mut Commands<Host>) {
    c.add("exec", |h, a| {
        if a.argc() != 2 {
            h.print("exec <filename> : execute a script file\n");
            return;
        }
        let name = a.argv(1);
        // A mod without its own user config starts from the base game's
        // (nearest pack below it first, then id1); runs after any config.cfg
        // the mod ships in its data, and is saved as the mod's own on quit.
        if name.eq_ignore_ascii_case("config.cfg")
            && let Some((g, t)) = h.inherited_config()
        {
            h.cbuf.insert_text(&t);
            h.print(&format!("execing config.cfg (inherited from {g})\n"));
            if let Some(d) = h.fs().open(name).filter(|_| h.fs().origin(name).is_some_and(|o| o.game.eq_ignore_ascii_case(h.game_name()))) {
                h.cbuf.insert_text(&String::from_utf8_lossy(&d));
                h.print("execing config.cfg (mod defaults)\n");
            }
            return;
        }
        match h.fs().open(name) {
            Some(d) => {
                let t = String::from_utf8_lossy(&d).into_owned();
                h.cbuf.insert_text(&t);
                h.print(&format!("execing {name}\n"));
            }
            None if name.eq_ignore_ascii_case("default.cfg") => {
                h.cbuf.insert_text(DEFAULT_CFG);
                h.print("execing default.cfg (built-in)\n");
            }
            None => h.print(&format!("couldn't exec {name}\n")),
        }
    });
    c.add("stuffcmds", |h, _| h.stuff_cmds());
    c.add("cvarlist", |h, a| {
        let p = a.argv(1).to_ascii_lowercase();
        let mut l: Vec<String> = h
            .server
            .core
            .cvars
            .iter()
            .filter(|c| c.name.to_ascii_lowercase().starts_with(&p))
            .map(|c| format!("{}{} {} \"{}\"\n", if c.flags & CVAR_ARCHIVE != 0 { '*' } else { ' ' }, if c.flags & CVAR_SERVERINFO != 0 { 's' } else { ' ' }, c.name, c.string))
            .collect();
        l.sort_unstable_by_key(|s| s[3..].to_ascii_lowercase());
        let n = l.len();
        for s in l {
            h.print(&s);
        }
        h.print(&format!("{n} cvars\n"));
    });
    c.add("cmdlist", |h, a| {
        let mut l: Vec<String> = h.cmds.complete(a.argv(1)).into_iter().map(str::to_string).collect();
        l.extend(h.server.complete_command(a.argv(1)).into_iter().map(str::to_string));
        l.sort_unstable();
        for s in &l {
            h.print(&format!("   {s}\n"));
        }
        h.print(&format!("{} commands\n", l.len()));
    });
    c.add("quit", |h, _| h.quit = true);
    c.add("screenshot", |h, a| h.screenshot = Some(a.argv(1).to_string()));
    c.add("toggleconsole", |h, _| h.toggle_console());
    c.add("togglemenu", |h, _| {
        if h.menu.active() {
            h.menu.screen = Screen::None;
        } else {
            h.menu.open(Screen::Main);
        }
    });
    c.add("menu_main", |h, _| h.menu.open(Screen::Main));
    c.add("menu_quit", |h, _| h.menu.open(Screen::Quit));
    c.add("menu_options", |h, a| {
        use crate::menu::Page;
        let page = match a.argv(1).to_ascii_lowercase().as_str() {
            "gameplay" => Some(Page::Gameplay),
            "input" => Some(Page::Input),
            "graphics" => Some(Page::Graphics),
            "audio" => Some(Page::Audio),
            _ => None,
        };
        if page == Some(Page::Graphics) {
            h.vid_menu();
        }
        h.menu.open(page.map_or(Screen::Options, Screen::OptionsPage));
    });
    c.add("clear", |h, _| h.con.lines.clear());
    c.add("bf", |h, _| h.view.bonus_flash());
    c.add("+showscores", |h, _| h.show_scores = true);
    c.add("-showscores", |h, _| h.show_scores = false);
    for n in ["startdemos", "demos", "stopdemo", "playdemo", "sizeup", "sizedown", "cd", "help", "scr_autoscale", "togglezoom", "messagemode", "messagemode2", "menu_multiplayer"] {
        c.add(n, |_, _| {});
    }
    c.add("cfg_unbindall", |_, _| {});
    c.add("fog", cmd_fog);
    // Host_Name_f / Host_Color_f: set the archived cvar, then tell the server.
    c.add("name", |h, a| {
        if a.argc() < 2 {
            let n = h.server.core.cvars.by_name("_cl_name").map(|c| c.string.clone()).unwrap_or_default();
            h.print(&format!("\"name\" is \"{n}\"\n"));
            return;
        }
        let n = if a.argc() == 2 { a.argv(1).to_string() } else { a.args().to_string() };
        h.server.core.cvars.set("_cl_name", &n);
        h.send_userinfo(&format!("name \"{n}\""));
    });
    c.add("color", |h, a| {
        if a.argc() < 2 {
            let c = h.cv("_cl_color") as i32;
            h.print(&format!("\"color\" is \"{} {}\"\ncolor <0-13> [0-13]\n", c >> 4, c & 15));
            return;
        }
        let p = |i: usize| a.argv(i).parse::<f32>().unwrap_or(0.0) as i32 & 15;
        let top = p(1).min(13);
        let bottom = if a.argc() == 2 { top } else { p(2).min(13) };
        h.server.core.cvars.set("_cl_color", &(top * 16 + bottom).to_string());
        h.send_userinfo(&format!("color {top} {bottom}"));
    });
    c.add("vid_apply", |h, _| h.vid_apply());
    c.add("vid_restart", |h, _| h.vid_apply());
    c.add("_vid_menu", |h, _| h.vid_menu());
    c.add("force_centerview", |h, _| h.cl.viewangles[0] = 0.0);
    c.add("centerview", |h, _| h.cl.viewangles[0] = 0.0);
    c.add("menu_load", |h, _| {
        h.scan_saves();
        h.menu.open(Screen::Load);
    });
    c.add("menu_save", |h, _| {
        if h.server.active() {
            h.scan_saves();
            h.menu.open(Screen::Save);
        }
    });
    c.add("pause", |h, _| {
        if h.server.active() {
            let p = !h.server.core.sv.paused;
            h.server.core.sv.paused = p;
            h.cl.paused = p;
            h.print(if p { "paused\n" } else { "unpaused\n" });
        }
    });
    c.add("disconnect", |h, _| {
        if let Some(i) = h.local {
            h.server.drop_client(i, false);
        }
        h.server.level = None;
        h.server.core.sv.active = false;
        h.disconnect_client();
    });
    c.add("music", |h, a| {
        if let Some(s) = &mut h.snd
            && !s.play_music(a.argv(1), true)
        {
            h.con.print(&format!("Couldn't handle music file {}\n", a.argv(1)));
        }
    });
    c.add("music_stop", |h, _| {
        if let Some(s) = &h.snd {
            s.stop_music();
        }
    });
    c.add("delay", |h, a| {
        if a.argc() < 3 {
            h.print("delay <seconds> <command...> : run a command later\n");
            return;
        }
        let t = h.realtime + a.argv(1).parse::<f64>().unwrap_or(0.0);
        let cmd = a.all()[2..].iter().map(|s| if s.contains(' ') { format!("\"{s}\"") } else { s.clone() }).collect::<Vec<_>>().join(" ");
        h.delayed.push((t, cmd));
    });
    c.add("version", |h, _| h.print("quake-source-port 0.1.0\n"));
    c.add("_scansaves", |h, _| h.scan_saves());
    c.add("_mods", |h, _| h.scan_mods());
    c.add("_modmaps", |h, a| h.scan_mod_maps(a.argv(1)));
    // menu_mods [dir]: the mod list, or one mod's maps
    c.add("menu_mods", |h, a| {
        h.scan_mods();
        if a.argc() > 1 && h.menu.mods.select(a.argv(1)) {
            h.scan_mod_maps(a.argv(1));
            h.menu.open(Screen::ModMaps);
        } else {
            h.menu.open(Screen::Mods);
        }
    });
    c.add("game", |h, a| {
        if a.argc() < 2 {
            let g = h.game_name().to_string();
            h.print(&format!("\"game\" is \"{g}\"\ngame <dir> [map] : switch to another mod\n"));
            return;
        }
        let map = (a.argc() > 2).then(|| a.argv(2).to_string());
        h.switch_game(a.argv(1), map.as_deref());
    });
    c.add("save", |h, a| {
        if a.argc() != 2 {
            h.print("save <savename> : save a game\n");
            return;
        }
        if !h.server.active() || h.cl.intermission != 0 || h.cl.stats[STAT_HEALTH] <= 0 {
            h.print("Can't save now.\n");
            return;
        }
        let text = h.server.save_game();
        let p = h.savegame_path(a.argv(1));
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        match std::fs::write(&p, text) {
            Ok(()) => h.print(&format!("Saved {}\n", p.display())),
            Err(e) => h.print(&format!("ERROR: couldn't write {}: {e}\n", p.display())),
        }
    });
    c.add("load", |h, a| {
        if a.argc() != 2 {
            h.print("load <savename> : load a game\n");
            return;
        }
        let p = h.savegame_path(a.argv(1));
        let Ok(text) = std::fs::read_to_string(&p) else {
            h.print(&format!("ERROR: couldn't open {}\n", p.display()));
            return;
        };
        h.print(&format!("Loading game from {}...\n", p.display()));
        if let Some(i) = h.local.take() {
            h.server.drop_client(i, false);
        }
        h.disconnect_client();
        if let Err(e) = h.server.load_game(&text) {
            h.print(&format!("load failed: {e}\n"));
        }
        h.drain_server_log();
    });
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

#[cfg(test)]
mod relaunch_tests {
    #[test]
    fn strips_game_and_commands() {
        let v = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
        let out = super::relaunch_args(&v("-basedir /q -hipnotic -game foo +map e1m1 +skill 3 -width 800 -height 600"));
        assert_eq!(out, v("-basedir /q -width 800 -height 600"));
        assert_eq!(super::relaunch_args(&v("+delay 3 screenshot x -window")), v("-window"));
    }
}
