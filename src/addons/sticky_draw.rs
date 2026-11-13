//! Stickybomb presentation: the launcher view model and every bomb from
//! `ClientState::stickies`, blinking with a small red glow.
use qrender::{DLight, ModelId, RenderEntity, Renderer, View};

use super::sticky_model;

fn model(key: &str, build: fn(&[u8]) -> qformats::mdl::Mdl, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<ModelId> {
    if !lv.by_name.contains_key(key) {
        let palette = fs.open("gfx/palette.lmp").unwrap_or_default();
        let m = r.load_alias(&build(&palette)).ok().map(|id| crate::assets::Model { id, kind: crate::assets::Kind::Alias, flags: 0 });
        lv.by_name.insert(key.into(), m);
    }
    lv.by_name.get(key).copied().flatten().map(|m| m.id)
}

/// Forward push past the portal gun's spot and drop below it.
const PULL: f32 = 6.0;
const DROP: f32 = 7.0;

/// The held launcher, placed like the portal gun.
pub fn weapon(time: f64, view: View, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer) -> Option<RenderEntity> {
    let id = model("@sticky_launcher", sticky_model::launcher, lv, fs, r)?;
    let bob = (time as f32 * 2.5).sin() * 0.15;
    // the imported mesh's muzzle is at -X: turn it round and push it out so
    // it spans the same depth as the portal gun
    // (alias pitch is inverted, so the reversed gun takes the view's pitch
    // as is and the opposite roll)
    let (origin, _) = crate::view::weapon::portal_pose(view, bob, 0.0);
    let (forward, _, up) = qcore::math::angle_vectors(view.angles.into());
    let origin = (qcore::math::Vec3::from(origin) + forward * PULL - up * DROP).to_array();
    // centred (the Right/Left setting adds the side offset), muzzle raised
    // so the barrel shows past the drum
    let angles = [view.angles[0] - 10.0, view.angles[1] + 180.0, -view.angles[2]];
    Some(RenderEntity { model: id, origin, angles, viewmodel: true, ..Default::default() })
}

/// Bomb `id` shows its lit skin now (a short blink about twice a second).
pub fn lit(id: u32, time: f64) -> bool {
    (time * 2.0 + f64::from(id % 7) * 0.13).fract() < 0.2
}

/// Every bomb as an entity, plus a dim red light on each blink.
pub fn bombs(cl: &qclient::ClientState, lv: &mut crate::assets::Level, fs: &qcore::fs::Fs, r: &mut Renderer, ents: &mut Vec<RenderEntity>, dls: &mut Vec<DLight>) {
    if cl.stickies.is_empty() {
        return;
    }
    let Some(id) = model("@sticky_bomb", sticky_model::bomb, lv, fs, r) else { return };
    for &(b, origin) in &cl.stickies {
        let on = lit(b, cl.time);
        let spin = (b % 360) as f32;
        ents.push(RenderEntity { model: id, origin, angles: [0.0, spin, 0.0], skin: on as u32, ..Default::default() });
        if on {
            dls.push(DLight { origin, radius: 48.0, minlight: 0.0, color: [1.0, 0.25, 0.15] });
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bombs_blink_briefly() {
        let lit = (0..100).filter(|&k| super::lit(3, k as f64 * 0.01)).count();
        assert!((10..=30).contains(&lit));
    }
}
