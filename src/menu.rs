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
mod enhancement_pages;
use enhancement_pages::*;
mod draw_util;
pub use draw_util::*;
mod osk;
pub use osk::*;
mod osk_draw;
mod draw;
mod options;
mod autoaim;
mod hitfeedback;
use hitfeedback::{HIT_SOUND, KILL_SOUND};
mod input;
mod page_info;
use page_info::*;
mod scroll;
mod prompt;
mod cards;
pub use cards::{SavePb, SaveSlot, SaveSource};
use cards::clean_title;
use prompt::Prompt;
mod navigation;
mod actions;
mod customize;
use customize::{CUSTOMIZE, MODIFIERS, NEW_GAME_CUSTOMIZE};
mod launch;
mod preset;
mod addon_settings;
use addon_settings::{ADDONS, NEW_GAME_ADDONS};
mod addon_browser;
mod addon_preview;
mod addon_bindings;
pub(crate) mod shader;
mod help;
mod search;
mod textedit;
mod search_keywords;
mod dir_browser;
mod weapon_priority;
pub mod demos;

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
    Death,
    Mods,
    ModMaps,
    /// Map Packs > Get more maps (Quaddicted archive).
    ModOnline,
    Multiplayer,
    /// The user manual (main menu → Help).
    Help,
    /// Main menu → Demos: saved and automatic demos.
    Demos,
    /// Options → HUD → Edit HUD.
    HudEditor,
    /// Options → Gameplay → User Data Directory explorer.
    DirBrowser,
}

/// Options sub-pages.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Gameplay,
    Controls,
    Input,
    Graphics,
    Enhancements,
    Audio,
    AudioMix,
    HitSound,
    KillSound,
    Accessibility,
    Hud,
    Crosshair,
    /// Sub-pages reached from an `Item::Sub` row.
    Mouse,
    Controller,
    Gyro,
    Touch,
    Autosave,
    WeaponPriority,
    Automap,
    AutoAim,
    GrenadeArc,
    Water,
    Bloom,
    RtLights,
    Debris,
    Blood,
    Impacts,
    ScreenShader,
    /// Main menu → Customize (a customized game is running).
    Customize,
    /// New Game with Customize ticked: modifiers, add-ons, Start.
    NewGameCustomize,
    /// Gameplay modifiers (reached from either Customize page).
    Modifiers,
    /// Add-ons of the running customized game, and its toy pages.
    Addons,
    /// Add-on picker for the next new game.
    NewGameAddons,
    PortalGun,
    Boomerang,
    Railgun,
    Gel,
    Washer,
    Hookshot,
    Cable,
    Sticky,
    Goggles,
    ProjectileShotguns,
    BulletTime,
    Hitstop,
    HitstopAdvanced,
    /// Options → Demos and its Video Recording page.
    Demos,
    Video,
    /// The demo export dialog.
    Export,
    /// Demos → Rename.
    RenameDemo,
    /// Multiplayer pages (reached from Screen::Multiplayer).
    Join,
    Host,
    Setup,
}

/// Every cvar the options pages edit (the host marks them archived).
pub fn option_cvars() -> impl Iterator<Item = &'static str> {
    [addon_settings::SHOTGUNS, addon_settings::BULLET_TIME, addon_settings::HITSTOP, addon_settings::HITSTOP_ADVANCED, ADDONS, NEW_GAME_ADDONS, PORTAL_GUN, BOOMERANG, GEL, WASHER, HOOKSHOT, CABLE, RAILGUN, STICKY, GOGGLES, GAMEPLAY, AUTOMAP, HUD, CROSSHAIR, ENHANCEMENTS, WATER, BLOOM, RT_LIGHTS, BLOOD, DEBRIS, IMPACTS, AUTOSAVE, MODIFIERS, AUTOAIM, INPUT, MOUSE, CONTROLLER, GYRO, GRAPHICS, AUDIO, AUDIO_MIX, HIT_SOUND, KILL_SOUND, ACCESSIBILITY, demos::DEMOS, demos::VIDEO].into_iter().flatten().filter_map(|i| match *i {
        Item::Slider { cvar, .. } | Item::Toggle { cvar, .. } | Item::Choice { cvar, .. } => Some(cvar),
        _ => None,
    })
}

