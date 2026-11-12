//! Basic menus (menu.c subset): main, single player, load/save, options,
//! quit, plus the mod menu entry (`crate::modmenu`).

use qcore::cvar::Cvars;
use qinput::keys::*;

use crate::draw::Canvas;
use crate::glyphs::{Family, Glyphs, is_pad_key};
use crate::modmenu::{self, ModMenu};
use crate::sbar::PicFn;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    None,
    Main,
    SinglePlayer,
    Load,
    Save,
    Options,
    OptionsPage(Page),
    Quit,
    Mods,
    ModMaps,
    Multiplayer,
}

/// Options sub-pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Gameplay,
    Controls,
    Input,
    Graphics,
    Audio,
    /// Multiplayer pages (reached from Screen::Multiplayer).
    Join,
    Host,
    Setup,
}

const PAGES: [(&str, Page); 5] = [
    ("Gameplay", Page::Gameplay),
    ("Controls", Page::Controls),
    ("Input", Page::Input),
    ("Graphics", Page::Graphics),
    ("Audio", Page::Audio),
];

/// One options row.
#[derive(Clone, Copy)]
enum Item {
    /// cvar slider; `invert` puts larger values on the left (gamma).
    Slider { label: &'static str, cvar: &'static str, min: f32, max: f32, step: f32, invert: bool },
    Toggle { label: &'static str, cvar: &'static str },
    /// Fixed list of (text, value).
    Choice { label: &'static str, cvar: &'static str, choices: &'static [(&'static str, f32)] },
    /// Pending display mode / resolution, committed by `Apply`.
    DisplayMode,
    Resolution,
    Apply,
    /// Editable text cvar.
    Text { label: &'static str, cvar: &'static str, max: usize },
    /// Shirt (`high`) or pants nibble of `_cl_color`.
    Color { label: &'static str, high: bool },
    /// Map for a hosted game (`_mp_map`), from the host's map list.
    Map,
    Run { label: &'static str, run: Run },
    /// A found LAN server (index into `Menu::servers`).
    Server(usize),
    /// Rebindable action: keyboard/mouse and gamepad keys bound to `cmd`.
    Bind { label: &'static str, cmd: &'static str },
    /// Restore the default bindings.
    ResetBinds,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Run {
    Connect,
    Search,
    Start,
}

/// A server answering the LAN search.
#[derive(Clone, Debug, PartialEq)]
pub struct ServerEntry {
    pub address: String,
    pub hostname: String,
    pub map: String,
    pub players: u8,
    pub max_players: u8,
}

const JOIN: &[Item] = &[
    Item::Text { label: "Address", cvar: "_mp_address", max: 40 },
    Item::Run { label: "Connect", run: Run::Connect },
    Item::Run { label: "Search LAN", run: Run::Search },
];

const fn bind(label: &'static str, cmd: &'static str) -> Item {
    Item::Bind { label, cmd }
}

const CONTROLS: &[Item] = &[
    bind("Attack", "+attack"),
    bind("Jump / Swim Up", "+jump"),
    bind("Weapon Wheel", "+weaponwheel"),
    bind("Next Weapon", "impulse 10"),
    bind("Previous Weapon", "impulse 12"),
    bind("Move Forward", "+forward"),
    bind("Move Back", "+back"),
    bind("Strafe Left", "+moveleft"),
    bind("Strafe Right", "+moveright"),
    bind("Swim Down", "+movedown"),
    bind("Run", "+speed"),
    bind("Turn Left", "+left"),
    bind("Turn Right", "+right"),
    bind("Look Up", "+lookup"),
    bind("Look Down", "+lookdown"),
    bind("Center View", "centerview"),
    bind("Scoreboard", "+showscores"),
    bind("Quick Save", "echo Quicksaving...; wait; save quick"),
    bind("Quick Load", "echo Quickloading...; wait; load quick"),
    Item::ResetBinds,
];

const PLAYERS: &[(&str, f32)] = &[("2", 2.0), ("3", 3.0), ("4", 4.0), ("6", 6.0), ("8", 8.0), ("12", 12.0), ("16", 16.0)];
const GAMETYPE: &[(&str, f32)] = &[("deathmatch", 0.0), ("cooperative", 1.0)];
const TEAMPLAY: &[(&str, f32)] = &[("off", 0.0), ("no team damage", 1.0), ("team damage", 2.0)];
const SKILL: &[(&str, f32)] = &[("easy", 0.0), ("normal", 1.0), ("hard", 2.0), ("nightmare", 3.0)];
const FRAGS: &[(&str, f32)] =
    &[("none", 0.0), ("10", 10.0), ("20", 20.0), ("30", 30.0), ("50", 50.0), ("75", 75.0), ("100", 100.0)];
const MINUTES: &[(&str, f32)] =
    &[("none", 0.0), ("5", 5.0), ("10", 10.0), ("15", 15.0), ("20", 20.0), ("30", 30.0), ("45", 45.0), ("60", 60.0)];

const HOST: &[Item] = &[
    Item::Choice { label: "Max Players", cvar: "_mp_maxplayers", choices: PLAYERS },
    Item::Choice { label: "Game Type", cvar: "_mp_coop", choices: GAMETYPE },
    Item::Choice { label: "Teamplay", cvar: "_mp_teamplay", choices: TEAMPLAY },
    Item::Choice { label: "Skill", cvar: "_mp_skill", choices: SKILL },
    Item::Choice { label: "Frag Limit", cvar: "_mp_fraglimit", choices: FRAGS },
    Item::Choice { label: "Time Limit", cvar: "_mp_timelimit", choices: MINUTES },
    Item::Map,
    Item::Run { label: "Start Game", run: Run::Start },
];

const SETUP: &[Item] = &[
    Item::Text { label: "Name", cvar: "_cl_name", max: 15 },
    Item::Color { label: "Shirt Color", high: true },
    Item::Color { label: "Pants Color", high: false },
];

const fn slider(label: &'static str, cvar: &'static str, min: f32, max: f32, step: f32) -> Item {
    Item::Slider { label, cvar, min, max, step, invert: false }
}
const fn toggle(label: &'static str, cvar: &'static str) -> Item {
    Item::Toggle { label, cvar }
}

const GAMEPLAY: &[Item] = &[
    slider("Field of View", "fov", 60.0, 130.0, 5.0),
    toggle("Always Run", "cl_alwaysrun"),
    toggle("Crosshair", "crosshair"),
    slider("View Bob", "cl_bob", 0.0, 0.02, 0.002),
    toggle("Gun Kick", "v_gunkick"),
    toggle("Show Weapon", "r_drawviewmodel"),
    toggle("Modern HUD", "scr_hudstyle"),
    toggle("Show FPS", "scr_showfps"),
    toggle("Level Select", "ui_levelselect"),
    slider("Autosaves to Keep (0 = off)", "sv_autosave_keep", 0.0, 10.0, 1.0),
    toggle("Save on Level Entry", "sv_autosave_entry"),
    Item::Choice { label: "Autosave Timer", cvar: "sv_autosave_interval", choices: MINUTES },
    toggle("Save on Secret Found", "sv_autosave_secret"),
    toggle("Save on All Enemies Killed", "sv_autosave_monsters"),
    toggle("Save on All Secrets Found", "sv_autosave_secrets"),
];

const INPUT: &[Item] = &[
    slider("Mouse Speed", "sensitivity", 1.0, 11.0, 0.5),
    toggle("Invert Mouse", "m_invert"),
    slider("Stick Look Yaw", "joy_sensitivity_yaw", 60.0, 480.0, 20.0),
    slider("Stick Look Pitch", "joy_sensitivity_pitch", 40.0, 300.0, 10.0),
    toggle("Invert Stick", "joy_invert"),
    toggle("Aim Assist", "aim_assist"),
    slider("Aim Slowdown", "aim_slowdown", 0.0, 1.0, 0.1),
    slider("Aim Rotation", "aim_rotation", 0.0, 1.0, 0.1),
    toggle("Gyro", "gyro_enable"),
    slider("Rumble", "joy_rumble", 0.0, 1.0, 0.25),
    Item::Choice { label: "Button Glyphs", cvar: "joy_glyphs", choices: GLYPHS },
    Item::Choice { label: "Wheel Slow Motion", cvar: "cl_wheel_timescale", choices: WHEEL_TIME },
];

const GLYPHS: &[(&str, f32)] = &[("auto", 0.0), ("xbox", 1.0), ("playstation", 2.0), ("nintendo", 3.0)];
const WHEEL_TIME: &[(&str, f32)] = &[("off", 1.0), ("50%", 0.5), ("25%", 0.25), ("10%", 0.1)];

const MSAA: &[(&str, f32)] = &[("off", 1.0), ("2x", 2.0), ("4x", 4.0), ("8x", 8.0)];
const SCALE: &[(&str, f32)] = &[("50%", 0.5), ("75%", 0.75), ("100%", 1.0), ("125%", 1.25), ("150%", 1.5), ("200%", 2.0)];
const FILTER: &[(&str, f32)] = &[("nearest", 0.0), ("linear", 1.0)];
const ANISO: &[(&str, f32)] = &[("off", 1.0), ("2x", 2.0), ("4x", 4.0), ("8x", 8.0), ("16x", 16.0)];
const SOFTEMU: &[(&str, f32)] = &[("off", 0.0), ("fine", 1.0), ("coarse", 2.0), ("banded", 3.0)];
const MODES: [&str; 3] = ["windowed", "fullscreen", "borderless"];

const GRAPHICS: &[Item] = &[
    Item::DisplayMode,
    Item::Resolution,
    Item::Apply,
    toggle("VSync", "vid_vsync"),
    slider("Max FPS", "host_maxfps", 60.0, 500.0, 10.0),
    Item::Choice { label: "Anti-aliasing", cvar: "r_msaa", choices: MSAA },
    toggle("Supersampling", "vid_fsaamode"),
    Item::Choice { label: "Render Scale", cvar: "r_renderscale", choices: SCALE },
    Item::Choice { label: "Texture Filter", cvar: "r_texfilter", choices: FILTER },
    Item::Choice { label: "Anisotropic", cvar: "r_anisotropy", choices: ANISO },
    Item::Choice { label: "8-bit Color", cvar: "r_softemu", choices: SOFTEMU },
    toggle("Dithering", "r_dither"),
    Item::Slider { label: "Brightness", cvar: "gamma", min: 0.5, max: 1.5, step: 0.05, invert: true },
    slider("Contrast", "contrast", 1.0, 2.0, 0.1),
    slider("Water Opacity", "r_wateralpha", 0.0, 1.0, 0.1),
    toggle("Dynamic Lights", "r_dynamic"),
    toggle("Interpolate Anim", "r_lerpmodels"),
    toggle("Interpolate Move", "r_lerpmove"),
];

const AUDIO: &[Item] = &[slider("Sound Volume", "volume", 0.0, 1.0, 0.1), slider("Music Volume", "bgmvolume", 0.0, 1.0, 0.1)];

fn page_items(p: Page) -> &'static [Item] {
    match p {
        Page::Gameplay => GAMEPLAY,
        Page::Controls => CONTROLS,
        Page::Input => INPUT,
        Page::Graphics => GRAPHICS,
        Page::Audio => AUDIO,
        Page::Join => JOIN,
        Page::Host => HOST,
        Page::Setup => SETUP,
    }
}

fn page_name(p: Page) -> &'static str {
    match p {
        Page::Join => "Join Game",
        Page::Host => "Host Game",
        Page::Setup => "Player Setup",
        _ => PAGES.iter().find(|x| x.1 == p).map_or("", |x| x.0),
    }
}

