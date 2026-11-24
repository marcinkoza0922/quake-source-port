//! Visible weapons (`cl_vwep`): players seen in third person, in split
//! screen, in portals or across the network hold the weapon they actually
//! use instead of player.mdl's generic gun. The gun and axe come off the
//! body (see [`rig`]); the axe is shown as the model's own, every other
//! weapon as its pickup (or view) model fitted to the hand, baked into a
//! model with the player's frames.
//!
//! Which weapon: on a local server each player's `weaponmodel`; otherwise
//! the own player's weapon stat and, for other players, their last fire
//! sound when it agrees with the animation (axe or gun frames), else the
//! stock model. Death frames keep the last weapon, since QC clears
//! `weaponmodel` on death.

use std::collections::HashMap;

use qclient::ClientState;
use qcore::fs::Fs;
use qformats::Mdl;
use qnet::protocol::STAT_WEAPON;
use crate::gfx::Renderer;
use qrender::{ModelId, RenderEntity};

use crate::assets::{Level, md3};

mod classic;
mod enhanced;
pub mod rig;
pub mod table;
#[cfg(test)]
mod tests;

use rig::{Rig, Spec};
use table::Held;

const PLAYER: &str = "progs/player.mdl";

struct Loaded {
    rig: Rig,
    body: ModelId,
    body_axe: ModelId,
}

/// The level's rigs (classic and enhanced player model) and baked weapon
/// models; they live as long as the level's renderer models.
#[derive(Default)]
pub struct Cache {
    rigs: HashMap<bool, Option<Loaded>>,
    guns: HashMap<(bool, String), Option<ModelId>>,
}

impl Cache {
    fn load(enhanced: bool, fs: &Fs, r: &mut Renderer) -> Option<Loaded> {
        let player = Mdl::parse(&fs.open(PLAYER)?).ok()?;
        if enhanced {
            let md5 = md3::parse_md5(fs, PLAYER)?;
            let (rig, body, body_axe) = enhanced::rig(&md5, &player)?;
            let skins = md3::md5_skins(fs, r, PLAYER, &md5, false);
            Some(Loaded { rig, body: r.load_md3(&body, &skins).ok()?, body_axe: r.load_md3(&body_axe, &skins).ok()? })
        } else {
            let (rig, body, body_axe) = classic::rig(&player)?;
            Some(Loaded { rig, body: r.load_alias(&body).ok()?, body_axe: r.load_alias(&body_axe).ok()? })
        }
    }

    /// The rig of the classic or the enhanced (re-release MD5) player model.
    fn rig(&mut self, enhanced: bool, fs: &Fs, r: &mut Renderer) -> Option<&Loaded> {
        self.rigs.entry(enhanced).or_insert_with(|| Self::load(enhanced, fs, r)).as_ref()
    }

    /// The weapon model per `spec` for the classic or enhanced body; the
    /// enhanced body holds enhanced pickups where the game has them.
    fn gun(&mut self, enhanced: bool, spec: &Spec, fs: &Fs, r: &mut Renderer) -> Option<ModelId> {
        let key = (enhanced, format!("{}|{}|{:?}", spec.source, spec.scale, spec.offset));
        if let Some(id) = self.guns.get(&key) {
            return *id;
        }
        let rig = &self.rigs.get(&enhanced)?.as_ref()?.rig;
        let md5 = enhanced.then(|| md3::parse_md5(fs, &spec.source)).flatten();
        let id = match md5 {
            Some(md5) => enhanced::bake(rig, &md5.md3, spec).and_then(|m| {
                let skins = md3::md5_skins(fs, r, &spec.source, &md5, false);
                r.load_md3(&m, &skins).ok()
            }),
            None => fs
                .open(&spec.source)
                .and_then(|d| Mdl::parse(&d).ok())
                .and_then(|src| classic::bake(rig, &src, spec))
                .and_then(|m| r.load_alias(&m).ok()),
        };
        self.guns.insert(key, id);
        id
    }
}

