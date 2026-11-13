//! Routing of the add-on toys' presentation events (portal, railgun,
//! hookshot, boomerang) from the client to their effect owners.

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
            ClientEvent::Hook(h) => self.hook_fx.receive(h, self.cl.time, self.snd.as_ref()),
            ClientEvent::Boomerang(line) => self.boomerang_fx.receive(&line, self.cl.time, &self.cl, self.snd.as_ref()),
            _ => {}
        }
    }

    /// Drops every toy effect in flight (new map).
    pub(super) fn clear_toy_fx(&mut self) {
        self.portal_fx.clear();
        self.rail_fx.clear();
        self.hook_fx.clear();
        self.boomerang_fx.clear();
    }
}
