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
        // Temp entities (beam segments) carry no key; match them by model and
        // rounded origin so only those player 1's client lacks are added.
        let tkey = |t: &qclient::TempEntity| (t.model, t.origin.map(|c| c.round() as i32));
        let p1_temps: std::collections::HashSet<_> = self.cl.temp_entities.iter().map(tkey).collect();
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
            let mut gcv = cv;
            if g.toy.held.is_some() { gcv.viewmodel = crate::view::weapon::Alignment::Hidden; }
            let (v, gun) = g.view.calc_refdef(&g.cl, &gcv, &self.level, ft);
            let me = g.cl.viewentity as u32;
            for e in ents.iter_mut().filter(|e| e.key != 0 && e.key == me) {
                e.views = if e.views == 0 { all } else { e.views } & !bit;
            }
            if draw_ents {
                let mut gents = Vec::new();
                g.view.with_self = !g.cl.portals.is_empty();
                g.view.entities(&mut g.cl, &cv, &mut self.level, &self.server.core.fs, &mut self.r, &mut gents);
                // this guest's own body, for its portal views only
                ents.extend(gents.iter().filter(|e| e.portal_only).map(|e| RenderEntity { views: bit, ..*e }));
                gents.retain(|e| !e.portal_only);
                ents.extend(
                    gents
                        .into_iter()
                        .filter(|e| e.key != 0 && !p1_keys.contains(&e.key))
                        .map(|e| RenderEntity { views: bit, ..e }),
                );
                for t in g.cl.temp_entities.iter().filter(|t| !p1_temps.contains(&tkey(t))) {
                    if let Some(m) = self.level.named(&self.server.core.fs, &mut self.r, t.model) {
                        ents.push(RenderEntity { model: m.id, origin: t.origin, angles: t.angles, views: bit, ..Default::default() });
                    }
                }
                if g.toy.held == Some(qserver::addons::Toy::PortalGun) && g.cl.intermission == 0 && g.cl.stats[0] > 0
                    && cv.viewmodel.visible() && g.cl.items & crate::sbar::IT_INVISIBILITY == 0
                    && let Some(mut weapon) = self.portal_fx.weapon(g.cl.viewentity.saturating_sub(1), self.cl.time, v,
                        &mut self.level, &self.server.core.fs, &mut self.r) {
                    weapon.origin = (Vec3::from(weapon.origin) + cv.viewmodel.offset(v.angles)).to_array();
                    ents.push(RenderEntity { views: bit, ..weapon });
                }
                let rail = g.toy.held == Some(qserver::addons::Toy::Railgun) && g.cl.intermission == 0 && g.cl.stats[0] > 0;
                self.rail_fx.hum(g.cl.viewentity.saturating_sub(1), rail, self.snd.as_ref());
                if rail && cv.viewmodel.visible() && g.cl.items & crate::sbar::IT_INVISIBILITY == 0
                    && let Some(mut weapon) = self.rail_fx.weapon(g.cl.viewentity.saturating_sub(1), self.cl.time, v,
                        &mut self.level, &self.server.core.fs, &mut self.r) {
                    weapon.origin = (Vec3::from(weapon.origin) + cv.viewmodel.offset(v.angles)).to_array();
                    ents.push(RenderEntity { views: bit, ..weapon });
                }
                if g.toy.held == Some(qserver::addons::Toy::Hookshot) && g.cl.intermission == 0 && g.cl.stats[0] > 0
                    && cv.viewmodel.visible() && g.cl.items & crate::sbar::IT_INVISIBILITY == 0
                    && let Some(mut weapon) = self.hook_fx.weapon(g.cl.viewentity.saturating_sub(1), self.cl.time, v,
                        &mut self.level, &self.server.core.fs, &mut self.r) {
                    weapon.origin = (Vec3::from(weapon.origin) + cv.viewmodel.offset(v.angles)).to_array();
                    ents.push(RenderEntity { views: bit, ..weapon });
                }
                if g.toy.held == Some(qserver::addons::Toy::Sticky) && g.cl.intermission == 0 && g.cl.stats[0] > 0
                    && cv.viewmodel.visible() && g.cl.items & crate::sbar::IT_INVISIBILITY == 0
                    && let Some(mut weapon) = crate::addons::sticky_draw::weapon(self.cl.time, v, &mut self.level, &self.server.core.fs, &mut self.r) {
                    weapon.origin = (Vec3::from(weapon.origin) + cv.viewmodel.offset(v.angles)).to_array();
                    ents.push(RenderEntity { views: bit, ..weapon });
                }
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
