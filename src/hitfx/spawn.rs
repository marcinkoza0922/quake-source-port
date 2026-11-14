//! Materials and the particle recipe each one throws off when hit.
use super::{HitFx, Kind, Part};
use qcore::math::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Material {
    Metal,
    Stone,
    Wood,
    Dirt,
    Water,
    Slime,
    Lava,
    /// Sky and other surfaces that show nothing.
    None,
}

/// BSP has no material metadata, so go by conventional texture names.
/// Unknown textures count as stone, the most common Quake surface.
pub(crate) fn classify(name: &str) -> Material {
    let n = name.to_ascii_lowercase();
    let has = |list: &[&str]| list.iter().any(|s| n.contains(s));
    if n.starts_with("sky") || n.contains("trigger") || n.contains("clip") {
        return Material::None;
    }
    if n.starts_with('*') || n.starts_with('!') {
        return if n.contains("lava") {
            Material::Lava
        } else if n.contains("slime") {
            Material::Slime
        } else if n.contains("tele") {
            Material::None
        } else {
            Material::Water
        };
    }
    if crate::impacts::surfaces::wood(&n) {
        return Material::Wood;
    }
    if has(&["metal", "tech", "comp", "plat", "pipe", "grate", "rivet", "steel", "iron", "copper", "bronze",
        "silver", "gold", "button", "light", "slip", "mtl", "chain", "vent", "door", "key", "+0exit", "exit"]) {
        return Material::Metal;
    }
    if has(&["ground", "dirt", "grass", "mud", "sand", "earth", "moss", "soil", "rubble"]) {
        return Material::Dirt;
    }
    Material::Stone
}

/// Average colour of a miptex, so chips and dust match the wall.
pub(crate) fn average(tex: &qformats::bsp::MipTex, pal: &[u8]) -> [u8; 3] {
    let Some(px) = tex.mips.last().filter(|m| !m.is_empty() && pal.len() >= 768) else {
        return [120, 115, 105];
    };
    let mut sum = [0u32; 3];
    for &i in px {
        for (c, s) in sum.iter_mut().enumerate() {
            *s += pal[i as usize * 3 + c] as u32;
        }
    }
    sum.map(|s| (s / px.len() as u32) as u8)
}

/// Particle count multiplier and size/life scale for `r_impact_fx` 1..=4.
pub(crate) fn scale(level: u8) -> (f32, f32) {
    match level {
        0 => (0.0, 0.0),
        1 => (1.0, 0.8),
        2 => (2.5, 1.0),
        3 => (5.0, 1.25),
        _ => (10.0, 1.6),
    }
}

fn shade(c: [u8; 3], k: f32) -> [u8; 3] {
    c.map(|v| (v as f32 * k).clamp(0.0, 255.0) as u8)
}

fn mix(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    [0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8)
}

struct Ctx {
    pos: Vec3,
    normal: Vec3,
    /// Reflection of the shot off the surface.
    bounce: Vec3,

    many: f32,
    big: f32,
}

impl HitFx {
    fn count(&mut self, n: f32) -> usize {
        // Fractional counts become a chance, so subtle can mean "sometimes".
        let whole = n.floor();
        whole as usize + usize::from(self.rand() < n - whole)
    }

    /// Random direction in a cone around `axis`; `spread` 0 = straight, 1 = hemisphere.
    fn cone(&mut self, axis: Vec3, spread: f32) -> Vec3 {
        let r = Vec3::new(self.crand(), self.crand(), self.crand());
        let d = (axis + r * spread).normalize();
        if d.dot(axis) < 0.0 { (d - axis * (2.0 * d.dot(axis))).normalize() } else { d }
    }