pub enum Action {
    Command(String),
    Sound(&'static str),
    Close,
}

pub struct Menu {
    pub screen: Screen,
    main_cursor: usize,
    sp_cursor: usize,
    load_cursor: usize,
    /// First card shown on the save/load screens.
    save_scroll: f32,
    opt_cursor: usize,
    /// First visible row of a long options page (follows the cursor).
    opt_scroll: std::cell::Cell<usize>,
    page_cursor: usize,
    /// Quit prompt button: 0 = yes, 1 = no.
    quit_cursor: usize,
    death_cursor: usize,
    /// Display settings being edited on the Graphics page (mode 0..=2, size).
    pub pending_mode: usize,
    pub pending_res: (u32, u32),
    /// Available resolutions (filled by the host) and device MSAA limit.
    pub resolutions: Vec<(u32, u32)>,
    pub max_msaa: u32,
    /// Saves listed on the current screen, newest first (Load: quick and
    /// manual saves; Autosaves: autosaves; Save: everything but autosaves).
    pub saves: Vec<SaveSlot>,
    /// Newest readable save of any kind (pinned at the top of Load).
    pub latest: Option<SaveSlot>,
    pub autosaves: bool,
    /// Modal confirmation or skill picker over the current screen.
    prompt: Option<Prompt>,
    pub save_inventory: crate::save_inventory::Inventory,
    pub save_pb: cards::SavePb,
    prev: Screen,
    /// Cursor position in 640x360 menu units, when the mouse has moved.
    mouse: Option<(f32, f32)>,
    /// Options scrollbar being dragged: pointer offset from the thumb top.
    drag: Option<f32>,
    /// Row of the options slider being dragged with the mouse.
    slider_drag: Option<usize>,
    /// Open dropdown on an options page.
    drop: Option<Drop>,
    /// Page and row an open sub-page was entered from (sub-pages reachable
    /// from several pages return there; otherwise `parent_page`).
    sub_return: Option<(Page, Page, usize)>,
    pub mods: ModMenu,
    pub demos: demos::DemoBrowser,
    /// Options → HUD → Edit HUD.
    pub hud_edit: crate::hud::editor::Editor,
    /// Options → Gameplay → User Data Directory explorer.
    dir_browser: dir_browser::DirBrowser,
    /// Options → Gameplay → Weapon Priority drag-and-drop list.
    wp: weapon_priority::WeaponPriorityUi,
    /// Pending user data directory change: (selected parent, copy existing data).
    pub pending_user_data_dir: Option<(std::path::PathBuf, bool)>,
    /// Export dialog: where Back goes; export and rename dialogs: info lines.
    export_back: Screen,
    dialog_info: Vec<String>,
    /// Rename dialog: the demo being renamed (user-dir relative).
    rename_from: String,
    mp_cursor: usize,
    /// Maps offered on the Host Game page (filled by the host).
    pub net_maps: Vec<qui::mods::MapInfo>,
    /// Slang presets for the Screen Shader page and the loaded preset's
    /// parameters (both filled by the host).
    pub shader_presets: Vec<String>,
    pub shader_params: Vec<qrender::ShaderParam>,
    /// Soundtrack packs `(id, label)` for Options → Audio (filled by the host).
    pub soundtracks: Vec<(String, String)>,
    /// LAN search results and whether a search is running.
    pub servers: Vec<ServerEntry>,
    pub searching: bool,
    /// Key bindings snapshot (host refreshes it while the menu is open).
    pub bindings: Vec<Option<String>>,
    /// Conditional Controls row, refreshed from cvars before draw/input.
    autoaim_conflicts: Vec<i32>,
    autoaim_prompt: bool,
    autoaim_yes: bool,
    autoaim_mapping: std::cell::Cell<u8>,
    /// Controls row waiting for a key to bind.
    capture: Option<usize>,
    addon_ui: addon_browser::Browser,
    bind_pending: Option<(i32, &'static str)>,
    addon_capture: Option<&'static str>,
    /// A game is running (set by the host); modifiers and add-ons are then
    /// fixed per game.
    pub game_running: bool,
    /// The running game is customized (main menu shows Customize).
    pub custom_running: bool,
    /// Editing the Customize pages for the next new game, so per-game rows
    /// stay editable even while a vanilla game runs.
    new_game_setup: bool,
    /// The new game behind the launch pop-up (kept while its Customize
    /// pages are open).
    launch: Option<launch::Launch>,
    /// First run of this game: ask for a graphics preset (`preset.rs`)
    /// the next time the main menu shows.
    pub preset_pending: bool,
    /// Prompts show gamepad glyphs (the pad was used last).
    pub pad_ui: bool,
    pub glyphs: Glyphs,
    /// On-screen keyboard over a text field (gamepad typing).
    osk: Option<Osk>,
    help: help::Help,
    /// Settings search on the Options hub.
    search: search::Search,
    /// Caret and selection of the text row being edited, where its
    /// characters were drawn, and the row a mouse drag is selecting in.
    edit: textedit::Edit,
    text_layouts: std::cell::RefCell<Vec<textedit::TextLayout>>,
    text_drag: Option<usize>,
    /// Shift is held (set by the host before each key; extends selections).
    pub shift: bool,
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
const CARD_X: f32 = 80.0;
const CARD_W: f32 = 480.0;
const CARD_H: f32 = 62.0;
const CARD_Y0: f32 = 40.0;
const CARD_PITCH: f32 = 66.0;
const CARD_BOTTOM: f32 = 338.0;
const CARDS_FULL: usize = 3;
/// Load screen: the newest save is pinned at the top; the rest scroll below
/// a divider starting here.
const CARD_SCROLL_Y0: f32 = CARD_Y0 + CARD_PITCH + 6.0;
/// Pixels scrolled per wheel notch on the card screens.
const CARD_WHEEL: f32 = 24.0;

/// Options panel: background box, label column (right-aligned), value column.
const PANEL_X: f32 = 136.0;
const PANEL_W: f32 = 368.0;
const LABEL_R: f32 = 304.0;
const VALUE_X: f32 = 320.0;
const ROW_H: f32 = 12.0;
/// Height of value boxes inside a row, and of entries in an open dropdown.
const BOX_H: f32 = 10.0;
/// First row of the options hub / of an options page (below its heading).
const ROW0: f32 = 64.0;
const PAGE_ROW0: f32 = 72.0;
/// Rows shown at once on an options page; longer pages scroll.
const PAGE_ROWS: usize = 20;
/// Slider track in options, menu x units (knob is 8 wide).
const SLIDER_X: (f32, f32) = (VALUE_X, VALUE_X + 92.0);
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
            save_scroll: 0.0,
            opt_cursor: 0,
            opt_scroll: Default::default(),
            page_cursor: 0,
            quit_cursor: 0,
            death_cursor: 0,
            pending_mode: 0,
            pending_res: (1280, 720),
            resolutions: Vec::new(),
            max_msaa: 8,
            saves: Vec::new(),
            latest: None,
            autosaves: false,
            prompt: None,
            save_inventory: Default::default(),
            save_pb: Default::default(),
            prev: Screen::None,
            mouse: None,
            drag: None,
            slider_drag: None,
            drop: None,
            sub_return: None,
            mods: ModMenu::default(),
            demos: Default::default(),
            hud_edit: Default::default(),
            dir_browser: Default::default(),
            wp: Default::default(),
            pending_user_data_dir: None,
            export_back: Screen::None,
            dialog_info: Vec::new(),
            rename_from: String::new(),
            mp_cursor: 0,
            net_maps: Vec::new(),
            shader_presets: Vec::new(),
            shader_params: Vec::new(),
            soundtracks: Vec::new(),
            servers: Vec::new(),
            searching: false,
            bindings: vec![None; MAX_KEYS],
            autoaim_conflicts: Vec::new(),
            autoaim_prompt: false,
            autoaim_yes: false,
            autoaim_mapping: std::cell::Cell::new(0),
            capture: None,
            addon_ui: Default::default(),
            bind_pending: None,
            addon_capture: None,
            game_running: false,
            custom_running: false,
            new_game_setup: false,
            launch: None,
            preset_pending: false,
            pad_ui: false,
            glyphs: Glyphs { family: Family::Xbox },
            osk: None,
            help: Default::default(),
            search: Default::default(),
            edit: Default::default(),
            text_layouts: Default::default(),
            text_drag: None,
            shift: false,
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

    /// `menu_help [section]`: opens the manual, optionally at a section
    /// whose title starts with `section` (case-insensitive).
    pub fn open_help(&mut self, section: &str) {
        self.open(Screen::Help);
        self.help.open_section(section);
    }

    /// Opens the User Data Directory explorer at the active location.
    pub fn open_dir_browser(&mut self) {
        self.open(Screen::DirBrowser);
        self.dir_browser.open(crate::userdata::parent());
    }

    /// Typed text goes to a text field under the cursor.
    pub fn text(&mut self, ch: char, cv: &mut Cvars) {
        if self.screen == Screen::HudEditor {
            return self.hud_edit.text(ch);
        }
        if self.drop.is_none() {
            self.text_insert(ch.encode_utf8(&mut [0; 4]), cv);
        }
    }

    /// The cursor is on a text row, which takes typed characters (the host
    /// then turns on text input).
    pub fn wants_text(&self) -> bool {
        if self.screen == Screen::HudEditor {
            return self.osk.is_none() && self.hud_edit.wants_text();
        }
        let Screen::OptionsPage(p) = self.screen else { return false };
        self.drop.is_none() && self.osk.is_none() && self.prompt.is_none() && matches!(self.page_item(p, self.opt_cursor), Item::Text { .. })
    }

    /// Waiting for a key to bind (the host keeps sticks from moving the cursor).
    pub fn capturing(&self) -> bool {
        self.capture.is_some() || self.addon_capture.is_some() || self.bind_pending.is_some()
    }

    pub fn active(&self) -> bool {
        self.screen != Screen::None
    }

    pub fn open(&mut self, s: Screen) {
        self.prev = self.screen;
        self.screen = s;
        self.new_game_setup = matches!(s, Screen::OptionsPage(Page::NewGameAddons | Page::NewGameCustomize));
        if !self.new_game_setup {
            self.launch = None;
        }
        self.drop = None;
        self.drag = None;
        self.slider_drag = None;
        self.text_drag = None;
        self.edit = Default::default();
        self.capture = None;
        self.addon_capture = None;
        self.bind_pending = None;
        self.osk = None;
        self.prompt = None;
        self.quit_cursor = 0;
        self.death_cursor = 0;
        if s == Screen::Options { self.clear_search(); }
        if s == Screen::OptionsPage(Page::WeaponPriority) { self.wp.reset_state(); }
        if matches!(s, Screen::Load | Screen::Save) { self.load_cursor = 0; self.save_scroll = 0.0; }
    }

    /// Card rows: Load = pinned latest + Browse Autosaves + saves; Autosaves
    /// = saves (one placeholder when empty); Save = New save + saves.
    fn save_rows(&self) -> usize {
        if self.screen == Screen::Save { 1 + self.saves.len() }
        else if self.autosaves { self.saves.len().max(1) }
        else { 2 + self.saves.len() }
    }

    /// The save on card row `i`, if that row shows one.
    pub fn save_at(&self, i: usize) -> Option<&SaveSlot> {
        if self.screen == Screen::Save { i.checked_sub(1).and_then(|i| self.saves.get(i)) }
        else if self.autosaves { self.saves.get(i) }
        else if i == 0 { self.latest.as_ref() }
        else { i.checked_sub(2).and_then(|i| self.saves.get(i)) }
    }

    /// Keeps the card cursor on a row after the list changed.
    pub fn clamp_save_cursor(&mut self) {
        self.load_cursor = self.load_cursor.min(self.save_rows() - 1);
        self.fix_card_scroll();
    }

    /// Name for a new save: the first free `sN`.
    pub fn new_save_name(&self) -> String {
        (0..).map(|n| format!("s{n}")).find(|n| !self.saves.iter().any(|s| s.name.eq_ignore_ascii_case(n))).unwrap()
    }

    /// Whether row `i` of the Load list is the Browse Autosaves entry.
    fn is_browse_row(&self, i: usize) -> bool {
        self.screen == Screen::Load && !self.autosaves && i == 1
    }

    /// Item under the menu-space point for the current screen.
    fn item_at(&self, (x, y): (f32, f32)) -> Option<usize> {
        let row = |top: f32, h: f32, n: usize, x0: f32, x1: f32| {
            (x >= x0 && x < x1 && y >= top && y < top + h * n as f32).then(|| ((y - top) / h) as usize)
        };
        let (cx, cy) = CLASSIC;
        match self.screen {
            Screen::Death => row(140.0, 24.0, 4, 160.0, 480.0),
            Screen::Main => row(cy + 32.0, 20.0, self.main_items(), cx + 54.0, cx + 312.0),
            Screen::SinglePlayer => row(cy + 32.0, 20.0, customize::SP_ITEMS, cx + 54.0, cx + 312.0),
            Screen::Load | Screen::Save => self.card_at(x, y),
            Screen::Options if self.searching() && self.search_hits() == 0 => None,
            Screen::Options => row(ROW0, ROW_H, self.hub_rows(), PANEL_X, PANEL_X + PANEL_W).map(|r| r + if self.searching() { self.search_scroll() } else { 0 }),
            Screen::OptionsPage(p) => {
                row(PAGE_ROW0, ROW_H, self.page_vis(p), PANEL_X, PANEL_X + PANEL_W).map(|r| r + self.page_scroll(p))
            }
            Screen::Multiplayer => row(cy + 32.0, 20.0, 3, cx + 54.0, cx + 312.0),
            Screen::Quit => QUIT_BUTTONS
                .iter()
                .position(|&bx| x >= bx && x < bx + QUIT_BTN.0 && y >= QUIT_BTN_Y && y < QUIT_BTN_Y + QUIT_BTN.1),
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => self.mods.item_at(self.mods_page(), (x, y)),
            Screen::Help => self.help.item_at((x, y)),
            Screen::Demos | Screen::None | Screen::HudEditor | Screen::DirBrowser => None,
        }
    }

    fn mods_page(&self) -> modmenu::Page {
        match self.screen {
            Screen::Mods => modmenu::Page::List,
            Screen::ModOnline => modmenu::Page::Online,
            _ => modmenu::Page::Maps,
        }
    }

    fn cursor_mut(&mut self) -> Option<&mut usize> {
        match self.screen {
            Screen::Death => Some(&mut self.death_cursor),
            Screen::Main => Some(&mut self.main_cursor),
            Screen::SinglePlayer => Some(&mut self.sp_cursor),
            Screen::Multiplayer => Some(&mut self.mp_cursor),
            Screen::Load | Screen::Save => Some(&mut self.load_cursor),
            Screen::Options if self.searching() => Some(&mut self.search.cursor),
            Screen::Options => Some(&mut self.page_cursor),
            Screen::OptionsPage(_) => Some(&mut self.opt_cursor),
            Screen::Quit => Some(&mut self.quit_cursor),
            Screen::Mods | Screen::ModMaps | Screen::ModOnline => {
                let p = self.mods_page();
                Some(self.mods.cursor_mut(p))
            }
            // hovering doesn't switch sections; a click does
            Screen::Help => None,
            Screen::Demos | Screen::None | Screen::HudEditor | Screen::DirBrowser => None,
        }
    }

}

#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) mod lang_keys;
