//! Add-ons: optional engine-side toys (Customize → Add-ons). They live in
//! the engine, not QuakeC, so they work with any progs. Off by default and
//! only allowed in single player and local split-screen: never in
//! deathmatch or with a remote client connected.
//!
//! While a player holds a toy, the client keeps the fire buttons away from
//! QC (`button0` stays 0) and sends them in usercmd bits QC never sees:
//! [`BUTTON_TOY_FIRE`], [`BUTTON_TOY_ALT`] and the toy id in the top bits.

use qcore::cvar::{CVAR_ARCHIVE, CVAR_ROM, Cvars};

use crate::ents::Ents;
use crate::server::{Core, Level};

pub mod boomerang;
pub mod fall;
pub mod dodge;
pub mod gel;
pub mod gel_phys;
pub mod hook_surface;
pub mod hookshot;
pub mod portal;
pub mod portal_move;
pub mod railgun;
pub mod save;
pub mod sticky;
pub mod shotgun;
pub(crate) mod settings;
pub mod washer;

/// (name, default, flags) of the add-on cvars.
pub const CVARS: &[(&str, &str, u32)] = &[
    // New Game "Customize" checkbox: start the next new game customized
    // (modifiers and add-ons); fixed at a game's start, see `Core::custom`
    ("custom_enable", "0", CVAR_ARCHIVE),
    // read-only mirror of `Core::custom` for the menu
    ("custom_active", "0", CVAR_ROM),
    ("addon_projectile_shotguns", "0", CVAR_ARCHIVE),
    ("addon_shotgun_speed", "10000", CVAR_ARCHIVE),
    ("addon_dodge", "0", CVAR_ARCHIVE),
    ("addon_bullet_time", "0", CVAR_ARCHIVE),
    ("addon_bullet_time_toggle", "0", CVAR_ARCHIVE),
    ("addon_bullet_time_meter", "1", CVAR_ARCHIVE),
    ("addon_bullet_time_duration", "5", CVAR_ARCHIVE),
    ("addon_bullet_time_regen", "1", CVAR_ARCHIVE),
    // hitstop (client side): master, weapons (0 all, 1 melee, 2 ranged, 3
    // explosives), targets (0 all, 1 tough), freeze ms, per-damage ms, kill
    // multiplier, cooldown ms, cap ms, minimum damage, tough health, shake
    ("addon_hitstop", "0", CVAR_ARCHIVE),
    ("addon_hitstop_weapons", "0", CVAR_ARCHIVE),
    ("addon_hitstop_targets", "0", CVAR_ARCHIVE),
    ("addon_hitstop_duration", "60", CVAR_ARCHIVE),
    ("addon_hitstop_scale_damage", "3", CVAR_ARCHIVE),
    ("addon_hitstop_kill_bonus", "2", CVAR_ARCHIVE),
    ("addon_hitstop_cooldown", "150", CVAR_ARCHIVE),
    ("addon_hitstop_max", "250", CVAR_ARCHIVE),
    ("addon_hitstop_min_damage", "10", CVAR_ARCHIVE),
    ("addon_hitstop_tough", "100", CVAR_ARCHIVE),
    ("addon_hitstop_shake", "0", CVAR_ARCHIVE),
    ("addon_portalgun", "0", CVAR_ARCHIVE),
    ("addon_sticky", "0", CVAR_ARCHIVE),
    ("addon_sticky_damage", "120", CVAR_ARCHIVE),
    // Weapon slot numbers; 10 is the slot labelled 0.
    ("addon_portalgun_slot", "9", CVAR_ARCHIVE),
    ("addon_boomerang_slot", "1", CVAR_ARCHIVE),
    ("addon_sticky_slot", "6", CVAR_ARCHIVE),
    ("addon_hookshot_slot", "1", CVAR_ARCHIVE),
    ("addon_railgun_slot", "8", CVAR_ARCHIVE),
    ("addon_gel_slot", "9", CVAR_ARCHIVE),
    ("addon_washer_slot", "5", CVAR_ARCHIVE),
    // see-through portals: on/off, live views per frame, live distance
    ("addon_portal_view", "1", CVAR_ARCHIVE),
    ("addon_portal_max", "4", CVAR_ARCHIVE),
    ("addon_portal_dist", "4096", CVAR_ARCHIVE),
    ("addon_portal_depth", "3", CVAR_ARCHIVE),
    // boomerang: on/off, stun seconds, flight speed (units/s)
    ("addon_boomerang", "0", CVAR_ARCHIVE),
    ("addon_boomerang_stun", "4", CVAR_ARCHIVE),
    ("addon_boomerang_speed", "700", CVAR_ARCHIVE),
    ("addon_railgun", "0", CVAR_ARCHIVE),
    // railgun damage per entity pierced (Quake III: 100)
    ("addon_rail_damage", "100", CVAR_ARCHIVE),
    // trail colour: 0 blue, 1 green, 2 red, 3 yellow, 4 purple, 5 white
    ("addon_rail_color", "0", CVAR_ARCHIVE),
    // hookshot: range and which targets latch
    ("addon_hookshot", "0", CVAR_ARCHIVE),
    ("addon_hook_range", "1024", CVAR_ARCHIVE),
    ("addon_hook_wood", "1", CVAR_ARCHIVE),
    ("addon_hook_metal", "1", CVAR_ARCHIVE),
    ("addon_hook_stone", "1", CVAR_ARCHIVE),
    ("addon_hook_enemies", "1", CVAR_ARCHIVE),
    ("addon_gel", "0", CVAR_ARCHIVE),
    ("addon_gel_quality", "1", CVAR_ARCHIVE),
    // powerwasher: on/off, push strength multiplier, jet geometry/shading quality
    ("addon_washer", "0", CVAR_ARCHIVE),
    ("addon_washer_push", "1", CVAR_ARCHIVE),
    ("addon_washer_quality", "1", CVAR_ARCHIVE),
];