fn is_net_page(p: Page) -> bool {
    matches!(p, Page::Join | Page::Host | Page::Setup)
}

/// Every cvar the options pages edit (the host marks them archived).
pub fn option_cvars() -> impl Iterator<Item = &'static str> {
    [GAMEPLAY, INPUT, GRAPHICS, AUDIO].into_iter().flatten().filter_map(|i| match *i {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } => Some(cvar),
        _ => None,
    })
}

pub enum Action {
    Command(String),
    Sound(&'static str),
    Close,
}

pub const MAX_SAVEGAMES: usize = 12;

pub struct Menu {
    pub screen: Screen,
    main_cursor: usize,
    sp_cursor: usize,
    load_cursor: usize,
    opt_cursor: usize,
    page_cursor: usize,
    /// Quit prompt button: 0 = yes, 1 = no.
    quit_cursor: usize,
    /// Display settings being edited on the Graphics page (mode 0..=2, size).
    pub pending_mode: usize,
    pub pending_res: (u32, u32),
    /// Available resolutions (filled by the host) and device MSAA limit.
    pub resolutions: Vec<(u32, u32)>,
    pub max_msaa: u32,
    /// Parsed preview of each save slot (`None` = unused or unreadable).
    pub saves: Vec<Option<crate::saveinfo::SaveInfo>>,
    pub autosaves: bool,
    pub save_inventory: crate::save_inventory::Inventory,
    prev: Screen,
    /// Cursor position in 640x360 menu units, when the mouse has moved.
    mouse: Option<(f32, f32)>,
    /// Open dropdown on an options page.
    drop: Option<Drop>,
    pub mods: ModMenu,
    mp_cursor: usize,
    /// Maps offered on the Host Game page (filled by the host).
    pub net_maps: Vec<qui::mods::MapInfo>,
    /// LAN search results and whether a search is running.
    pub servers: Vec<ServerEntry>,
    pub searching: bool,
    /// Key bindings snapshot (host refreshes it while the menu is open).
    pub bindings: Vec<Option<String>>,
    /// Controls row waiting for a key to bind.
    capture: Option<usize>,
    /// Prompts show gamepad glyphs (the pad was used last).
    pub pad_ui: bool,
    pub glyphs: Glyphs,
    /// On-screen keyboard over a text field (gamepad typing).
    osk: Option<Osk>,
}

/// On-screen keyboard cursor: `row` 0..=4 (4 = action row), `col` 0..10.
#[derive(Clone, Copy, Debug, Default)]
struct Osk {
    row: usize,
    col: usize,
    shift: bool,
}

const OSK_ROWS: [&str; 4] = ["1234567890", "qwertyuiop", "asdfghjkl.", "zxcvbnm-_:"];
const OSK_ACTIONS: [&str; 4] = ["Shift", "Space", "Del", "Done"];
const OSK_KEY: (f32, f32) = (20.0, 14.0);
const OSK_Y: f32 = 240.0;

impl Osk {
    /// Action under the cursor on the bottom row.
    fn action(&self) -> usize {
        self.col * OSK_ACTIONS.len() / 10
    }
}

/// Keys bound to exactly `cmd` (whitespace/case-insensitive).
pub fn keys_for(bindings: &[Option<String>], cmd: &str) -> Vec<i32> {
    bindings
        .iter()
        .enumerate()
        .filter(|(_, b)| b.as_deref().is_some_and(|b| b.trim().eq_ignore_ascii_case(cmd)))
        .map(|(k, _)| k as i32)
        .collect()
}

fn bind_cmd(key: i32, cmd: &str) -> String {
    let name = keynum_to_string(key);
    format!("bind \"{name}\" \"{cmd}\"\n")
}

/// Open dropdown list: the page row it belongs to, highlighted entry and
/// first visible entry.
#[derive(Clone, Copy, Debug)]
struct Drop {
    item: usize,
    cursor: usize,
    scroll: usize,
}

/// Menus are laid out in a 640x360 (16:9) virtual area. The classic
/// 320x200 screens (main, single player, load/save, quit) sit centred in it.
const MENU_H: f32 = 360.0;
const CLASSIC: (f32, f32) = (160.0, 80.0);

/// Options panel: background box, label column (right-aligned), value column.
const PANEL_X: f32 = 136.0;
const PANEL_W: f32 = 368.0;
const LABEL_R: f32 = 304.0;
const VALUE_X: f32 = 320.0;
const ROW_H: f32 = 10.0;
/// First row of the options hub / of an options page (below its heading).
const ROW0: f32 = 64.0;
const PAGE_ROW0: f32 = 72.0;
/// Slider track in options, menu x units.
const SLIDER_X: (f32, f32) = (VALUE_X + 8.0, VALUE_X + 88.0);
/// Dropdown value box minimum width and visible entries.
const DROP_W: f32 = 120.0;
const DROP_ROWS: usize = 10;
/// Controls page: gamepad column.
const BIND_PAD_X: f32 = 424.0;

impl Menu {
    pub fn new() -> Self {
        Self {
            screen: Screen::None,
            main_cursor: 0,
            sp_cursor: 0,
            load_cursor: 0,
            opt_cursor: 0,
            page_cursor: 0,
            quit_cursor: 0,
            pending_mode: 0,
            pending_res: (1280, 720),
            resolutions: Vec::new(),
            max_msaa: 8,
            saves: vec![None; MAX_SAVEGAMES + 2],
            autosaves: false,
            save_inventory: Default::default(),
            prev: Screen::None,
            mouse: None,
            drop: None,
            mods: ModMenu::default(),
            mp_cursor: 0,
            net_maps: Vec::new(),
            servers: Vec::new(),
            searching: false,
            bindings: vec![None; MAX_KEYS],
            capture: None,
            pad_ui: false,
            glyphs: Glyphs { atlas: None, family: Family::Xbox },
            osk: None,
        }
    }

    pub fn set_mods(&mut self, mods: Vec<qui::mods::ModInfo>, current: &str) {
        self.mods.set_mods(mods, current);
    }

    pub fn set_mod_maps(&mut self, maps: Vec<qui::mods::MapInfo>, stats: qui::stats::Stats) {
        self.mods.set_maps(maps, stats);
    }

    /// `menu_multiplayer [join|host|setup]`.
    pub fn open_multiplayer(&mut self, page: &str) -> Option<Action> {
        let p = match page {
            "join" => Page::Join,
            "host" => Page::Host,
            "setup" => Page::Setup,
            _ => {
                self.open(Screen::Multiplayer);
                return None;
            }
        };
        self.open(Screen::OptionsPage(p));
        self.opt_cursor = 0;
        (p == Page::Host).then(|| Action::Command("_netmaps\n".into()))
    }

    /// Rows on a page: its static items, plus found servers on Join.
    fn page_len(&self, p: Page) -> usize {
        page_items(p).len() + if p == Page::Join { self.servers.len() } else { 0 }
    }

    fn page_item(&self, p: Page, i: usize) -> Item {
        let items = page_items(p);
        items.get(i).copied().unwrap_or(Item::Server(i.saturating_sub(items.len())))
    }

    /// Typed text goes to a text field under the cursor.
    pub fn text(&mut self, ch: char, cv: &mut Cvars) {
        let Screen::OptionsPage(p) = self.screen else { return };
        if self.drop.is_some() || !(' '..='~').contains(&ch) || ch == '"' || ch == ';' {
            return;
        }
        if let Item::Text { cvar, max, .. } = self.page_item(p, self.opt_cursor) {
            let mut v = cv.by_name(cvar).map_or(String::new(), |c| c.string.clone());
            if v.len() < max {
                v.push(ch);
                cv.set(cvar, &v);
            }
        }
    }

    /// Whether the on-screen keyboard is open.
    #[cfg(test)]
    fn osk_open(&self) -> bool {
        self.osk.is_some()
    }

    fn osk_key(&mut self, key: i32, mut o: Osk, cv: &mut Cvars, out: &mut Vec<Action>) {
        let Screen::OptionsPage(p) = self.screen else {
            self.osk = None;
            return;
        };
        let Item::Text { cvar, .. } = self.page_item(p, self.opt_cursor) else {
            self.osk = None;
            return;
        };
        let backspace = |cv: &mut Cvars| {
            let mut v = cv.by_name(cvar).map_or(String::new(), |c| c.string.clone());
            v.pop();
            cv.set(cvar, &v);
        };
        let n = OSK_ACTIONS.len();
        match key {
            K_UPARROW | K_DPAD_UP => o.row = (o.row + 4) % 5,
            K_DOWNARROW | K_DPAD_DOWN => o.row = (o.row + 1) % 5,
            K_LEFTARROW | K_DPAD_LEFT if o.row == 4 => o.col = ((o.action() + n - 1) % n * 10).div_ceil(n),
            K_RIGHTARROW | K_DPAD_RIGHT if o.row == 4 => o.col = ((o.action() + 1) % n * 10).div_ceil(n),
            K_LEFTARROW | K_DPAD_LEFT => o.col = (o.col + 9) % 10,
            K_RIGHTARROW | K_DPAD_RIGHT => o.col = (o.col + 1) % 10,
            K_XBUTTON | K_BACKSPACE => backspace(cv),
            K_YBUTTON => o.shift = !o.shift,
            K_BBUTTON | K_ESCAPE | K_START => {
                self.osk = None;
                out.push(Action::Sound("misc/menu3.wav"));
                return;
            }
            K_ABUTTON | K_ENTER | K_KP_ENTER => {
                if o.row < 4 {
                    let ch = OSK_ROWS[o.row].as_bytes()[o.col] as char;
                    self.text(if o.shift { ch.to_ascii_uppercase() } else { ch }, cv);
                } else {
                    match o.action() {
                        0 => o.shift = !o.shift,
                        1 => self.text(' ', cv),
                        2 => backspace(cv),
                        _ => {
                            self.osk = None;
                            out.push(Action::Sound("misc/menu2.wav"));
                            return;
                        }
                    }
                }
            }
            _ => return,
        }
        out.push(Action::Sound("misc/menu1.wav"));
        self.osk = Some(o);
    }

