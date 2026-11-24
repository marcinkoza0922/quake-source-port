//! Routing of the add-on toys' presentation events (portal, railgun,
//! hookshot, cable, boomerang) from the client to their effect owners, and
//! player 1's toy view models.

use super::*;

impl Host {
    /// Hands one toy presentation event to its effect owner.
    pub(super) fn toy_event(&mut self, e: ClientEvent) {
        match e {
            ClientEvent::PortalEffect(fx) => self.portal_fx.receive(fx, self.cl.time, self.snd.as_ref()),
            ClientEvent::RailEffect(fx) => {
                let choice = self.cv("addon_rail_color").max(0.0) as usize;
                self.rail_fx.receive(fx, self.cl.time, choice, self.snd.as_ref());
            }
            ClientEvent::Gun(e) => match qserver::addons::Toy::from_id(e.toy) {
                Some(qserver::addons::Toy::Akimbo) => self.akimbo_fx.receive(&e, self.cl.time, self.snd.as_ref()),
                Some(qserver::addons::Toy::Rifle) => {
                    self.rifle_fx.receive(&e, self.cl.time, self.snd.as_ref());
                    if e.kind == qclient::guns::FIRE {
                        self.rifle_kick(e.owner as usize);
                    }
                }
                _ => {}
            },
            ClientEvent::Hook(h) => self.hook_fx.receive(h, self.cl.time, self.snd.as_ref()),
            ClientEvent::Cable(c) => self.cable_fx.receive(&c, self.snd.as_ref()),
            ClientEvent::Gel(line) => self.gel_fx.receive(&line, self.cl.time, self.snd.as_ref()),
            ClientEvent::Water(line) => self.water_fx.receive(&line, self.cl.time, self.snd.as_ref()),
            ClientEvent::Boomerang(line) => self.boomerang_fx.receive(&line, self.cl.time, &self.cl, self.snd.as_ref()),
            _ => {}
        }
    }

    /// A rifle shot kicks its local shooter's view.
    fn rifle_kick(&mut self, owner: usize) {
        if self.demo.is_some() {
            return;
        }
        let recoil = self.cv("addon_rifle_recoil");
        if owner + 1 == self.cl.viewentity {
            self.ads.kick(recoil);
        } else if let Some(g) = self.guests.iter_mut().find(|g| g.cl.viewentity == owner + 1) {
            g.ads.kick(recoil);
        }
    }

    /// The raised rifle sight of player 1 or a guest in their `view`.
    pub(super) fn rifle_sight(&self, guest: Option<usize>, view: &qrender::View) -> Option<crate::addons::rifle_dot::Sight> {
        let (cl, ads, held) = match guest {
            Some(k) => (&self.guests[k].cl, &self.guests[k].ads, self.guests[k].toy.held),
            None => (&self.cl, &self.ads, self.toy.held),
        };
        let me = cl.viewentity.saturating_sub(1);
        // with the view model hidden there is no sight
        if held != Some(qserver::addons::Toy::Rifle) || ads.amount() <= 0.3 || !self.rifle_fx.placed(me, self.cl.time) {
            return None;
        }
        let dir = qcore::math::angle_vectors(cl.viewangles.into()).0;
        Some(crate::addons::rifle_dot::Sight {
            view: *view,
            angles: cl.viewangles,
            alpha: self.rifle_fx.dot(me, self.cl.time, view.origin.into(), dir),
            colour: self.cv("addon_rifle_dot").max(0.0) as usize,
            pic: self.rifle_fx.pic(),
        })
    }

    /// Drops every toy effect in flight (new map).
    pub(super) fn clear_toy_fx(&mut self) {
        self.portal_fx.clear();
        self.rail_fx.clear();
        self.akimbo_fx.clear();
        self.rifle_fx.clear();
        self.hook_fx.clear();
        self.cable_fx.clear();
        self.boomerang_fx.clear();
        self.gel_fx.clear();
        self.water_fx.clear();
    }

