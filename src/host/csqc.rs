//! Host glue for Client-Side QuakeC (`crate::csqc`).

use super::*;
use crate::csqc::CsqcEnv;

impl Host {
    fn run_csqc<R>(&mut self, f: impl FnOnce(&mut crate::csqc::Csqc, &mut CsqcEnv<'_>) -> R) -> R {
        let (width, height) = self.r.size();
        let not_menu = !self.menu.active() && !self.con.open;
        let mut env = CsqcEnv {
            fs: &self.server.core.fs,
            cvars: &mut self.server.core.cvars,
            cl: &self.cl,
            level: &mut self.level,
            r: &mut self.r,
            snd: self.snd.as_mut(),
            con: &mut self.con,
            cbuf: &mut self.cbuf,
            canvas: None,
            base_view: None,
            is_demo: self.demo.is_some(),
            realtime: self.realtime,
            frametime: 0.0,
            width,
            height,
            virt: [width.max(1) as f32, height.max(1) as f32],
            show_scores: self.show_scores,
            not_menu,
        };
        f(&mut self.csqc, &mut env)
    }

    pub(super) fn load_csqc(&mut self) {
        self.run_csqc(|csqc, env| csqc.load(env));
        self.hud.has_mod_hud = self.csqc.has_simple_hud();
        self.menu.hud_edit.has_mod_hud = self.hud.has_mod_hud;
    }

    pub(super) fn csqc_parse_event(&mut self, data: &[u8]) {
        if self.csqc.active() {
            self.run_csqc(|csqc, env| csqc.parse_event(data, env));
        }
    }

    pub(super) fn csqc_console_command(&mut self, line: &str) -> bool {
        if !self.csqc.active() {
            return false;
        }
        self.run_csqc(|csqc, env| csqc.console_command(line, env))
    }

    pub(super) fn csqc_parse_print(&mut self, msg: &str) -> bool {
        if !self.csqc.active() {
            return false;
        }
        self.run_csqc(|csqc, env| csqc.parse_print(msg, env))
    }

    pub(super) fn csqc_parse_centerprint(&mut self, msg: &str) -> bool {
        if !self.csqc.active() {
            return false;
        }
        self.run_csqc(|csqc, env| csqc.parse_centerprint(msg, env))
    }

    pub(super) fn csqc_parse_stuffcmd(&mut self, msg: &str) -> bool {
        if !self.csqc.active() {
            return false;
        }
        self.run_csqc(|csqc, env| csqc.parse_stuffcmd(msg, env))
    }

    /// Full CSQC (`CSQC_UpdateView`): returns `(draw_engine_sbar, draw_crosshair)`.
    pub(super) fn csqc_update_view(
        &mut self,
        view: &mut Option<qrender::View>,
        ents: &mut Vec<qrender::RenderEntity>,
        dls: &mut Vec<qrender::DLight>,
        canvas: &mut Canvas,
        ft: f32,
    ) -> (bool, bool) {
        let (width, height) = self.r.size();
        let virt = [canvas.vw(), canvas.vh()];
        let not_menu = !self.menu.active() && !self.con.open;
        let base_view = *view;
        let orig_ents = std::mem::take(ents);
        {
            let mut env = CsqcEnv {
                fs: &self.server.core.fs,
                cvars: &mut self.server.core.cvars,
                cl: &self.cl,
                level: &mut self.level,
                r: &mut self.r,
                snd: self.snd.as_mut(),
                con: &mut self.con,
                cbuf: &mut self.cbuf,
                canvas: Some(canvas),
                base_view,
                is_demo: self.demo.is_some(),
                realtime: self.realtime,
                frametime: ft,
                width,
                height,
                virt,
                show_scores: self.show_scores,
                not_menu,
            };
            self.csqc.update_view(&mut env);
        }
        let sc = &self.csqc.state.scene;
        if sc.rendered {
            if let Some(v) = base_view {
                *view = Some(sc.view(v));
            }
            for e in &orig_ents {
                if (e.viewmodel && sc.include_viewmodel) || (!e.viewmodel && sc.include_engine_ents) {
                    ents.push(*e);
                }
            }
            ents.extend_from_slice(&sc.entities);
            dls.extend_from_slice(&sc.dlights);
            (sc.draw_engine_sbar, sc.draw_crosshair)
        } else {
            *ents = orig_ents;
            (true, true)
        }
    }

    /// Simple CSQC HUD (`CSQC_DrawHud` / `CSQC_DrawScores`) drawn as a `Kind::ModHud` widget.
    pub(super) fn csqc_draw_hud_elem(
        &mut self,
        canvas: &mut Canvas,
        ft: f32,
        elem: &crate::hud::layout::Elem,
        rect: crate::hud::place::Rect,
        preview: bool,
    ) {
        let (width, height) = self.r.size();
        let virt = csqc_hud_virt(width, height, elem.scale);
        let not_menu = !preview && !self.menu.active() && !self.con.open;
        let show_scores = if preview { false } else { self.show_scores };
        let start = canvas.list.len();
        let mut env = CsqcEnv {
            fs: &self.server.core.fs,
            cvars: &mut self.server.core.cvars,
            cl: &self.cl,
            level: &mut self.level,
            r: &mut self.r,
            snd: self.snd.as_mut(),
            con: &mut self.con,
            cbuf: &mut self.cbuf,
            canvas: Some(canvas),
            base_view: None,
            is_demo: self.demo.is_some(),
            realtime: self.realtime,
            frametime: ft,
            width,
            height,
            virt,
            show_scores,
            not_menu,
        };
        self.csqc.draw_hud(&mut env);
        let dx = (rect.x - (canvas.vw() - rect.w) / 2.0) * canvas.scale;
        let dy = (rect.bottom() - canvas.vh()) * canvas.scale;
        if dx.abs() > 0.01 || dy.abs() > 0.01 {
            crate::draw::translate(&mut canvas.list[start..], dx, dy);
        }
        let alpha = elem.opacity * if preview && elem.hidden { 0.3 } else { 1.0 };
        if alpha < 0.999 {
            crate::hud::draw::fade_list(&mut canvas.list[start..], alpha);
        }
    }

    /// Intermission CSQC scores (`CSQC_DrawScores`).
    pub(super) fn csqc_draw_scores(&mut self, canvas: &mut Canvas, ft: f32, elem: Option<&crate::hud::layout::Elem>) {
        let (width, height) = self.r.size();
        let scale = elem.map_or(1.0, |e| e.scale);
        let virt = csqc_hud_virt(width, height, scale);
        let not_menu = !self.menu.active() && !self.con.open;
        let start = canvas.list.len();
        let mut env = CsqcEnv {
            fs: &self.server.core.fs,
            cvars: &mut self.server.core.cvars,
            cl: &self.cl,
            level: &mut self.level,
            r: &mut self.r,
            snd: self.snd.as_mut(),
            con: &mut self.con,
            cbuf: &mut self.cbuf,
            canvas: Some(canvas),
            base_view: None,
            is_demo: self.demo.is_some(),
            realtime: self.realtime,
            frametime: ft,
            width,
            height,
            virt,
            show_scores: self.show_scores,
            not_menu,
        };
        self.csqc.draw_scores(&mut env);
        if let Some(e) = elem
            && e.opacity < 0.999
        {
            crate::hud::draw::fade_list(&mut canvas.list[start..], e.opacity);
        }
    }
}

/// Resolution-independent virtual display for Simple CSQC HUDs (~640×360 at 16:9,
/// scaled by the `ModHud` widget's scale), matching vkQuake/ironwail defaults.
fn csqc_hud_virt(width: u32, height: u32, elem_scale: f32) -> [f32; 2] {
    let aspect = (width.max(1) as f32) / (height.max(1) as f32);
    let vh = (360.0 / elem_scale.max(0.25)).max(200.0);
    let vw = (vh * aspect).max(320.0);
    [vw, vh]
}
