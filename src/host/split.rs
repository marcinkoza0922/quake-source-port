//! Split-screen views: one camera, entity set and polyblend per local player.

use super::*;
use qrender::{RenderEntity, SplitView};

impl Host {
    /// Join or drop split-screen guests to match `cl_splitscreen` once
    /// player 1 is on the local server.
    pub(super) fn sync_guests(&mut self) {
        let on = self.local.is_some_and(|i| self.server.core.clients.get(i).is_some_and(|c| c.active));
        if !on {
            return;
        }
        let mut want = self.cv("cl_splitscreen").max(1.0) as usize;
        if self.server.core.maxclients() == 1 && want > 1 {
            // Single player after a split-screen game: back to one player.
            self.server.core.cvars.set("cl_splitscreen", "1");
            want = 1;
        }
        if want == 1 && self.guests.is_empty() {
            return;
        }
        let warn = crate::splitscreen::sync(&mut self.guests, &mut self.server, want);
        for g in self.guests.iter_mut().filter(|g| g.input.bindings.iter().all(Option::is_none)) {
            // Guests share player 1's binds (gamepad binds in practice).
            g.input.bindings = self.input.bindings.clone();
            g.input.gamepad_connected = true;
        }
        if let Some(w) = warn {
            self.con.print(&w);
            // Don't retry every frame: settle on what fits.
            let n = (self.guests.len() + 1).to_string();
            self.server.core.cvars.set("cl_splitscreen", &n);
        }
        self.drain_server_log();
    }

    /// One view per local player once a guest is in the game. `ents` holds
    /// player 1's entities (drawn in every view); entities only a guest's
    /// client knows about (outside player 1's PVS, or player 1's own body)
    /// are added for that guest's view only, and each player's own body is
    /// hidden from their own view. Returns nothing without live guests.
    pub(super) fn split_views(
        &mut self,
        p1: qrender::View,
        p1_blend: [f32; 4],
        ft: f32,
        ents: &mut Vec<RenderEntity>,
    ) -> Vec<SplitView> {
        let live: Vec<usize> = (0..self.guests.len()).filter(|&k| self.guests[k].cl.signon == 4).collect();
        if live.is_empty() {
            return Vec::new();
        }
        let n = live.len() + 1;
        let all = ((1u16 << n) - 1) as u8;
        let (w, h) = self.r.size();
        let rects = crate::splitscreen::layout(n, w, h, self.cv("r_splitscreen_layout") != 0.0);
        // The classic bar's reserved strip is sized for the whole screen.
        let mut out = vec![SplitView { view: qrender::View { reserve_bottom: 0, ..p1 }, rect: rects[0], blend: p1_blend }];
        for e in ents.iter_mut().filter(|e| e.viewmodel) {
            e.views = 1;
        }
        let p1_keys: std::collections::HashSet<u32> = ents.iter().map(|e| e.key).filter(|&k| k != 0).collect();
        let cv = self.view_cvars();
        let draw_ents = self.cv("r_drawentities") != 0.0;
        for (vi, &k) in live.iter().enumerate() {
            let bit = 1u8 << (vi + 1);
            let g = &mut self.guests[k];
            for ev in std::mem::take(&mut g.view_events) {
                match ev {
                    ClientEvent::Damage { armor, blood, from } => g.view.parse_damage(&g.cl, &cv, armor, blood, from),
                    ClientEvent::BonusFlash => g.view.bonus_flash(),
                    _ => {}
                }
            }
            let (v, gun) = g.view.calc_refdef(&g.cl, &cv, &self.level, ft);
            let me = g.cl.viewentity as u32;
            for e in ents.iter_mut().filter(|e| e.key != 0 && e.key == me) {
                e.views = if e.views == 0 { all } else { e.views } & !bit;
            }
            if draw_ents {
                let mut gents = Vec::new();
                g.view.entities(&mut g.cl, &cv, &mut self.level, &self.server.core.fs, &mut self.r, &mut gents);
                ents.extend(
                    gents
                        .into_iter()
                        .filter(|e| e.key != 0 && !p1_keys.contains(&e.key))
                        .map(|e| RenderEntity { views: bit, ..e }),
                );
                if let Some(gun) = gun {
                    ents.push(RenderEntity { views: bit, ..gun });
                }
            }
            let contents = self.contents(v.origin);
            let g = &mut self.guests[k];
            let blend = g.view.blend(&g.cl, contents, ft, cv.flash);
            out.push(SplitView { view: v, rect: rects[vi + 1], blend });
        }
        out
    }
}
