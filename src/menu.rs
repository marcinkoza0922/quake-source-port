//! Basic menus (menu.c subset): main, single player, load/save, options,
//! quit, plus the mod menu entry (`crate::modmenu`).

use qcore::cvar::Cvars;
use qinput::keys::*;

use crate::draw::Canvas;
use crate::glyphs::{Family, Glyphs, is_pad_key};
use crate::modmenu::{self, ModMenu};
use crate::sbar::PicFn;
mod pages;
pub use pages::*;
mod draw_util;
pub use draw_util::*;
mod osk;
pub use osk::*;
mod osk_draw;
mod draw;
mod options;
mod input;
mod page_info;
use page_info::*;
mod scroll;
mod actions;

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
    Accessibility,
    Hud,
    /// Sub-pages reached from an `Item::Sub` row.
    Mouse,
    Controller,
    Autosave,
    Assists,
    HudLayout,
    Messages,
    Water,
    /// Multiplayer pages (reached from Screen::Multiplayer).
    Join,
    Host,
    Setup,
}

/// Every cvar the options pages edit (the host marks them archived).
pub fn option_cvars() -> impl Iterator<Item = &'static str> {
    [GAMEPLAY, HUD, HUD_LAYOUT, MESSAGES, WATER, AUTOSAVE, ASSISTS, INPUT, MOUSE, CONTROLLER, GRAPHICS, AUDIO, ACCESSIBILITY].into_iter().flatten().filter_map(|i| match *i {
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
    /// First card shown on the save/load screens.
    save_scroll: usize,
    opt_cursor: usize,
    /// First visible row of a long options page (follows the cursor).
    opt_scroll: std::cell::Cell<usize>,
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
    /// Options scrollbar being dragged: pointer offset from the thumb top.
    drag: Option<f32>,
    /// Open dropdown on an options page.
    drop: Option<Drop>,
    /// Page and row an open sub-page was entered from (sub-pages reachable
    /// from several pages return there; otherwise `parent_page`).
    sub_return: Option<(Page, Page, usize)>,
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
/// Save/load cards: left edge, size, first top, pitch, clip bottom (above
/// the hint row) and how many fit whole (the next one shows cut off).
const CARD_X: f32 = 24.0;
const CARD_W: f32 = 592.0;
const CARD_H: f32 = 62.0;
const CARD_Y0: f32 = 40.0;
const CARD_PITCH: f32 = 66.0;
const CARD_BOTTOM: f32 = 338.0;
const CARDS_FULL: usize = 4;

/// Options panel: background box, label column (right-aligned), value column.
const PANEL_X: f32 = 136.0;
const PANEL_W: f32 = 368.0;
const LABEL_R: f32 = 304.0;
const VALUE_X: f32 = 320.0;
const ROW_H: f32 = 12.0;
/// Height of value boxes inside a row, and of entries in an open dropdown.
const BOX_H: f32 = 10.0;
/// Clickable Back button, menu units.
const BACK_BTN: (f32, f32, f32, f32) = (12.0, 341.0, 60.0, 16.0);
/// First row of the options hub / of an options page (below its heading).
const ROW0: f32 = 64.0;
const PAGE_ROW0: f32 = 72.0;
/// Rows shown at once on an options page; longer pages scroll.
const PAGE_ROWS: usize = 20;
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
            save_scroll: 0,
            opt_cursor: 0,
            opt_scroll: Default::default(),
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
            drag: None,
            drop: None,
            sub_return: None,
            mods: ModMenu::default(),
            mp_cursor: 0,
            net_maps: Vec::new(),
            servers: Vec::new(),
            searching: false,
            bindings: vec![None; MAX_KEYS],
            capture: None,
            pad_ui: false,
            glyphs: Glyphs { family: Family::Xbox },
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
        self.drag = None;
        self.capture = None;
        self.osk = None;
        self.quit_cursor = 0;
        if matches!(s, Screen::Load | Screen::Save) { self.load_cursor = 0; self.save_scroll = 0; }
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
                let row = ((y - CARD_Y0) / CARD_PITCH).floor();
                let inside = (CARD_X..CARD_X + CARD_W).contains(&x)
                    && (CARD_Y0..CARD_BOTTOM).contains(&y)
                    && y - CARD_Y0 - row * CARD_PITCH < CARD_H;
                let i = self.save_scroll + row as usize;
                (inside && i < self.save_rows()).then_some(i)
            },
            Screen::Options => row(ROW0, ROW_H, PAGES.len(), PANEL_X, PANEL_X + PANEL_W),
            Screen::OptionsPage(p) => {
                row(PAGE_ROW0, ROW_H, self.page_vis(p), PANEL_X, PANEL_X + PANEL_W).map(|r| r + self.page_scroll(p))
            }
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

}

#[cfg(test)]
mod tests;