    fn draw_osk(&self, c: &mut Canvas, o: Osk) {
        let (kw, kh) = OSK_KEY;
        let x0 = 320.0 - kw * 5.0;
        c.fill(x0 - 6.0, OSK_Y - 6.0, kw * 10.0 + 12.0, kh * 5.0 + 12.0, [0.05, 0.04, 0.03, 0.95]);
        c.frame(x0 - 6.0, OSK_Y - 6.0, kw * 10.0 + 12.0, kh * 5.0 + 12.0, BORDER);
        for (r, row) in OSK_ROWS.iter().enumerate() {
            for (k, ch) in row.bytes().enumerate() {
                let (x, y) = (x0 + k as f32 * kw, OSK_Y + r as f32 * kh);
                if o.row == r && o.col == k {
                    c.fill(x, y, kw, kh, HILITE);
                }
                c.frame(x + 1.0, y + 1.0, kw - 2.0, kh - 2.0, [0.35, 0.27, 0.16, 0.9]);
                let ch = if o.shift { ch.to_ascii_uppercase() } else { ch };
                c.ch(x + (kw - 8.0) / 2.0, y + (kh - 8.0) / 2.0, ch);
            }
        }
        let aw = kw * 10.0 / OSK_ACTIONS.len() as f32;
        let y = OSK_Y + 4.0 * kh;
        for (i, label) in OSK_ACTIONS.iter().enumerate() {
            let x = x0 + i as f32 * aw;
            if o.row == 4 && o.action() == i {
                c.fill(x, y, aw, kh, HILITE);
            }
            c.frame(x + 1.0, y + 1.0, aw - 2.0, kh - 2.0, [0.35, 0.27, 0.16, 0.9]);
            let lx = x + ((aw - label.len() as f32 * 8.0) / 2.0).floor();
            if i == 0 && o.shift {
                gold(c, lx, y + 3.0, label);
            } else {
                c.string(lx, y + 3.0, label);
            }
        }
    }

    /// Commands that apply Player Setup (name and colours).
    fn setup_commands(cv: &Cvars) -> String {
        let name = cv.by_name("_cl_name").map_or("player", |c| c.string.as_str());
        let c = cv.value_of("_cl_color") as i32;
        format!("name \"{name}\"\ncolor {} {}\n", c >> 4, c & 15)
    }

    /// `_mp_map`, or a sensible default from the list (a dm map if any).
    fn host_map(&self, cv: &Cvars) -> Option<String> {
        let cur = cv.by_name("_mp_map").map_or("", |c| c.string.as_str());
        if self.net_maps.iter().any(|m| m.name == cur) {
            return Some(cur.to_string());
        }
        let m = self.net_maps.iter().find(|m| m.name.starts_with("dm")).or(self.net_maps.first())?;
        Some(m.name.clone())
    }

    fn run(&mut self, run: Run, cv: &mut Cvars, out: &mut Vec<Action>) {
        match run {
            Run::Connect => {
                let a = cv.by_name("_mp_address").map_or(String::new(), |c| c.string.trim().to_string());
                if a.is_empty() {
                    return;
                }
                self.screen = Screen::None;
                out.push(Action::Close);
                out.push(Action::Command(format!("connect {a}\n")));
            }
            Run::Search => {
                if !self.searching {
                    out.push(Action::Command("slist\n".into()));
                }
            }
            Run::Start => {
                let Some(map) = self.host_map(cv) else { return };
                let n = |c: &str| cv.value_of(c) as i32;
                let coop = n("_mp_coop");
                let cmd = format!(
                    "disconnect\nmaxplayers {}\ncoop {coop}\ndeathmatch {}\nteamplay {}\nskill {}\nfraglimit {}\ntimelimit {}\nmap {map}\n",
                    n("_mp_maxplayers").max(2),
                    (coop == 0) as i32,
                    n("_mp_teamplay"),
                    n("_mp_skill"),
                    n("_mp_fraglimit"),
                    n("_mp_timelimit"),
                );
                cv.set("_mp_map", &map);
                self.screen = Screen::None;
                out.push(Action::Close);
                out.push(Action::Command(cmd));
            }
        }
        out.push(Action::Sound("misc/menu2.wav"));
    }

    /// Waiting for a key to bind (the host keeps sticks from moving the cursor).
    pub fn capturing(&self) -> bool {
        self.capture.is_some() && self.screen == Screen::OptionsPage(Page::Controls)
    }

    pub fn active(&self) -> bool {
        self.screen != Screen::None
    }

    pub fn open(&mut self, s: Screen) {
        self.prev = self.screen;
        self.screen = s;
        self.drop = None;
        self.capture = None;
        self.osk = None;
        self.quit_cursor = 0;
        if matches!(s, Screen::Load | Screen::Save) { self.load_cursor = 0; }
    }

    fn save_rows(&self) -> usize {
        if self.screen == Screen::Load {
            if self.autosaves { self.saves.iter().take(crate::autosave::MAX).rposition(Option::is_some).map_or(1, |i| i + 1) } else { MAX_SAVEGAMES + 2 }
        } else { MAX_SAVEGAMES }
    }

    pub fn save_slot_name(&self, i: usize) -> Option<String> {
        if self.screen == Screen::Save { Some(format!("s{i}")) }
        else if self.autosaves { Some(format!("autosave{i}")) }
        else { match i { 0 => Some("autosave0".into()), 1 => None, _ => Some(format!("s{}", i - 2)) } }
    }

    /// Item under the menu-space point for the current screen.
    fn item_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let row = |top: f32, h: f32, n: usize, x0: f32, x1: f32| {
            (x >= x0 && x < x1 && y >= top && y < top + h * n as f32).then(|| ((y - top) / h) as usize)
        };
        let (cx, cy) = CLASSIC;
        match self.screen {
            Screen::Main => row(cy + 32.0, 20.0, MAIN_ITEMS, cx + 54.0, cx + 312.0),
            Screen::SinglePlayer => row(cy + 32.0, 20.0, 3, cx + 54.0, cx + 312.0),
            Screen::Load | Screen::Save => {
                let col = ((x - 24.0) / 300.0).floor() as usize;
                let row = ((y - 56.0) / 128.0).floor() as usize;
                let i = self.load_cursor / 4 * 4 + row * 2 + col;
                ((24.0..616.0).contains(&x) && (56.0..312.0).contains(&y) && col < 2 && row < 2 && i < self.save_rows()).then_some(i)
            },
            Screen::Options => row(ROW0, ROW_H, PAGES.len(), PANEL_X, PANEL_X + PANEL_W),
            Screen::OptionsPage(p) => row(PAGE_ROW0, ROW_H, self.page_len(p), PANEL_X, PANEL_X + PANEL_W),
            Screen::Multiplayer => row(cy + 32.0, 20.0, 3, cx + 54.0, cx + 312.0),
            Screen::Quit => QUIT_BUTTONS
                .iter()
                .position(|&bx| x >= bx && x < bx + QUIT_BTN.0 && y >= QUIT_BTN_Y && y < QUIT_BTN_Y + QUIT_BTN.1),
            Screen::Mods => self.mods.item_at(modmenu::Page::List, (x, y)),
            Screen::ModMaps => self.mods.item_at(modmenu::Page::Maps, (x, y)),
            Screen::None => None,
        }
    }

    fn cursor_mut(&mut self) -> Option<&mut usize> {
        match self.screen {
            Screen::Main => Some(&mut self.main_cursor),
            Screen::SinglePlayer => Some(&mut self.sp_cursor),
            Screen::Multiplayer => Some(&mut self.mp_cursor),
            Screen::Load | Screen::Save => Some(&mut self.load_cursor),
            Screen::Options => Some(&mut self.page_cursor),
            Screen::OptionsPage(_) => Some(&mut self.opt_cursor),
            Screen::Quit => Some(&mut self.quit_cursor),
            Screen::Mods => Some(self.mods.cursor_mut(modmenu::Page::List)),
            Screen::ModMaps => Some(self.mods.cursor_mut(modmenu::Page::Maps)),
            Screen::None => None,
        }
    }

