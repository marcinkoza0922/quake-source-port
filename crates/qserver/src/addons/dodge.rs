//! Ground and airborne directional launches and a model-independent third-person pose.
use qcore::math::{Vec3, angle_vectors};
use qvm::Vm;
use crate::{client::ClientMove, server::Core};

#[derive(Default)]
pub struct Dodge {
    start: Option<f64>,
    forward: f32,
    side: f32,
}
impl Dodge {
    /// Pitch, roll and body lowering; eased to neutral at both ends.
    pub fn pose(&self, time: f64) -> [f32; 3] {
        let Some(start) = self.start else { return [0.0; 3] };
        let t = ((time - start) / 0.65) as f32;
        if !(0.0..1.0).contains(&t) { return [0.0; 3]; }
        let weight = (std::f32::consts::PI * t).sin().powi(2);
        [-self.forward * 65.0 * weight, self.side * 30.0 * weight, -6.0 * weight]
    }
}

pub(crate) fn apply(states: &mut Vec<Dodge>, vm: &mut Vm, c: &mut Core, i: usize, mv: ClientMove) {
    states.resize_with(c.maxclients(), Dodge::default);
    let enabled = super::active(c) && c.cvars.value_of("addon_dodge") != 0.0;
    let st = &mut states[i];
    let pressed = mv.buttons & super::BUTTON_DODGE != 0;
    c.clients[i].cmd.buttons &= !super::BUTTON_DODGE;
    let e = i as u32 + 1;
    let f = &c.sv.fields;
    if !enabled || vm.ent_f(e, f.health) <= 0.0 {
        st.start = None;
        return;
    }
    if !pressed || st.start.is_some_and(|t| vm.time - t < 0.8)
        || vm.ent_f(e, f.movetype) != 3.0 || vm.ent_f(e, f.waterlevel) >= 2.0 {
        return;
    }
    let Some((forward, side)) = direction(mv.forward, mv.side) else { return };
    let (front, right, _) = angle_vectors(Vec3::new(0.0, mv.angles[1], 0.0));
    let velocity = (front * forward + right * side) * 550.0;
    vm.set_ent_v(e, f.velocity, [velocity.x, velocity.y, if forward > 0.5 { 190.0 } else { 250.0 }]);
    let flags = vm.ent_f(e, f.flags) as i32;
    vm.set_ent_f(e, f.flags, (flags & !512) as f32);
    st.start = Some(vm.time);
    st.forward = forward;
    st.side = side;
}
fn direction(forward: f32, side: f32) -> Option<(f32, f32)> {
    let length = forward.hypot(side);
    (length.is_finite() && length > 1.0).then(|| (forward / length, side / length))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagonals_have_equal_speed_and_no_direction_does_nothing() {
        assert!(direction(0.0, 0.0).is_none());
        let (f, s) = direction(400.0, -400.0).unwrap();
        assert!((f.hypot(s) - 1.0).abs() < 0.0001);
        assert!(f > 0.0 && s < 0.0);
    }
    #[test]
    fn animation_recovers_and_distinguishes_direction() {
        let d = Dodge { start: Some(2.0), forward: 1.0, side: 0.0 };
        assert_eq!(d.pose(2.0), [0.0; 3]);
        assert!(d.pose(2.325)[0] < -60.0);
        assert_eq!(d.pose(3.0), [0.0; 3]);
        let side = Dodge { forward: 0.0, side: -1.0, ..d };
        assert!(side.pose(2.325)[1] < -25.0);
    }
}

#[cfg(test)]
mod integration {
    use super::*;
    #[test]
    fn launch_is_gated_and_consumed_once_on_ground_and_in_air() {
        use qcore::{cvar::Cvars, fs::Fs};
        let base = Fs::default_basedir();
        if !base.join("id1").is_dir() { return; }
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = crate::Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        sv.connect_local().unwrap();
        let f = sv.core.sv.fields.clone();
        let l = sv.level.as_mut().unwrap();
        l.vm.set_ent_f(1, f.health, 100.0);
        l.vm.set_ent_f(1, f.movetype, 3.0);
        l.vm.set_ent_f(1, f.flags, 512.0);
        l.vm.set_ent_f(1, f.waterlevel, 0.0);
        l.vm.set_ent_v(1, f.velocity, [0.0; 3]);
        let mv = ClientMove { forward: 400.0, buttons: super::super::BUTTON_DODGE, ..Default::default() };
        apply(&mut l.dodges, &mut l.vm, &mut sv.core, 0, mv);
        assert_eq!(l.vm.ent_v(1, f.velocity), [0.0; 3]);
        sv.core.set_addons(true);
        apply(&mut l.dodges, &mut l.vm, &mut sv.core, 0, mv);
        assert_eq!(l.vm.ent_v(1, f.velocity), [550.0, 0.0, 190.0]);
        assert_eq!(l.vm.ent_f(1, f.flags) as i32 & 512, 0);
        assert_eq!(sv.core.clients[0].cmd.buttons & super::super::BUTTON_DODGE, 0);
        l.vm.set_ent_v(1, f.velocity, [0.0; 3]);
        apply(&mut l.dodges, &mut l.vm, &mut sv.core, 0, mv);
        assert_eq!(l.vm.ent_v(1, f.velocity), [0.0; 3]);
        // A new press after recovery can launch during a jump or fall.
        for vertical in [270.0, -300.0] {
            l.vm.time += 1.0;
            l.vm.set_ent_f(1, f.flags, 0.0);
            l.vm.set_ent_v(1, f.velocity, [100.0, 0.0, vertical]);
            sv.core.clients[0].cmd = mv;
            apply(&mut l.dodges, &mut l.vm, &mut sv.core, 0, mv);
            assert_eq!(l.vm.ent_v(1, f.velocity), [550.0, 0.0, 190.0]);
            assert_eq!(sv.core.clients[0].cmd.buttons & super::super::BUTTON_DODGE, 0);
            assert!(l.dodges[0].pose(l.vm.time + 0.325)[0] < -60.0);
            // Reusing the consumed command cannot launch again after recovery.
            l.vm.time += 1.0;
            l.vm.set_ent_v(1, f.velocity, [0.0; 3]);
            let consumed = sv.core.clients[0].cmd;
            apply(&mut l.dodges, &mut l.vm, &mut sv.core, 0, consumed);
            assert_eq!(l.vm.ent_v(1, f.velocity), [0.0; 3]);
        }
    }
}
