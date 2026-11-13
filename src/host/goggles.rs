//! Host side of the add-on equipment (night-vision / thermal goggles):
//! availability, the equipment wheel, console commands, switch sounds and
//! the per-frame render settings. Player 1 only; split-screen guests share
//! the renderer's post effect, so they see player 1's goggles.

use std::sync::Arc;

use qsound::{Sfx, SoundEvent};

use super::*;
use crate::addons::equip_wheel::EquipWheel;
use crate::addons::goggles::{Equip, Equipment, HeatTable};

/// Player 1's equipment state.
#[derive(Default)]
pub(super) struct Goggles {
    pub(super) equip: Equipment,
    pub(super) wheel: EquipWheel,
    heat: HeatTable,
    /// (device rate, [off, on]) switch cues.
    cues: Option<(u32, [Arc<Sfx>; 2])>,
}

/// (name, default, flags) of the goggle cvars.
pub(super) const CVARS: &[(&str, &str, u32)] = &[
    ("addon_goggles", "1", qcore::cvar::CVAR_ARCHIVE),
    ("addon_goggles_intensity", "1", qcore::cvar::CVAR_ARCHIVE),
    ("addon_goggles_noise", "1", qcore::cvar::CVAR_ARCHIVE),
];

pub(super) fn register_goggle_commands(c: &mut Commands<Host>) {
    // `goggles` toggles night vision; `goggles nv|thermal|off` sets them
    c.add("goggles", |h, a| {
        let arg = a.argv(1);
        if !h.goggles_allowed() {
            h.print("goggles: needs a game started with Add-ons and Goggles enabled\n");
            return;
        }
        match arg {
            "" => h.toggle_equip(Equip::NightVision),
            "off" | "0" => {
                if h.goggles.equip.any() {
                    h.goggles.equip.clear();
                    h.goggle_cue(false);
                }
            }
            s => match Equip::from_key(s) {
                Some(e) if h.goggles.equip.is_on(e) => {}
                Some(e) => h.toggle_equip(e),
                None => h.print("goggles [nv|thermal|off]\n"),
            },
        }
    });
    c.add("equip", |h, a| match Equip::from_key(a.argv(1)) {
        Some(e) if h.goggles_allowed() => h.toggle_equip(e),
        Some(_) => h.print("equip: needs a game started with Add-ons and Goggles enabled\n"),
        None => h.print("equip <nv|thermal> : toggle a piece of equipment\n"),
    });
}

impl Host {
    /// Goggles may be used: an add-on game where toys are allowed, with the
    /// Goggles add-on ticked.
    pub(super) fn goggles_allowed(&self) -> bool {
        if !self.server.core.addons_on || !self.server.active() || self.cv("addon_goggles") == 0.0 {
            return false;
        }
        let all_local = qserver::addons::all_local(&self.server.core.clients);
        qserver::addons::allowed(true, self.cv("deathmatch") != 0.0, all_local)
    }

    pub(super) fn toggle_equip(&mut self, e: Equip) {
        let on = self.goggles.equip.toggle(e);
        self.goggle_cue(on);
        let s = format!("{} {}", e.name(), if on { "on" } else { "off" });
        let (s, t, cols) = self.center_args(&s);
        self.con.center_print(&s, t, cols);
    }

    fn goggle_cue(&mut self, on: bool) {
        let Some(snd) = self.snd.as_ref() else { return };
        let rate = snd.rate();
        if self.goggles.cues.as_ref().is_none_or(|c| c.0 != rate) {
            let mk = |on| Arc::new(crate::addons::goggles::cue(on, rate));
            self.goggles.cues = Some((rate, [mk(false), mk(true)]));
        }
        let sfx = self.goggles.cues.as_ref().unwrap().1[on as usize].clone();
        let origin = self.cl.entities.get(self.cl.viewentity).map_or([0.0; 3], |e| e.origin);
        snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin, volume: 0.7, attenuation: 0.0 });
    }

    /// Per frame: the equipment wheel; everything turns off when goggles
    /// are not allowed (no add-ons, deathmatch, remote client, Goggles off).
    pub(super) fn goggles_frame(&mut self, in_game: bool, alive: bool) {
        let allowed = self.local.is_some() && self.goggles_allowed();
        if !allowed && self.goggles.equip.any() {
            self.goggles.equip.clear();
        }
        let held = self.input.equip_wheel_held();
        let aim = self.input.wheel_aim;
        if let Some(e) = self.goggles.wheel.update(held, allowed && in_game && alive && self.demo.is_none(), aim) {
            self.toggle_equip(e);
        }
    }

    /// The renderer's goggle settings; tags entity heat while thermal is on.
    pub(super) fn goggles_render(&mut self, ents: &mut [qrender::RenderEntity]) -> qrender::Goggles {
        let mode = if self.cl.intermission == 0 { self.goggles.equip.goggles_mode() } else { 0 };
        if mode == 2 {
            self.goggles.heat.refresh(&self.level.mapname, &self.cl.model_precache, &self.level);
            self.goggles.heat.tag(ents);
        }
        qrender::Goggles {
            mode,
            intensity: self.cv("addon_goggles_intensity").clamp(0.25, 3.0),
            noise: self.cv("addon_goggles_noise").clamp(0.0, 2.0),
        }
    }

    pub(super) fn equip_wheel_key(&self) -> i32 {
        crate::menu::keys_for(&self.input.bindings, "+equipwheel")
            .into_iter()
            .find(|&k| crate::glyphs::is_pad_key(k))
            .unwrap_or(qinput::keys::K_YBUTTON)
    }
}
