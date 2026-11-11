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

const WEAPONS: [&str; 7] = ["shotgun", "sshotgun", "nailgun", "snailgun", "rlaunch", "srlaunch", "lightng"];
const ITEMS: [&str; 6] = ["sb_key1", "sb_key2", "sb_invis", "sb_invuln", "sb_suit", "sb_quad"];

pub struct Sbar<'a, 'b> {
    pub c: &'a mut Canvas,
    pub pic: &'a mut PicFn<'b>,
    /// Top of the status bar in virtual units.
    y0: f32,
    x0: f32,
}

impl<'a, 'b> Sbar<'a, 'b> {
    pub fn new(c: &'a mut Canvas, pic: &'a mut PicFn<'b>) -> Self {
        c.reset_origin();
        let x0 = ((c.vw() - 320.0) / 2.0).floor();
        let y0 = c.vh() - 24.0;
        Self { c, pic, y0, x0 }
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

    /// Modern corner HUD: armor + health bottom-left, ammo bottom-right,
    /// keys/powerups/sigils above the ammo, weapon inventory bottom-centre.
    pub fn draw_modern(&mut self, cl: &ClientState, show_scores: bool, face_anim: bool) {
        let st = &cl.stats;
        let items = cl.items;
        if show_scores || st[STAT_HEALTH] <= 0 {
            self.draw_pic(0.0, 0.0, "scorebar");
            self.solo_scoreboard(cl);
            return;
        }
        let (vw, vh) = (self.c.vw(), self.c.vh());
        // switch to screen-corner coordinates
        self.x0 = 0.0;
        self.y0 = 0.0;
        let m = 8.0;
        let y = vh - 24.0 - m;
        // health with face
        let face = Self::face_name(cl, face_anim);
        self.draw_pic(m, y, &face);
        self.draw_num(m + 28.0, y, st[STAT_HEALTH], 3, st[STAT_HEALTH] <= 25);
        // armor above health
        let armor_pic = if items & IT_INVULNERABILITY != 0 {
            Some("disc")
        } else {
            [(IT_ARMOR3, "sb_armor3"), (IT_ARMOR2, "sb_armor2"), (IT_ARMOR1, "sb_armor1")]
                .into_iter()
                .find(|(b, _)| items & b != 0)
                .map(|(_, n)| n)
        };
        if let Some(p) = armor_pic {
            let ya = y - 28.0;
            self.draw_pic(m, ya, p);
            if items & IT_INVULNERABILITY != 0 {
                self.draw_num(m + 28.0, ya, 666, 3, true);
            } else {
                self.draw_num(m + 28.0, ya, st[STAT_ARMOR], 3, st[STAT_ARMOR] <= 25);
            }
        }
        // current ammo bottom-right
        let ammo_pic = [(IT_SHELLS, "sb_shells"), (IT_NAILS, "sb_nails"), (IT_ROCKETS, "sb_rocket"), (IT_CELLS, "sb_cells")]
            .into_iter()
            .find(|(b, _)| items & b != 0)
            .map(|(_, n)| n);
        let xr = vw - m - 24.0 * 3.0;
        if let Some(p) = ammo_pic {
            self.draw_num(xr - 28.0, y, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
            self.draw_pic(vw - m - 24.0, y, p);
        }
        // all ammo counts (small), current type highlighted, above the ammo
        let ammo_bits = [IT_SHELLS, IT_NAILS, IT_ROCKETS, IT_CELLS];
        let names = ["shells", "nails", "rockets", "cells"];
        for i in 0..4 {
            let yy = y - 8.0 - (4 - i) as f32 * 9.0;
            let s = format!("{} {:>3}", names[i], st[STAT_SHELLS + i]);
            let x = vw - m - s.len() as f32 * 8.0;
            let gold = items & ammo_bits[i] != 0;
            for (k, b) in s.bytes().enumerate() {
                // conchars: +128 is the gold/brown alternate set
                let ch = if gold { b | 128 } else { b };
                self.c.ch(x + k as f32 * 8.0, yy, ch);
            }
        }
        // owned weapons, small icons in a row above the ammo list
        let owned: Vec<usize> = (0..WEAPONS.len()).filter(|i| items & (IT_SHOTGUN << i) != 0).collect();
        let yw = y - 8.0 - 4.0 * 9.0 - 20.0;
        let mut x = vw - m - owned.len() as f32 * 24.0;
        for i in owned {
            let active = st[STAT_ACTIVEWEAPON] == IT_SHOTGUN << i;
            let pre = if active { "inv2_" } else { "inv_" };
            self.draw_pic(x, yw, &format!("{pre}{}", WEAPONS[i]));
            x += 24.0;
        }
        // keys, powerups, sigils: row above the weapons
        let mut x = vw - m - 16.0;
        let yi = yw - 20.0;
        for (i, n) in ITEMS.iter().enumerate().rev() {
            if items & (1 << (17 + i)) != 0 {
                self.draw_pic(x, yi, n);
                x -= 18.0;
            }
        }
        for i in (0..4).rev() {
            if items & (1 << (28 + i)) != 0 {
                self.draw_pic(x + 8.0, yi, &format!("sb_sigil{}", i + 1));
                x -= 10.0;
            }
        }
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
        self.c.pic(self.x0 + x, self.y0 + y, p);
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
        self.draw_pic(0.0, -24.0, "ibar");
        for (i, wname) in WEAPONS.iter().enumerate() {
            if items & (IT_SHOTGUN << i) == 0 {
                continue;
            }
            let t = cl.item_gettime[i];
            let mut flash = ((cl.time - t) * 10.0) as i32;
            if flash >= 10 {
                flash = (st[STAT_ACTIVEWEAPON] == IT_SHOTGUN << i) as i32;
            } else {
                flash = flash % 5 + 2;
            }
            let pre = match flash {
                0 => "inv_".to_string(),
                1 => "inv2_".to_string(),
                n => format!("inva{}_", n - 1),
            };
            self.draw_pic(i as f32 * 24.0, -16.0, &format!("{pre}{wname}"));
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
            if items & (1 << (17 + i)) != 0 {
                self.draw_pic(192.0 + i as f32 * 16.0, -16.0, n);
            }
        }
        for i in 0..4 {
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
        // armor
        if items & IT_INVULNERABILITY != 0 {
            self.draw_num(24.0, 0.0, 666, 3, true);
            self.draw_pic(0.0, 0.0, "disc");
        } else {
            self.draw_num(24.0, 0.0, st[STAT_ARMOR], 3, st[STAT_ARMOR] <= 25);
            for (bit, n) in [(IT_ARMOR3, "sb_armor3"), (IT_ARMOR2, "sb_armor2"), (IT_ARMOR1, "sb_armor1")] {
                if items & bit != 0 {
                    self.draw_pic(0.0, 0.0, n);
                    break;
                }
            }
        }
        // face
        let face = Self::face_name(cl, face_anim);
        self.draw_pic(112.0, 0.0, &face);
        self.draw_num(136.0, 0.0, st[STAT_HEALTH], 3, st[STAT_HEALTH] <= 25);
        for (bit, n) in [(IT_SHELLS, "sb_shells"), (IT_NAILS, "sb_nails"), (IT_ROCKETS, "sb_rocket"), (IT_CELLS, "sb_cells")] {
            if items & bit != 0 {
                self.draw_pic(224.0, 0.0, n);
                break;
            }
        }
        self.draw_num(248.0, 0.0, st[STAT_AMMO], 3, st[STAT_AMMO] <= 10);
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

/// Sbar_FinaleOverlay.
pub fn finale(c: &mut Canvas, pic: &mut PicFn) {
    let p = pic("gfx/finale.lmp");
    if let Some((_, w, _)) = p {
        c.reset_origin();
        let x = ((c.vw() - w as f32) / 2.0).floor();
        c.pic(x, 16.0, p);
    }
}