    /// Mouse hover: moves the cursor onto the item under the pointer.
    /// Returns a sound to play when the selection changed.
    pub fn mouse_move(&mut self, pos: (f32, f32)) -> Option<&'static str> {
        self.mouse = Some(pos);
        if let Some(d) = self.drop {
            let i = self.drop_at(pos, d)?;
            let dc = &mut self.drop.as_mut()?.cursor;
            return (*dc != i).then(|| {
                *dc = i;
                "misc/menu1.wav"
            });
        }
        let i = self.item_at(pos)?;
        let c = self.cursor_mut()?;
        (*c != i).then(|| {
            *c = i;
            "misc/menu1.wav"
        })
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        let mut out = Vec::new();
        if let Some(row) = self.capture.take() {
            if self.screen != Screen::OptionsPage(Page::Controls) || key == K_ESCAPE {
                out.push(Action::Sound("misc/menu3.wav"));
                return out;
            }
            if let Item::Bind { cmd, .. } = page_items(Page::Controls)[row] {
                // two keys per device kind: a third replaces both
                let same: Vec<i32> = keys_for(&self.bindings, cmd).into_iter().filter(|&k| is_pad_key(k) == is_pad_key(key)).collect();
                if same.len() >= 2 && !same.contains(&key) {
                    for k in same {
                        out.push(Action::Command(format!("unbind \"{}\"\n", keynum_to_string(k))));
                        self.bindings[k as usize] = None;
                    }
                }
                out.push(Action::Command(bind_cmd(key, cmd)));
                if let Some(b) = self.bindings.get_mut(key as usize) {
                    *b = Some(cmd.to_string());
                }
                out.push(Action::Sound("misc/menu1.wav"));
            }
            return out;
        }
        if let Some(o) = self.osk {
            self.osk_key(key, o, cv, &mut out);
            return out;
        }
        if let (Some(d), Screen::OptionsPage(p)) = (self.drop, self.screen) {
            let item = self.page_item(p, d.item);
            self.drop_key(key, d, item, cv, &mut out);
            return out;
        }
        // left click acts on the item under the pointer, never on a stale cursor
        if key == K_MOUSE1 {
            let Some(pos) = self.mouse else { return out };
            let Some(i) = self.item_at(pos) else { return out };
            if let Some(c) = self.cursor_mut() {
                *c = i;
            }
            if let Screen::OptionsPage(p) = self.screen
                && let Item::Slider { cvar, min, max, step, invert, .. } = self.page_item(p, i)
                && pos.0 >= SLIDER_X.0 - 8.0
                && pos.0 < SLIDER_X.1 + 8.0
            {
                let mut f = ((pos.0 - SLIDER_X.0) / (SLIDER_X.1 - SLIDER_X.0 - 8.0)).clamp(0.0, 1.0);
                if invert {
                    f = 1.0 - f;
                }
                let v = min + ((f * (max - min)) / step).round() * step;
                set_num(cv, cvar, v.clamp(min, max));
                out.push(Action::Sound("misc/menu3.wav"));
                return out;
            }
        }
        let key = match key {
            K_MWHEELUP => K_UPARROW,
            K_MWHEELDOWN => K_DOWNARROW,
            k => k,
        };
        let up = matches!(key, K_UPARROW | K_DPAD_UP);
        let down = matches!(key, K_DOWNARROW | K_DPAD_DOWN);
        let left = matches!(key, K_LEFTARROW | K_DPAD_LEFT);
        let right = matches!(key, K_RIGHTARROW | K_DPAD_RIGHT);
        let enter = matches!(key, K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1);
        let back = matches!(key, K_ESCAPE | K_BBUTTON | K_MOUSE2);
        let wrap = |c: &mut usize, n: usize, out: &mut Vec<Action>| {
            if up {
                *c = (*c + n - 1) % n;
                out.push(Action::Sound("misc/menu1.wav"));
            } else if down {
                *c = (*c + 1) % n;
                out.push(Action::Sound("misc/menu1.wav"));
            }
        };
        match self.screen {
            Screen::None => {}
            Screen::Main => {
                if back {
                    self.screen = Screen::None;
                    out.push(Action::Close);
                    out.push(Action::Sound("misc/menu3.wav"));
                    return out;
                }
                wrap(&mut self.main_cursor, MAIN_ITEMS, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.main_cursor {
                        0 => self.screen = Screen::SinglePlayer,
                        1 => self.screen = Screen::Multiplayer,
                        2 => self.screen = Screen::Options,
                        4 => self.open(Screen::Quit),
                        5 => {
                            out.push(Action::Command("_mods\n".into()));
                            self.screen = Screen::Mods;
                        }
                        _ => {}
                    }
                }
            }
            Screen::SinglePlayer => {
                if back {
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.sp_cursor, 3, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.sp_cursor {
                        0 => {
                            self.screen = Screen::None;
                            out.push(Action::Close);
                            out.push(Action::Command("maxplayers 1\ndeathmatch 0\ncoop 0\nmap start\n".into()));
                        }
                        1 => {
                            out.push(Action::Command("_scansaves\n".into()));
                            self.autosaves = false;
                            self.open(Screen::Load);
                        }
                        _ => {
                            if server_active {
                                self.autosaves = false;
                                out.push(Action::Command("_scansaves\n".into()));
                                self.open(Screen::Save);
                            }
                        }
                    }
                }
            }
            Screen::Multiplayer => {
                if back {
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.mp_cursor, 3, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    let p = [Page::Join, Page::Host, Page::Setup][self.mp_cursor];
                    if p == Page::Host {
                        out.push(Action::Command("_netmaps\n".into()));
                    }
                    self.opt_cursor = 0;
                    self.screen = Screen::OptionsPage(p);
                }
            }
            Screen::Load | Screen::Save => {
                if back {
                    if self.autosaves && self.screen == Screen::Load {
                        self.autosaves = false;
                        self.load_cursor = 1;
                        out.push(Action::Command("_scansaves\n".into()));
                    } else {
                        self.screen = Screen::SinglePlayer;
                    }
                    return out;
                }
                let count = self.save_rows();
                self.load_cursor = self.load_cursor.min(count - 1);
                if up { self.load_cursor = (self.load_cursor + count - (2 % count)) % count; }
                if down { self.load_cursor = (self.load_cursor + 2) % count; }
                if left { self.load_cursor = (self.load_cursor + count - 1) % count; }
                if right { self.load_cursor = (self.load_cursor + 1) % count; }
                if up || down || left || right { out.push(Action::Sound("misc/menu1.wav")); }
                if enter {
                    let i = self.load_cursor;
                    if self.screen == Screen::Load {
                        if !self.autosaves && i == 1 {
                            self.autosaves = true;
                            self.load_cursor = 0;
                            out.push(Action::Command("_scansaves\n".into()));
                        } else if self.saves[i].is_some() {
                            let name = self.save_slot_name(i).unwrap();
                            self.screen = Screen::None;
                            out.push(Action::Close);
                            out.push(Action::Command(format!("load {name}\n")));
                        }
                    } else {
                        self.screen = Screen::None;
                        out.push(Action::Close);
                        out.push(Action::Command(format!("save s{i}\n")));
                    }
                }
            }
            Screen::Options => {
                if back {
                    self.screen = Screen::Main;
                    return out;
                }
                wrap(&mut self.page_cursor, PAGES.len(), &mut out);
                if enter || right {
                    out.push(Action::Sound("misc/menu2.wav"));
                    let p = PAGES[self.page_cursor].1;
                    if p == Page::Graphics {
                        out.push(Action::Command("_vid_menu\n".into()));
                    }
                    self.opt_cursor = 0;
                    self.screen = Screen::OptionsPage(p);
                }
            }
            Screen::OptionsPage(p) => {
                if back {
                    if p == Page::Setup {
                        out.push(Action::Command(Self::setup_commands(cv)));
                    }
                    self.screen = if is_net_page(p) { Screen::Multiplayer } else { Screen::Options };
                    return out;
                }
                let n = self.page_len(p);
                self.opt_cursor = self.opt_cursor.min(n - 1);
                wrap(&mut self.opt_cursor, n, &mut out);
                let item = self.page_item(p, self.opt_cursor);
                match item {
                    Item::Text { cvar, .. } => {
                        if key == K_ABUTTON {
                            self.osk = Some(Osk::default());
                            out.push(Action::Sound("misc/menu2.wav"));
                        } else if key == K_BACKSPACE {
                            let mut v = cv.by_name(cvar).map_or(String::new(), |c| c.string.clone());
                            v.pop();
                            cv.set(cvar, &v);
                        } else if enter && cvar == "_mp_address" {
                            self.run(Run::Connect, cv, &mut out);
                        }
                        return out;
                    }
                    Item::Run { run, .. } if enter => {
                        self.run(run, cv, &mut out);
                        return out;
                    }
                    Item::Server(k) if enter => {
                        if let Some(sv) = self.servers.get(k) {
                            cv.set("_mp_address", &sv.address.clone());
                            self.run(Run::Connect, cv, &mut out);
                        }
                        return out;
                    }
                    Item::Run { .. } | Item::Server(_) => return out,
                    Item::Bind { cmd, .. } => {
                        if enter {
                            self.capture = Some(self.opt_cursor);
                            out.push(Action::Sound("misc/menu2.wav"));
                        } else if matches!(key, K_BACKSPACE | K_DEL | K_XBUTTON) {
                            // clear this device kind's keys for the action
                            let pad = key == K_XBUTTON;
                            for k in keys_for(&self.bindings, cmd).into_iter().filter(|&k| is_pad_key(k) == pad) {
                                out.push(Action::Command(format!("unbind \"{}\"\n", keynum_to_string(k))));
                                self.bindings[k as usize] = None;
                            }
                            out.push(Action::Sound("misc/menu3.wav"));
                        }
                        return out;
                    }
                    Item::ResetBinds => {
                        if enter {
                            out.push(Action::Command("_resetbinds\n".into()));
                            out.push(Action::Sound("misc/menu2.wav"));
                        }
                        return out;
                    }
                    _ => {}
                }
                if enter && self.options(item).len() > 2 {
                    let cur = self.option_index(item, cv);
                    let mut d = Drop { item: self.opt_cursor, cursor: cur, scroll: 0 };
                    clamp_scroll(&mut d);
                    self.drop = Some(d);
                    out.push(Action::Sound("misc/menu2.wav"));
                    return out;
                }
                let dir = if left { -1 } else if right || enter { 1 } else { 0 };
                if dir != 0 && self.adjust(item, dir, enter, cv, &mut out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Screen::Mods | Screen::ModMaps => {
                self.mods.level_select = cv.value_of("ui_levelselect") != 0.0;
                let page = if self.screen == Screen::Mods { modmenu::Page::List } else { modmenu::Page::Maps };
                self.screen = match self.mods.key(page, key, &mut out) {
                    Some(modmenu::Page::List) => Screen::Mods,
                    Some(modmenu::Page::Maps) => Screen::ModMaps,
                    None if out.iter().any(|a| matches!(a, Action::Close)) => Screen::None,
                    None => Screen::Main,
                };
            }
            Screen::Quit => {
                if left || right {
                    self.quit_cursor ^= 1;
                    out.push(Action::Sound("misc/menu1.wav"));
                }
                let yes = matches!(key, 0x79 | 0x59) || (enter && self.quit_cursor == 0);
                let no = matches!(key, 0x6e | 0x4e) || back || (enter && self.quit_cursor == 1);
                if yes {
                    out.push(Action::Command("quit\n".into()));
                } else if no {
                    self.screen = if self.prev == Screen::Quit { Screen::Main } else { self.prev };
                    if self.screen == Screen::None {
                        out.push(Action::Close);
                    }
                }
            }
        }
        out
    }

    /// Changes one option by `dir` steps. Returns true if something changed.
    fn adjust(&mut self, item: Item, dir: i32, enter: bool, cv: &mut Cvars, out: &mut Vec<Action>) -> bool {
        match item {
            Item::Slider { cvar, min, max, step, invert, .. } => {
                let d = if invert { -dir } else { dir } as f32;
                let v = (cv.value_of(cvar) + d * step).clamp(min, max);
                set_num(cv, cvar, v);
            }
            Item::Toggle { cvar, .. } => {
                let v = cv.value_of(cvar);
                cv.set(cvar, if v != 0.0 { "0" } else { "1" });
            }
            Item::Choice { cvar, choices, .. } => {
                let n = self.choice_count(cvar, choices);
                let cur = nearest_choice(cv.value_of(cvar), &choices[..n]);
                let i = (cur as i32 + dir).rem_euclid(n as i32) as usize;
                set_num(cv, cvar, choices[i].1);
            }
            Item::DisplayMode => {
                self.pending_mode = (self.pending_mode as i32 + dir).rem_euclid(MODES.len() as i32) as usize;
            }
            Item::Resolution => {
                if self.resolutions.is_empty() {
                    return false;
                }
                let n = self.resolutions.len() as i32;
                let cur = self.resolutions.iter().position(|&r| r == self.pending_res);
                // list is largest first; "right" goes to larger sizes
                let i = match cur {
                    Some(i) => (i as i32 - dir).rem_euclid(n),
                    None => 0,
                };
                self.pending_res = self.resolutions[i as usize];
            }
            Item::Color { high, .. } => {
                let c = cv.value_of("_cl_color") as i32;
                let (mut t, mut b) = (c >> 4, c & 15);
                let v = if high { &mut t } else { &mut b };
                *v = (*v + dir).rem_euclid(14);
                cv.set("_cl_color", &(t * 16 + b).to_string());
            }
            Item::Map => {
                let n = self.net_maps.len() as i32;
                if n == 0 {
                    return false;
                }
                let cur = self.host_map(cv).and_then(|m| self.net_maps.iter().position(|x| x.name == m)).unwrap_or(0);
                let i = (cur as i32 + dir).rem_euclid(n) as usize;
                cv.set("_mp_map", &self.net_maps[i].name.clone());
            }
            Item::Text { .. } | Item::Run { .. } | Item::Server(_) | Item::Bind { .. } | Item::ResetBinds => return false,
            Item::Apply => {
                if !enter {
                    return false;
                }
                let (w, h) = self.pending_res;
                out.push(Action::Command(format!("vid_fullscreen {}\nvid_width {w}\nvid_height {h}\nvid_apply\n", self.pending_mode)));
            }
        }
        true
    }

    /// Display names of an item's discrete values (empty for sliders/Apply).
    /// Items with more than two get a dropdown.
    fn options(&self, item: Item) -> Vec<String> {
        match item {
            Item::Toggle { .. } => vec!["off".into(), "on".into()],
            Item::Choice { cvar, choices, .. } => choices[..self.choice_count(cvar, choices)].iter().map(|c| c.0.to_string()).collect(),
            Item::DisplayMode => MODES.iter().map(|m| m.to_string()).collect(),
            Item::Resolution if self.pending_mode != 2 => self.resolutions.iter().map(|(w, h)| format!("{w}x{h}")).collect(),
            Item::Map => self
                .net_maps
                .iter()
                .map(|m| {
                    let t = format!("{:<8} {}", m.name, m.title);
                    t.chars().take(36).collect::<String>().trim_end().to_string()
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// Index of the item's current value in `options`.
    fn option_index(&self, item: Item, cv: &Cvars) -> usize {
        match item {
            Item::Toggle { cvar, .. } => (cv.value_of(cvar) != 0.0) as usize,
            Item::Choice { cvar, choices, .. } => nearest_choice(cv.value_of(cvar), &choices[..self.choice_count(cvar, choices)]),
            Item::DisplayMode => self.pending_mode.min(MODES.len() - 1),
            Item::Resolution => self.resolutions.iter().position(|&r| r == self.pending_res).unwrap_or(0),
            Item::Map => self.host_map(cv).and_then(|m| self.net_maps.iter().position(|x| x.name == m)).unwrap_or(0),
            _ => 0,
        }
    }

    fn set_option(&mut self, item: Item, i: usize, cv: &mut Cvars) {
        match item {
            Item::Toggle { cvar, .. } => {
                cv.set(cvar, if i != 0 { "1" } else { "0" });
            }
            Item::Choice { cvar, choices, .. } => set_num(cv, cvar, choices[i].1),
            Item::DisplayMode => self.pending_mode = i,
            Item::Resolution => self.pending_res = self.resolutions[i],
            Item::Map => {
                if let Some(m) = self.net_maps.get(i) {
                    cv.set("_mp_map", &m.name.clone());
                }
            }
            _ => {}
        }
    }

    /// Dropdown box: (x, y, w) and visible entry count. Opens below its row,
    /// or above when it would run off the bottom.
    fn drop_geom(&self, d: Drop, opts: &[String]) -> (f32, f32, f32, usize) {
        let vis = opts.len().min(DROP_ROWS);
        let w = opts.iter().map(|o| o.len() as f32 * 8.0 + 20.0).fold(DROP_W, f32::max);
        let row_y = PAGE_ROW0 + d.item as f32 * ROW_H;
        let h = vis as f32 * ROW_H + 2.0;
        let y = if row_y + ROW_H + h > MENU_H - 4.0 { row_y - h } else { row_y + ROW_H };
        (VALUE_X - 3.0, y, w, vis)
    }

    /// Dropdown entry under the point.
    fn drop_at(&self, (x, y): (f32, f32), d: Drop) -> Option<usize> {
        let Screen::OptionsPage(p) = self.screen else { return None };
        let opts = self.options(self.page_item(p, d.item));
        let (bx, by, bw, vis) = self.drop_geom(d, &opts);
        let k = ((y - by - 1.0) / ROW_H).floor();
        (x >= bx && x < bx + bw && k >= 0.0 && (k as usize) < vis).then(|| d.scroll + k as usize)
    }

    /// Keys while a dropdown is open: navigate, pick, or close.
    fn drop_key(&mut self, key: i32, mut d: Drop, item: Item, cv: &mut Cvars, out: &mut Vec<Action>) {
        let n = self.options(item).len();
        if n == 0 {
            self.drop = None;
            return;
        }
        let pick = |m: &mut Self, i: usize, cv: &mut Cvars, out: &mut Vec<Action>| {
            m.set_option(item, i, cv);
            m.drop = None;
            out.push(Action::Sound("misc/menu3.wav"));
        };
        match key {
            K_MOUSE1 => match self.mouse.and_then(|p| self.drop_at(p, d)) {
                Some(i) => pick(self, i, cv, out),
                None => self.drop = None,
            },
            K_ENTER | K_KP_ENTER | K_ABUTTON => pick(self, d.cursor, cv, out),
            K_ESCAPE | K_BBUTTON | K_MOUSE2 => self.drop = None,
            K_UPARROW | K_DPAD_UP | K_DOWNARROW | K_DPAD_DOWN => {
                let up = matches!(key, K_UPARROW | K_DPAD_UP);
                d.cursor = if up { d.cursor.saturating_sub(1) } else { (d.cursor + 1).min(n - 1) };
                clamp_scroll(&mut d);
                self.drop = Some(d);
                out.push(Action::Sound("misc/menu1.wav"));
            }
            K_MWHEELUP | K_MWHEELDOWN => {
                let max = n.saturating_sub(DROP_ROWS);
                d.scroll = if key == K_MWHEELUP { d.scroll.saturating_sub(1) } else { (d.scroll + 1).min(max) };
                // keep the highlight on the entry under the pointer
                if let Some(i) = self.mouse.and_then(|p| self.drop_at(p, d)) {
                    d.cursor = i;
                }
                self.drop = Some(d);
            }
            _ => {}
        }
    }

    /// Choices usable on this device (MSAA limited by the GPU).
    fn choice_count(&self, cvar: &str, choices: &[(&str, f32)]) -> usize {
        if cvar == "r_msaa" {
            choices.iter().filter(|c| c.1 as u32 <= self.max_msaa.max(1)).count().max(1)
        } else {
            choices.len()
        }
    }

    /// True when the pending display settings differ from the cvars.
    fn display_dirty(&self, cv: &Cvars) -> bool {
        self.pending_mode != cv.value_of("vid_fullscreen") as usize
            || self.pending_res != (cv.value_of("vid_width") as u32, cv.value_of("vid_height") as u32)
    }

    pub fn draw(&self, c: &mut Canvas, pic: &mut PicFn, cv: &Cvars, realtime: f64) {
        if self.screen == Screen::None {
            return;
        }
        c.fade_screen();
        let base_scale = c.scale;
        c.menu_layout();
        if !matches!(self.screen, Screen::Options | Screen::OptionsPage(_) | Screen::Quit | Screen::Mods | Screen::ModMaps | Screen::Load | Screen::Save) {
            c.ox += CLASSIC.0;
            c.oy += CLASSIC.1;
        }
        let title_c = |c: &mut Canvas, pic: &mut PicFn, name: &str| {
            let p = pic(name);
            if let Some((_, w, _)) = p {
                c.pic(((640 - w as i32) / 2) as f32, 16.0, p);
            }
        };
        let dot = |pic: &mut PicFn| pic(&format!("gfx/menudot{}.lmp", (realtime * 10.0) as i64 % 6 + 1));
        let title = |c: &mut Canvas, pic: &mut PicFn, name: &str| {
            let p = pic(name);
            if let Some((_, w, _)) = p {
                c.pic(((320 - w as i32) / 2) as f32, 4.0, p);
            }
        };
        match self.screen {
            Screen::Main => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/ttl_main.lmp");
                let p = pic("gfx/mainmenu.lmp");
                c.pic(72.0, 32.0, p);
                // extra entry under the classic graphic
                let (y, w) = (32.0 + 5.0 * 20.0 + 2.0, 112.0);
                c.fill(72.0, y, w, 16.0, [0.0, 0.0, 0.0, 0.55]);
                c.frame(72.0, y, w, 16.0, BORDER);
                gold(c, 72.0 + (w - 32.0) / 2.0, y + 4.0, "Mods");
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.main_cursor as f32 * 20.0, d);
            }
            Screen::Mods => self.mods.draw(c, modmenu::Page::List),
            Screen::ModMaps => self.mods.draw(c, modmenu::Page::Maps),
            Screen::SinglePlayer => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/ttl_sgl.lmp");
                let p = pic("gfx/sp_menu.lmp");
                c.pic(72.0, 32.0, p);
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.sp_cursor as f32 * 20.0, d);
            }
            Screen::Multiplayer => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/p_multi.lmp");
                let p = pic("gfx/mp_menu.lmp");
                c.pic(72.0, 32.0, p);
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.mp_cursor as f32 * 20.0, d);
            }
            Screen::Load | Screen::Save => {
                let heading = if self.screen == Screen::Save { "SAVE GAME" } else if self.autosaves { "AUTOSAVES - NEWEST FIRST" } else { "LOAD GAME" };
                c.string(24.0, 24.0, heading);
                let page = self.load_cursor / 4;
                for i in page * 4..((page + 1) * 4).min(self.save_rows()) {
                    let x = 24.0 + (i % 2) as f32 * 300.0;
                    let y = 56.0 + (i % 4 / 2) as f32 * 128.0;
                    let selected = i == self.load_cursor;
                    c.fill(x, y, 292.0, 126.0, if selected { [0.30, 0.20, 0.09, 0.98] } else { [0.08, 0.07, 0.06, 0.96] });
                    if selected { c.fill(x, y, 3.0, 126.0, [0.85, 0.59, 0.22, 1.0]); }
                    let line = |c: &mut Canvas, row: usize, text: &str| {
                        let text: String = text.chars().map(|ch| if ch.is_ascii_graphic() || ch == ' ' { ch } else { '?' }).take(34).collect();
                        c.string(x + 10.0, y + 7.0 + row as f32 * 10.0, &text);
                    };
                    if self.screen == Screen::Load && !self.autosaves && i == 1 {
                        line(c, 0, "Browse Autosaves >");
                        line(c, 2, "All retained autosaves");
                        line(c, 3, "Newest to oldest");
                        continue;
                    }
                    if let Some(s) = &self.saves[i] {
                        line(c, 0, &s.level);
                        line(c, 1, &s.timestamp);
                        line(c, 2, &format!("Health {}   Armor {}", s.health, s.armor));
                        let keys = self.save_inventory.keys(s);
                        line(c, 9, if keys.is_empty() { "Keys: none" } else { "Keys:" });
                        for (j, icon) in keys.iter().enumerate() {
                            draw_inventory_icon(c, pic, x + 60.0 + j as f32 * 32.0, y + 94.0, 28.0, 16.0, icon);
                        }
                        line(c, 3, &format!("Kills {}/{}   Secrets {}/{}", s.kills.0, s.kills.1, s.secrets.0, s.secrets.1));
                        let icons = self.save_inventory.weapons(s);
                        if icons.is_empty() { line(c, 4, "Weapons: none"); }
                        let columns = if icons.len() <= 20 { 10 } else { 14 };
                        let cell = if columns == 10 { 27.0 } else { 19.0 };
                        for (j, icon) in icons.iter().take(42).enumerate() {
                            draw_inventory_icon(c, pic, x + 10.0 + (j % columns) as f32 * cell,
                                y + 47.0 + (j / columns) as f32 * 14.0, cell - 2.0, 13.0, icon);
                        }
                        let label = if self.autosaves { format!("Auto {}: {}", i + 1, s.reason.as_deref().unwrap_or("Unknown trigger")) }
                            else if self.screen == Screen::Load && i == 0 { format!("Latest auto: {}", s.reason.as_deref().unwrap_or("Unknown")) }
                            else { format!("Manual slot {}", if self.screen == Screen::Load { i - 1 } else { i + 1 }) };
                        let missing_weapons: Vec<_> = icons.iter().filter(|icon| pic(&icon.name).is_none()).map(|icon| icon.fallback.as_str()).collect();
                        if !missing_weapons.is_empty() { line(c, 8, &missing_weapons.join(", ")); }
                        let missing_keys: Vec<_> = keys.iter().filter(|icon| pic(&icon.name).is_none()).map(|icon| icon.fallback.as_str()).collect();
                        if !missing_keys.is_empty() { line(c, 10, &missing_keys.join(", ")); }
                        line(c, 11, &label);
                    } else {
                        let label = if self.screen == Screen::Load && self.autosaves { "No autosaves yet".into() }
                            else if self.screen == Screen::Load && i == 0 { "No autosave yet".into() }
                            else { format!("Empty manual slot {}", if self.screen == Screen::Load { i - 1 } else { i + 1 }) };
                        line(c, 0, &label);
                        if self.screen == Screen::Save { line(c, 2, "Select to save here"); }
                    }
                }
                c.string(520.0, 338.0, &format!("Page {}/{}", page + 1, self.save_rows().div_ceil(4)));
            }
            Screen::Options => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(PANEL_X - 40.0, 16.0, p);
                title_c(c, pic, "gfx/p_option.lmp");
                panel(c, ROW0, PAGES.len());
                highlight(c, ROW0 + self.page_cursor as f32 * ROW_H);
                for (i, (label, _)) in PAGES.iter().enumerate() {
                    let y = ROW0 + i as f32 * ROW_H + 1.0;
                    c.string(((640.0 - label.len() as f32 * 8.0) / 2.0).floor(), y, label);
                }
            }
            Screen::OptionsPage(page) => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(PANEL_X - 40.0, 16.0, p);
                title_c(c, pic, if is_net_page(page) { "gfx/p_multi.lmp" } else { "gfx/p_option.lmp" });
                let name = page_name(page);
                let nx = if page == Page::Controls { LABEL_R - name.len() as f32 * 8.0 } else { ((640.0 - name.len() as f32 * 8.0) / 2.0).floor() };
                gold(c, nx, PAGE_ROW0 - 16.0, name);
                let n = self.page_len(page);
                panel(c, PAGE_ROW0, n);
                highlight(c, PAGE_ROW0 + self.opt_cursor as f32 * ROW_H);
                for i in 0..n {
                    let blink = (realtime * 4.0) as i64 & 1 == 0 && i == self.opt_cursor;
                    self.draw_item(c, cv, self.page_item(page, i), PAGE_ROW0 + i as f32 * ROW_H + 1.0, blink);
                }
                if page == Page::Setup {
                    // M_Setup_Draw: the player in a box, in the chosen colours.
                    let v = cv.value_of("_cl_color") as i32;
                    let y0 = PAGE_ROW0 + n as f32 * ROW_H + 12.0;
                    let bx = pic("gfx/bigbox.lmp");
                    let bw = bx.map_or(64.0, |b| b.1 as f32);
                    c.pic(((640.0 - bw) / 2.0).floor(), y0, bx);
                    let p = pic(&format!("gfx/menuplyr.lmp#{},{}", (v >> 4) & 15, v & 15));
                    let pw = p.map_or(48.0, |p| p.1 as f32);
                    c.pic(((640.0 - pw) / 2.0).floor(), y0 + 8.0, p);
                }
                if page == Page::Join && self.servers.is_empty() && !self.searching {
                    let s = "no servers found yet";
                    c.string(((640.0 - s.len() as f32 * 8.0) / 2.0).floor(), PAGE_ROW0 + n as f32 * ROW_H + 8.0, s);
                }
                if page == Page::Controls {
                    c.string(VALUE_X, PAGE_ROW0 - 16.0, "Keyboard");
                    c.string(BIND_PAD_X, PAGE_ROW0 - 16.0, "Controller");
                }
                if let Some(d) = self.drop {
                    self.draw_drop(c, cv, self.page_item(page, d.item), d);
                }
                if let Some(o) = self.osk {
                    self.draw_osk(c, o);
                }
            }
            Screen::Quit => {
                let (bx, by, bw, bh) = QUIT_BOX;
                c.fill(bx, by, bw, bh, [0.05, 0.04, 0.03, 0.92]);
                c.frame(bx, by, bw, bh, BORDER);
                let msg = "Really quit?";
                c.string(((640.0 - msg.len() as f32 * 8.0) / 2.0).floor(), by + 12.0, msg);
                for (i, (&x, label)) in QUIT_BUTTONS.iter().zip(["(Y)es", "(N)o"]).enumerate() {
                    let (w, h) = QUIT_BTN;
                    if i == self.quit_cursor {
                        c.fill(x, QUIT_BTN_Y, w, h, HILITE);
                    }
                    c.frame(x, QUIT_BTN_Y, w, h, BORDER);
                    let tx = x + ((w - label.len() as f32 * 8.0) / 2.0).floor();
                    // gold hotkey letter
                    c.string(tx, QUIT_BTN_Y + 3.0, "(");
                    gold(c, tx + 8.0, QUIT_BTN_Y + 3.0, &label[1..2]);
                    c.string(tx + 16.0, QUIT_BTN_Y + 3.0, &label[2..]);
                }
            }
            Screen::None => {}
        }
        c.menu_layout();
        self.draw_hints(c);
        c.reset_origin();
        c.scale = base_scale;
    }

    /// Bottom row of button prompts, for the device in use.
    fn draw_hints(&self, c: &mut Canvas) {
        let pad = self.pad_ui;
        let (sel, back) = if pad { (K_ABUTTON, K_BBUTTON) } else { (K_ENTER, K_ESCAPE) };
        let y = 344.0;
        if self.capturing() {
            let s = if pad { "Press a button or key to bind" } else { "Press a key or button to bind" };
            let w = s.len() as f32 * 8.0;
            c.fill(320.0 - w / 2.0 - 8.0, y - 4.0, w + 16.0, 18.0, [0.0, 0.0, 0.0, 0.8]);
            gold(c, 320.0 - w / 2.0, y + 1.0, s);
            return;
        }
        if self.osk.is_some() {
            let items = [(K_ABUTTON, "Type"), (K_XBUTTON, "Delete"), (K_YBUTTON, "Shift"), (K_BBUTTON, "Done")];
            self.glyphs.hints_centered(c, 320.0, y, &items);
            return;
        }
        let action = match self.screen {
            Screen::Save => "Save",
            Screen::Load => "Load",
            Screen::OptionsPage(Page::Controls) => "Bind",
            _ => "Select",
        };
        let mut items = vec![(sel, action), (back, "Back")];
        if self.screen == Screen::OptionsPage(Page::Controls) && self.drop.is_none() {
            items.insert(1, (if pad { K_XBUTTON } else { K_BACKSPACE }, "Clear"));
        }
        if matches!(self.screen, Screen::Quit) {
            items = vec![(sel, "Confirm"), (back, "Cancel")];
        }
        self.glyphs.hints_centered(c, 320.0, y, &items);
    }

    fn draw_drop(&self, c: &mut Canvas, cv: &Cvars, item: Item, d: Drop) {
        let opts = self.options(item);
        let (x, y, w, vis) = self.drop_geom(d, &opts);
        let h = vis as f32 * ROW_H + 2.0;
        c.fill(x, y, w, h, [0.05, 0.04, 0.03, 0.97]);
        c.frame(x, y, w, h, BORDER);
        let cur = self.option_index(item, cv);
        for (k, o) in opts.iter().enumerate().skip(d.scroll).take(vis) {
            let ry = y + 1.0 + (k - d.scroll) as f32 * ROW_H;
            if k == d.cursor {
                c.fill(x + 1.0, ry, w - 2.0, ROW_H, HILITE);
            }
            if k == cur {
                gold(c, x + 4.0, ry + 1.0, o);
            } else {
                c.string(x + 4.0, ry + 1.0, o);
            }
        }
        // scroll hints
        if d.scroll > 0 {
            c.ch(x + w - 10.0, y + 2.0, b'^' | 128);
        }
        if d.scroll + vis < opts.len() {
            c.ch(x + w - 10.0, y + h - 10.0, b'v' | 128);
        }
    }

    fn draw_item(&self, c: &mut Canvas, cv: &Cvars, item: Item, y: f32, blink: bool) {
        let label = |c: &mut Canvas, l: &str| c.string(LABEL_R - l.len() as f32 * 8.0, y, l);
        // values with a dropdown get a combo box
        if self.options(item).len() > 2 {
            c.fill(VALUE_X - 3.0, y - 1.0, DROP_W, ROW_H, [0.0, 0.0, 0.0, 0.5]);
            c.frame(VALUE_X - 3.0, y - 1.0, DROP_W, ROW_H, BORDER);
            c.ch(VALUE_X + DROP_W - 13.0, y, b'v' | 128);
        }
        match item {
            Item::Slider { label: l, cvar, min, max, invert, .. } => {
                label(c, l);
                let v = cv.value_of(cvar);
                let mut f = ((v - min) / (max - min)).clamp(0.0, 1.0);
                if invert {
                    f = 1.0 - f;
                }
                c.ch(VALUE_X, y, 128);
                for k in 0..10 {
                    c.ch(VALUE_X + 8.0 + k as f32 * 8.0, y, 129);
                }
                c.ch(VALUE_X + 88.0, y, 130);
                c.ch(VALUE_X + 8.0 + 72.0 * f, y, 131);
                c.string(VALUE_X + 100.0, y, &qcore::cvar::format_float((v * 1000.0).round() / 1000.0));
            }
            Item::Toggle { label: l, cvar } => {
                label(c, l);
                c.string(VALUE_X, y, if cv.value_of(cvar) != 0.0 { "on" } else { "off" });
            }
            Item::Choice { label: l, cvar, choices } => {
                label(c, l);
                let n = self.choice_count(cvar, choices);
                c.string(VALUE_X, y, choices[nearest_choice(cv.value_of(cvar), &choices[..n])].0);
            }
            Item::DisplayMode => {
                label(c, "Display Mode");
                c.string(VALUE_X, y, MODES[self.pending_mode.min(2)]);
            }
            Item::Resolution => {
                label(c, "Resolution");
                let s = if self.pending_mode == 2 {
                    "desktop".to_string()
                } else {
                    format!("{}x{}", self.pending_res.0, self.pending_res.1)
                };
                c.string(VALUE_X, y, &s);
            }
            Item::Text { label: l, cvar, max } => {
                label(c, l);
                let v = cv.by_name(cvar).map_or("", |c| c.string.as_str());
                // fits inside the panel: 20 visible characters at most
                let w = (max.min(20) as f32 * 8.0 + 14.0).min(PANEL_X + PANEL_W - VALUE_X - 4.0);
                c.fill(VALUE_X - 3.0, y - 1.0, w, ROW_H, [0.0, 0.0, 0.0, 0.5]);
                let shown: String = v.chars().skip(v.len().saturating_sub(20)).collect();
                c.string(VALUE_X, y, &shown);
                if blink {
                    c.ch(VALUE_X + shown.len() as f32 * 8.0, y, 11);
                }
            }
            Item::Color { label: l, high } => {
                label(c, l);
                let v = cv.value_of("_cl_color") as i32;
                let k = if high { v >> 4 } else { v & 15 }.clamp(0, 13) as usize;
                let [r, g, b] = crate::draw::PLAYER_COLORS[k].map(|x| x as f32 / 255.0);
                c.fill(VALUE_X, y - 1.0, 40.0, ROW_H - 1.0, [r, g, b, 1.0]);
                c.frame(VALUE_X, y - 1.0, 40.0, ROW_H - 1.0, BORDER);
                c.string(VALUE_X + 48.0, y, &k.to_string());
            }
            Item::Map => {
                label(c, "Map");
                let m = self.host_map(cv).unwrap_or_else(|| "(none)".into());
                let t = self.net_maps.iter().find(|x| x.name == m).map_or(String::new(), |x| x.title.clone());
                let s: String = format!("{m} {t}").chars().take(13).collect();
                c.string(VALUE_X, y, &s);
            }
            Item::Run { label: l, run } => {
                let l = if run == Run::Search && self.searching { "Searching..." } else { l };
                gold(c, ((640.0 - l.len() as f32 * 8.0) / 2.0).floor(), y, l);
            }
            Item::Server(k) => {
                if let Some(s) = self.servers.get(k) {
                    let name: String = s.hostname.chars().take(20).collect();
                    label(c, &name);
                    let m: String = s.map.chars().take(10).collect();
                    c.string(VALUE_X, y, &format!("{m:<10} {}/{}", s.players, s.max_players));
                }
            }
            Item::Bind { label: l, cmd } => {
                label(c, l);
                let keys = keys_for(&self.bindings, cmd);
                let capturing = self.capture.is_some_and(|r| matches!(page_items(Page::Controls).get(r), Some(Item::Bind { cmd: c2, .. }) if *c2 == cmd));
                if capturing {
                    if blink {
                        gold(c, VALUE_X, y, "press a key...");
                    }
                    return;
                }
                let g = self.glyphs;
                let (mut x, mut px) = (VALUE_X, BIND_PAD_X);
                for &k in &keys {
                    if is_pad_key(k) {
                        if px + crate::glyphs::width(k, g.family) < PANEL_X + PANEL_W - 2.0 {
                            px += g.key(c, px, y - 1.0, k) + 3.0;
                        }
                    } else if x + crate::glyphs::width(k, g.family) < BIND_PAD_X - 4.0 {
                        x += g.key(c, x, y - 1.0, k) + 3.0;
                    }
                }
                if x == VALUE_X {
                    c.string(VALUE_X, y, "---");
                }
                if px == BIND_PAD_X {
                    c.string(BIND_PAD_X, y, "---");
                }
            }
            Item::ResetBinds => {
                let l = "Reset to Defaults";
                gold(c, ((640.0 - l.len() as f32 * 8.0) / 2.0).floor(), y, l);
            }
            Item::Apply => {
                let dirty = self.display_dirty(cv);
                label(c, "Apply Display");
                if dirty {
                    // gold text: pending changes
                    for (k, b) in "press enter".bytes().enumerate() {
                        c.ch(VALUE_X + k as f32 * 8.0, y, b | 128);
                    }
                }
            }
        }
    }
}