/// Switches every add-on on (they all default to off); for tests and tools.
pub fn enable_all(cvars: &mut qcore::cvar::Cvars) {
    cvars.set("custom_enable", "1");
    for name in Toy::ALL.iter().map(|t| t.cvar()).chain(["addon_projectile_shotguns", "addon_dodge", "addon_bullet_time"]) {
        cvars.set(name, "1");
    }
}

/// Usercmd button bits (bit 0 attack and bit 1 jump are QC's).
pub const BUTTON_TOY_FIRE: u8 = 1 << 2;
pub const BUTTON_TOY_ALT: u8 = 1 << 3;
/// The held toy's id sits in bits 4..6 (0 = none). Bit 7 is dodge.
pub const TOY_SHIFT: u8 = 4;
/// Directional dodge press, independent of weapon buttons.
pub const BUTTON_DODGE: u8 = 1 << 7;

/// Add-on weapons, in stable order within a shared weapon slot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Toy {
    PortalGun,
    Boomerang,
    Sticky,
    Hookshot,
    Railgun,
    GelSprayer,
    Washer,
}

impl Toy {
    pub const ALL: [Toy; 7] = [Toy::PortalGun, Toy::Boomerang, Toy::Sticky, Toy::Hookshot, Toy::Railgun, Toy::GelSprayer, Toy::Washer];

