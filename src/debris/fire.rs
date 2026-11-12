//! Where a shot came from. Impact temp entities carry only the hit point,
//! so the line of fire is inferred: a fast projectile (nail, laser) that
//! vanished next to the impact gives its flight line; otherwise the nearest
//! entity with a recent muzzle flash and a clear line to the impact is the
//! shooter (the player's view when it was the player's own flash); with no
//! candidate the view is used.
use super::*;

/// Alias entities faster than this (units/s) count as projectiles.
const MISSILE_SPEED: f32 = 250.0;
/// A projectile counts for an impact this close to its last position.
const MISSILE_REACH: f32 = 160.0;
/// Muzzle flashes are remembered this long (seconds).
const FLASH_MEMORY: f64 = 0.15;
/// Eye height above an entity's origin.
const EYE: f32 = 16.0;

fn velocity(cl: &ClientState, e: &qclient::ClientEntity) -> Vec3 {
    let dt = cl.mtime[0] - cl.mtime[1];
    let d = Vec3::from(e.msg_origins[0]) - Vec3::from(e.msg_origins[1]);
    if !(0.001..0.5).contains(&dt) || d.length() > 400.0 {
        return Vec3::ZERO;
    }
    d / dt as f32
}

impl Debris {
    /// Remembers this frame's projectiles and muzzle flashes for the next
    /// frame's impacts (a projectile is gone by the time its impact arrives).
    pub(super) fn track_shooters(&mut self, cl: &ClientState, lv: &Level, time: f64) {
        self.missiles.clear();
        self.flashes.retain(|f| time - f.1 < FLASH_MEMORY);
        for (i, e) in cl.active_entities() {
            if e.effects & qclient::EF_MUZZLEFLASH != 0 {
                let eye = (i != cl.viewentity).then(|| Vec3::from(e.origin) + Vec3::new(0.0, 0.0, EYE));
                self.flashes.retain(|f| f.2 != i);
                self.flashes.push((eye, time, i));
            }
            if i == cl.viewentity || lv.models.get(e.model).copied().flatten().is_none_or(|m| m.kind != Kind::Alias) {
                continue;
            }
            let v = velocity(cl, e);
            if v.length() > MISSILE_SPEED {
                self.missiles.push((Vec3::from(e.origin), v));
            }
        }
    }

    /// A point on the line of fire behind impact `p`.
    pub(super) fn shot_origin(&self, view: Vec3, p: Vec3) -> Vec3 {
        let cm = self.cm.as_deref();
        let missile = self
            .missiles
            .iter()
            .map(|&(o, v)| (o, v, (p - o).length()))
            .filter(|&(o, v, d)| d < MISSILE_REACH && (p - o).dot(v) > -16.0 * v.length())
            .min_by(|a, b| a.2.total_cmp(&b.2));
        if let Some((_, v, _)) = missile {
            let dir = v.normalize();
            let back = p - dir * 512.0;
            return cm.map_or(back, |cm| cm.trace(p - dir * 2.0, back).1);
        }
        let clear = |eye: Vec3| cm.is_none_or(|cm| (cm.trace(eye, p).1 - p).length() < 24.0);
        self.flashes
            .iter()
            .map(|f| f.0.unwrap_or(view))
            .filter(|&eye| clear(eye))
            .min_by(|a, b| (*a - p).length_sq().total_cmp(&(*b - p).length_sq()))
            .unwrap_or(view)
    }
}