    /// The held toy's view model entities for player 1 (the cable's two
    /// hands and plugs, every other toy's one model); true while the cable
    /// is held.
    pub(super) fn toy_view_models(&mut self, v: &qrender::View, cv: &ViewCvars, ents: &mut Vec<qrender::RenderEntity>) -> bool {
        if self.toy.held == Some(qserver::addons::Toy::PortalGun) && self.cl.intermission == 0 && self.cl.stats[0] > 0
            && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0
            && let Some(mut g) = self.portal_fx.weapon(self.cl.viewentity.saturating_sub(1), self.cl.time, *v,
                &mut self.level, &self.server.core.fs, &mut self.r) {
            g.origin = (Vec3::from(g.origin) + cv.viewmodel.offset(v.angles)).to_array();
            ents.push(g);
        }
        let rail = self.toy.held == Some(qserver::addons::Toy::Railgun) && self.cl.intermission == 0 && self.cl.stats[0] > 0;
        self.rail_fx.hum(self.cl.viewentity.saturating_sub(1), rail, self.snd.as_ref());
        if rail && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0
            && let Some(mut g) = self.rail_fx.weapon(self.cl.viewentity.saturating_sub(1), self.cl.time, *v,
                &mut self.level, &self.server.core.fs, &mut self.r) {
            g.origin = (Vec3::from(g.origin) + cv.viewmodel.offset(v.angles)).to_array();
            ents.push(g);
        }
        if self.toy.held == Some(qserver::addons::Toy::Boomerang) && self.cl.intermission == 0 && self.cl.stats[0] > 0
            && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0
            && let Some(mut g) = self.boomerang_fx.weapon(self.cl.viewentity.saturating_sub(1), self.cl.time, *v,
                &mut self.level, &self.server.core.fs, &mut self.r) {
            g.origin = (Vec3::from(g.origin) + cv.viewmodel.offset(v.angles)).to_array();
            ents.push(g);
        }
        if self.toy.held == Some(qserver::addons::Toy::Hookshot) && self.cl.intermission == 0 && self.cl.stats[0] > 0
            && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0
            && let Some(mut g) = self.hook_fx.weapon(self.cl.viewentity.saturating_sub(1), self.cl.time, *v,
                &mut self.level, &self.server.core.fs, &mut self.r) {
            g.origin = (Vec3::from(g.origin) + cv.viewmodel.offset(v.angles)).to_array();
            ents.push(g);
        }
        let cable_held = self.toy.held == Some(qserver::addons::Toy::Cable) && self.cl.intermission == 0 && self.cl.stats[0] > 0;
        // two-handed, so it ignores the left/right view model alignment
        if cable_held && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0 {
            ents.extend(self.cable_fx.weapon(self.cl.viewentity.saturating_sub(1), &self.cl, self.cl.time, *v, &mut self.level, &self.server.core.fs, &mut self.r));
        }
        if self.toy.held == Some(qserver::addons::Toy::Sticky) && self.cl.intermission == 0 && self.cl.stats[0] > 0
            && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0
            && let Some(mut g) = crate::addons::sticky_draw::weapon(self.cl.time, *v, &mut self.level, &self.server.core.fs, &mut self.r) {
            g.origin = (Vec3::from(g.origin) + cv.viewmodel.offset(v.angles)).to_array();
            ents.push(g);
        }
        let me = self.cl.viewentity.saturating_sub(1);
        let held = |t| self.toy.held == Some(t) && self.cl.intermission == 0 && self.cl.stats[0] > 0
            && cv.viewmodel.visible() && self.cl.items & crate::sbar::IT_INVISIBILITY == 0;
        let toy_gun = if held(qserver::addons::Toy::GelSprayer) {
            self.gel_fx.weapon(me, self.cl.time, *v, &mut self.level, &self.server.core.fs, &mut self.r)
        } else if held(qserver::addons::Toy::Washer) {
            self.water_fx.weapon(me, self.cl.time, *v, &mut self.level, &self.server.core.fs, &mut self.r)
        } else {
            None
        };
        if let Some(mut g) = toy_gun {
            g.origin = (Vec3::from(g.origin) + cv.viewmodel.offset(v.angles)).to_array();
            ents.push(g);
        }
        if held(qserver::addons::Toy::Akimbo) {
            // two-handed: no left/right view model alignment
            ents.extend(self.akimbo_fx.weapons(me, &self.cl, self.cl.time, *v, &mut self.level, &self.server.core.fs, &mut self.r));
        }
        if held(qserver::addons::Toy::Rifle) {
            let hold = crate::addons::rifle_fx::Hold { aim: self.ads.amount(), side: cv.viewmodel.side() };
            ents.extend(self.rifle_fx.weapons(me, hold, &self.cl, self.cl.time, *v, &mut self.level, &self.server.core.fs, &mut self.r));
        }
        cable_held
    }
}
