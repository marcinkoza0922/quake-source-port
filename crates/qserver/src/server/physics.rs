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

    /// A point MOVETYPE_BOUNCE step owned by `pass` (grenade arc preview):
    /// the trace plus whether the hit entity is DAMAGE_AIM, which a grenade
    /// explodes on instead of bouncing.
    /// `missile` traces as MOVE_MISSILE (monster boxes grown by 15, as the
    /// sticky and gel add-ons do).
    pub fn grenade_trace(&mut self, a: qworld::Vec3, b: qworld::Vec3, pass: u32, missile: bool) -> Option<(qworld::Trace, bool)> {
        let t = {
            let l = self.level.as_mut()?;
            let ents = crate::ents::Ents { vm: &mut l.vm, c: &mut self.core };
            let ty = if missile { qworld::MoveType::Missile } else { qworld::MoveType::Normal };
            l.world.trace(&ents, a, qworld::Vec3::ZERO, qworld::Vec3::ZERO, b, ty, Some(pass))
        };
        let l = self.level.as_ref()?;
        let aim = t.ent.is_some_and(|e| e != 0 && l.vm.ent_f(e, self.core.sv.fields.takedamage) == 2.0);
        Some((t, aim))
    }

    /// Gravity-affected projectiles (MOVETYPE_TOSS/BOUNCE) owned by `owner`,
    /// plus the owner's origin and v_angle: lets the host learn how each
    /// weapon throws for the arc assist, whatever the mod.
    pub fn thrown_by(&self, owner: u32) -> Option<Thrown> {
        let l = self.level.as_ref()?;
        let f = &self.core.sv.fields;
        let vm = &l.vm;
        if owner as usize >= vm.num_edicts() || vm.is_free(owner) { return None; }
        let v = |e: u32, o: usize| qworld::Vec3::from_array(vm.ent_v(e, o));
        let mut out = Thrown { time: vm.time, origin: v(owner, f.origin), v_angle: v(owner, f.v_angle), list: Vec::new() };
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || vm.ent_i(e, f.owner) != owner as i32 { continue; }
            let mt = vm.ent_f(e, f.movetype);
            if mt != qworld::MOVETYPE_TOSS as f32 && mt != qworld::MOVETYPE_BOUNCE as f32 { continue; }
            let g = f.gravity.map_or(0.0, |o| vm.ent_f(e, o));
            out.list.push(ThrownEnt { ent: e, origin: v(e, f.origin), velocity: v(e, f.velocity), bounce: mt == qworld::MOVETYPE_BOUNCE as f32, gravity: if g == 0.0 { 1.0 } else { g } });
        }
        Some(out)
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
            let mut cmd = qworld::user::UserCmd { forwardmove: mv.forward, sidemove: mv.side, upmove: mv.up };
            let mut p = p;
            let toys = !l.gel.splats.is_empty() && crate::addons::active(c);
            let mut ents = Ents { vm: &mut l.vm, c };
            if toys {
                crate::addons::gel_phys::tune(&l.gel, &ents, e, &mut p, &mut cmd);
            }
            qworld::user::client_think(&l.world, &mut ents, &p, e, &cmd);
            crate::addons::dodge::apply(&mut l.dodges, &mut l.vm, c, i, mv);
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
        // Like C SV_Physics, leave `frametime` alone: Host_ServerFrame
        // (`frame_held`) sets it, so SV_SpawnServer's two settle frames run
        // with the value the progs loaded with (0), as in ironwail/vkQuake.
        vm.set_g_i(g.self_, 0);
        vm.set_g_i(g.other, 0);
        vm.set_g_f(g.time, vm.time as f32);
        exec(vm, c, w, vm.g_i(g.f_start_frame));
        let toys = crate::addons::active(c);
        if toys {
            crate::addons::portal_move::snapshot(&mut l.portals, &Ents { vm: &mut *vm, c: &mut *c });
            crate::addons::gel_phys::snapshot(&mut l.gel, &Ents { vm: &mut *vm, c: &mut *c });
        }
        // QuakeSpasm/ironwail/vkQuake `entity_cap`: entities spawned during
        // this frame (projectiles fired from a think, sparks) get their first
        // physics next frame. WinQuake re-read num_edicts and ran them now.
        let cap = if freeze { maxclients + 1 } else { vm.num_edicts() as u32 };
        let mut post_think_weapons: Vec<(u32, crate::weaponswitch::WeaponSnapshot)> = Vec::new();
        let mut i = 0u32;
        loop {
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
                let ci = e as usize - 1;
                if !c.clients[ci].active {
                    continue;
                }
                let local_client = c.clients[ci].local;
                vm.set_g_f(g.time, vm.time as f32);
                vm.set_self(e);
                exec(vm, c, w, vm.g_i(g.f_player_pre_think));
                let pre_move_weapon = local_client.then(|| crate::weaponswitch::WeaponSnapshot::capture(vm, &f, e));
                let ok = qworld::phys::physics_client_move(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &p, e);
                if ok && let Some(snap) = pre_move_weapon {
                    snap.apply(vm, &f, &c.cvars, e);
                }
                if ok && toys {
                    crate::addons::gel_phys::land(&mut l.gel, &mut Ents { vm: &mut *vm, c: &mut *c }, e);
                }
                if ok {
                    vm.set_g_f(g.time, vm.time as f32);
                    vm.set_self(e);
                    exec(vm, c, w, vm.g_i(g.f_player_post_think));
                    if local_client && !vm.is_free(e) {
                        post_think_weapons.push((e, crate::weaponswitch::WeaponSnapshot::capture(vm, &f, e)));
                    }
                }
            } else {
                let mut ep = p;
                if vm.ent_str(e, f.classname).ok() == Some(b"qsp_shotgun_pellet".as_slice()) {
                    ep.maxvelocity = ep.maxvelocity.max(30000.0);
                }
                qworld::phys::physics_entity(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &ep, e);
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
        if c.sv.error.is_none() {
            for (e, snap) in post_think_weapons {
                snap.apply(vm, &f, &c.cvars, e);
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
            crate::modifiers::frame(l, c);
            if toys {
                crate::addons::frame(l, c);
            } else {
                l.falls.clear();
                l.hooks.reset();
                if !l.portals.pairs.is_empty() || !l.stickies.bombs.is_empty() || !l.boomerangs.is_empty() {
                    crate::addons::shut_down(l, c);
                }
            }
        }
        self.process_queued_client_cmds();
    }
}

/// Snapshot from [`Server::thrown_by`].
#[derive(Clone, Debug, Default)]
pub struct Thrown {
    pub time: f64,
    pub origin: qworld::Vec3,
    pub v_angle: qworld::Vec3,
    pub list: Vec<ThrownEnt>,
}

#[derive(Clone, Copy, Debug)]
pub struct ThrownEnt {
    pub ent: u32,
    pub origin: qworld::Vec3,
    pub velocity: qworld::Vec3,
    /// MOVETYPE_BOUNCE (else TOSS).
    pub bounce: bool,
    /// `.gravity` scale (0 in QC means 1).
    pub gravity: f32,
}