    /// Wire id (1..=15).
    pub fn id(self) -> u8 {
        self as u8 + 1
    }
    pub fn from_id(id: u8) -> Option<Toy> {
        Self::ALL.get((id as usize).checked_sub(1)?).copied()
    }
    pub fn name(self) -> &'static str {
        match self {
            Toy::PortalGun => qcore::t!("Portal Gun"),
            Toy::Boomerang => qcore::t!("Boomerang"),
            Toy::Sticky => qcore::t!("Stickybomb Launcher"),
            Toy::Railgun => qcore::t!("Railgun"),
            Toy::Hookshot => qcore::t!("Hookshot"),
            Toy::GelSprayer => qcore::t!("Gel Sprayer"),
            Toy::Washer => qcore::t!("Powerwasher"),
        }
    }
    /// Name in savegame headers.
    pub fn key(self) -> &'static str {
        match self {
            Toy::PortalGun => "portalgun",
            Toy::Boomerang => "boomerang",
            Toy::Sticky => "sticky",
            Toy::Railgun => "railgun",
            Toy::Hookshot => "hookshot",
            Toy::GelSprayer => "gel",
            Toy::Washer => "washer",
        }
    }
    pub fn from_key(key: &str) -> Option<Toy> {
        Self::ALL.into_iter().find(|t| t.key() == key)
    }
    /// The cvar that selects this toy in a game that has add-ons.
    pub fn cvar(self) -> &'static str {
        match self {
            Toy::PortalGun => "addon_portalgun",
            Toy::Boomerang => "addon_boomerang",
            Toy::Sticky => "addon_sticky",
            Toy::Railgun => "addon_railgun",
            Toy::Hookshot => "addon_hookshot",
            Toy::GelSprayer => "addon_gel",
            Toy::Washer => "addon_washer",
        }
    }
    /// Archived weapon slot assignment (10 is displayed as 0).
    pub fn slot_cvar(self) -> &'static str {
        match self {
            Toy::PortalGun => "addon_portalgun_slot",
            Toy::Boomerang => "addon_boomerang_slot",
            Toy::Sticky => "addon_sticky_slot",
            Toy::Hookshot => "addon_hookshot_slot",
            Toy::Railgun => "addon_railgun_slot",
            Toy::GelSprayer => "addon_gel_slot",
            Toy::Washer => "addon_washer_slot",
        }
    }
    /// The toys switched on in the menu.
    pub fn enabled(cv: &Cvars) -> impl Iterator<Item = Toy> + '_ {
        Self::ALL.into_iter().filter(|t| cv.value_of(t.cvar()) != 0.0)
    }
    /// The toy (if any) a usercmd's buttons say is held.
    pub fn from_buttons(buttons: u8) -> Option<Toy> {
        Self::from_id((buttons & !BUTTON_DODGE) >> TOY_SHIFT)
    }
}

/// Add-ons may run: this game was started with them, not deathmatch, and
/// every connected client plays on this machine.
pub fn allowed(on: bool, deathmatch: bool, all_local: bool) -> bool {
    on && !deathmatch && all_local
}

/// Add-ons may run on this server this frame (cheap when off).
pub(crate) fn active(c: &Core) -> bool {
    c.custom && c.cvars.value_of("deathmatch") == 0.0 && all_local(&c.clients)
}

impl Core {
    /// Fixes whether this is a customized game (modifiers and add-ons), at
    /// a new game or a load. It never changes mid-game.
    pub fn set_custom(&mut self, on: bool) {
        self.custom = on;
        self.cvars.set_value("custom_active", on as u8 as f32);
    }
}

