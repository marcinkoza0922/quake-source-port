//! The Demos screen's glue to the menu: its keys and clicks, and the two
//! dialogs it opens as options pages — Export Video (`menu_export`) and
//! Rename (renaming an auto-demo keeps it, like Keep).

use super::*;
use crate::draw::Canvas;
use crate::menu::{MUTED, TXT};
use qcore::cvar::Cvars;

impl crate::menu::Menu {
    /// Opens the export dialog; Back returns to `back` (the Demos list, or
    /// the demo itself). `info` lines go under the rows.
    pub fn open_export(&mut self, back: Screen, info: Vec<String>, clip: bool) {
        self.export_back = back;
        self.dialog_info = info;
        self.opt_cursor = if clip { EXPORT_CLIP_ROW } else { EXPORT_CLIP_ROW + 1 };
        self.screen = Screen::OptionsPage(Page::Export);
    }

    /// Opens the rename dialog for `file` with `name` filled in.
    fn open_rename(&mut self, file: String, name: &str, auto: bool, cv: &mut Cvars) {
        cv.set("_demo_rename", name);
        self.rename_from = file;
        self.dialog_info = if auto { vec![qcore::tr!("Renaming an automatic demo moves it to the saved demos.")] } else { Vec::new() };
        self.opt_cursor = 0;
        self.screen = Screen::OptionsPage(Page::RenameDemo);
    }

    /// The rename dialog's button (or Enter in its name field).
    pub(in crate::menu) fn run_rename(&mut self, cv: &Cvars, out: &mut Vec<Action>) {
        let name = cv.by_name("_demo_rename").map_or(String::new(), |c| c.string.trim().to_string());
        if name.is_empty() || self.rename_from.is_empty() {
            return;
        }
        self.screen = Screen::Demos;
        out.push(Action::Sound("misc/menu2.wav"));
        out.push(Action::Command(format!("_demo_rename \"{}\" \"{name}\"\n", self.rename_from)));
    }

    /// Lines under the export and rename dialogs' rows.
    pub(in crate::menu) fn draw_dialog_notes(&self, c: &mut Canvas, rows: usize) {
        let mut y = crate::menu::PAGE_ROW0 + rows as f32 * crate::menu::ROW_H + 30.0;
        for l in &self.dialog_info {
            c.text_center(320.0, y, l, TXT, MUTED);
            y += 11.0;
        }
    }

    /// A key on the Demos screen (the wheel is handled before, in `Menu::key`).
    pub(in crate::menu) fn demos_key(&mut self, key: i32, cv: &mut Cvars, out: &mut Vec<Action>) {
        let a = self.demos.key(key);
        self.demo_action(a, cv, out);
    }

    fn demo_action(&mut self, a: DemoAction, cv: &mut Cvars, out: &mut Vec<Action>) {
        match a {
            DemoAction::None => {}
            DemoAction::Moved => out.push(Action::Sound("misc/menu1.wav")),
            DemoAction::Back => {
                self.screen = Screen::Main;
                out.push(Action::Sound("misc/menu3.wav"));
            }
            DemoAction::Play(n) => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("playdemo \"{n}\"\n")));
                out.push(Action::Close);
                self.screen = Screen::None;
            }
            DemoAction::Export(n) => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("menu_export \"{n}\"\n")));
            }
            DemoAction::Keep(n) => {
                out.push(Action::Sound("misc/menu2.wav"));
                out.push(Action::Command(format!("_demo_keep_file \"{n}\"\n")));
            }
            DemoAction::Rename { file, name, auto } => {
                out.push(Action::Sound("misc/menu2.wav"));
                self.open_rename(file, &name, auto, cv);
            }
            DemoAction::Delete(n, shown) => {
                self.confirm(vec![qcore::tr!("Delete {}?", shown), qcore::tr!("This can't be undone.")], qcore::t!("Delete"), format!("_demo_delete \"{n}\"\n"), false);
            }
        }
    }

    /// Left click: Back, the scrollbar (drag), or the list's tabs, sort,
    /// rows and buttons.
    pub(in crate::menu) fn demos_click(&mut self, cv: &mut Cvars, server_active: bool) -> Vec<Action> {
        let Some(pos) = self.mouse else { return Vec::new() };
        if let Some((bx, by, bw, bh)) = self.back_button()
            && (bx..bx + bw).contains(&pos.0)
            && (by..by + bh).contains(&pos.1)
        {
            return self.key(K_ESCAPE, cv, server_active);
        }
        if let Some((x, top, h, ty, th)) = self.demos.scrollbar()
            && (x - 4.0..x + 7.0).contains(&pos.0)
            && (top..top + h).contains(&pos.1)
        {
            let grab = if (ty..ty + th).contains(&pos.1) { pos.1 - ty } else { th / 2.0 };
            self.demos.drag_to(pos.1, grab);
            self.drag = Some(grab);
            return Vec::new();
        }
        let mut out = Vec::new();
        let a = self.demos.click(pos);
        self.demo_action(a, cv, &mut out);
        out
    }
}
