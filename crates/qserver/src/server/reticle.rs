//! Reticle target query uses the same collision trace as weapon fire.
use super::*;

impl Server {
    /// True when the first solid hit can take damage, including buttons/props.
    pub fn reticle_on_target(&mut self, shooter: u32, eye: Vec3, direction: Vec3) -> bool {
        let Some(l) = &mut self.level else { return false };
        let damage = self.core.sv.fields.takedamage;
        let ents = Ents { vm: &mut l.vm, c: &mut self.core };
        let trace = l.world.trace(&ents, eye, Vec3::ZERO, Vec3::ZERO,
            eye + direction * 8192.0, qworld::MoveType::Normal, Some(shooter));
        trace.ent.is_some_and(|e| e != 0 && ents.vm.ent_f(e, damage) != 0.0)
    }
}