/// After physics: portal crossings, then new shots, then client updates.
pub(crate) fn frame(l: &mut Level, c: &mut Core) {
    let st = &mut l.portals;
    // players giving no movement input: they bob in place on floor portals
    let idle: Vec<qworld::Ent> =
        c.clients.iter().enumerate().filter(|(_, cl)| cl.cmd.forward == 0.0 && cl.cmd.side == 0.0).map(|(i, _)| i as qworld::Ent + 1).collect();
    let crossed = std::mem::take(&mut c.portal_crossed);
    let mut ents = Ents { vm: &mut l.vm, c: &mut *c };
    portal_move::cross(st, &mut l.world, &mut ents, &idle, &crossed);
    let gun = c.cvars.value_of(Toy::PortalGun.cvar()) != 0.0;
    for (i, slot) in st.presses(&c.clients) {
        if !gun {
            continue;
        }
        let ents = Ents { vm: &mut l.vm, c: &mut *c };
        let others: Vec<portal::Portal> =
            st.pairs.iter().enumerate().flat_map(|(o, p)| p.iter().enumerate().filter(move |&(s, _)| (o, s) != (i, slot)).filter_map(|(_, p)| *p)).collect();
        use qcore::math::{Vec3, angle_vectors};
        use qworld::{Edicts, MoveType, V};
        let e = i as u32 + 1;
        let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
        let (fwd, right, up) = angle_vectors(ents.v(e, V::VAngle));
        let start = eye + fwd * 40.0 + right * 5.0 - up * 8.0;
        let p = portal::place(&l.world, &ents, e, &others);
        let end = p.map(|p| p.origin).unwrap_or_else(|| {
            l.world.trace(&ents, eye, Vec3::ZERO, Vec3::ZERO, eye + fwd * 8192.0, MoveType::NoMonsters, Some(e)).endpos
        });
        st.effect(i, slot, u8::from(p.is_some()), start, end);
        if let Some(p) = p {
            st.set(i, slot, Some(p));
        }
    }
    st.send(&mut c.clients);
    st.sync_links(&mut c.linked_portals);
    boomerang_frame(l, c);
    if c.cvars.value_of(Toy::Sticky.cvar()) != 0.0 {
        sticky::frame(&mut l.stickies, &mut l.vm, c, &mut l.world);
        l.stickies.send(&mut c.clients);
    } else if !l.stickies.bombs.is_empty() {
        l.stickies.clear(&mut c.clients);
    }
    if c.cvars.value_of(Toy::Railgun.cvar()) != 0.0 {
        let mut rails = std::mem::take(&mut l.rails);
        railgun::frame(&mut rails, l, c);
        l.rails = rails;
    }
    let hook = c.cvars.value_of(Toy::Hookshot.cvar()) != 0.0;
    let rules = hookshot::Rules::read(&c.cvars);
    let time = l.vm.time;
    let mut clients = std::mem::take(&mut c.clients);
    hookshot::frame(&mut l.hooks, &l.world, &mut Ents { vm: &mut l.vm, c: &mut *c }, &mut clients, rules, hook, time);
    c.clients = clients;
    gel_frame(l, c);
    if gun {
        fall::frame(&mut l.falls, &mut l.vm, c, &mut l.world);
    } else {
        l.falls.clear();
    }
}

impl crate::Server {
    /// This is a customized game (modifiers and add-ons): its saves are
    /// `.qsa` files.
    pub fn custom(&self) -> bool {
        self.core.custom
    }
    /// Saves of this game are `.qsa` files: a customized game, or a game
    /// with several player slots (split-screen, co-op), which other ports
    /// refuse to save and would load as single player.
    pub fn qsa_saves(&self) -> bool {
        self.core.custom || self.core.maxclients() > 1
    }
    /// A v5 savegame body, wrapped in the `.qsa` header when
    /// [`Self::qsa_saves`]: toys, add-on state and modifiers of a customized
    /// game (`vanilla` otherwise), and the player slots and rules of a
    /// multiplayer game.
    pub(crate) fn wrap_save(&self, body: String) -> String {
        if !self.qsa_saves() {
            return body;
        }
        let c = &self.core;
        let l = self.level.as_ref();
        let mut extra = String::new();
        let mut toys = Vec::new();
        let mut portals = None;
        if c.custom {
            toys = Toy::enabled(&c.cvars).collect();
            portals = l.and_then(|l| l.portals.save_text());
            extra += &l.map(|l| l.stickies.save_text() + &l.gel.save_text()).unwrap_or_default();
            extra += &settings::save(&c.cvars);
            extra += &crate::modifiers::save_text(&c.cvars);
        } else {
            extra += "vanilla\n";
        }
        if c.maxclients() > 1 {
            let p = save::Players {
                slots: c.maxclients(),
                local: (c.cvars.value_of("cl_splitscreen") as usize).max(1),
                coop: c.cvars.value_of("coop"),
                deathmatch: c.cvars.value_of("deathmatch"),
                teamplay: c.cvars.value_of("teamplay"),
            };
            extra += &p.line();
        }
        save::wrap_with(&toys, portals.as_deref(), &extra, &body)
    }
}