/// Quit prompt: box (x, y, w, h), button size, button row and left edges.
const QUIT_BOX: (f32, f32, f32, f32) = (224.0, 144.0, 192.0, 64.0);
const QUIT_BTN: (f32, f32) = (64.0, 14.0);
const QUIT_BTN_Y: f32 = 178.0;
const QUIT_BUTTONS: [f32; 2] = [248.0, 328.0];

/// Main menu rows: the five of mainmenu.lmp plus Mods.
fn draw_inventory_icon(c: &mut Canvas, pic: &mut PicFn, x: f32, y: f32, max_w: f32, max_h: f32, icon: &crate::save_inventory::Icon) {
    if let Some((id, w, h)) = pic(&icon.name) {
        let frame = icon.frame;
        let width = if frame.is_some() { w / 16 } else { w };
        let scale = (max_w / width.max(1) as f32).min(max_h / h.max(1) as f32).min(1.0);
        let px = (c.ox + x) * c.scale;
        let py = (c.oy + y) * c.scale;
        let uv = frame.map_or([0.0,0.0,1.0,1.0], |n| [n as f32 / 16.0,0.0,(n+1) as f32 / 16.0,1.0]);
        c.list.push(qrender::Draw2D::Pic { x: px, y: py, w: width as f32 * scale * c.scale,
            h: h as f32 * scale * c.scale, pic: id, uv, color: [1.0; 4] });
    }
}

