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
    ("r_dynamic", "1", 0),
    ("vid_vsync", "0", CVAR_ARCHIVE),
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
        for &(n, d, f) in HOST_CVARS {
            cvars.register(n, d, f);
        }
        let input = Input::new(&mut cvars);
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
        };
        qinput::register_commands(&mut h.cmds);
        register_commands(&mut h.cmds);
        h.con.forced = true;
        h.con.frac = 1.0;
        h.cbuf.add_text("exec quake.rc\n");
        h
    }

    pub fn fs(&self) -> &qcore::fs::Fs {
        &self.server.core.fs
    }

    fn cv(&self, name: &str) -> f32 {
        self.server.core.cvars.value_of(name)
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
                if let Some(w) = &self.window {
                    let (w, h) = w.drawable_size();
                    self.r.resize(w, h);
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
                c.center_320x200();
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
                    self.con.center_print(&s, t);
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
                ClientEvent::NewMap => self.new_map(),
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
        self.view.reset();
        self.con.print(&format!("loaded {} in {:.2}s\n", self.level.mapname, t0.elapsed().as_secs_f32()));
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
                        let mut sb = crate::sbar::Sbar::new(&mut c, &mut pf);
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
        let fog = if self.cl.fog.density > 0.0 {
            qrender::Fog { density: self.cl.fog.density, color: self.cl.fog.color }
        } else {
            self.level.world_fog
        };
        let v = |n: &str| self.cv(n);
        let settings = Settings {
            gamma: v("gamma").clamp(0.3, 3.0),
            contrast: v("contrast").clamp(0.5, 3.0),
            wateralpha: v("r_wateralpha").clamp(0.0, 1.0),
            skyfog: v("r_skyfog"),
            dynamic: v("r_dynamic") != 0.0,
            vsync: v("vid_vsync") != 0.0,
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
    c.add("menu_options", |h, _| h.menu.open(Screen::Options));
    c.add("clear", |h, _| h.con.lines.clear());
    c.add("bf", |h, _| h.view.bonus_flash());
    c.add("+showscores", |h, _| h.show_scores = true);
    c.add("-showscores", |h, _| h.show_scores = false);
    for n in ["startdemos", "demos", "stopdemo", "playdemo", "sizeup", "sizedown", "cd", "fog", "vid_restart", "help", "scr_autoscale", "togglezoom", "messagemode", "messagemode2", "menu_multiplayer"] {
        c.add(n, |_, _| {});
    }
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