/// Per-view state: what each player slot holds.
#[derive(Default)]
pub struct VWep {
    /// `cl_vwep`.
    pub enabled: bool,
    /// Each player slot's `weaponmodel` on the local server (None: not
    /// playing on one).
    pub server: Option<Vec<String>>,
    /// Remote players: the weapon of the last fire sound heard.
    heard: Vec<Option<&'static str>>,
    /// The last weapon each slot was seen with (shown through death frames).
    last: Vec<Option<Held>>,
}

impl VWep {
    pub fn reset(&mut self) {
        self.heard.clear();
        self.last.clear();
    }

    /// A sound started on entity `ent`.
    pub fn sound(&mut self, cl: &ClientState, ent: i32, sound: usize) {
        if ent < 1 || ent as usize > cl.maxclients {
            return;
        }
        if let Some(w) = cl.sound_precache.get(sound).and_then(|n| table::from_sound(n)) {
            let slot = ent as usize - 1;
            if self.heard.len() <= slot {
                self.heard.resize(slot + 1, None);
            }
            self.heard[slot] = Some(w);
        }
    }

    fn resolve(&mut self, idx: usize, cl: &ClientState, rig: &Rig, frame: u32, fs: &Fs) -> Option<Held> {
        let slot = idx - 1;
        if self.last.len() <= slot {
            self.last.resize(slot + 1, None);
        }
        if self.heard.len() <= slot {
            self.heard.resize(slot + 1, None);
        }
        let death = rig.death_frame(frame);
        let guessed = self.server.is_none() && idx != cl.viewentity;
        let wm = match &self.server {
            Some(s) => s.get(slot).map(String::as_str),
            None if idx == cl.viewentity => cl.model_name(cl.stats[STAT_WEAPON] as usize),
            None => self.heard[slot],
        };
        let held = wm
            .and_then(|w| table::lookup(w, |p| fs.exists(p)))
            // a guess must agree with the animation
            .filter(|h| !guessed || (*h == Held::Axe) == rig.axe_frame(frame));
        if death && guessed {
            // whatever they respawn with is unknown until they fire
            self.heard[slot] = None;
        }
        match held {
            Some(h) if !death || !guessed => {
                self.last[slot] = Some(h.clone());
                Some(h)
            }
            _ if death => self.last[slot].clone(),
            _ => None,
        }
    }

    /// Player `idx`'s body was just pushed onto `out`: give it the weapon
    /// it holds.
    pub fn attach(&mut self, idx: usize, cl: &ClientState, lv: &mut Level, fs: &Fs, r: &mut Renderer, out: &mut Vec<RenderEntity>) {
        if !self.enabled || idx == 0 || idx > cl.maxclients {
            return;
        }
        let Some(e) = cl.entities.get(idx) else { return };
        if !cl.model_name(e.model).is_some_and(|n| n.eq_ignore_ascii_case(PLAYER)) {
            return;
        }
        // the model the level draws for player.mdl: classic, or an enhanced
        // replacement
        let Some(enhanced) = lv.models.get(e.model).copied().flatten().map(|m| m.md3) else { return };
        let Some(&body) = out.last() else { return };
        let Some(l) = lv.vwep.rig(enhanced, fs, r) else { return };
        let (body_id, body_axe) = (l.body, l.body_axe);
        let Some(held) = self.resolve(idx, cl, &l.rig, body.frame, fs) else { return };
        let gun = match &held {
            Held::Axe => None,
            Held::Gun(spec) => match lv.vwep.gun(enhanced, spec, fs, r) {
                Some(id) => Some(id),
                None => return,
            },
        };
        let last = out.last_mut().expect("body pushed");
        last.model = if gun.is_some() { body_id } else { body_axe };
        if let Some(model) = gun {
            out.push(RenderEntity {
                model,
                skin: 0,
                colormap: None,
                key: if body.key == 0 { 0 } else { body.key | qrender::ATTACHED_KEY },
                ..body
            });
        }
    }
}