const MAIN_ITEMS: usize = 6;

pub const BORDER: [f32; 4] = [0.55, 0.42, 0.25, 1.0];
pub const HILITE: [f32; 4] = [0.6, 0.38, 0.12, 0.45];

/// Brown/gold conchars text.
pub fn gold(c: &mut Canvas, x: f32, y: f32, s: &str) {
    for (k, b) in s.bytes().enumerate() {
        c.ch(x + k as f32 * 8.0, y, b | 128);
    }
}

/// Translucent box behind `n` option rows starting at `top`.
fn panel(c: &mut Canvas, top: f32, n: usize) {
    let h = n as f32 * ROW_H + 8.0;
    c.fill(PANEL_X, top - 4.0, PANEL_W, h, [0.0, 0.0, 0.0, 0.55]);
    c.frame(PANEL_X, top - 4.0, PANEL_W, h, [0.35, 0.27, 0.16, 0.9]);
}

fn highlight(c: &mut Canvas, y: f32) {
    c.fill(PANEL_X + 2.0, y, PANEL_W - 4.0, ROW_H, HILITE);
}

fn clamp_scroll(d: &mut Drop) {
    if d.cursor < d.scroll {
        d.scroll = d.cursor;
    } else if d.cursor >= d.scroll + DROP_ROWS {
        d.scroll = d.cursor + 1 - DROP_ROWS;
    }
}

