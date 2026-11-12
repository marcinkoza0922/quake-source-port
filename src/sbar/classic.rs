use super::*;

impl<'a, 'b> Sbar<'a, 'b> {
    pub(super) fn draw_pic(&mut self, x: f32, y: f32, name: &str) {
        let p = (self.pic)(name);
        self.c.pic_color(self.x0 + x, self.y0 + y, p, [1.0, 1.0, 1.0, self.alpha]);
    }
    pub(super) fn draw_char(&mut self, x: f32, y: f32, ch: u8) {
        self.c.ch(self.x0 + x + 4.0, self.y0 + y, ch);
    }
    pub(super) fn draw_string(&mut self, x: f32, y: f32, s: &str) {
        self.c.string(self.x0 + x, self.y0 + y, s);
    }

    pub(super) fn draw_num(&mut self, x: f32, y: f32, num: i32, digits: usize, red: bool) {
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
    pub(super) fn rogue_team_face(&mut self, cl: &ClientState) -> bool {
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

    pub(super) fn solo_scoreboard(&mut self, cl: &ClientState) {
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
