//! Status bar, inventory, solo scoreboard and intermission (sbar.c).

use qclient::ClientState;
use qnet::protocol::*;
use qrender::PicId;

use crate::draw::Canvas;
mod rows;
mod classic;
mod screens;
pub use screens::*;
pub mod time;

pub type PicFn<'a> = dyn FnMut(&str) -> Option<(PicId, u32, u32)> + 'a;

pub const IT_SHOTGUN: i32 = 1;
pub const IT_SHELLS: i32 = 256;
pub const IT_NAILS: i32 = 512;
pub const IT_ROCKETS: i32 = 1024;
pub const IT_CELLS: i32 = 2048;
pub const IT_ARMOR1: i32 = 8192;
pub const IT_ARMOR2: i32 = 16384;
pub const IT_ARMOR3: i32 = 32768;
pub const IT_INVISIBILITY: i32 = 524288;
pub const IT_INVULNERABILITY: i32 = 1048576;
pub const IT_SUIT: i32 = 2097152;
pub const IT_QUAD: i32 = 4194304;

// Rogue (quakedef.h RIT_*): ammo, armor and items move to other bits.
const RIT_SHELLS: i32 = 128;
const RIT_NAILS: i32 = 256;
const RIT_ROCKETS: i32 = 512;
const RIT_CELLS: i32 = 1024;
const RIT_LAVA_NAILGUN: i32 = 4096;
const RIT_ARMOR1: i32 = 8388608;
const RIT_ARMOR2: i32 = 16777216;
const RIT_ARMOR3: i32 = 33554432;
const RIT_LAVA_NAILS: i32 = 67108864;
const RIT_PLASMA_AMMO: i32 = 134217728;
const RIT_MULTI_ROCKETS: i32 = 268435456;
/// Powered-up weapons replacing slots 2..7 when active.
const ROGUE_WEAPONS: [&str; 5] = ["r_lava", "r_superlava", "r_gren", "r_multirock", "r_plasma"];
const ROGUE_ITEMS: [&str; 2] = ["r_shield1", "r_agrav1"];

// Hipnotic (HIT_*): extra weapons by item bit; 4 = grenade launcher slot.
const HIP_WEAPON_BITS: [usize; 4] = [23, 7, 4, 16];
const HIP_WEAPONS: [&str; 5] = ["laser", "mjolnir", "gren_prox", "prox_gren", "prox"];
const HIT_PROXIMITY_GUN: i32 = 1 << 16;
const HIP_ITEMS: [&str; 2] = ["sb_wsuit", "sb_eshld"];

/// Which mission-pack status bar rules apply (ironwail's `hipnotic`/`rogue`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Pack {
    #[default]
    Id,
    Hipnotic,
    Rogue,
}