fn set_num(cv: &mut Cvars, name: &str, v: f32) {
    cv.set(name, &qcore::cvar::format_float((v * 1000.0).round() / 1000.0));
}

/// Index of the choice closest to `v`.
fn nearest_choice(v: f32, choices: &[(&str, f32)]) -> usize {
    let mut best = 0;
    for (i, c) in choices.iter().enumerate() {
        if (c.1 - v).abs() < (choices[best].1 - v).abs() {
            best = i;
        }
    }
    best
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropdown_picks_value() {
        let mut cv = Cvars::default();
        cv.register("r_anisotropy", "1", 0);
        let mut m = Menu::new();
        m.open(Screen::OptionsPage(Page::Graphics));
        let row = GRAPHICS.iter().position(|i| matches!(i, Item::Choice { cvar: "r_anisotropy", .. })).unwrap();
        m.opt_cursor = row;
        m.key(K_ENTER, &mut cv, false);
        assert!(m.drop.is_some());
        m.key(K_DOWNARROW, &mut cv, false);
        m.key(K_DOWNARROW, &mut cv, false);
        m.key(K_ENTER, &mut cv, false);
        assert!(m.drop.is_none());
        assert_eq!(cv.value_of("r_anisotropy"), 4.0);
        // escape closes without changing
        m.key(K_ENTER, &mut cv, false);
        m.key(K_DOWNARROW, &mut cv, false);
        m.key(K_ESCAPE, &mut cv, false);
        assert!(m.drop.is_none());
        assert_eq!(m.screen, Screen::OptionsPage(Page::Graphics));
        assert_eq!(cv.value_of("r_anisotropy"), 4.0);
        // two-value choices still cycle
        let filt = GRAPHICS.iter().position(|i| matches!(i, Item::Choice { cvar: "r_texfilter", .. })).unwrap();
        m.opt_cursor = filt;
        m.key(K_ENTER, &mut cv, false);
        assert!(m.drop.is_none());
    }
}

