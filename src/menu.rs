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
    Quit,
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
    /// Comment line of each save slot (`None` = unused).
    pub saves: Vec<Option<String>>,
    prev: Screen,
}

/// (label, cvar, min, max, step); step 0 = toggle.
const OPTIONS: &[(&str, &str, f32, f32, f32)] = &[
    ("Mouse Speed", "sensitivity", 1.0, 11.0, 0.5),
    ("Invert Mouse", "m_invert", 0.0, 1.0, 0.0),
    ("Sound Volume", "volume", 0.0, 1.0, 0.1),
    ("Music Volume", "bgmvolume", 0.0, 1.0, 0.1),
    ("Field of View", "fov", 60.0, 130.0, 5.0),
    ("Max FPS", "host_maxfps", 60.0, 500.0, 10.0),
    ("Stick Look Yaw", "joy_sensitivity_yaw", 60.0, 480.0, 20.0),
    ("Stick Look Pitch", "joy_sensitivity_pitch", 40.0, 300.0, 10.0),
    ("Invert Stick", "joy_invert", 0.0, 1.0, 0.0),
    ("Aim Assist", "aim_assist", 0.0, 1.0, 0.0),
    ("Aim Slowdown", "aim_slowdown", 0.0, 1.0, 0.1),
    ("Aim Rotation", "aim_rotation", 0.0, 1.0, 0.1),
    ("Gyro", "gyro_enable", 0.0, 1.0, 0.0),
];

impl Menu {
    pub fn new() -> Self {
        Self {
            screen: Screen::None,
            main_cursor: 0,
            sp_cursor: 0,
            load_cursor: 0,
            opt_cursor: 0,
            saves: vec![None; MAX_SAVEGAMES],
            prev: Screen::None,
        }
    }

    pub fn active(&self) -> bool {
        self.screen != Screen::None
    }

    pub fn open(&mut self, s: Screen) {
        self.prev = self.screen;
        self.screen = s;
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        let mut out = Vec::new();
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
                wrap(&mut self.opt_cursor, OPTIONS.len(), &mut out);
                let (_, name, min, max, step) = OPTIONS[self.opt_cursor];
                let dir = if left { -1.0 } else if right || enter { 1.0 } else { 0.0 };
                if dir != 0.0 {
                    let v = cv.value_of(name);
                    let nv = if step == 0.0 { if v != 0.0 { 0.0 } else { 1.0 } } else { (v + dir * step).clamp(min, max) };
                    let s = qcore::cvar::format_float((nv * 1000.0).round() / 1000.0);
                    cv.set(name, &s);
                    out.push(Action::Sound("misc/menu3.wav"));
                }
            }
            Screen::Quit => {
                if matches!(key, K_ENTER | K_ABUTTON) || key == b'y' as i32 {
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
                for (i, &(label, name, min, max, step)) in OPTIONS.iter().enumerate() {
                    let y = 32.0 + i as f32 * 8.0;
                    c.string(16.0 + (17 - label.len()) as f32 * 8.0, y, label);
                    let v = cv.value_of(name);
                    if step == 0.0 {
                        c.string(160.0, y, if v != 0.0 { "on" } else { "off" });
                    } else {
                        let f = ((v - min) / (max - min)).clamp(0.0, 1.0);
                        c.ch(160.0, y, 128);
                        for k in 0..10 {
                            c.ch(168.0 + k as f32 * 8.0, y, 129);
                        }
                        c.ch(248.0, y, 130);
                        c.ch(168.0 + 72.0 * f, y, 131);
                        c.string(260.0, y, &qcore::cvar::format_float((v * 100.0).round() / 100.0));
                    }
                }
                if (realtime * 4.0) as i64 & 1 == 0 {
                    c.ch(152.0, 32.0 + self.opt_cursor as f32 * 8.0, 13);
                }
            }
            Screen::Quit => {
                let msg = ["  Really quit?  ", "", " Press Y or Enter ", "   to quit, N or   ", "   Escape to stay  "];
                c.fill(56.0, 76.0, 208.0, 56.0, [0.0, 0.0, 0.0, 0.85]);
                for (i, l) in msg.iter().enumerate() {
                    c.string(64.0, 80.0 + i as f32 * 8.0, l);
                }
            }
            Screen::None => {}
        }
        c.reset_origin();
    }
}
