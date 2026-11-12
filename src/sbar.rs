//! Status bar, inventory, solo scoreboard and intermission (sbar.c).

use qclient::ClientState;
use qnet::protocol::*;
use qrender::PicId;

use crate::draw::Canvas;

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

    fn ammo_pic(self, items: i32) -> Option<&'static str> {
        let mut t = vec![];
        if self == Pack::Rogue {
            t.extend([(RIT_SHELLS, "sb_shells"), (RIT_NAILS, "sb_nails"), (RIT_ROCKETS, "sb_rocket"), (RIT_CELLS, "sb_cells")]);
            // Sbar_AmmoPic's table order (plasma shows the multi icon, as in the original).
            t.extend([(RIT_LAVA_NAILS, "r_ammolava"), (RIT_PLASMA_AMMO, "r_ammomulti"), (RIT_MULTI_ROCKETS, "r_ammoplasma")]);
        } else {
            t.extend([(IT_SHELLS, "sb_shells"), (IT_NAILS, "sb_nails"), (IT_ROCKETS, "sb_rocket"), (IT_CELLS, "sb_cells")]);
        }
        t.into_iter().find(|(b, _)| items & b != 0).map(|(_, n)| n)
    }

    /// Sbar_ArmorPic.
    fn armor_pic(self, items: i32) -> Option<&'static str> {
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
    fn extra_items(self) -> &'static [(u32, &'static str)] {
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

fn weapon_prefix(flash: i32) -> String {
    match flash {
        0 => "inv_".into(),
        1 => "inv2_".into(),
        n => format!("inva{}_", n - 1),
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
const ITEMS: [&str; 6] = ["sb_key1", "sb_key2", "sb_invis", "sb_invuln", "sb_suit", "sb_quad"];

/// Modern HUD elements, in stacking order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudElem {
    Health,
    Armor,
    Ammo,
    AmmoList,
    Weapons,
    Items,
}

impl HudElem {
    pub const ALL: [HudElem; 6] =
        [HudElem::Health, HudElem::Armor, HudElem::Ammo, HudElem::AmmoList, HudElem::Weapons, HudElem::Items];
    /// Cvar suffix: `hud_<name>` (mode) and `hud_<name>_pos` (anchor).
    pub fn name(self) -> &'static str {
        ["health", "armor", "ammo", "ammolist", "weapons", "items"][self as usize]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudMode {
    Off,
    On,
    /// Shown only briefly after the element's value changes.
    Immersive,
}

impl HudMode {
    pub fn from_cvar(v: f32) -> Self {
        match v as i32 {
            0 => HudMode::Off,
            2 => HudMode::Immersive,
            _ => HudMode::On,
        }
    }
}

/// Screen anchor; cvar value is the discriminant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudAnchor {
    BottomLeft,
    BottomCenter,
    BottomRight,
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
}

impl HudAnchor {
    pub const COUNT: usize = 8;
    pub fn from_cvar(v: f32) -> Self {
        use HudAnchor::*;
        [BottomLeft, BottomCenter, BottomRight, TopLeft, TopCenter, TopRight, MiddleLeft, MiddleRight]
            [(v.max(0.0) as usize).min(7)]
    }
    /// 0 left, 1 center, 2 right.
    fn col(self) -> u8 {
        use HudAnchor::*;
        match self {
            BottomLeft | TopLeft | MiddleLeft => 0,
            BottomCenter | TopCenter => 1,
            _ => 2,
        }
    }
    /// 0 top, 1 middle, 2 bottom.
    fn row(self) -> u8 {
        use HudAnchor::*;
        match self {
            TopLeft | TopCenter | TopRight => 0,
            MiddleLeft | MiddleRight => 1,
            _ => 2,
        }
    }
}

/// Modern HUD settings (from `hud_*` cvars).
#[derive(Clone, Debug)]
pub struct HudConfig {
    pub mode: [HudMode; 6],
    pub anchor: [HudAnchor; 6],
    pub face: bool,
}

impl HudConfig {
    pub fn from_cvars(cv: &qcore::cvar::Cvars) -> Self {
        let mut c = HudConfig { mode: [HudMode::On; 6], anchor: [HudAnchor::BottomLeft; 6], face: cv.value_of("hud_face") != 0.0 };
        for e in HudElem::ALL {
            c.mode[e as usize] = HudMode::from_cvar(cv.value_of(&format!("hud_{}", e.name())));
            c.anchor[e as usize] = HudAnchor::from_cvar(cv.value_of(&format!("hud_{}_pos", e.name())));
        }
        c
    }
}

/// Seconds an immersive element stays fully visible, then fades.
const IMMERSIVE_HOLD: f64 = 3.0;
const IMMERSIVE_FADE: f64 = 0.5;

/// Change tracking for immersive HUD elements.
#[derive(Default)]
pub struct HudFade {
    last: [Option<i64>; 6],
    changed: [f64; 6],
}

impl HudFade {
    fn signature(cl: &ClientState, e: HudElem) -> i64 {
        let st = &cl.stats;
        let it = cl.items as i64 & 0xffff_ffff;
        match e {
            HudElem::Health => st[STAT_HEALTH] as i64,
            HudElem::Armor => (st[STAT_ARMOR] as i64) << 32 | it,
            HudElem::Ammo => (st[STAT_AMMO] as i64) << 32 | st[STAT_ACTIVEWEAPON] as i64 & 0xffff_ffff,
            HudElem::AmmoList => (0..4).fold(0i64, |h, i| h << 16 | (st[STAT_SHELLS + i] as i64 & 0xffff)),
            HudElem::Weapons => it << 32 ^ st[STAT_ACTIVEWEAPON] as i64,
            HudElem::Items => it,
        }
    }

    /// Records value changes; the first sighting counts as a change.
    pub fn update(&mut self, cl: &ClientState, now: f64) {
        for e in HudElem::ALL {
            let s = Self::signature(cl, e);
            if self.last[e as usize] != Some(s) {
                self.last[e as usize] = Some(s);
                self.changed[e as usize] = now;
            }
        }
    }

    pub fn alpha(&self, e: HudElem, now: f64) -> f32 {
        let t = now - self.changed[e as usize];
        if t < IMMERSIVE_HOLD {
            1.0
        } else {
            (1.0 - (t - IMMERSIVE_HOLD) / IMMERSIVE_FADE).max(0.0) as f32
        }
    }
}

pub struct Sbar<'a, 'b> {
    pub c: &'a mut Canvas,
    pub pack: Pack,
    pub pic: &'a mut PicFn<'b>,
    /// Top of the status bar in virtual units.
    y0: f32,
    x0: f32,
    /// Opacity for the element being drawn (immersive fade).
    alpha: f32,
    /// Local `teamplay` cvar (rogue CTF modes 4..6 replace the face).
    pub teamplay: f32,
}

impl<'a, 'b> Sbar<'a, 'b> {
    pub fn new(c: &'a mut Canvas, pic: &'a mut PicFn<'b>, pack: Pack) -> Self {
        c.reset_origin();
        let x0 = ((c.vw() - 320.0) / 2.0).floor();
        let y0 = c.vh() - 24.0;
        Self { c, pack, pic, y0, x0, alpha: 1.0, teamplay: 0.0 }
    }
    /// Classic layout: fills the strip below the 3D view (`lines` virtual
    /// rows) edge to edge with the backtile before drawing the bar.
    pub fn backtile(&mut self, lines: f32) {
        let Some((id, tw, th)) = (self.pic)("backtile") else { return };
        let (vw, vh) = (self.c.vw(), self.c.vh());
        let (tw, th) = (tw.max(1) as f32, th.max(1) as f32);
        let mut y = vh - lines;
        while y < vh {
            let mut x = 0.0;
            while x < vw {
                self.c.pic(x, y, Some((id, tw as u32, th as u32)));
                x += tw;
            }
            y += th;
        }
    }

    /// Modern HUD: each element (`HudElem`) is drawn in a box at its
    /// configured anchor; elements sharing an anchor stack away from the
    /// screen edge in `HudElem::ALL` order. Immersive elements show only for
    /// a few seconds after their value changes.
    pub fn draw_modern(&mut self, cl: &ClientState, show_scores: bool, face_anim: bool, cfg: &HudConfig, fade: &mut HudFade, now: f64) {
        let st = &cl.stats;
        if show_scores || st[STAT_HEALTH] <= 0 {
            self.draw_pic(0.0, 0.0, "scorebar");
            self.solo_scoreboard(cl);
            return;
        }
        let (vw, vh) = (self.c.vw(), self.c.vh());
        // screen-corner coordinates
        self.x0 = 0.0;
        self.y0 = 0.0;
        let weapons = self.weapon_row(cl);
        let items = self.item_row(cl);
        fade.update(cl, now);
        let m = 8.0;
        let gap = 4.0;
        // per-anchor offset from the edge consumed so far
        let mut used = [0.0f32; HudAnchor::COUNT];
        // middle anchors stack downward from a centered start: precompute heights
        let mut mid_total = [0.0f32; HudAnchor::COUNT];
        let mut boxes = Vec::new();
        for e in HudElem::ALL {
            let mode = cfg.mode[e as usize];
            let mut alpha = match mode {
                HudMode::Off => continue,
                HudMode::On => 1.0,
                HudMode::Immersive => fade.alpha(e, now),
            };
            // low health / armor always visible
            if matches!(e, HudElem::Health) && st[STAT_HEALTH] <= 25 {
                alpha = 1.0;
            }
            let (w, h) = match e {
                HudElem::Health => (if cfg.face { 100.0 } else { 72.0 }, 24.0),
                HudElem::Armor => {
                    if self.pack.armor_pic(cl.items).is_none() {
                        continue;
                    }
                    (100.0, 24.0)
                }
                HudElem::Ammo => {
                    if self.pack.ammo_pic(cl.items).is_none() {
                        continue;
                    }
                    (100.0, 24.0)
                }
                HudElem::AmmoList => (88.0, 36.0),
                HudElem::Weapons => (weapons.len() as f32 * 24.0, 16.0),
                HudElem::Items => (items.iter().map(|i| i.1).sum::<f32>(), 16.0),
            };
            if w <= 0.0 || alpha <= 0.0 {
                continue;
            }
            let a = cfg.anchor[e as usize];
            mid_total[a as usize] += h + gap;
            boxes.push((e, a, w, h, alpha));
        }
        for (e, a, w, h, alpha) in boxes {
            let ai = a as usize;
            let x = match a.col() {
                0 => m,
                1 => ((vw - w) / 2.0).floor(),
                _ => vw - m - w,
            };
            let y = match a.row() {
                0 => m + used[ai],
                1 => ((vh - mid_total[ai] + gap) / 2.0).floor() + used[ai],
                _ => vh - m - h - used[ai],
            };
            used[ai] += h + gap;
            let right = a.col() == 2;
            self.alpha = alpha;
            self.draw_elem(cl, e, x, y, w, right, face_anim, cfg.face, &weapons, &items);
        }
        self.alpha = 1.0;
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_elem(&mut self, cl: &ClientState, e: HudElem, x: f32, y: f32, w: f32, right: bool, face_anim: bool, face: bool, weapons: &[String], items: &[(String, f32, f32)]) {
        let st = &cl.stats;
        match e {
            HudElem::Health => {
                let mut nx = x;
                if face {
                    let f = Self::face_name(cl, face_anim);
                    self.draw_pic(x, y, &f);
                    nx += 28.0;
                }
                self.draw_num(nx, y, st[STAT_HEALTH], 3, st[STAT_HEALTH] <= 25);
            }
            HudElem::Armor => {
                if let Some(p) = self.pack.armor_pic(cl.items) {
                    self.draw_pic(x, y, p);
                }
                if cl.items & IT_INVULNERABILITY != 0 {
                    self.draw_num(x + 28.0, y, 666, 3, true);
                } else {
                    self.draw_num(x + 28.0, y, st[STAT_ARMOR], 3, st[STAT_ARMOR] <= 25);
                }
            }
            HudElem::Ammo => {
                let p = self.pack.ammo_pic(cl.items).unwrap_or("sb_shells");
                if right {
                    self.draw_num(x, y, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
                    self.draw_pic(x + 76.0, y, p);
                } else {
                    self.draw_pic(x, y, p);
                    self.draw_num(x + 28.0, y, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
                }
            }
            HudElem::AmmoList => {
                let bits = if self.pack == Pack::Rogue {
                    [RIT_SHELLS, RIT_NAILS, RIT_ROCKETS, RIT_CELLS]
                } else {
                    [IT_SHELLS, IT_NAILS, IT_ROCKETS, IT_CELLS]
                };
                let names = ["shells", "nails", "rockets", "cells"];
                for i in 0..4 {
                    let s = if right {
                        format!("{:>7} {:>3}", names[i], st[STAT_SHELLS + i])
                    } else {
                        format!("{:<7} {:>3}", names[i], st[STAT_SHELLS + i])
                    };
                    let sx = if right { x + w - s.len() as f32 * 8.0 } else { x };
                    let gold = cl.items & bits[i] != 0;
                    for (k, b) in s.bytes().enumerate() {
                        // conchars: +128 is the gold/brown alternate set
                        let ch = if gold && b != b' ' { b | 128 } else { b };
                        self.c.ch_color(sx + k as f32 * 8.0, y + i as f32 * 9.0, ch, [1.0, 1.0, 1.0, self.alpha]);
                    }
                }
            }
            HudElem::Weapons => {
                for (i, p) in weapons.iter().enumerate() {
                    self.draw_pic(x + i as f32 * 24.0, y, p);
                }
            }
            HudElem::Items => {
                let mut ix = x;
                for (p, adv, dx) in items {
                    self.draw_pic(ix + dx, y, p);
                    ix += adv;
                }
            }
        }
    }

    /// Owned weapon icons, left to right.
    fn weapon_row(&self, cl: &ClientState) -> Vec<String> {
        let st = &cl.stats;
        let items = cl.items;
        let mut row: Vec<String> = Vec::new();
        let hip = if self.pack == Pack::Hipnotic { hip_weapons(cl) } else { Vec::new() };
        for (i, w) in WEAPONS.iter().enumerate() {
            if items & (IT_SHOTGUN << i) == 0 && !(i == 4 && hip.iter().any(|h| h.0 == 2)) {
                continue;
            }
            if self.pack == Pack::Hipnotic && i == 4 {
                // grenade launcher slot shows the prox variants
                if let Some(h) = hip.iter().find(|h| h.0 == 2) {
                    row.push(h.1.clone());
                    continue;
                }
            }
            let active = st[STAT_ACTIVEWEAPON] == IT_SHOTGUN << i;
            if self.pack == Pack::Rogue && i >= 2 && st[STAT_ACTIVEWEAPON] == RIT_LAVA_NAILGUN << (i - 2) {
                row.push(ROGUE_WEAPONS[i - 2].into());
                continue;
            }
            row.push(format!("{}{w}", if active { "inv2_" } else { "inv_" }));
        }
        for h in hip.iter().filter(|h| h.0 < 2) {
            row.push(h.1.clone());
        }
        row
    }

    /// Keys, powerups, pack items and sigils: (pic, advance, x offset).
    fn item_row(&self, cl: &ClientState) -> Vec<(String, f32, f32)> {
        let items = cl.items;
        let mut row = Vec::new();
        let sigils = if self.pack == Pack::Rogue { 0 } else { 4 };
        for i in 0..sigils {
            if items & (1 << (28 + i)) != 0 {
                row.push((format!("sb_sigil{}", i + 1), 10.0, 0.0));
            }
        }
        if !row.is_empty() {
            row.last_mut().unwrap().1 = 18.0;
        }
        for (i, n) in ITEMS.iter().enumerate() {
            if items & (1 << (17 + i)) != 0 {
                row.push(((*n).into(), 18.0, 0.0));
            }
        }
        for &(bit, n) in self.pack.extra_items() {
            if items & (1 << bit) != 0 {
                row.push((n.into(), 18.0, 0.0));
            }
        }
        if let Some(l) = row.last_mut() {
            l.1 = 16.0;
        }
        row
    }

    fn face_name(cl: &ClientState, face_anim: bool) -> String {
        let items = cl.items;
        if items & (IT_INVISIBILITY | IT_INVULNERABILITY) == IT_INVISIBILITY | IT_INVULNERABILITY {
            "face_inv2".to_string()
        } else if items & IT_QUAD != 0 {
            "face_quad".into()
        } else if items & IT_INVISIBILITY != 0 {
            "face_invis".into()
        } else if items & IT_INVULNERABILITY != 0 {
            "face_invul2".into()
        } else {
            let f = (cl.stats[STAT_HEALTH] / 20).clamp(0, 4);
            if face_anim { format!("face_p{}", 5 - f) } else { format!("face{}", 5 - f) }
        }
    }

    fn draw_pic(&mut self, x: f32, y: f32, name: &str) {
        let p = (self.pic)(name);
        self.c.pic_color(self.x0 + x, self.y0 + y, p, [1.0, 1.0, 1.0, self.alpha]);
    }
    fn draw_char(&mut self, x: f32, y: f32, ch: u8) {
        self.c.ch(self.x0 + x + 4.0, self.y0 + y, ch);
    }
    fn draw_string(&mut self, x: f32, y: f32, s: &str) {
        self.c.string(self.x0 + x, self.y0 + y, s);
    }

    fn draw_num(&mut self, x: f32, y: f32, num: i32, digits: usize, red: bool) {
        let s = num.to_string();
        let s = if s.len() > digits { &s[s.len() - digits..] } else { &s[..] };
        let mut x = x + (digits - s.len()) as f32 * 24.0;
        let pre = if red { "anum_" } else { "num_" };
        for b in s.bytes() {
            let name = if b == b'-' { format!("{pre}minus") } else { format!("{pre}{}", b as char) };
            self.draw_pic(x, y, &name);
            x += 24.0;
        }
    }

    pub fn draw(&mut self, cl: &ClientState, show_scores: bool, face_anim: bool) {
        let st = &cl.stats;
        let items = cl.items;
        // inventory
        self.draw_pic(0.0, -24.0, self.pack.ibar(st[STAT_ACTIVEWEAPON]));
        for (i, wname) in WEAPONS.iter().enumerate() {
            if items & (IT_SHOTGUN << i) == 0 {
                continue;
            }
            let fl = flash(cl, i, st[STAT_ACTIVEWEAPON] == IT_SHOTGUN << i);
            self.draw_pic(i as f32 * 24.0, -16.0, &format!("{}{wname}", weapon_prefix(fl)));
        }
        match self.pack {
            Pack::Hipnotic => {
                for (slot, pic, _) in hip_weapons(cl) {
                    let x = if slot == 2 { 96.0 } else { 176.0 + slot as f32 * 24.0 };
                    self.draw_pic(x, -16.0, &pic);
                }
            }
            Pack::Rogue if st[STAT_ACTIVEWEAPON] >= RIT_LAVA_NAILGUN => {
                for (i, n) in ROGUE_WEAPONS.iter().enumerate() {
                    if st[STAT_ACTIVEWEAPON] == RIT_LAVA_NAILGUN << i {
                        self.draw_pic((i + 2) as f32 * 24.0, -16.0, n);
                    }
                }
            }
            _ => {}
        }
        for i in 0..4 {
            let s = format!("{:3}", st[STAT_SHELLS + i]);
            for (k, b) in s.bytes().enumerate() {
                if b != b' ' {
                    self.draw_char(((6 * i + 1 + k) * 8) as f32 - 2.0, -24.0, 18 + b - b'0');
                }
            }
        }
        for (i, n) in ITEMS.iter().enumerate() {
            // hipnotic draws its keys on the main bar instead
            if items & (1 << (17 + i)) != 0 && (self.pack != Pack::Hipnotic || i > 1) {
                self.draw_pic(192.0 + i as f32 * 16.0, -16.0, n);
            }
        }
        for (i, &(bit, n)) in self.pack.extra_items().iter().enumerate() {
            if items & (1 << bit) != 0 {
                self.draw_pic(288.0 + i as f32 * 16.0, -16.0, n);
            }
        }
        let sigils = if self.pack == Pack::Rogue { 0 } else { 4 };
        for i in 0..sigils {
            if items & (1 << (28 + i)) != 0 {
                self.draw_pic(320.0 - 32.0 + i as f32 * 8.0, -16.0, &format!("sb_sigil{}", i + 1));
            }
        }

        if show_scores || st[STAT_HEALTH] <= 0 {
            self.draw_pic(0.0, 0.0, "scorebar");
            self.solo_scoreboard(cl);
            return;
        }
        self.draw_pic(0.0, 0.0, "sbar");
        if self.pack == Pack::Hipnotic {
            if items & (1 << 17) != 0 {
                self.draw_pic(209.0, 3.0, ITEMS[0]);
            }
            if items & (1 << 18) != 0 {
                self.draw_pic(209.0, 12.0, ITEMS[1]);
            }
        }
        // armor
        if items & IT_INVULNERABILITY != 0 {
            self.draw_num(24.0, 0.0, 666, 3, true);
            self.draw_pic(0.0, 0.0, "disc");
        } else {
            self.draw_num(24.0, 0.0, st[STAT_ARMOR], 3, st[STAT_ARMOR] <= 25);
            if let Some(n) = self.pack.armor_pic(items) {
                self.draw_pic(0.0, 0.0, n);
            }
        }
        // face (rogue CTF: team colour swatch with frags instead)
        if !self.rogue_team_face(cl) {
            let face = Self::face_name(cl, face_anim);
            self.draw_pic(112.0, 0.0, &face);
        }
        self.draw_num(136.0, 0.0, st[STAT_HEALTH], 3, st[STAT_HEALTH] <= 25);
        if let Some(n) = self.pack.ammo_pic(items) {
            self.draw_pic(224.0, 0.0, n);
        }
        self.draw_num(248.0, 0.0, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
    }

    /// Rogue's Sbar_DrawFace team swatch: r_teambord, shirt over pants,
    /// and the frag count (gold digits on a white shirt). Only in
    /// multiplayer with teamplay 4..6 (the CTF modes).
    fn rogue_team_face(&mut self, cl: &ClientState) -> bool {
        if self.pack != Pack::Rogue || cl.maxclients <= 1 || !(self.teamplay > 3.0 && self.teamplay < 7.0) {
            return false;
        }
        let Some(s) = cl.scores.get(cl.viewentity.wrapping_sub(1)) else { return false };
        let (top, bottom) = ((s.colors >> 4) & 15, s.colors & 15);
        let frags = s.frags;
        self.draw_pic(112.0, 0.0, "r_teambord");
        let (x, y) = (self.x0 + 113.0, self.y0);
        self.c.fill(x, y + 3.0, 22.0, 9.0, crate::draw::player_color(top));
        self.c.fill(x, y + 12.0, 22.0, 9.0, crate::draw::player_color(bottom));
        let num = format!("{frags:3}");
        for (i, b) in num.bytes().take(3).enumerate() {
            let ch = if top == 0 && b.is_ascii_digit() { 18 + b - b'0' } else { b };
            if top == 0 && b == b' ' {
                continue;
            }
            self.draw_char(113.0 + i as f32 * 7.0, 3.0, ch);
        }
        true
    }

    fn solo_scoreboard(&mut self, cl: &ClientState) {
        let st = &cl.stats;
        self.draw_string(8.0, 4.0, &format!("Monsters:{:3} /{:3}", st[STAT_MONSTERS], st[STAT_TOTALMONSTERS]));
        self.draw_string(8.0, 12.0, &format!("Secrets :{:3} /{:3}", st[STAT_SECRETS], st[STAT_TOTALSECRETS]));
        let t = cl.time as i32;
        let (m, s) = (t / 60, t % 60);
        self.draw_string(184.0, 4.0, &format!("Time :{:3}:{}{}", m, s / 10, s % 10));
        let name: String = cl.levelname.chars().take(38).collect();
        let x = 232.0 - name.len() as f32 * 4.0;
        self.draw_string(x.max(0.0), 12.0, &name);
    }
}

fn intermission_number(c: &mut Canvas, pic: &mut PicFn, x: f32, y: f32, num: i32, digits: usize) {
    let s = num.to_string();
    let s = if s.len() > digits { &s[s.len() - digits..] } else { &s[..] };
    let mut x = x + (digits - s.len()) as f32 * 24.0;
    for b in s.bytes() {
        let n = if b == b'-' { "num_minus".to_string() } else { format!("num_{}", b as char) };
        let p = pic(&n);
        c.pic(x, y, p);
        x += 24.0;
    }
}

/// Sbar_IntermissionOverlay.
pub fn intermission(c: &mut Canvas, pic: &mut PicFn, cl: &ClientState) {
    c.center_320x200();
    let p = pic("gfx/complete.lmp");
    c.pic(64.0, 24.0, p);
    let p = pic("gfx/inter.lmp");
    c.pic(0.0, 56.0, p);
    let t = cl.completed_time as i32;
    intermission_number(c, pic, 160.0, 64.0, t / 60, 3);
    let colon = pic("num_colon");
    c.pic(234.0, 64.0, colon);
    let s = t % 60;
    intermission_number(c, pic, 246.0, 64.0, s / 10, 1);
    intermission_number(c, pic, 266.0, 64.0, s % 10, 1);
    let slash = pic("num_slash");
    let st = &cl.stats;
    intermission_number(c, pic, 160.0, 104.0, st[STAT_SECRETS], 3);
    c.pic(232.0, 104.0, slash);
    intermission_number(c, pic, 240.0, 104.0, st[STAT_TOTALSECRETS], 3);
    intermission_number(c, pic, 160.0, 144.0, st[STAT_MONSTERS], 3);
    c.pic(232.0, 144.0, slash);
    intermission_number(c, pic, 240.0, 144.0, st[STAT_TOTALMONSTERS], 3);
    c.reset_origin();
}

/// Sbar_DeathmatchOverlay, with QuakeSpasm's ping column: players by
/// frags, each with a shirt/pants box, the viewer's row bracketed.
pub fn deathmatch_overlay(c: &mut Canvas, pic: &mut PicFn, cl: &ClientState) {
    c.center_320x200();
    let p = pic("gfx/ranking.lmp");
    if let Some((_, w, _)) = p {
        c.pic(((320 - w as i32) / 2) as f32, 8.0, p);
    }
    let mut order: Vec<usize> = (0..cl.scores.len()).filter(|&k| !cl.scores[k].name.is_empty()).collect();
    order.sort_by_key(|&k| std::cmp::Reverse(cl.scores[k].frags));
    let me = cl.viewentity.wrapping_sub(1);
    let mut y = 40.0;
    for k in order {
        let s = &cl.scores[k];
        c.string(40.0, y, &format!("{:4}", s.ping.clamp(0, 9999)));
        c.fill(80.0, y, 40.0, 4.0, crate::draw::player_color((s.colors >> 4) & 15));
        c.fill(80.0, y + 4.0, 40.0, 4.0, crate::draw::player_color(s.colors & 15));
        c.string(88.0, y, &format!("{:3}", s.frags));
        if k == me {
            c.ch(80.0, y, 16);
            c.ch(112.0, y, 17);
        }
        c.string(128.0, y, &s.name);
        y += 10.0;
    }
    c.reset_origin();
}

/// Sbar_FinaleOverlay.
pub fn finale(c: &mut Canvas, pic: &mut PicFn) {
    let p = pic("gfx/finale.lmp");
    if let Some((_, w, _)) = p {
        c.reset_origin();
        let x = ((c.vw() - w as f32) / 2.0).floor();
        c.pic(x, 16.0, p);
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