#[cfg(test)]
mod quit_tests {
    use super::*;

    fn quits(acts: &[Action]) -> bool {
        acts.iter().any(|a| matches!(a, Action::Command(c) if c == "quit\n"))
    }

    #[test]
    fn quit_prompt_buttons_and_keys() {
        let mut cv = Cvars::default();
        let mut m = Menu::new();
        m.open(Screen::Main);
        m.open(Screen::Quit);
        assert!(quits(&m.key(b'y' as i32, &mut cv, false)));
        // click "No"
        m.mouse_move((QUIT_BUTTONS[1] + 4.0, QUIT_BTN_Y + 4.0));
        assert!(!quits(&m.key(K_MOUSE1, &mut cv, false)));
        assert_eq!(m.screen, Screen::Main);
        // click "Yes"
        m.open(Screen::Quit);
        m.mouse_move((QUIT_BUTTONS[0] + 4.0, QUIT_BTN_Y + 4.0));
        assert!(quits(&m.key(K_MOUSE1, &mut cv, false)));
        // keyboard: right then enter = No
        m.mouse = None;
        m.quit_cursor = 0;
        m.key(K_RIGHTARROW, &mut cv, false);
        assert!(!quits(&m.key(K_ENTER, &mut cv, false)));
        assert_eq!(m.screen, Screen::Main);
    }
}

#[cfg(test)]
mod multiplayer_tests {
    use super::*;

    pub(super) fn cvars() -> Cvars {
        let mut cv = Cvars::default();
        for (n, d) in [
            ("_cl_name", "player"),
            ("_cl_color", "0"),
            ("_mp_address", ""),
            ("_mp_maxplayers", "8"),
            ("_mp_coop", "0"),
            ("_mp_teamplay", "0"),
            ("_mp_skill", "1"),
            ("_mp_fraglimit", "0"),
            ("_mp_timelimit", "0"),
            ("_mp_map", ""),
        ] {
            cv.register(n, d, 0);
        }
        cv
    }

    pub(super) fn commands(acts: &[Action]) -> String {
        acts.iter().filter_map(|a| if let Action::Command(c) = a { Some(c.as_str()) } else { None }).collect()
    }

    #[test]
    fn join_by_typed_address() {
        let mut cv = cvars();
        let mut m = Menu::new();
        m.open(Screen::Main);
        m.main_cursor = 1;
        m.key(K_ENTER, &mut cv, false);
        assert_eq!(m.screen, Screen::Multiplayer);
        m.key(K_ENTER, &mut cv, false);
        assert_eq!(m.screen, Screen::OptionsPage(Page::Join));
        for ch in "10.0.0.9:x".chars() {
            m.text(ch, &mut cv);
        }
        m.key(K_BACKSPACE, &mut cv, false);
        m.text('7', &mut cv);
        let acts = m.key(K_ENTER, &mut cv, false);
        assert_eq!(commands(&acts), "connect 10.0.0.9:7\n");
        assert_eq!(m.screen, Screen::None);
    }

    #[test]
    fn join_found_server_and_search() {
        let mut cv = cvars();
        let mut m = Menu::new();
        m.open_multiplayer("join");
        m.opt_cursor = 2;
        assert_eq!(commands(&m.key(K_ENTER, &mut cv, false)), "slist\n");
        m.servers = vec![ServerEntry { address: "192.168.1.5:26000".into(), hostname: "h".into(), map: "dm3".into(), players: 1, max_players: 8 }];
        m.key(K_DOWNARROW, &mut cv, false);
        assert!(matches!(m.page_item(Page::Join, m.opt_cursor), Item::Server(0)));
        assert_eq!(commands(&m.key(K_ENTER, &mut cv, false)), "connect 192.168.1.5:26000\n");
    }

    #[test]
    fn host_game_start() {
        let mut cv = cvars();
        let mut m = Menu::new();
        assert!(m.open_multiplayer("host").is_some());
        m.net_maps = ["e1m1", "dm1", "dm2"].iter().map(|n| qui::mods::MapInfo { name: n.to_string(), title: String::new() }).collect();
        m.key(K_RIGHTARROW, &mut cv, false); // 8 -> 12 players
        m.opt_cursor = 4;
        m.key(K_RIGHTARROW, &mut cv, false); // frag limit 10
        m.opt_cursor = 6;
        m.key(K_RIGHTARROW, &mut cv, false); // dm1 (default) -> dm2
        m.opt_cursor = 7;
        let c = commands(&m.key(K_ENTER, &mut cv, false));
        assert_eq!(c, "disconnect\nmaxplayers 12\ncoop 0\ndeathmatch 1\nteamplay 0\nskill 1\nfraglimit 10\ntimelimit 0\nmap dm2\n");
        assert_eq!(cv.by_name("_mp_map").unwrap().string, "dm2");
    }

    #[test]
    fn setup_applies_name_and_colors_on_back() {
        let mut cv = cvars();
        let mut m = Menu::new();
        m.open_multiplayer("setup");
        for _ in 0..6 {
            m.key(K_BACKSPACE, &mut cv, false);
        }
        for ch in "Ranger".chars() {
            m.text(ch, &mut cv);
        }
        m.opt_cursor = 1;
        m.key(K_LEFTARROW, &mut cv, false); // shirt 0 -> 13
        m.opt_cursor = 2;
        m.key(K_RIGHTARROW, &mut cv, false); // pants 0 -> 1
        let c = commands(&m.key(K_ESCAPE, &mut cv, false));
        assert_eq!(c, "name \"Ranger\"\ncolor 13 1\n");
        assert_eq!(m.screen, Screen::Multiplayer);
    }
}

#[cfg(test)]
mod save_menu_tests {
    use super::*;
    #[test]
    fn autosave_submenu_navigation_and_slot_mapping() {
        let mut m = Menu::new();
        let mut cv = Cvars::new();
        m.open(Screen::Load);
        assert_eq!(m.save_slot_name(0).as_deref(), Some("autosave0"));
        assert_eq!(m.save_slot_name(2).as_deref(), Some("s0"));
        m.key(K_RIGHTARROW, &mut cv, true);
        assert_eq!(m.load_cursor, 1);
        let actions = m.key(K_ENTER, &mut cv, true);
        assert!(m.autosaves);
        assert!(actions.iter().any(|a| matches!(a, Action::Command(s) if s == "_scansaves\n")));
        // An empty archive still handles every direction without underflow.
        for key in [K_UPARROW, K_DOWNARROW, K_LEFTARROW, K_RIGHTARROW] {
            m.key(key, &mut cv, true);
            assert_eq!(m.load_cursor, 0);
        }
        m.key(K_ESCAPE, &mut cv, true);
        assert!(!m.autosaves);
        assert_eq!(m.screen, Screen::Load);
        assert_eq!(m.load_cursor, 1);
        m.open(Screen::Save);
        assert_eq!(m.save_slot_name(0).as_deref(), Some("s0"));
        assert_eq!(m.save_rows(), MAX_SAVEGAMES);
    }
    #[test]
    fn card_mouse_coordinates_follow_keyboard_pages() {
        let mut m = Menu::new();
        m.open(Screen::Load);
        assert_eq!(m.item_at((40.0, 64.0)), Some(0));
        assert_eq!(m.item_at((340.0, 200.0)), Some(3));
        m.load_cursor = 4;
        assert_eq!(m.item_at((40.0, 64.0)), Some(4));
        assert_eq!(m.item_at((20.0, 64.0)), None);
    }

    #[test]
    fn controls_rebind_and_clear() {
        use super::multiplayer_tests::{commands, cvars};
        let mut cv = cvars();
        let mut m = Menu::new();
        m.bindings[K_MOUSE1 as usize] = Some("+attack".into());
        m.bindings[K_RTRIGGER as usize] = Some("+attack".into());
        m.open(Screen::OptionsPage(Page::Controls));
        m.opt_cursor = 0;
        m.key(K_ENTER, &mut cv, false);
        assert!(m.capturing());
        let c = commands(&m.key(b'f' as i32, &mut cv, false));
        assert_eq!(c, "bind \"f\" \"+attack\"\n");
        assert!(!m.capturing());
        // a third keyboard key replaces both keyboard keys, pad untouched
        m.key(K_ENTER, &mut cv, false);
        let c = commands(&m.key(b'g' as i32, &mut cv, false));
        assert!(c.contains("unbind \"MOUSE1\"") && c.contains("unbind \"f\"") && c.ends_with("bind \"g\" \"+attack\"\n"), "{c}");
        assert_eq!(keys_for(&m.bindings, "+attack"), vec![b'g' as i32, K_RTRIGGER]);
        // escape cancels capture
        m.key(K_ENTER, &mut cv, false);
        assert!(commands(&m.key(K_ESCAPE, &mut cv, false)).is_empty());
        assert_eq!(m.screen, Screen::OptionsPage(Page::Controls));
        // X clears the pad keys only
        let c = commands(&m.key(K_XBUTTON, &mut cv, false));
        assert_eq!(c, "unbind \"RTRIGGER\"\n");
        assert_eq!(keys_for(&m.bindings, "+attack"), vec![b'g' as i32]);
    }

    #[test]
    fn onscreen_keyboard_types_into_field() {
        use super::multiplayer_tests::cvars;
        let mut cv = cvars();
        cv.set("_cl_name", "");
        let mut m = Menu::new();
        m.open(Screen::OptionsPage(Page::Setup));
        m.opt_cursor = 0;
        m.key(K_ABUTTON, &mut cv, false);
        assert!(m.osk_open());
        m.key(K_DPAD_DOWN, &mut cv, false); // q row
        m.key(K_ABUTTON, &mut cv, false); // q
        m.key(K_YBUTTON, &mut cv, false); // shift
        m.key(K_DPAD_RIGHT, &mut cv, false);
        m.key(K_ABUTTON, &mut cv, false); // W
        m.key(K_DPAD_UP, &mut cv, false);
        m.key(K_DPAD_UP, &mut cv, false); // action row (wraps)
        m.key(K_DPAD_RIGHT, &mut cv, false); // shift -> space
        m.key(K_ABUTTON, &mut cv, false);
        assert_eq!(cv.by_name("_cl_name").unwrap().string, "qW ");
        m.key(K_XBUTTON, &mut cv, false);
        assert_eq!(cv.by_name("_cl_name").unwrap().string, "qW");
        m.key(K_BBUTTON, &mut cv, false);
        assert!(!m.osk_open());
        assert_eq!(m.screen, Screen::OptionsPage(Page::Setup));
    }
}
