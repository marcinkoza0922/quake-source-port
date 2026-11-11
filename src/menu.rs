//! Basic menus (menu.c subset): main, single player, load/save, options,
//! quit. The mod menu is a later milestone and will live elsewhere.

use qcore::cvar::Cvars;
use qinput::keys::*;

use crate::draw::Canvas;
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
}

/// Options sub-pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Gameplay,
    Input,
    Graphics,
    Audio,
}

const PAGES: [(&str, Page); 4] = [
    ("Gameplay", Page::Gameplay),
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
}

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
];

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
        Page::Input => INPUT,
        Page::Graphics => GRAPHICS,
        Page::Audio => AUDIO,
    }
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
    /// Display settings being edited on the Graphics page (mode 0..=2, size).
    pub pending_mode: usize,
    pub pending_res: (u32, u32),
    /// Available resolutions (filled by the host) and device MSAA limit.
    pub resolutions: Vec<(u32, u32)>,
    pub max_msaa: u32,
    /// Comment line of each save slot (`None` = unused).
    pub saves: Vec<Option<String>>,
    prev: Screen,
    /// Cursor position in 320x200 menu units, when the mouse has moved.
    mouse: Option<(f32, f32)>,
}

/// Slider track in options, menu x units.
const SLIDER_X: (f32, f32) = (168.0, 248.0);

/// Value column / first row of the options pages.
const VALUE_X: f32 = 160.0;
const ROW0: f32 = 32.0;
/// First row on an options page (below the page heading).
const PAGE_ROW0: f32 = 44.0;

impl Menu {
    pub fn new() -> Self {
        Self {
            screen: Screen::None,
            main_cursor: 0,
            sp_cursor: 0,
            load_cursor: 0,
            opt_cursor: 0,
            page_cursor: 0,
            pending_mode: 0,
            pending_res: (1280, 720),
            resolutions: Vec::new(),
            max_msaa: 8,
            saves: vec![None; MAX_SAVEGAMES],
            prev: Screen::None,
            mouse: None,
        }
    }

    pub fn active(&self) -> bool {
        self.screen != Screen::None
    }

    pub fn open(&mut self, s: Screen) {
        self.prev = self.screen;
        self.screen = s;
    }

