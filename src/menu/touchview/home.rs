//! Touch tile menus: the main menu (the Single Player rows inline, with
//! Resume, Save and Load first while a game runs), Single Player and
//! Multiplayer.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Tile {
    Resume,
    Save,
    Load,
    NewGame,
    Levels,
    Options,
    Touch,
    MapPacks,
    Multiplayer,
    Demos,
    Customize,
    Help,
    Quit,
    Join,
    Host,
    Setup,
}

impl Tile {
    fn label(self) -> &'static str {
        match self {
            Tile::Resume => qcore::t!("Resume Game"),
            Tile::Save => qcore::t!("Save Game"),
            Tile::Load => qcore::t!("Load Game"),
            Tile::NewGame => qcore::t!("New Game"),
            Tile::Levels => qcore::t!("Level Select"),
            Tile::Options => qcore::t!("Options"),
            Tile::Touch => qcore::t!("Touch Controls"),
            Tile::MapPacks => qcore::t!("Map Packs"),
            Tile::Multiplayer => qcore::t!("Multiplayer"),
            Tile::Demos => qcore::t!("Demos"),
            Tile::Customize => qcore::t!("Customize"),
            Tile::Help => qcore::t!("Help"),
            Tile::Quit => qcore::t!("Quit"),
            Tile::Join => page_name(Page::Join),
            Tile::Host => page_name(Page::Host),
            Tile::Setup => page_name(Page::Setup),
        }
    }
}

/// Primary and secondary tile heights, and the gap between tiles.
const BIG: f32 = 64.0;
const SMALL_TILE: f32 = 52.0;
const GAP: f32 = 10.0;

impl Menu {
    /// A game (or a demo) is behind the menu.
    fn tv_in_game(&self) -> bool {
        self.game_running || self.touch_shortcut
    }

    /// Main actions first, the rest below.
    pub(super) fn tv_home_tiles(&self) -> (Vec<Tile>, Vec<Tile>) {
        match self.screen {
            Screen::SinglePlayer => {
                let mut big = vec![Tile::NewGame, Tile::Load];
                if self.game_running {
                    big.push(Tile::Save);
                }
                return (big, vec![Tile::Levels]);
            }
            Screen::Multiplayer => return (vec![Tile::Join, Tile::Host, Tile::Setup], Vec::new()),
            _ => {}
        }
        if self.tv_in_game() {
            let mut primary = vec![Tile::Resume];
            if self.game_running {
                primary.push(Tile::Save);
            }
            primary.extend([Tile::Load, Tile::Options]);
            let mut rest = Vec::new();
            if self.touch_shortcut {
                rest.push(Tile::Touch);
            }
            if self.custom_running {
                rest.push(Tile::Customize);
            }
            rest.extend([Tile::NewGame, Tile::Levels, Tile::MapPacks, Tile::Multiplayer, Tile::Demos, Tile::Help, Tile::Quit]);
            (primary, rest)
        } else {
            (
                vec![Tile::NewGame, Tile::Load, Tile::Options, Tile::MapPacks],
                vec![Tile::Levels, Tile::Multiplayer, Tile::Demos, Tile::Help, Tile::Quit],
            )
        }
    }

    /// Tiles and their rectangles (scrolled).
    pub(super) fn tv_home_rects(&self) -> Vec<(Tile, Rect, bool)> {
        let (x0, cw) = self.tv_col();
        let (primary, rest) = self.tv_home_tiles();
        let mut out = Vec::new();
        let mut y = BAR_H + 4.0 - self.tv_scroll();
        for (tiles, h, big) in [(primary, BIG, true), (rest, SMALL_TILE, false)] {
            let cols = if big {
                tiles.len().min(if cw >= 600.0 { 4 } else { 2 })
            } else if cw >= 480.0 {
                3
            } else {
                2
            }
            .max(1);
            let tw = (cw - GAP * (cols - 1) as f32) / cols as f32;
            for (k, t) in tiles.iter().enumerate() {
                let (col, row) = (k % cols, k / cols);
                out.push((*t, [x0 + col as f32 * (tw + GAP), y + row as f32 * (h + GAP), tw, h], big));
            }
            y += tiles.len().div_ceil(cols) as f32 * (h + GAP) + 6.0;
        }
        out
    }

    pub(super) fn tv_home_h(&self) -> f32 {
        let scroll = self.tv_scroll();
        self.tv_home_rects().iter().map(|(_, r, _)| r[1] + r[3] + scroll - BAR_H).fold(0.0, f32::max) + 4.0
    }

    pub(super) fn tv_draw_home(&self, c: &mut Canvas, pic: &mut PicFn) {
        let (w, h) = self.tv_size();
        c.clip_rows(Some((BAR_H, h)));
        for (t, r, big) in self.tv_home_rects() {
            let look = if self.tv_pressed(r) && self.screen != Screen::Quit {
                Look::Pressed
            } else if big {
                Look::Primary
            } else {
                Look::Plain
            };
            tv_button(c, r, t.label(), look);
        }
        c.clip_rows(None);
        // the classic title plaque, scaled up to the bar
        let plaque = match self.screen {
            Screen::SinglePlayer => "gfx/ttl_sgl.lmp",
            Screen::Multiplayer => "gfx/p_multi.lmp",
            _ => "gfx/ttl_main.lmp",
        };
        if let Some(p @ (_, pw, ph)) = pic(plaque) {
            let k = ((BAR_H - 12.0) / ph as f32).min(2.0);
            let (dw, dh) = (pw as f32 * k, ph as f32 * k);
            c.pic_stretch_px([((w - dw) / 2.0 + c.ox) * c.scale, (6.0 + c.oy) * c.scale, dw * c.scale, dh * c.scale], p.0);
        }
    }

    pub(super) fn tv_home_click(&mut self, pos: (f32, f32), cv: &mut Cvars, server_active: bool, out: &mut Vec<Action>) {
        let Some((t, ..)) = self.tv_home_rects().into_iter().find(|(_, r, _)| inside(*r, pos)) else { return };
        out.push(Action::Sound("misc/menu2.wav"));
        match t {
            Tile::Resume => {
                self.screen = Screen::None;
                out.push(Action::Close);
            }
            Tile::Save | Tile::Load => {
                let home = self.screen == Screen::Main;
                self.sp_select(if t == Tile::Save { 2 } else { 1 }, cv, server_active, out);
                self.cards_from_home = home && matches!(self.screen, Screen::Load | Screen::Save);
            }
            Tile::NewGame => self.sp_select(0, cv, server_active, out),
            Tile::Levels => self.sp_select(3, cv, server_active, out),
            Tile::Options => self.open(Screen::Options),
            Tile::Touch => self.open_touch_from_main(),
            Tile::MapPacks => {
                out.push(Action::Command("_mods\n".into()));
                self.screen = Screen::Mods;
            }
            Tile::Multiplayer => self.screen = Screen::Multiplayer,
            Tile::Demos => out.push(Action::Command("menu_demos\n".into())),
            Tile::Customize => self.open_customize(false),
            Tile::Help => self.open(Screen::Help),
            Tile::Quit => self.open(Screen::Quit),
            Tile::Join | Tile::Host | Tile::Setup => {
                // the classic row's Enter (Host asks for the map list)
                self.mp_cursor = [Tile::Join, Tile::Host, Tile::Setup].iter().position(|&x| x == t).unwrap_or(0);
                out.pop();
                out.extend(self.key(K_ENTER, cv, server_active));
            }
        }
    }
}
