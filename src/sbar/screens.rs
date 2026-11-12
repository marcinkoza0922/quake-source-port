use super::*;

pub(super) fn intermission_number(c: &mut Canvas, pic: &mut PicFn, x: f32, y: f32, num: i32, digits: usize) {
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