impl Pack {
    /// From the mounted game dirs, as COM_AddGameDirectory: rogue, or
    /// hipnotic/quoth.
    pub fn from_game_dirs<'s>(dirs: impl IntoIterator<Item = &'s str>) -> Self {
        let mut p = Pack::Id;
        for d in dirs {
            if d.eq_ignore_ascii_case("rogue") {
                p = Pack::Rogue;
            } else if d.eq_ignore_ascii_case("hipnotic") || d.eq_ignore_ascii_case("quoth") {
                p = Pack::Hipnotic;
            }
        }
        p
    }

    pub fn ammo_pic(self, items: i32) -> Option<&'static str> {
        const ROGUE: &[(i32, &str)] = &[
            (RIT_SHELLS, "sb_shells"),
            (RIT_NAILS, "sb_nails"),
            (RIT_ROCKETS, "sb_rocket"),
            (RIT_CELLS, "sb_cells"),
            // Sbar_AmmoPic's table order (plasma shows the multi icon, as in the original).
            (RIT_LAVA_NAILS, "r_ammolava"),
            (RIT_PLASMA_AMMO, "r_ammomulti"),
            (RIT_MULTI_ROCKETS, "r_ammoplasma"),
        ];
        const ID: &[(i32, &str)] = &[
            (IT_SHELLS, "sb_shells"),
            (IT_NAILS, "sb_nails"),
            (IT_ROCKETS, "sb_rocket"),
            (IT_CELLS, "sb_cells"),
        ];
        let table = if self == Pack::Rogue { ROGUE } else { ID };
        table.iter().find(|&&(b, _)| items & b != 0).map(|&(_, n)| n)
    }

    /// Item bits of shells, nails, rockets and cells.
    pub fn ammo_bits(self) -> [i32; 4] {
        if self == Pack::Rogue { [RIT_SHELLS, RIT_NAILS, RIT_ROCKETS, RIT_CELLS] } else { [IT_SHELLS, IT_NAILS, IT_ROCKETS, IT_CELLS] }
    }

    /// Sbar_ArmorPic.
    pub fn armor_pic(self, items: i32) -> Option<&'static str> {
        if items & IT_INVULNERABILITY != 0 {
            return Some("disc");
        }
        let bits = if self == Pack::Rogue { [RIT_ARMOR3, RIT_ARMOR2, RIT_ARMOR1] } else { [IT_ARMOR3, IT_ARMOR2, IT_ARMOR1] };
        bits.into_iter().zip(["sb_armor3", "sb_armor2", "sb_armor1"]).find(|(b, _)| items & b != 0).map(|(_, n)| n)
    }

    /// Sbar_InventoryBarPic.
    fn ibar(self, active: i32) -> &'static str {
        match self {
            Pack::Rogue if active >= RIT_LAVA_NAILGUN => "r_invbar1",
            Pack::Rogue => "r_invbar2",
            _ => "ibar",
        }
    }

    /// Extra pack items: (item bit, pic).
    pub fn extra_items(self) -> &'static [(u32, &'static str)] {
        match self {
            Pack::Hipnotic => &[(24, HIP_ITEMS[0]), (25, HIP_ITEMS[1])],
            Pack::Rogue => &[(29, ROGUE_ITEMS[0]), (30, ROGUE_ITEMS[1])],
            Pack::Id => &[],
        }
    }
}

/// Weapon flash state as in Sbar_DrawInventory: 0 owned, 1 active, 2..6 pickup flash.
fn flash(cl: &ClientState, bit: usize, active: bool) -> i32 {
    let f = ((cl.time - cl.item_gettime[bit]) * 10.0) as i32;
    if f >= 10 { active as i32 } else { f % 5 + 2 }
}

fn weapon_prefix(flash: i32) -> &'static str {
    match flash {
        0 => "inv_",
        1 => "inv2_",
        2 => "inva1_",
        3 => "inva2_",
        4 => "inva3_",
        5 => "inva4_",
        _ => "inva5_",
    }
}

/// Hipnotic extra weapon pics: (slot, pic) where slot 0..=1 are laser/mjolnir
/// and slot 2 is the grenade launcher position (prox variants).
fn hip_weapons(cl: &ClientState) -> Vec<(usize, String, bool)> {
    let items = cl.items;
    let active_w = cl.stats[STAT_ACTIVEWEAPON];
    let mut out = Vec::new();
    let mut grenade_flashing = false;
    for (i, &bit) in HIP_WEAPON_BITS.iter().enumerate() {
        if items & (1 << bit) == 0 {
            continue;
        }
        let active = active_w == 1 << bit;
        let fl = flash(cl, bit, active);
        let pic = |k: usize, f: i32| format!("{}{}", weapon_prefix(f), HIP_WEAPONS[k]);
        match i {
            2 => {
                if items & HIT_PROXIMITY_GUN != 0 && fl != 0 {
                    grenade_flashing = true;
                    out.push((2, pic(2, fl), active));
                }
            }
            3 => {
                if items & (IT_SHOTGUN << 4) != 0 {
                    if !grenade_flashing {
                        out.push((2, pic(3, fl), active));
                    }
                } else {
                    out.push((2, pic(4, fl), active));
                }
            }
            _ => out.push((i, pic(i, fl), active)),
        }
    }
    out
}

const WEAPONS: [&str; 7] = ["shotgun", "sshotgun", "nailgun", "snailgun", "rlaunch", "srlaunch", "lightng"];
pub const ITEMS: [&str; 6] = ["sb_key1", "sb_key2", "sb_invis", "sb_invuln", "sb_suit", "sb_quad"];

