use super::*;

impl Server {
    /// The QC `traceline` builtin from outside the VM (tools, bots):
    /// a point trace against the world and linked entities, skipping `pass`.
    pub fn traceline(&mut self, a: qworld::Vec3, b: qworld::Vec3, nomonsters: bool, pass: u32) -> Option<qworld::Trace> {
        let l = self.level.as_mut()?;
        let ents = crate::ents::Ents { vm: &mut l.vm, c: &mut self.core };
        let ty = qworld::MoveType::from(nomonsters as i32);
        Some(l.world.trace(&ents, a, qworld::Vec3::ZERO, qworld::Vec3::ZERO, b, ty, Some(pass)))
    }

    pub(super) fn phys_params(&self, ft: f64) -> PhysParams {
        let cv = &self.core.cvars;
        PhysParams {
            time: self.level.as_ref().map_or(0.0, |l| l.vm.time),
            frametime: ft,
            gravity: cv.value_of("sv_gravity"),
            friction: cv.value_of("sv_friction"),
            stopspeed: cv.value_of("sv_stopspeed"),
            maxvelocity: cv.value_of("sv_maxvelocity"),
            nostep: cv.value_of("sv_nostep") != 0.0,
            edgefriction: cv.value_of("edgefriction"),
            accelerate: cv.value_of("sv_accelerate"),
            maxspeed: cv.value_of("sv_maxspeed"),
            gameplayfix_elevators: cv.value_of("sv_gameplayfix_elevators"),
            maxclients: self.core.maxclients() as u32,
            altnoclip: cv.value_of("sv_altnoclip") != 0.0,
            rollangle: cv.value_of("cl_rollangle"),
            rollspeed: cv.value_of("cl_rollspeed"),
            idealpitchscale: cv.value_of("sv_idealpitchscale"),
        }
    }

    /// SV_RunClients.
    pub(super) fn run_clients(&mut self, ft: f64) {
        for i in 0..self.core.maxclients() {
            if !self.core.clients[i].active {
                continue;
            }
            while let Some(s) = self.core.clients[i].stringcmds.pop_front() {
                self.client_command(i, &s);
            }
            if self.level.is_none() || !self.core.clients[i].active {
                continue;
            }
            let p = self.phys_params(ft);
            let Some(l) = &mut self.level else { return };
            let c = &mut self.core;
            let e = i as u32 + 1;
            let f = c.sv.fields.clone();
            let cl = &mut c.clients[i];
            let mv = cl.cmd;
            if cl.new_move {
                cl.new_move = false;
                l.vm.set_ent_v(e, f.v_angle, mv.angles);
                l.vm.set_ent_f(e, f.button0, (mv.buttons & 1) as f32);
                l.vm.set_ent_f(e, f.button2, ((mv.buttons & 2) >> 1) as f32);
                if mv.impulse != 0 {
                    l.vm.set_ent_f(e, f.impulse, mv.impulse as f32);
                }
                cl.cmd.impulse = 0;
            }
            if !cl.spawned {
                cl.cmd = ClientMove::default();
                continue;
            }
            if c.sv.paused {
                continue;
            }
            let cmd = qworld::user::UserCmd { forwardmove: mv.forward, sidemove: mv.side, upmove: mv.up };
            let mut ents = Ents { vm: &mut l.vm, c };
            qworld::user::client_think(&l.world, &mut ents, &p, e, &cmd);
        }
    }

    /// SV_Physics for one frame of `ft` seconds.
    pub(crate) fn physics(&mut self, ft: f64) {
        let mut p = self.phys_params(ft);
        self.core.sv.phys = p;
        let maxclients = self.core.maxclients() as u32;
        let freeze = self.core.cvars.value_of("sv_freezenonclients") != 0.0;
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let g = c.sv.globals.clone();
        let f = c.sv.fields.clone();
        let vm = &mut l.vm;
        let w = &mut l.world;
        vm.set_g_f(g.frametime, ft as f32);
        vm.set_g_i(g.self_, 0);
        vm.set_g_i(g.other, 0);
        vm.set_g_f(g.time, vm.time as f32);
        exec(vm, c, w, vm.g_i(g.f_start_frame));
        let toys = crate::addons::active(c);
        if toys {
            crate::addons::portal_move::snapshot(&mut l.portals, &Ents { vm: &mut *vm, c: &mut *c });
        }
        let mut i = 0u32;
        loop {
            let cap = if freeze { maxclients + 1 } else { vm.num_edicts() as u32 };
            if i >= cap || c.sv.error.is_some() {
                break;
            }
            let e = i;
            i += 1;
            if vm.is_free(e) {
                continue;
            }
            p.time = vm.time;
            if vm.g_f(g.force_retouch) != 0.0 {
                w.link_edict(&mut Ents { vm: &mut *vm, c: &mut *c }, e, true);
            }
            if e > 0 && e <= maxclients {
                if !c.clients[e as usize - 1].active {
                    continue;
                }
                vm.set_g_f(g.time, vm.time as f32);
                vm.set_self(e);
                exec(vm, c, w, vm.g_i(g.f_player_pre_think));
                let ok = qworld::phys::physics_client_move(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &p, e);
                if ok {
                    vm.set_g_f(g.time, vm.time as f32);
                    vm.set_self(e);
                    exec(vm, c, w, vm.g_i(g.f_player_post_think));
                }
            } else {
                qworld::phys::physics_entity(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &p, e);
            }
            if c.sv.error.is_some() {
                break;
            }
            // johnfitz: lerp interval
            let x = c.ext(e);
            x.sendinterval = false;
            if !vm.is_free(e) {
                let nt = vm.ent_f(e, f.nextthink);
                let mt = vm.ent_f(e, f.movetype) as i32;
                let fr = vm.ent_f(e, f.frame);
                let x = c.ext(e);
                if nt as f64 > vm.time && (mt == MOVETYPE_STEP || mt == MOVETYPE_WALK || fr != x.oldframe) {
                    let j = q_rint((nt - x.oldthinktime) * 255.0);
                    if (0..256).contains(&j) && j != 25 && j != 26 {
                        x.sendinterval = true;
                    }
                }
            }
        }
        let fr = vm.g_f(g.force_retouch);
        if fr != 0.0 {
            vm.set_g_f(g.force_retouch, fr - 1.0);
        }
        if !freeze {
            vm.time += ft;
        }
        if c.sv.error.is_none() {
            crate::assists::frame(l, c);
            if toys {
                crate::addons::frame(l, c);
            } else {
                l.falls.clear();
                if !l.portals.pairs.is_empty() || !l.boomerangs.is_empty() {
                    crate::addons::shut_down(l, c);
                }
            }
        }
        self.process_queued_client_cmds();
    }
}
