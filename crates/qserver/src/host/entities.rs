use super::*;

impl Ctx<'_> {
    pub(super) fn aim(&mut self, vm: &mut Vm) -> Result<()> {
        let ent = ent_arg(vm, 0)?;
        let f = self.c.sv.fields.clone();
        let g = self.c.sv.globals.clone();
        let teamplay = self.c.cvars.value_of("teamplay") != 0.0;
        let mut start = Vec3::from_array(vm.ent_v(ent, f.origin));
        start.z += 20.0;
        let fwd = Vec3::from_array(vm.g_v(g.v_forward));
        let end = start.mul_add(2048.0, fwd);
        let team = vm.ent_f(ent, f.team);
        let tr = {
            let (ents, w) = self.ents(vm);
            w.trace(&ents, start, Vec3::ZERO, Vec3::ZERO, end, MoveType::Normal, Some(ent))
        };
        if let Some(t) = tr.ent
            && vm.ent_f(t, f.takedamage) == DAMAGE_AIM
            && (!teamplay || team <= 0.0 || team != vm.ent_f(t, f.team))
        {
            vm.ret_v(fwd.to_array());
            return Ok(());
        }
        let mut bestdist = self.c.cvars.value_of("sv_aim");
        let mut best = None;
        for check in 1..vm.num_edicts() as u32 {
            if vm.ent_f(check, f.takedamage) != DAMAGE_AIM || check == ent {
                continue;
            }
            if teamplay && team > 0.0 && team == vm.ent_f(check, f.team) {
                continue;
            }
            let (o, mn, mx) = (vm.ent_v(check, f.origin), vm.ent_v(check, f.mins), vm.ent_v(check, f.maxs));
            let end = Vec3::new(o[0] + 0.5 * (mn[0] + mx[0]), o[1] + 0.5 * (mn[1] + mx[1]), o[2] + 0.5 * (mn[2] + mx[2]));
            let dir = (end - start).normalize();
            let dist = dir.dot(fwd);
            if dist < bestdist {
                continue;
            }
            let tr = {
                let (ents, w) = self.ents(vm);
                w.trace(&ents, start, Vec3::ZERO, Vec3::ZERO, end, MoveType::Normal, Some(ent))
            };
            if tr.ent == Some(check) {
                bestdist = dist;
                best = Some(check);
            }
        }
        if let Some(b) = best {
            let dir = Vec3::from_array(vm.ent_v(b, f.origin)) - Vec3::from_array(vm.ent_v(ent, f.origin));
            let dist = dir.dot(fwd);
            let mut end = fwd * dist;
            end.z = dir.z;
            vm.ret_v(end.normalize().to_array());
        } else {
            vm.ret_v(fwd.to_array());
        }
        Ok(())
    }

    pub(super) fn makestatic(&mut self, vm: &mut Vm) -> Result<()> {
        let e = ent_arg(vm, 0)?;
        let f = self.c.sv.fields.clone();
        let alpha = match f.alpha {
            Some(o) => entalpha_encode(vm.ent_f(e, o)),
            None => self.c.ext(e).alpha,
        };
        if alpha == ENTALPHA_ZERO {
            ed_free(vm, self.c, self.w, e);
            return Ok(());
        }
        let mi = self.c.model_index(vm.ent_str(e, f.model).unwrap_or(b""));
        let frame = vm.ent_f(e, f.frame) as i32;
        let protocol = self.c.sv.protocol;
        let mut bits = 0;
        if protocol == PROTOCOL_NETQUAKE {
            if mi & 0xff00 != 0 || frame & 0xff00 != 0 {
                ed_free(vm, self.c, self.w, e);
                return Ok(());
            }
        } else {
            if mi & 0xff00 != 0 {
                bits |= B_LARGEMODEL;
            }
            if frame & 0xff00 != 0 {
                bits |= B_LARGEFRAME;
            }
            if alpha != ENTALPHA_DEFAULT {
                bits |= B_ALPHA;
            }
        }
        let scale = match f.scale {
            Some(o) if protocol == PROTOCOL_RMQ => entscale_encode(vm.ent_f(e, o)),
            _ => ENTSCALE_DEFAULT,
        };
        if scale != ENTSCALE_DEFAULT {
            bits |= B_SCALE;
        }
        let pf = self.c.sv.protocolflags;
        let (o, a) = (vm.ent_v(e, f.origin), vm.ent_v(e, f.angles));
        let (cm, sk) = (vm.ent_f(e, f.colormap) as i32, vm.ent_f(e, f.skin) as i32);
        let m = self.c.signon(34);
        if bits != 0 {
            m.byte(SVC_SPAWNSTATIC2);
            m.byte(bits);
        } else {
            m.byte(SVC_SPAWNSTATIC);
        }
        if bits & B_LARGEMODEL != 0 { m.short(mi) } else { m.byte(mi) }
        if bits & B_LARGEFRAME != 0 { m.short(frame) } else { m.byte(frame) }
        m.byte(cm);
        m.byte(sk);
        for i in 0..3 {
            m.coord(o[i], pf);
            m.angle(a[i], pf);
        }
        if bits & B_ALPHA != 0 {
            m.byte(alpha as i32);
        }
        if bits & B_SCALE != 0 {
            m.byte(scale as i32);
        }
        ed_free(vm, self.c, self.w, e);
        Ok(())
    }

    pub(super) fn ambientsound(&mut self, vm: &mut Vm) -> Result<()> {
        let pos = vm.g_v(p(0));
        let samp = str_arg(vm, 1)?;
        let (vol, att) = (vm.g_f(p(2)), vm.g_f(p(3)));
        let Some(snum) = self.c.sv.sound_precache.iter().position(|s| s.as_bytes() == samp.as_slice()) else {
            self.c.print(&format!("no precache: {}\n", lossy(&samp)));
            return Ok(());
        };
        let large = snum > 255;
        if large && self.c.sv.protocol == PROTOCOL_NETQUAKE {
            return Ok(());
        }
        let pf = self.c.sv.protocolflags;
        let m = self.c.signon(17);
        m.byte(if large { SVC_SPAWNSTATICSOUND2 } else { SVC_SPAWNSTATICSOUND });
        for x in pos {
            m.coord(x, pf);
        }
        if large { m.short(snum as i32) } else { m.byte(snum as i32) }
        m.byte((vol * 255.0) as i32);
        m.byte((att * 64.0) as i32);
        Ok(())
    }

    pub(super) fn lightstyle(&mut self, vm: &mut Vm) -> Result<()> {
        let style = vm.g_f(p(0)) as i32;
        let val = str_arg(vm, 1)?;
        if !(0..MAX_LIGHTSTYLES as i32).contains(&style) {
            return Ok(());
        }
        self.c.sv.lightstyles[style as usize] = val.clone();
        if !self.c.sv.active {
            return Ok(());
        }
        for cl in self.c.clients.iter_mut().filter(|c| c.active || c.spawned) {
            cl.message.char(SVC_LIGHTSTYLE);
            cl.message.char(style);
            cl.message.string(&val);
        }
        Ok(())
    }

    pub(super) fn findradius(&mut self, vm: &mut Vm) {
        let f = &self.c.sv.fields;
        let org = vm.g_v(p(0));
        let rad = vm.g_f(p(1));
        let rad = rad * rad;
        let mut chain = 0u32;
        for e in 1..vm.num_edicts() as u32 {
            if vm.is_free(e) || vm.ent_f(e, f.solid) as i32 == SOLID_NOT {
                continue;
            }
            let (o, mn, mx) = (vm.ent_v(e, f.origin), vm.ent_v(e, f.mins), vm.ent_v(e, f.maxs));
            let mut lensq = 0.0;
            let mut far = false;
            for i in 0..3 {
                let d = org[i] - (o[i] + (mn[i] + mx[i]) * 0.5);
                lensq += d * d;
                if lensq > rad {
                    far = true;
                    break;
                }
            }
            if far {
                continue;
            }
            vm.set_ent_i(e, f.chain, chain as i32);
            chain = e;
        }
        vm.ret_e(chain);
    }

    pub(super) fn find(&mut self, vm: &mut Vm) -> Result<()> {
        let start = vm.g_i(p(0)).max(0) as u32;
        let fofs = vm.g_i(p(1));
        let s = str_arg(vm, 2)?;
        if fofs < 0 || fofs as usize >= vm.entityfields() {
            return Err(vm.run_error("PF_Find: bad field"));
        }
        for e in start + 1..vm.num_edicts() as u32 {
            if vm.is_free(e) {
                continue;
            }
            if vm.ent_str(e, fofs as usize).is_ok_and(|t| t == s.as_slice()) {
                vm.ret_e(e);
                return Ok(());
            }
        }
        vm.ret_e(0);
        Ok(())
    }

    pub(super) fn give_spawn_parms(&mut self, vm: &mut Vm) -> Result<()> {
        let e = vm.g_i(p(0));
        let Some(i) = self.client_of(e) else { return Err(vm.run_error("Entity is not a client")) };
        let g = self.c.sv.globals.parm1;
        for k in 0..NUM_SPAWN_PARMS {
            vm.set_g_f(g + k, self.c.clients[i].spawn_parms[k]);
        }
        Ok(())
    }
}