pub struct Sbar<'a, 'b> {
    pub c: &'a mut Canvas,
    pub pack: Pack,
    pub pic: &'a mut PicFn<'b>,
    /// Top of the status bar in virtual units.
    y0: f32,
    x0: f32,
    /// Opacity of the pictures drawn.
    alpha: f32,
    /// Classic bar: the inventory strip above the status bar.
    pub inventory: bool,
    /// Local `teamplay` cvar (rogue CTF modes 4..6 replace the face).
    pub teamplay: f32,
    pub addon_weapons: Vec<crate::wheel::Entry>,
    pub held_addon: Option<qserver::addons::Toy>,
}

impl<'a, 'b> Sbar<'a, 'b> {
    pub fn new(c: &'a mut Canvas, pic: &'a mut PicFn<'b>, pack: Pack) -> Self {
        c.reset_origin();
        let x0 = ((c.vw() - 320.0) / 2.0).floor();
        let y0 = c.vh() - 24.0;
        Self { c, pack, pic, y0, x0, alpha: 1.0, inventory: true, teamplay: 0.0, addon_weapons: Vec::new(), held_addon: None }
    }
    /// Draws at the canvas origin as it is (a HUD element's own origin):
    /// the status bar's top left at (0, `top`).
    pub fn at(c: &'a mut Canvas, pic: &'a mut PicFn<'b>, pack: Pack, top: f32) -> Self {
        Self { c, pack, pic, y0: top, x0: 0.0, alpha: 1.0, inventory: true, teamplay: 0.0, addon_weapons: Vec::new(), held_addon: None }
    }
    /// The scoreboard strip at the top centre (no status bar on screen).
    pub fn draw_scores_top(&mut self, cl: &ClientState) {
        self.y0 = 0.0;
        self.draw_pic(0.0, 0.0, "scorebar");
        self.solo_scoreboard(cl);
    }

    pub fn face_name(cl: &ClientState, face_anim: bool) -> &'static str {
        const FACES: [&str; 5] = ["face5", "face4", "face3", "face2", "face1"];
        const PAIN: [&str; 5] = ["face_p5", "face_p4", "face_p3", "face_p2", "face_p1"];
        let items = cl.items;
        if items & (IT_INVISIBILITY | IT_INVULNERABILITY) == IT_INVISIBILITY | IT_INVULNERABILITY {
            "face_inv2"
        } else if items & IT_QUAD != 0 {
            "face_quad"
        } else if items & IT_INVISIBILITY != 0 {
            "face_invis"
        } else if items & IT_INVULNERABILITY != 0 {
            "face_invul2"
        } else {
            let f = (cl.stats[STAT_HEALTH] / 20).clamp(0, 4) as usize;
            if face_anim { PAIN[f] } else { FACES[f] }
        }
    }

}

#[cfg(test)]
mod team_face_tests {
    use super::*;
    use qrender::Draw2D;

    fn draw(pack: Pack, teamplay: f32, maxclients: usize) -> (Vec<String>, usize) {
        let mut cl = ClientState::new();
        cl.maxclients = maxclients;
        cl.scores = vec![Default::default(); maxclients];
        cl.scores[0].colors = 0x4d;
        cl.scores[0].frags = 7;
        cl.viewentity = 1;
        cl.stats[STAT_HEALTH] = 100;
        let mut c = Canvas::new(640, 480);
        let mut names = Vec::new();
        let mut pf = |n: &str| {
            names.push(n.to_string());
            None
        };
        let mut sb = Sbar::new(&mut c, &mut pf, pack);
        sb.teamplay = teamplay;
        sb.draw(&cl, false, false);
        let fills = c.list.iter().filter(|d| matches!(d, Draw2D::Fill { .. })).count();
        (names, fills)
    }

    #[test]
    fn rogue_ctf_swaps_face_for_team_colours() {
        let (names, fills) = draw(Pack::Rogue, 4.0, 2);
        assert!(names.iter().any(|n| n == "r_teambord"), "{names:?}");
        assert!(!names.iter().any(|n| n.starts_with("face")));
        assert_eq!(fills, 2);
        // Not in single player, outside teamplay 4..6, or without rogue.
        for (pack, tp, mc) in [(Pack::Rogue, 4.0, 1), (Pack::Rogue, 3.0, 2), (Pack::Rogue, 7.0, 2), (Pack::Id, 4.0, 2)] {
            let (names, _) = draw(pack, tp, mc);
            assert!(names.iter().any(|n| n.starts_with("face")), "{pack:?} {tp} {mc}");
        }
    }
}