/// Gel sprayer and powerwasher (the washer cleans gel, so they run together).
fn gel_frame(l: &mut Level, c: &mut Core) {
    let gel_on = c.cvars.value_of(Toy::GelSprayer.cvar()) != 0.0;
    if !gel_on {
        if !l.gel.splats.is_empty() {
            l.gel.clear(&mut c.clients);
        }
    } else {
        let (time, dt) = (l.vm.time, l.vm.g_f(c.sv.globals.frametime));
        let gravity = c.cvars.value_of("sv_gravity");
        let clients = std::mem::take(&mut c.clients);
        let linked = std::mem::take(&mut c.linked_portals);
        let mut ents = Ents { vm: &mut l.vm, c: &mut *c };
        gel_phys::after(&mut l.gel, &mut ents, clients.len(), time);
        l.gel.frame(&l.world, &ents, &clients, &linked, time, dt, gravity);
        c.linked_portals = linked;
        c.clients = clients;
    }
    if c.cvars.value_of(Toy::Washer.cvar()) != 0.0 {
        let dt = l.vm.g_f(c.sv.globals.frametime);
        let scale = c.cvars.value_of("addon_washer_push").clamp(0.0, 4.0);
        let mut clients = std::mem::take(&mut c.clients);
        let mut ents = Ents { vm: &mut l.vm, c: &mut *c };
        l.washers.frame(&mut l.world, &mut ents, &mut clients, &mut l.gel, scale, dt);
        c.clients = clients;
    }
    l.gel.send(&mut c.clients);
}

fn boomerang_frame(l: &mut Level, c: &mut Core) {
    let on = c.cvars.value_of(Toy::Boomerang.cvar()) != 0.0;
    if !on && l.boomerangs.is_empty() {
        return;
    }
    let tune = boomerang::Tuning {
        stun: c.cvars.value_of("addon_boomerang_stun").clamp(0.5, 30.0),
        speed: c.cvars.value_of("addon_boomerang_speed").clamp(200.0, 2000.0),
    };
    let (time, dt) = (l.vm.time, l.vm.g_f(c.sv.globals.frametime));
    let clients = std::mem::take(&mut c.clients);
    let mut ents = Ents { vm: &mut l.vm, c: &mut *c };
    l.boomerangs.frame(&l.world, &mut ents, &clients, time, dt, tune, on);
    c.clients = clients;
    l.boomerangs.send(&mut c.clients);
}

/// Add-ons were switched off (or a remote client joined): remove every
/// portal from the level and the clients.
pub(crate) fn shut_down(l: &mut Level, c: &mut Core) {
    let st = &mut l.portals;
    for i in 0..st.pairs.len() {
        for slot in 0..2 {
            if st.pairs[i][slot].is_some() {
                st.set(i, slot, None);
            }
        }
    }
    st.send(&mut c.clients);
    *st = Default::default();
    c.linked_portals.clear();
    c.portal_crossed.clear();
    l.boomerangs.clear(&mut c.clients);
    l.stickies.clear(&mut c.clients);
    l.gel.clear(&mut c.clients);
    l.washers.reset();
    l.hooks.reset();
}

/// No remote client is connected.
pub fn all_local(clients: &[crate::client::Client]) -> bool {
    clients.iter().filter(|c| c.active).all(|c| c.local)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toy_ids_round_trip_through_buttons() {
        for t in Toy::ALL {
            let b = t.id() << TOY_SHIFT | BUTTON_TOY_FIRE | 1;
            assert_eq!(Toy::from_buttons(b), Some(t));
        }
        assert_eq!(Toy::from_buttons(0b11), None);
        assert_eq!(Toy::from_id(15), None);
    }
}
