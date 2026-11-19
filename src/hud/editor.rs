//! The HUD editor (Options → HUD → Edit HUD): a dialog that can be moved
//! but not closed, listing every element with its scale and offset, the
//! selected element's settings, and buttons to add (from a tree of kinds),
//! delete, reset and finish. Elements can also be clicked and dragged on
//! screen. It edits the `hud_layout` cvar; the host places the elements
//! each frame (the selected one gives way when they would overlap) and
//! writes the settled offsets back.

use qcore::cvar::Cvars;
use qinput::keys::*;

use super::editor_rows::{self, Prop};
use super::kind::{CATEGORIES, Kind};
use super::layout::Layout;
use super::place::Rect;

pub const CVAR: &str = "hud_layout";
/// 0 modern, 1 classic, 2 custom.
pub const PRESET: &str = "hud_preset";

/// Dialog geometry, in menu units.
pub(super) const W: f32 = 240.0;
pub(super) const TITLE_H: f32 = 14.0;
pub(super) const ROW: f32 = 11.0;
pub(super) const LIST_ROWS: usize = 8;
pub(super) const BTN_H: f32 = 14.0;
/// Buttons, two to a row.
pub(super) const BUTTONS: [&str; 4] = ["Add", "Delete", "Reset to Defaults", "Done"];
/// Add Element tree and position grid popups.
pub(super) const ADD_W: f32 = 150.0;
pub(super) const CELL: (f32, f32) = (18.0, 13.0);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Row {
    Elem(usize),
    /// Button row 0 or 1.
    Buttons(usize),
    Prop(Prop),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum AddRow {
    Cat(usize),
    Kind(Kind),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Popup {
    Add { open: Vec<bool>, cursor: usize },
    /// Anchor grid, cursor 0..9.
    Grid(usize),
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Drag {
    Dialog((f32, f32)),
    /// Element and the pointer's offset from its top left (HUD units).
    Elem(usize, (f32, f32)),
    /// Controller moving an element: index, original `(x, y)` for cancel, and
    /// floating-point screen position in HUD units so stick motion stays smooth.
    Pad { elem: usize, orig: (f32, f32), want: Option<(f32, f32)> },
}

/// Screen sizes: HUD and menu pixels per unit, the menu's centred origin,
/// and the screen in menu and HUD units.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Geo {
    pub hud: f32,
    pub menu: f32,
    pub ox: f32,
    pub oy: f32,
    pub sw: f32,
    pub sh: f32,
    pub vw: f32,
    pub vh: f32,
}

/// Where the dialog's parts are (menu units, screen origin).
pub(super) struct Dims {
    pub x: f32,
    pub y: f32,
    pub list_y: f32,
    pub btn_y: f32,
    pub hdr_y: f32,
    pub props_y: f32,
    pub h: f32,
}

/// What a key or click asks of the menu.
#[derive(Default)]
pub struct Out {
    pub sound: Option<&'static str>,
    pub exit: bool,
}

pub(super) fn sound(s: &'static str) -> Out {
    Out { sound: Some(s), exit: false }
}

#[derive(Default)]
pub struct Editor {
    /// Selected element (layout index).
    pub sel: usize,
    /// True when the last input came from a gamepad (keeps list/options navigation separated).
    pub pad_ui: bool,
    pub(super) cursor: usize,
    /// Column of the focused button.
    pub(super) button: usize,
    pub(super) scroll: usize,
    pub(super) collapsed: bool,
    pub(super) pos: Option<(f32, f32)>,
    pub(super) drag: Option<Drag>,
    pub(super) popup: Option<Popup>,
    pub(super) typing: Option<(Prop, String)>,
    pub(super) confirm_reset: bool,
    /// Pointer in menu units from the screen's top left.
    pub(super) mouse: Option<(f32, f32)>,
    pub(super) rects: Vec<Rect>,
    pub(super) geo: Geo,
}

pub(super) fn layout(cv: &Cvars) -> Layout {
    Layout::parse(cv.by_name(CVAR).map_or("", |c| c.string.as_str()))
}

/// Writes the edited layout; it is now the custom preset.
pub(super) fn save(cv: &mut Cvars, l: &Layout) {
    cv.set(CVAR, &l.to_text());
    cv.set(PRESET, "2");
}

impl Editor {
    /// Starts editing: the first element selected, popups closed.
    pub fn open(&mut self) {
        let pos = self.pos;
        let pad_ui = self.pad_ui;
        *self = Editor { pos, pad_ui, geo: self.geo, ..Default::default() };
    }

    /// The frame's placement and screen, from the host.
    pub fn set_frame(&mut self, rects: &[Rect], hud_scale: f32, (w, h): (u32, u32)) {
        let mut c = crate::draw::Canvas::new(w, h);
        c.menu_layout();
        let (sw, sh) = (w as f32 / c.scale, h as f32 / c.scale);
        self.geo = Geo { hud: hud_scale, menu: c.scale, ox: c.ox, oy: c.oy, sw, sh, vw: w as f32 / hud_scale, vh: h as f32 / hud_scale };
        self.rects.clear();
        self.rects.extend_from_slice(rects);
        let (x, y) = self.pos.unwrap_or((sw - W - 8.0, 8.0));
        self.pos = Some((x.clamp(0.0, (sw - W).max(0.0)), y.clamp(0.0, (sh - 40.0).max(0.0))));
    }

    pub(super) fn rows(&self, l: &Layout) -> Vec<Row> {
        let mut out: Vec<Row> = (0..l.elems.len()).map(Row::Elem).collect();
        out.extend([Row::Buttons(0), Row::Buttons(1)]);
        if let Some(e) = l.elems.get(self.sel) {
            out.extend(editor_rows::props(e.kind).into_iter().map(Row::Prop));
        }
        out
    }

    pub(super) fn dims(&self, nprops: usize) -> Dims {
        let (x, y) = self.pos.unwrap_or((8.0, 8.0));
        let list_y = y + TITLE_H + 2.0;
        let btn_y = list_y + LIST_ROWS as f32 * ROW + 4.0;
        let hdr_y = btn_y + 2.0 * BTN_H + 3.0 + 5.0;
        let props_y = hdr_y + ROW + 1.0;
        let h = if self.collapsed { TITLE_H } else { props_y + nprops as f32 * ROW + 4.0 - y };
        Dims { x, y, list_y, btn_y, hdr_y, props_y, h }
    }

    /// Click target for the hamburger collapse/expand button in the title bar.
    pub(super) fn toggle_rect(d: &Dims) -> Rect {
        Rect { x: d.x + W - 18.0, y: d.y, w: 18.0, h: TITLE_H }
    }

    pub(super) fn toggle_collapse(&mut self) -> Out {
        self.collapsed = !self.collapsed;
        if self.collapsed {
            self.popup = None;
            self.typing = None;
            self.confirm_reset = false;
        }
        sound("misc/menu2.wav")
    }

    pub(super) fn button_rect(d: &Dims, b: usize) -> Rect {
        let bw = (W - 12.0) / 2.0;
        Rect { x: d.x + 4.0 + (b % 2) as f32 * (bw + 4.0), y: d.btn_y + (b / 2) as f32 * (BTN_H + 3.0), w: bw, h: BTN_H }
    }

    fn clamp(&mut self, l: &Layout) {
        self.sel = self.sel.min(l.elems.len().saturating_sub(1));
        let n = self.rows(l).len();
        self.cursor = self.cursor.min(n.saturating_sub(1));
        self.scroll = self.scroll.min(l.elems.len().saturating_sub(LIST_ROWS));
    }

    pub(super) fn select(&mut self, i: usize, l: &Layout) {
        self.sel = i;
        self.cursor = i;
        if i < self.scroll {
            self.scroll = i;
        } else if i >= self.scroll + LIST_ROWS {
            self.scroll = i + 1 - LIST_ROWS;
        }
        self.clamp(l);
    }

    pub(super) fn cursor_to(&mut self, row: Row, l: &Layout) {
        if let Some(i) = self.rows(l).iter().position(|&r| r == row) {
            self.cursor = i;
        }
    }

    /// Moves the cursor a row; landing on an element selects it.
    /// With a gamepad, list and property navigation stay within their section.
    fn move_cursor(&mut self, dir: i32, l: &Layout) -> Out {
        let rows = self.rows(l);
        if self.pad_ui {
            match rows.get(self.cursor).copied() {
                Some(Row::Elem(i)) if !l.elems.is_empty() => {
                    let next = (i as i32 + dir).rem_euclid(l.elems.len() as i32) as usize;
                    self.select(next, l);
                    return sound("misc/menu1.wav");
                }
                Some(Row::Prop(_)) => {
                    let first = rows.iter().position(|r| matches!(r, Row::Prop(_))).unwrap_or(0);
                    let count = rows.len() - first;
                    if count > 0 {
                        let off = (self.cursor - first) as i32;
                        self.cursor = first + (off + dir).rem_euclid(count as i32) as usize;
                        return sound("misc/menu1.wav");
                    }
                }
                _ => {}
            }
        }
        let n = rows.len() as i32;
        self.cursor = (self.cursor as i32 + dir).rem_euclid(n) as usize;
        if let Row::Elem(i) = rows[self.cursor] {
            self.select(i, l);
        }
        sound("misc/menu1.wav")
    }

    pub fn key(&mut self, key: i32, cv: &mut Cvars) -> Out {
        let mut l = layout(cv);
        self.clamp(&l);
        if let Some(t) = self.typing.take() {
            return self.type_key(key, t, &mut l, cv);
        }
        if self.moving() {
            return self.pad_move_key(key, &mut l, cv);
        }
        if key != K_MOUSE1 && key != K_BACK && !matches!(self.rows(&l).get(self.cursor), Some(Row::Buttons(1))) {
            self.confirm_reset = false;
        }
        if self.popup.is_some() {
            return self.popup_key(key, &mut l, cv);
        }
        let row = self.rows(&l).get(self.cursor).copied();
        match key {
            K_ESCAPE => Out { sound: Some("misc/menu3.wav"), exit: true },
            K_BBUTTON => match row {
                Some(Row::Prop(_) | Row::Buttons(_)) => {
                    self.select(self.sel, &l);
                    sound("misc/menu3.wav")
                }
                _ => Out { sound: Some("misc/menu3.wav"), exit: true },
            },
            K_MOUSE1 => self.click(&mut l, cv),
            K_MWHEELUP | K_MWHEELDOWN => {
                let up = key == K_MWHEELUP;
                let d = self.dims(0);
                let over_list = !self.collapsed && self.mouse.is_some_and(|(_, my)| (d.list_y..d.list_y + LIST_ROWS as f32 * ROW).contains(&my));
                if over_list {
                    self.scroll = if up { self.scroll.saturating_sub(1) } else { self.scroll + 1 };
                    self.clamp(&l);
                    Out::default()
                } else {
                    self.move_cursor(if up { -1 } else { 1 }, &l)
                }
            }
            K_UPARROW | K_DPAD_UP | K_KP_UPARROW => self.move_cursor(-1, &l),
            K_DOWNARROW | K_DPAD_DOWN | K_KP_DOWNARROW => self.move_cursor(1, &l),
            K_LEFTARROW | K_DPAD_LEFT | K_RIGHTARROW | K_DPAD_RIGHT => {
                let dir = if matches!(key, K_LEFTARROW | K_DPAD_LEFT) { -1 } else { 1 };
                match row {
                    Some(Row::Buttons(_)) => {
                        self.button = 1 - self.button;
                        sound("misc/menu1.wav")
                    }
                    Some(Row::Prop(Prop::Position)) => self.open_grid(&l),
                    Some(Row::Prop(p)) => self.step(p, dir, &mut l, cv),
                    _ => Out::default(),
                }
            }
            K_ABUTTON => match row {
                Some(Row::Elem(_)) => self.start_move(&l),
                Some(Row::Buttons(r)) => self.press(r * 2 + self.button, &mut l, cv),
                Some(Row::Prop(p @ (Prop::X | Prop::Y))) => self.step(p, 1, &mut l, cv),
                Some(Row::Prop(p)) => self.activate(p, 1, &mut l, cv),
                None => Out::default(),
            },
            K_ENTER | K_KP_ENTER => match row {
                Some(Row::Elem(_)) => self.focus_props(&l),
                Some(Row::Buttons(r)) => self.press(r * 2 + self.button, &mut l, cv),
                Some(Row::Prop(p)) => self.activate(p, 1, &mut l, cv),
                None => Out::default(),
            },
            K_XBUTTON => match row {
                Some(Row::Prop(_)) => {
                    self.select(self.sel, &l);
                    sound("misc/menu1.wav")
                }
                _ => self.focus_props(&l),
            },
            K_DEL | K_LSHOULDER => self.press(1, &mut l, cv),
            K_YBUTTON | K_INS => {
                self.collapsed = false;
                self.press(0, &mut l, cv)
            }
            K_RSHOULDER => self.toggle_collapse(),
            K_BACK => {
                self.collapsed = false;
                self.press(2, &mut l, cv)
            }
            K_START => self.press(3, &mut l, cv),
            K_LTRIGGER | K_RTRIGGER => self.scale_selected(if key == K_LTRIGGER { -1 } else { 1 }, &mut l, cv),
            _ => Out::default(),
        }
    }

    /// Typed text: digits and a minus sign for a coordinate.
    pub fn text(&mut self, ch: char) {
        if let Some((_, s)) = &mut self.typing
            && (ch.is_ascii_digit() || (ch == '-' && s.is_empty()))
            && s.len() < 6
        {
            s.push(ch);
        }
    }

    fn type_key(&mut self, key: i32, (p, mut s): (Prop, String), l: &mut Layout, cv: &mut Cvars) -> Out {
        match key {
            K_BACKSPACE | K_XBUTTON => {
                s.pop();
                self.typing = Some((p, s));
                Out::default()
            }
            K_ESCAPE | K_BBUTTON => sound("misc/menu3.wav"),
            K_ENTER | K_KP_ENTER | K_ABUTTON | K_MOUSE1 => {
                if let (Ok(v), Some(e)) = (s.parse::<f32>(), l.elems.get_mut(self.sel)) {
                    if p == Prop::X { e.x = v } else { e.y = v }
                    save(cv, l);
                }
                if key == K_MOUSE1 {
                    return self.key(key, cv);
                }
                sound("misc/menu2.wav")
            }
            _ => {
                self.typing = Some((p, s));
                Out::default()
            }
        }
    }

    pub(super) fn step(&mut self, p: Prop, dir: i32, l: &mut Layout, cv: &mut Cvars) -> Out {
        let Some(e) = l.elems.get_mut(self.sel) else { return Out::default() };
        if editor_rows::step(p, e, cv, dir) {
            save(cv, l);
        }
        sound("misc/menu3.wav")
    }

    /// Enter or a click on a property row.
    pub(super) fn activate(&mut self, p: Prop, dir: i32, l: &mut Layout, cv: &mut Cvars) -> Out {
        match p {
            Prop::Position => self.open_grid(l),
            Prop::X | Prop::Y => {
                self.typing = Some((p, String::new()));
                sound("misc/menu2.wav")
            }
            _ => self.step(p, dir, l, cv),
        }
    }

    /// Button `b`: 0 Add, 1 Delete, 2 Reset to Defaults, 3 Done.
    pub(super) fn press(&mut self, b: usize, l: &mut Layout, cv: &mut Cvars) -> Out {
        match b {
            0 => {
                self.popup = Some(Popup::Add { open: vec![false; CATEGORIES.len()], cursor: 0 });
                sound("misc/menu2.wav")
            }
            1 => {
                if l.elems.get(self.sel).is_none_or(|e| e.kind.permanent()) {
                    return sound("misc/menu3.wav");
                }
                l.elems.remove(self.sel);
                save(cv, l);
                let row = self.cursor;
                self.clamp(l);
                if row < l.elems.len() {
                    self.select(self.sel, l);
                }
                sound("misc/menu2.wav")
            }
            2 => {
                if self.confirm_reset {
                    cv.set(CVAR, "default");
                    cv.set(PRESET, "0");
                    self.confirm_reset = false;
                    self.sel = 0;
                } else {
                    self.confirm_reset = true;
                }
                sound("misc/menu2.wav")
            }
            _ => Out { sound: Some("misc/menu2.wav"), exit: true },
        }
    }

    /// The element being dragged (the host keeps it where the pointer is).
    pub fn dragging(&self) -> Option<usize> {
        match self.drag {
            Some(Drag::Elem(i, _)) | Some(Drag::Pad { elem: i, .. }) => Some(i),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests;
