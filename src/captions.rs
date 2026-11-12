//! Sound captions (accessibility, `cl_captions`): short text shown at the
//! bottom of the screen for sounds that carry gameplay information.

use crate::console::TEXT_BG;
use crate::draw::{Canvas, WHITE};
use crate::font::Shape;

/// How long a caption stays up, in seconds.
const HOLD: f64 = 2.5;
const MAX_SHOWN: usize = 4;

/// (sound path prefix, caption). The first matching prefix wins, so put
/// specific paths before their directory.
const TABLE: &[(&str, &str)] = &[
    ("misc/secret", "[Secret found]"),
    ("misc/talk", "[Message]"),
    ("misc/trigger1", "[Switch activated]"),
    ("misc/medkey", "[Key picked up]"),
    ("misc/runekey", "[Rune picked up]"),
    ("misc/basekey", "[Key picked up]"),
    ("misc/power", "[Teleporter]"),
    ("misc/r_tele", "[Teleport]"),
    ("misc/h2ohit", "[Splash]"),
    ("misc/water", "[Splash]"),
    ("misc/outwater", "[Leaving water]"),
    ("misc/null", ""),
    ("doors/", "[Door]"),
    ("plats/", "[Platform moving]"),
    ("buttons/", "[Button]"),
    ("items/r_item", "[Health picked up]"),
    ("items/health", "[Health picked up]"),
    ("items/armor", "[Armor picked up]"),
    ("items/damage3", "[Quad Damage wearing off]"),
    ("items/damage", "[Quad Damage]"),
    ("items/protect3", "[Pentagram wearing off]"),
    ("items/protect", "[Pentagram of Protection]"),
    ("items/inv3", "[Ring wearing off]"),
    ("items/inv", "[Ring of Shadows]"),
    ("items/suit2", "[Biosuit wearing off]"),
    ("items/suit", "[Biosuit]"),
    ("items/itembk2", "[Item picked up]"),
    ("weapons/pkup", "[Weapon picked up]"),
    ("weapons/lock4", "[Ammo picked up]"),
    ("weapons/r_exp3", "[Explosion]"),
    ("weapons/grenade", "[Grenade launched]"),
    ("weapons/bounce", "[Grenade bounce]"),
    ("weapons/sgun1", "[Rocket fired]"),
    ("weapons/spike2", "[Nails fired]"),
    ("weapons/lhit", "[Lightning]"),
    ("ambience/thunder", "[Thunder]"),
    ("enforcer/sight", "[Enforcer alerted]"),
    ("soldier/sight", "[Grunt alerted]"),
    ("dog/dsight", "[Rottweiler alerted]"),
    ("knight/ksight", "[Knight alerted]"),
    ("hknight/sight", "[Death Knight alerted]"),
    ("wizard/wsight", "[Scrag alerted]"),
    ("ogre/ogwake", "[Ogre alerted]"),
    ("demon/sight", "[Fiend alerted]"),
    ("shambler/ssight", "[Shambler alerted]"),
    ("zombie/z_idle", "[Zombie groaning]"),
    ("shalrath/sight", "[Vore alerted]"),
    ("shalrath/attack", "[Vore attacking]"),
    ("tarbaby/sight", "[Spawn alerted]"),
    ("fish/", "[Rotfish]"),
    ("blob/", "[Spawn]"),
    ("boss1/", "[Chthon]"),
    ("player/land2", "[Hard landing]"),
    ("player/drown", "[Drowning]"),
    ("player/gasp", "[Gasping for air]"),
    ("player/lburn", "[Burning]"),
    ("player/slimbrn", "[Burning]"),
    ("player/teledth", "[Telefrag]"),
    ("player/tornoff", "[Gibbed]"),
    ("player/gib", "[Gibbed]"),
    ("player/udeath", "[Gibbed]"),
];

/// Caption for a sound path, if it has one.
pub fn caption_for(sound: &str) -> Option<&'static str> {
    let s = sound.strip_prefix("sound/").unwrap_or(sound);
    TABLE.iter().find(|(p, _)| s.starts_with(p)).map(|&(_, c)| c).filter(|c| !c.is_empty())
}

#[derive(Default)]
pub struct Captions {
    /// (caption, time shown, repeat count), oldest first.
    shown: Vec<(&'static str, f64, u32)>,
}

impl Captions {
    /// Records a played sound at `now`.
    pub fn sound(&mut self, sound: &str, now: f64) {
        let Some(text) = caption_for(sound) else { return };
        if let Some(e) = self.shown.iter_mut().find(|e| e.0 == text) {
            e.1 = now;
            e.2 += 1;
        } else {
            self.shown.push((text, now, 1));
            if self.shown.len() > MAX_SHOWN {
                self.shown.remove(0);
            }
        }
    }

    /// Draws the live captions centred just above `bottom` (virtual units)
    /// in the modern UI font; `scale` is `scr_textscale`.
    pub fn draw(&mut self, c: &mut Canvas, now: f64, bottom: f32, scale: f32) {
        self.shown.retain(|e| now - e.1 < HOLD && e.1 <= now);
        if self.shown.is_empty() {
            return;
        }
        if c.ui.is_none() {
            return self.draw_classic(c, bottom, scale);
        }
        c.reset_origin();
        let size = 8.0 * scale.clamp(1.0, 3.0);
        let (pad_x, pad_y, gap) = (size * 0.6, size * 0.3, size * 0.25);
        let row = size + pad_y * 2.0 + gap;
        let cx = c.vw() / 2.0;
        let mut y = bottom - row * self.shown.len() as f32;
        let r = c.upx(size * 0.35);
        for &(text, _, n) in &self.shown {
            let s = if n > 1 { format!("{text}  x{n}") } else { text.to_string() };
            let w = c.text_w(&s, size);
            c.shape(cx - w / 2.0 - pad_x, y, w + pad_x * 2.0, size + pad_y * 2.0, Shape::Round { r }, TEXT_BG);
            c.text_center(cx, y + pad_y, &s, size, WHITE);
            y += row;
        }
    }

    /// conchars fallback when the UI font atlas isn't available.
    fn draw_classic(&self, c: &mut Canvas, bottom: f32, scale: f32) {
        let base = c.scale;
        c.scale = (base * scale.clamp(1.0, 3.0)).round().max(1.0);
        let bottom = bottom * base / c.scale;
        c.reset_origin();
        let vw = c.vw();
        let mut y = bottom - 12.0 * self.shown.len() as f32;
        for &(text, _, n) in &self.shown {
            let s = if n > 1 { format!("{text} x{n}") } else { text.to_string() };
            let w = s.len() as f32 * 8.0;
            let x = ((vw - w) / 2.0).floor();
            c.fill(x - 4.0, y - 2.0, w + 8.0, 12.0, TEXT_BG);
            c.string(x, y, &s);
            y += 12.0;
        }
        c.scale = base;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_and_merge() {
        assert_eq!(caption_for("doors/drclos4.wav"), Some("[Door]"));
        assert_eq!(caption_for("items/damage3.wav"), Some("[Quad Damage wearing off]"));
        assert_eq!(caption_for("items/damage.wav"), Some("[Quad Damage]"));
        assert_eq!(caption_for("misc/null.wav"), None);
        assert_eq!(caption_for("player/axhit1.wav"), None);
        let mut c = Captions::default();
        c.sound("doors/drclos4.wav", 1.0);
        c.sound("doors/doormv1.wav", 1.1);
        assert_eq!(c.shown.len(), 1);
        assert_eq!(c.shown[0].2, 2);
    }
}