    /// Item under the menu-space point for the current screen.
    fn item_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let row = |top: f32, h: f32, n: usize, x0: f32| {
            (x >= x0 && x < 312.0 && y >= top && y < top + h * n as f32).then(|| ((y - top) / h) as usize)
        };
        match self.screen {
            Screen::Main => row(32.0, 20.0, 5, 54.0),
            Screen::SinglePlayer => row(32.0, 20.0, 3, 54.0),
            Screen::Load | Screen::Save => row(32.0, 8.0, MAX_SAVEGAMES, 8.0),
            Screen::Options => row(ROW0, 8.0, PAGES.len(), 16.0),
            Screen::OptionsPage(p) => row(PAGE_ROW0, 8.0, page_items(p).len(), 16.0),
            Screen::Quit | Screen::None => None,
        }
    }

    fn cursor_mut(&mut self) -> Option<&mut usize> {
        match self.screen {
            Screen::Main => Some(&mut self.main_cursor),
            Screen::SinglePlayer => Some(&mut self.sp_cursor),
            Screen::Load | Screen::Save => Some(&mut self.load_cursor),
            Screen::Options => Some(&mut self.page_cursor),
            Screen::OptionsPage(_) => Some(&mut self.opt_cursor),
            Screen::Quit | Screen::None => None,
        }
    }

    /// Mouse hover: moves the cursor onto the item under the pointer.
    /// Returns a sound to play when the selection changed.
    pub fn mouse_move(&mut self, pos: (f32, f32)) -> Option<&'static str> {
        self.mouse = Some(pos);
        let i = self.item_at(pos)?;
        let c = self.cursor_mut()?;
        (*c != i).then(|| {
            *c = i;
            "misc/menu1.wav"
        })
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        let mut out = Vec::new();
        // left click acts on the item under the pointer, never on a stale cursor
        if key == K_MOUSE1 && self.screen != Screen::Quit {
            let Some(pos) = self.mouse else { return out };
            let Some(i) = self.item_at(pos) else { return out };
            if let Some(c) = self.cursor_mut() {
                *c = i;
            }
            if let Screen::OptionsPage(p) = self.screen
                && let Item::Slider { cvar, min, max, step, invert, .. } = page_items(p)[i]
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
                wrap(&mut self.main_cursor, 5, &mut out);
                if enter {
                    out.push(Action::Sound("misc/menu2.wav"));
                    match self.main_cursor {
                        0 => self.screen = Screen::SinglePlayer,
                        2 => self.screen = Screen::Options,
                        4 => self.open(Screen::Quit),
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
                            self.screen = Screen::Load;
                        }
                        _ => {
                            if server_active {
                                out.push(Action::Command("_scansaves\n".into()));
                                self.screen = Screen::Save;
                            }
                        }
                    }
                }
            }
            Screen::Load | Screen::Save => {
                if back {
                    self.screen = Screen::SinglePlayer;
                    return out;
                }
                wrap(&mut self.load_cursor, MAX_SAVEGAMES, &mut out);
                if enter {
                    let i = self.load_cursor;
                    if self.screen == Screen::Load {
                        if self.saves[i].is_some() {
                            self.screen = Screen::None;
                            out.push(Action::Close);
                            out.push(Action::Command(format!("load s{i}\n")));
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
                    self.screen = Screen::Options;
                    return out;
                }
                let items = page_items(p);
                wrap(&mut self.opt_cursor, items.len(), &mut out);
                let dir = if left { -1 } else if right || enter { 1 } else { 0 };
                if dir != 0 && self.adjust(items[self.opt_cursor], dir, enter, cv, &mut out) {
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Screen::Quit => {
                if matches!(key, K_ENTER | K_ABUTTON | K_MOUSE1) || key == b'y' as i32 {
                    out.push(Action::Command("quit\n".into()));
                } else if back || key == b'n' as i32 {
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
        c.center_320x200();
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
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.main_cursor as f32 * 20.0, d);
            }
            Screen::SinglePlayer => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/ttl_sgl.lmp");
                let p = pic("gfx/sp_menu.lmp");
                c.pic(72.0, 32.0, p);
                let d = dot(pic);
                c.pic(54.0, 32.0 + self.sp_cursor as f32 * 20.0, d);
            }
            Screen::Load | Screen::Save => {
                title(c, pic, if self.screen == Screen::Load { "gfx/p_load.lmp" } else { "gfx/p_save.lmp" });
                for (i, s) in self.saves.iter().enumerate() {
                    c.string(16.0, 32.0 + 8.0 * i as f32, s.as_deref().unwrap_or("--- UNUSED SLOT ---"));
                }
                if (realtime * 4.0) as i64 & 1 == 0 {
                    c.ch(8.0, 32.0 + self.load_cursor as f32 * 8.0, 13);
                }
            }
            Screen::Options => {
                let p = pic("gfx/qplaque.lmp");
                c.pic(16.0, 4.0, p);
                title(c, pic, "gfx/p_option.lmp");
                for (i, (label, _)) in PAGES.iter().enumerate() {
                    c.string(VALUE_X - 8.0 * 9.0, ROW0 + i as f32 * 8.0, label);
                }
                if (realtime * 4.0) as i64 & 1 == 0 {
                    c.ch(VALUE_X - 8.0 * 11.0, ROW0 + self.page_cursor as f32 * 8.0, 13);
                }
            }
            Screen::OptionsPage(page) => {
                // no plaque here: long labels need the left margin
                title(c, pic, "gfx/p_option.lmp");
                let name = PAGES.iter().find(|x| x.1 == page).map_or("", |x| x.0);
                let hx = ((320.0 - name.len() as f32 * 8.0) / 2.0).floor();
                for (k, b) in name.bytes().enumerate() {
                    c.ch(hx + k as f32 * 8.0, PAGE_ROW0 - 12.0, b | 128);
                }
                for (i, item) in page_items(page).iter().enumerate() {
                    self.draw_item(c, cv, *item, PAGE_ROW0 + i as f32 * 8.0);
                }
                if (realtime * 4.0) as i64 & 1 == 0 {
                    c.ch(VALUE_X - 8.0, PAGE_ROW0 + self.opt_cursor as f32 * 8.0, 13);
                }
            }
            Screen::Quit => {
                let msg = ["  Really quit?  ", "", " Y, Enter or click ", "   to quit, N or   ", "  Escape/right-click"];
                c.fill(56.0, 76.0, 208.0, 56.0, [0.0, 0.0, 0.0, 0.85]);
                for (i, l) in msg.iter().enumerate() {
                    c.string(64.0, 80.0 + i as f32 * 8.0, l);
                }
            }
            Screen::None => {}
        }
        c.reset_origin();
    }

    fn draw_item(&self, c: &mut Canvas, cv: &Cvars, item: Item, y: f32) {
        let label = |c: &mut Canvas, l: &str| c.string(VALUE_X - 16.0 - l.len() as f32 * 8.0, y, l);
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