    fn sparks(&mut self, c: &Ctx, n: f32, speed: f32, color: [u8; 3]) {
        for _ in 0..self.count(n * c.many) {
            let axis = (c.normal + c.bounce).normalize();
            let d = self.cone(axis, 0.9);
            let v = d * speed * (0.4 + self.rand() * 0.9);
            let life = (0.25 + self.rand() * 0.45) * c.big;
            self.parts.push(Part { kind: Kind::Spark, pos: c.pos, vel: v, life, size: 0.5 * c.big, color,
                gravity: 0.9, drag: 1.2, bounces: 2, ..Default::default() });
        }
    }

    fn flash(&mut self, c: &Ctx, size: f32, color: [u8; 3]) {
        self.parts.push(Part { kind: Kind::Flash, pos: c.pos + c.normal, life: 0.07, size: size * c.big, color,
            ..Default::default() });
    }

    fn smoke(&mut self, c: &Ctx, n: f32, color: [u8; 3], alpha: f32, size: f32, rise: f32) {
        for _ in 0..self.count(n * c.many.sqrt()) {
            let d = self.cone(c.normal, 0.6);
            let v = d * (15.0 + self.rand() * 35.0) + Vec3::new(0.0, 0.0, rise);
            let life = (0.9 + self.rand() * 1.0) * c.big;
            let col = shade(color, 0.85 + self.rand() * 0.3);
            let (sz, angle, spin) = (size * c.big * (0.7 + self.rand() * 0.6), self.rand() * 6.3, self.crand() * 1.5);
            self.parts.push(Part { kind: Kind::Smoke, pos: c.pos + c.normal * 2.0, vel: v, life,
                size: sz, grow: size * 3.0 * c.big, color: col, alpha,
                gravity: -0.004, drag: 2.5, angle, spin, ..Default::default() });
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn chips(&mut self, c: &Ctx, n: f32, speed: f32, color: [u8; 3], size: f32, stretch: f32, bounces: u8) {
        for _ in 0..self.count(n * c.many) {
            let d = self.cone((c.normal * 1.5 + c.bounce).normalize(), 0.8);
            let v = d * speed * (0.4 + self.rand() * 0.8);
            let s = size * c.big * (0.6 + self.rand() * 0.8);
            let life = (1.2 + self.rand() * 1.5) * c.big;
            let (color, angle, spin) = (shade(color, 0.7 + self.rand() * 0.5), self.rand() * 6.3, self.crand() * 20.0);
            self.parts.push(Part { kind: Kind::Chip, pos: c.pos, vel: v, life, size: s, stretch,
                color, gravity: 1.0, drag: 0.4, angle, spin, bounces, ..Default::default() });
        }
    }
}

/// A bullet (`kind` 0) or nail (1) hitting `mat` at `pos`, travelling along `dir`.
pub(crate) fn hit(fx: &mut HitFx, (mat, tint): (Material, [u8; 3]), pos: Vec3, normal: Vec3, dir: Vec3, kind: u8, level: u8) {
    let (many, big) = scale(level);
    // Nails are single heavy projectiles; shotguns send one hit per pellet.
    let many = if kind == 1 { many * 1.6 } else { many };
    let bounce = dir - normal * (2.0 * dir.dot(normal));
    let c = Ctx { pos, normal, bounce, many, big };
    let dust = mix(tint, [200, 195, 185], 0.35);
    match mat {
        Material::None => {}
        Material::Metal => {
            fx.flash(&c, 5.0, [255, 210, 140]);
            fx.sparks(&c, 3.0, 420.0, [255, 200, 110]);
            fx.smoke(&c, 0.5, [90, 88, 85], 0.35, 2.5, 6.0);
            fx.chips(&c, 0.3, 160.0, shade(tint, 0.6), 0.5, 1.0, 2);
        }
        Material::Stone => {
            fx.chips(&c, 2.0, 180.0, tint, 0.8, 1.0, 2);
            fx.smoke(&c, 1.5, dust, 0.45, 3.5, 4.0);
            fx.sparks(&c, 0.4, 300.0, [255, 190, 120]);
        }
        Material::Wood => {
            fx.chips(&c, 2.5, 170.0, mix(tint, [190, 150, 95], 0.4), 0.55, 4.0, 2);
            fx.smoke(&c, 0.8, mix(dust, [170, 140, 100], 0.4), 0.35, 3.0, 3.0);
        }
        Material::Dirt => {
            fx.chips(&c, 2.5, 200.0, shade(tint, 0.8), 0.9, 1.0, 1);
            fx.smoke(&c, 2.0, mix(dust, [130, 105, 75], 0.5), 0.5, 4.5, 2.0);
        }
        Material::Water | Material::Slime => {
            let col = if mat == Material::Slime { [90, 160, 40] } else { mix(tint, [170, 190, 210], 0.6) };
            let up = Ctx { bounce: normal, ..c };
            fx.chips(&up, 4.0, 220.0, col, 0.5, 2.0, 0);
            fx.smoke(&up, 1.0, mix(col, [230, 235, 240], 0.6), 0.3, 3.0, 10.0);
        }
        Material::Lava => {
            fx.sparks(&c, 3.0, 200.0, [255, 120, 30]);
            fx.smoke(&c, 1.0, [45, 38, 35], 0.5, 4.0, 25.0);
        }
    }
}

/// Debris and a dust cloud from the surface nearest an explosion.
pub(crate) fn blast(fx: &mut HitFx, (mat, tint): (Material, [u8; 3]), pos: Vec3, normal: Vec3, level: u8) {
    if level < 2 || matches!(mat, Material::None | Material::Lava) {
        return;
    }
    let (many, big) = scale(level);
    let c = Ctx { pos: pos + normal * 2.0, normal, bounce: normal, many, big };
    let dust = mix(tint, [200, 195, 185], 0.35);
    match mat {
        Material::Water | Material::Slime => {
            fx.chips(&c, 8.0, 380.0, mix(tint, [200, 210, 220], 0.6), 0.7, 2.0, 0);
            fx.smoke(&c, 4.0, [215, 220, 225], 0.35, 8.0, 20.0);
        }
        _ => {
            let stretch = if mat == Material::Wood { 4.0 } else { 1.0 };
            fx.chips(&c, 6.0, 420.0, tint, 1.4, stretch, 3);
            fx.smoke(&c, 5.0, dust, 0.55, 9.0, 12.0);
            if mat == Material::Metal {
                fx.sparks(&c, 8.0, 600.0, [255, 200, 110]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texture_names_pick_materials() {
        for (name, m) in [
            ("metal1_2", Material::Metal), ("TECH04_1", Material::Metal), ("+0button", Material::Metal),
            ("wood1_1", Material::Wood), ("crate0", Material::Wood), ("ground1_6", Material::Dirt),
            ("*water0", Material::Water), ("*slime", Material::Slime), ("*lava1", Material::Lava),
            ("*teleport", Material::None), ("sky1", Material::None), ("wbrick1_5", Material::Stone),
            ("city4_7", Material::Stone),
        ] {
            assert_eq!(classify(name), m, "{name}");
        }
    }

    #[test]
    fn intensity_grows_from_subtle_to_spectacle() {
        let count = |level| {
            let mut fx = HitFx::default();
            for _ in 0..20 {
                hit(&mut fx, (Material::Metal, [100; 3]), Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0),
                    Vec3::new(-1.0, 0.0, 0.0), 0, level);
            }
            fx.parts.len()
        };
        let n: Vec<_> = (1..=4).map(count).collect();
        assert!(n[0] > 0 && n.windows(2).all(|w| w[0] < w[1]), "{n:?}");
    }

    #[test]
    fn sky_and_subtle_explosions_show_nothing() {
        let mut fx = HitFx::default();
        hit(&mut fx, (Material::None, [0; 3]), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.0, -1.0), 0, 4);
        blast(&mut fx, (Material::Stone, [0; 3]), Vec3::ZERO, Vec3::new(0.0, 0.0, 1.0), 1);
        assert!(fx.parts.is_empty());
    }
}
