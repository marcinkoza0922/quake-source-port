//! The HUD each frame (`crate::hud`): reading `hud_layout` (made from the
//! old HUD settings the first time), placing its elements, telling the
//! console and captions where the messages go, and what the elements show.
//! While the HUD editor is open the settled positions are written back and
//! the HUD is drawn as a preview with a sample player.

use super::*;
use crate::hud::draw::{Ctx, Room};
use crate::hud::editor::{CVAR, PRESET};
use crate::hud::kind::Kind;
use crate::hud::layout::Layout;
use crate::hud::{MsgPlace, place};

/// What the elements show this frame, besides the player.
pub(super) struct HudFrame {
    pub room: Room,
    pub edit: bool,
    pub info: crate::sbar::time::LevelInfo,
    pub metrics: [String; 6],
    pub pb_full: bool,
    pub pb_color: [f32; 4],
    /// The editor's sample player.
    pub sample: Option<qclient::ClientState>,
}

impl HudFrame {
    /// Drawing context for `cl` (the editor's sample player when editing).
    pub fn ctx<'a>(&'a self, cl: &'a qclient::ClientState, face_anim: bool, teamplay: f32, addons: &'a [crate::wheel::Entry], held: Option<qserver::addons::Toy>, show_scores: bool) -> Ctx<'a> {
        Ctx {
            cl: self.sample.as_ref().unwrap_or(cl),
            pack: self.room.pack,
            face_anim,
            teamplay,
            addon_weapons: addons,
            held_addon: held,
            info: &self.info,
            metrics: &self.metrics,
            pb_full: self.pb_full,
            pb_color: self.pb_color,
            show_scores: show_scores && !self.edit,
            preview: self.edit,
        }
    }
}

pub(super) fn sizes(c: &Canvas, l: &Layout, room: &Room) -> Vec<(f32, f32)> {
    l.elems.iter().map(|e| crate::hud::draw::size(c, e, room)).collect()
}

impl Host {
    /// HUD units at the bottom of the screen a full-width status bar takes
    /// (the 3D view ends above it).
    pub(super) fn hud_reserve(&self) -> f32 {
        let (w, h) = self.r.size();
        self.hud.reserve_bottom(Canvas::new(w, h).vh())
    }

    /// Picking the modern or classic preset rebuilds the layout from it.
    fn hud_preset(&mut self) {
        if !self.cbuf.is_empty() {
            return;
        }
        let p = self.cv(PRESET);
        if self.hud.preset.is_some_and(|old| old != p) {
            match p as i32 {
                0 => self.server.core.cvars.set(CVAR, "default"),
                1 => self.server.core.cvars.set(CVAR, "classic"),
                _ => None,
            };
        }
        self.hud.preset = Some(p);
    }

    /// Reads `hud_layout`; empty (first run), "default" or "classic" (the
    /// presets) becomes a layout here, where elements can be measured.
    fn hud_sync(&mut self, c: &Canvas, room: &Room) {
        let text = self.cvs(CVAR).to_string();
        // configs still being read could hold the old settings
        if text.is_empty() && !self.cbuf.is_empty() {
            return;
        }
        let stacked = |mut l: Layout| {
            let s = sizes(c, &l, room);
            place::stack(&mut l.elems, &s);
            l.to_text()
        };
        let t = match text.as_str() {
            "default" => stacked(Layout::default_layout()),
            "classic" => stacked(Layout::classic_layout()),
            _ if self.hud.sync(&text) => {
                // a saved layout that is not its preset's (saved before
                // presets existed) is custom
                if !self.hud.preset_checked && self.cbuf.is_empty() {
                    self.hud.preset_checked = true;
                    let preset = match self.cv(PRESET) as i32 {
                        0 => Some(Layout::default_layout()),
                        1 => Some(Layout::classic_layout()),
                        _ => None,
                    };
                    if preset.is_some_and(|l| stacked(l) != text) {
                        self.server.core.cvars.set(PRESET, "2");
                        self.hud.preset = Some(2.0);
                    }
                }
                return;
            }
            _ => {
                // first run: the old settings, and the preset they match
                let cv = &self.server.core.cvars;
                let t = stacked(Layout::from_legacy(&|n| cv.value_of(n)));
                let preset = if t == stacked(Layout::default_layout()) {
                    "0"
                } else if t == stacked(Layout::classic_layout()) {
                    "1"
                } else {
                    "2"
                };
                self.server.core.cvars.set(PRESET, preset);
                self.hud.preset = Some(self.cv(PRESET));
                self.hud.preset_checked = true;
                t
            }
        };
        self.server.core.cvars.set(CVAR, &t);
        self.hud.sync(&t);
    }

    /// Lays the HUD out on `c` (the whole screen) for this frame.
    pub(super) fn hud_frame(&mut self, c: &Canvas, addons: &[crate::wheel::Entry]) -> HudFrame {
        let pack = crate::sbar::Pack::from_game_dirs(self.server.core.fs.game_dirs().iter().map(String::as_str));
        let room = Room { pack, addons: addons.len(), vw: c.vw() };
        self.hud_preset();
        self.hud_sync(c, &room);
        let edit = self.menu.screen == crate::menu::Screen::HudEditor;
        let (vw, vh) = (c.vw(), c.vh());
        let s = sizes(c, &self.hud.layout, &room);
        let edited = self.menu.hud_edit.dragging().unwrap_or(self.menu.hud_edit.sel);
        self.hud.place(s, vw, vh, edit.then_some(edited));
        if edit {
            if let Some(l) = self.hud.settled(vw, vh) {
                let t = l.to_text();
                self.server.core.cvars.set(CVAR, &t);
                self.hud.sync(&t);
            }
            let (w, h) = self.r.size();
            self.menu.hud_edit.set_frame(&self.hud.rects, c.scale, (w, h));
        }
        let place = |k| self.hud.rect_of(k).map(|(e, r)| MsgPlace::of(e, r)).unwrap_or_default();
        (self.con.center_place, self.con.notify_place) = (place(Kind::Center), place(Kind::Notify));
        let want_pb = edit || self.hud.layout.elems.iter().any(|e| e.kind == Kind::Pb);
        HudFrame {
            room,
            edit,
            info: self.level_info(want_pb),
            metrics: self.metric_values(),
            pb_full: self.cv("hud_pb_100") != 0.0,
            pb_color: crate::reticle::color_of(&self.server.core.cvars, "hud_pb_color"),
            sample: edit.then(|| crate::hud::sample_client(&self.cl)),
        }
    }

    /// Where the captions go: their centre, bottom, text size and colour.
    pub(super) fn caption_place(&self) -> Option<((f32, f32), f32, [f32; 4])> {
        let (e, r) = self.hud.rect_of(Kind::Captions)?;
        Some(((r.x + r.w / 2.0, r.bottom()), (e.size / 8.0).round().clamp(1.0, 3.0), crate::hud::draw::value_color(e)))
    }
}
