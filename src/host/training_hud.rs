//! Feed the HUD from routed gameplay inputs and completed loopback physics.

use super::*;
use crate::hud::kind::Kind;
use crate::hud::strafe::Strafe;

impl Host {
    fn training_enabled(&self, kind: Kind) -> bool {
        self.hud
            .layout
            .elems
            .iter()
            .any(|e| e.kind == kind && !e.hidden)
    }

    pub(super) fn record_hud_input(&mut self, guest: Option<usize>, event: InputEvent) {
        if !matches!(
            event,
            InputEvent::Key { down: true, .. }
                | InputEvent::MouseButton { down: true, .. }
                | InputEvent::MouseWheel { .. }
                | InputEvent::GamepadButton { down: true, .. }
                | InputEvent::GamepadAxis { .. }
        ) || self.key_dest() != KeyDest::Game
            || self.demo.is_some()
            || !self.training_enabled(Kind::InputSequence)
        {
            return;
        }
        let threshold = self.cv("joy_deadzone_trigger");
        let (cl, input, sequence, pad) = if let Some(i) = guest {
            let g = &mut self.guests[i];
            (&g.cl, &g.input, &mut g.training.sequence, g.pad)
        } else {
            (&self.cl, &self.input, &mut self.training.sequence, None)
        };
        if cl.signon == 4 && cl.intermission == 0 && cl.stats[STAT_HEALTH] > 0 {
            let family = pad
                .and_then(|id| self.pad_families.get(&id).copied())
                .unwrap_or(self.pad_family);
            sequence.record(event, input, family, threshold);
        }
    }

    pub(super) fn sample_strafe_hud(&mut self) {
        if !self.training_enabled(Kind::Strafe) {
            self.training.strafe = Strafe::default();
            for g in &mut self.guests {
                g.training.strafe = Strafe::default();
            }
            return;
        }
        let slot = self.local.filter(|_| {
            self.demo.is_none()
                && self.freecam.cam.is_none()
                && self.cl.signon == 4
                && self.cl.intermission == 0
        });
        sample_strafe(&mut self.training.strafe, &self.server, slot);
        for g in &mut self.guests {
            let slot = (g.cl.signon == 4 && g.cl.intermission == 0).then_some(g.slot);
            sample_strafe(&mut g.training.strafe, &self.server, slot);
        }
    }
}

fn sample_strafe(strafe: &mut Strafe, server: &qserver::Server, slot: Option<usize>) {
    let Some((l, slot)) = server.level.as_ref().zip(slot) else {
        *strafe = Strafe::default();
        return;
    };
    let vm = &l.vm;
    let f = &server.core.sv.fields;
    let e = slot as u32 + 1;
    if !server
        .core
        .clients
        .get(slot)
        .is_some_and(|c| c.active && c.spawned)
        || vm.is_free(e)
    {
        *strafe = Strafe::default();
        return;
    }
    let flags = vm.ent_f(e, f.flags) as i32;
    if vm.ent_f(e, f.health) <= 0.0
        || vm.ent_f(e, f.movetype) as i32 != qworld::MOVETYPE_WALK
        || vm.ent_f(e, f.waterlevel) >= 2.0
        || flags & qworld::FL_WATERJUMP != 0
    {
        *strafe = Strafe::default();
        return;
    }

    let velocity = vm.ent_v(e, f.velocity);
    let cmd = server.core.clients[slot].cmd;
    strafe.tick(
        flags & qworld::FL_ONGROUND != 0,
        cmd.buttons & 2 != 0,
        velocity[2] > 0.0,
    );

    // Use the body angles and teleport back-move clamp from SV_AirMove.
    let (forward, right, _) = qcore::math::angle_vectors(vm.ent_v(e, f.angles).into());
    let fmove = if vm.time < f64::from(vm.ent_f(e, f.teleport_time)) {
        cmd.forward.max(0.0)
    } else {
        cmd.forward
    };
    let wish = forward * fmove + right * cmd.side;
    let phys = &server.core.sv.phys;
    let magnitude = wish.x.hypot(wish.y);
    let wishspeed = if magnitude > 0.0 {
        magnitude.min(phys.maxspeed)
    } else {
        phys.maxspeed
    };
    let step = (f64::from(phys.accelerate) * f64::from(wishspeed) * phys.frametime) as f32;
    strafe.guide(
        [velocity[0], velocity[1]],
        [wish.x, wish.y],
        wishspeed,
        step,
    );
}
