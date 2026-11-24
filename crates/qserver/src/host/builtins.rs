use super::*;

impl Host for Ctx<'_> {
    fn before_damage(&mut self, vm: &mut Vm) -> Result<bool> {
        crate::addons::shotgun::damage(vm, self.c);
        let skip = crate::modifiers::before_damage(vm, self.c)?;
        if !skip {
            self.damage_calls.push(feedback::begin(vm, self.c));
        }
        Ok(skip)
    }
    fn after_damage(&mut self, vm: &mut Vm) -> Result<()> {
        if let Some(Some(call)) = self.damage_calls.pop() {
            feedback::finish(vm, self.c, call);
        }
        Ok(())
    }
    fn builtin(&mut self, vm: &mut Vm, b: Builtin) -> Result<()> {
        if let Some(e) = &self.c.sv.error {
            return Err(e.clone());
        }
        use Builtin as B;
        match b {
            B::Setorigin => {
                let e = ent_arg(vm, 0)?;
                let o = vm.g_v(p(1));
                let fo = self.c.sv.fields.origin;
                vm.set_ent_v(e, fo, o);
                self.link(vm, e, false);
            }
            B::Setmodel => self.setmodel(vm)?,
            B::Setsize => {
                let e = ent_arg(vm, 0)?;
                let (mn, mx) = (vm.g_v(p(1)), vm.g_v(p(2)));
                self.set_min_max_size(vm, e, mn, mx)?;
            }
            B::Sound => {
                let e = ent_arg(vm, 0)?;
                let ch = vm.g_f(p(1)) as i32;
                let s = str_arg(vm, 2)?;
                let vol = (vm.g_f(p(3)) * 255.0) as i32;
                let att = vm.g_f(p(4));
                if !(0..=255).contains(&vol) || !(0.0..=4.0).contains(&att) || !(0..=7).contains(&ch) {
                    return Err(vm.run_error(format!("SV_StartSound: bad volume {vol} / attenuation {att} / channel {ch}")));
                }
                crate::send::start_sound(vm, self.c, e, ch, &s, vol, att);
            }
            B::Error => {
                let s = lossy(&vm.var_string(0)?);
                let fname = vm.stack_trace().first().cloned().unwrap_or_default();
                let me = vm.self_ent().min(vm.num_edicts() as u32 - 1);
                self.c.print(&format!("======SERVER ERROR in {fname}:\n{s}\n{}", vm.ed_print(me)));
                return Err(vm.run_error(format!("Program error: {}", s.trim_end())));
            }
            B::Objerror => {
                let s = lossy(&vm.var_string(0)?);
                let me = vm.self_ent();
                self.c.print(&format!("======OBJECT ERROR:\n{s}\n"));
                if (me as usize) < vm.num_edicts() && me != 0 {
                    ed_free(vm, self.c, self.w, me);
                }
            }
            B::Spawn => {
                let e = vm.ed_alloc()?;
                let x = self.c.ext(e);
                *x = Default::default();
                vm.ret_e(e);
            }
            B::Remove => {
                let e = ent_arg(vm, 0)?;
                if e == 0 {
                    self.c.print("PF_Remove: tried to remove world\n");
                } else {
                    ed_free(vm, self.c, self.w, e);
                }
            }
            B::Traceline => self.traceline(vm)?,
            B::Checkclient => self.checkclient(vm),
            B::Find => self.find(vm)?,
            B::PrecacheSound | B::PrecacheSound2 => self.precache_sound(vm)?,
            B::PrecacheModel | B::PrecacheModel2 => self.precache_model(vm)?,
            B::PrecacheFile | B::PrecacheFile2 => {
                let h = vm.g_i(p(0));
                vm.ret_i(h);
            }
            B::Stuffcmd => {
                let e = vm.g_i(p(0));
                let Some(i) = self.client_of(e) else { return Err(vm.run_error("Parm 0 not a client")) };
                let s = str_arg(vm, 1)?;
                self.client_msg(i, SVC_STUFFTEXT, &s);
            }
            B::Findradius => self.findradius(vm),
            B::Bprint => {
                let s = self.loc_var_string(vm, 0)?;
                self.c.print(&lossy(&s));
                for cl in self.c.clients.iter_mut().filter(|c| c.active && c.spawned) {
                    cl.message.byte(SVC_PRINT);
                    cl.message.string(&s);
                }
            }
            B::Sprint | B::Centerprint => {
                let e = vm.g_i(p(0));
                let s = self.loc_var_string(vm, 1)?;
                match self.client_of(e) {
                    Some(i) => {
                        let svc = if b == B::Sprint { SVC_PRINT } else { SVC_CENTERPRINT };
                        self.client_msg(i, svc, &s);
                    }
                    None => self.c.print("tried to sprint to a non-client\n"),
                }
            }
            B::Coredump => {
                for e in 0..vm.num_edicts() as u32 {
                    let s = vm.ed_print(e);
                    self.c.print(&s);
                }
            }
            B::Eprint => {
                let e = ent_arg(vm, 0)?;
                let s = vm.ed_print(e);
                self.c.print(&s);
            }
            B::Walkmove => {
                let me = vm.self_ent();
                if me as usize >= vm.num_edicts() {
                    return Err(vm.run_error("walkmove: bad self"));
                }
                let (yaw, dist) = (vm.g_f(p(0)), vm.g_f(p(1)));
                let fl = vm.ent_f(me, self.c.sv.fields.flags) as i32;
                if fl & (FL_ONGROUND | FL_FLY | FL_SWIM) == 0 {
                    vm.ret_f(0.0);
                    return Ok(());
                }
                let yaw = (yaw as f64 * std::f64::consts::PI * 2.0 / 360.0) as f32;
                // C: cos(yaw) * dist in double, stored as float
                let (y, d) = (yaw as f64, dist as f64);
                let mv = Vec3::new((y.cos() * d) as f32, (y.sin() * d) as f32, 0.0);
                let r = {
                    let (mut ents, w) = self.ents(vm);
                    qworld::monster::movestep(w, &mut ents, me, mv, true)
                };
                vm.set_self(me);
                vm.ret_f(r as i32 as f32);
            }
            B::Droptofloor => {
                let me = vm.self_ent();
                if me as usize >= vm.num_edicts() {
                    return Err(vm.run_error("droptofloor: bad self"));
                }
                let f = self.c.sv.fields.clone();
                let o = Vec3::from_array(vm.ent_v(me, f.origin));
                let (mn, mx) = (Vec3::from_array(vm.ent_v(me, f.mins)), Vec3::from_array(vm.ent_v(me, f.maxs)));
                let end = Vec3::new(o.x, o.y, o.z - 256.0);
                let tr = {
                    let (ents, w) = self.ents(vm);
                    w.trace(&ents, o, mn, mx, end, MoveType::Normal, Some(me))
                };
                if tr.fraction == 1.0 || tr.allsolid {
                    vm.ret_f(0.0);
                } else {
                    vm.set_ent_v(me, f.origin, tr.endpos.to_array());
                    self.link(vm, me, false);
                    let fl = vm.ent_f(me, f.flags) as i32 | FL_ONGROUND;
                    vm.set_ent_f(me, f.flags, fl as f32);
                    vm.set_ent_i(me, f.groundentity, tr.ent.unwrap_or(0) as i32);
                    vm.ret_f(1.0);
                }
            }
            B::Lightstyle => self.lightstyle(vm)?,
            B::Checkbottom => {
                let e = ent_arg(vm, 0)?;
                let (ents, w) = self.ents(vm);
                let r = qworld::monster::check_bottom(w, &ents, e);
                vm.ret_f(r as i32 as f32);
            }
            B::Pointcontents => {
                let v = Vec3::from_array(vm.g_v(p(0)));
                vm.ret_f(self.w.point_contents(v) as f32);
            }
            B::Aim => self.aim(vm)?,
            B::Cvar => {
                let s = lossy(&str_arg(vm, 0)?);
                vm.ret_f(self.c.cvars.value_of(&s));
            }
            B::CvarSet => {
                let (n, v) = (lossy(&str_arg(vm, 0)?), lossy(&str_arg(vm, 1)?));
                if self.c.cvars.set(&n, &v).is_none() {
                    self.c.print(&format!("Cvar_Set: variable {n} not found\n"));
                }
            }
            B::Localcmd => {
                let s = lossy(&str_arg(vm, 0)?);
                self.c.cbuf.add_text(&s);
            }
            B::Nextent => {
                let mut i = vm.g_i(p(0)).max(0) as usize;
                loop {
                    i += 1;
                    if i >= vm.num_edicts() {
                        vm.ret_e(0);
                        break;
                    }
                    if !vm.is_free(i as u32) {
                        vm.ret_e(i as u32);
                        break;
                    }
                }
            }
            B::Particle => {
                let (org, dir) = (vm.g_v(p(0)), vm.g_v(p(1)));
                let (color, count) = (vm.g_f(p(2)) as i32, vm.g_f(p(3)) as i32);
                let pf = self.c.sv.protocolflags;
                let m = &mut self.c.sv.datagram;
                if m.len() <= MAX_DATAGRAM - 18 {
                    m.byte(SVC_PARTICLE);
                    for x in org {
                        m.coord(x, pf);
                    }
                    for x in dir {
                        m.char(((x * 16.0) as i32).clamp(-128, 127));
                    }
                    m.byte(count);
                    m.byte(color);
                }
            }
            B::ChangeYaw => {
                let me = vm.self_ent();
                if (me as usize) < vm.num_edicts() {
                    let (mut ents, _) = self.ents(vm);
                    qworld::monster::change_yaw(&mut ents, me);
                }
            }
            B::WriteByte => {
                let v = crate::addons::shotgun::temp_entity(vm, self.c, vm.g_f(p(1)) as i32);
                self.with_dest(vm, |m, _| m.byte(v))?;
            }
            B::WriteChar => {
                let v = vm.g_f(p(1)) as i32;
                self.with_dest(vm, |m, _| m.char(v))?;
            }
            B::WriteShort => {
                let v = vm.g_f(p(1)) as i32;
                self.with_dest(vm, |m, _| m.short(v))?;
            }
            B::WriteLong => {
                let v = vm.g_f(p(1)) as i32;
                self.with_dest(vm, |m, _| m.long(v))?;
            }
            B::WriteCoord => {
                let v = vm.g_f(p(1));
                self.with_dest(vm, |m, pf| m.coord(v, pf))?;
            }
            B::WriteAngle => {
                let v = vm.g_f(p(1));
                self.with_dest(vm, |m, pf| m.angle(v, pf))?;
            }
            B::WriteString => {
                let s = self.c.loc.get_string(vm.g_str(p(1))?).to_vec();
                self.with_dest(vm, |m, _| m.string(&s))?;
            }
            B::WriteEntity => {
                let v = vm.g_i(p(1));
                self.with_dest(vm, |m, _| m.short(v))?;
            }
            B::Movetogoal => {
                let me = vm.self_ent();
                if (me as usize) < vm.num_edicts() {
                    let dist = vm.g_f(p(0));
                    let (mut ents, w) = self.ents(vm);
                    qworld::monster::move_to_goal(w, &mut ents, me, dist);
                    vm.set_self(me);
                }
            }
            B::Makestatic => self.makestatic(vm)?,
            B::Changelevel => {
                if !self.c.changelevel_issued {
                    self.c.changelevel_issued = true;
                    let s = lossy(&str_arg(vm, 0)?);
                    self.c.cbuf.add_text(&format!("changelevel {s}\n"));
                }
            }
            B::Ambientsound => self.ambientsound(vm)?,
            B::Setspawnparms => self.give_spawn_parms(vm)?,
            B::FinaleFinished => vm.ret_f(0.0),
            B::Localsound => {
                let e = vm.g_i(p(0));
                let s = str_arg(vm, 1)?;
                let Some(i) = self.client_of(e) else {
                    self.c.print("tried to localsound to a non-client\n");
                    return Ok(());
                };
                let Some(snum) = self.c.sv.sound_precache.iter().skip(1).position(|x| x.as_bytes() == s.as_slice()) else {
                    self.c.print(&format!("SV_LocalSound: {} not precached\n", lossy(&s)));
                    return Ok(());
                };
                let snum = snum as i32 + 1;
                let large = snum >= 256;
                if large && self.c.sv.protocol == PROTOCOL_NETQUAKE {
                    return Ok(());
                }
                let m = &mut self.c.clients[i].message;
                if m.len() <= m.maxsize - 4 {
                    m.byte(SVC_LOCALSOUND);
                    m.byte(if large { SND_LARGESOUND } else { 0 });
                    if large { m.short(snum) } else { m.byte(snum) }
                }
            }
            // handled inside the VM
            B::Makevectors | B::Break | B::Random | B::Normalize | B::Vlen | B::Vectoyaw | B::Dprint
            | B::Ftos | B::Vtos | B::Traceon | B::Traceoff | B::Rint | B::Floor | B::Ceil | B::Fabs
            | B::Vectoangles => return Err(vm.run_error(format!("builtin {} not handled", b.name()))),
        }
        Ok(())
    }

    fn ext_builtin(&mut self, vm: &mut Vm, name: &str) -> Result<()> {
        if let Some(e) = &self.c.sv.error {
            return Err(e.clone());
        }
        match name {
            "clientstat" => {
                let idx = vm.g_f(p(0)) as i32;
                let ty = vm.g_f(p(1)) as i32;
                let fld = vm.g_i(p(2)) as usize;
                if (32..qnet::protocol::MAX_CL_STATS as i32).contains(&idx) {
                    let idx = idx as usize;
                    self.c.sv.custom_stats.retain(|s| s.idx != idx);
                    self.c.sv.custom_stats.push(crate::server::CustomStat {
                        idx,
                        ty,
                        field: Some(fld),
                        global: None,
                    });
                }
            }
            "globalstat" => {
                let idx = vm.g_f(p(0)) as i32;
                let ty = vm.g_f(p(1)) as i32;
                let gname = lossy(&str_arg(vm, 2)?);
                if (32..qnet::protocol::MAX_CL_STATS as i32).contains(&idx)
                    && let Some(ofs) = vm.global_ofs(&gname)
                {
                    let idx = idx as usize;
                    self.c.sv.custom_stats.retain(|s| s.idx != idx);
                    self.c.sv.custom_stats.push(crate::server::CustomStat {
                        idx,
                        ty,
                        field: None,
                        global: Some(ofs),
                    });
                }
            }
            "fopen" => {
                let path = lossy(&str_arg(vm, 0)?);
                let mode = vm.g_f(p(1)) as i32;
                let h = self.c.sv.qc_files.open(&self.c.fs, &path, mode);
                vm.ret_f(h as f32);
            }
            "fclose" => {
                let h = vm.g_f(p(0)) as i32;
                self.c.sv.qc_files.close(h);
            }
            "fgets" => {
                let h = vm.g_f(p(0)) as i32;
                match self.c.sv.qc_files.gets(h) {
                    Some(line) => vm.ret_temp(&line),
                    None => vm.ret_i(0),
                }
            }
            "fputs" => {
                let h = vm.g_f(p(0)) as i32;
                let s = vm.var_string(1)?;
                self.c.sv.qc_files.puts(h, &s);
            }
            "fseek" => {
                let h = vm.g_f(p(0)) as i32;
                let pos = (vm.argc > 1).then(|| vm.g_i(p(1)).max(0) as usize);
                let prev = self.c.sv.qc_files.seek(h, pos);
                vm.ret_i(prev);
            }
            "getsurfacenumpoints" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let n = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0)
                    .and_then(|(sm, sub)| sm.face(*sub, si as usize))
                    .map_or(0, |f| f.verts.len());
                vm.ret_f(n as f32);
            }
            "getsurfacepoint" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let pi = vm.g_f(p(2)) as i32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let pt = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0 && pi >= 0)
                    .and_then(|(sm, sub)| sm.face(*sub, si as usize)?.verts.get(pi as usize).copied())
                    .unwrap_or([0.0; 3]);
                vm.ret_v(pt);
            }
            "getsurfacenormal" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let n = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0)
                    .and_then(|(sm, sub)| sm.face(*sub, si as usize))
                    .map(crate::surface::SurfaceModel::face_outward_normal)
                    .unwrap_or([0.0; 3]);
                vm.ret_v(n);
            }
            "getsurfacetexture" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let tex = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0)
                    .and_then(|(sm, sub)| sm.face(*sub, si as usize))
                    .map(|f| f.texture.as_bytes().to_vec());
                match tex {
                    Some(t) => {
                        let h = vm.strings.engine(&t);
                        vm.ret_i(h);
                    }
                    None => vm.ret_i(0),
                }
            }
            "getsurfacenearpoint" => {
                let e = ent_arg(vm, 0)?;
                let pt = vm.g_v(p(1));
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let r = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .and_then(|(sm, sub)| sm.near_point(*sub, pt))
                    .map_or(-1.0, |i| i as f32);
                vm.ret_f(r);
            }
            "getsurfaceclippedpoint" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let pt = vm.g_v(p(2));
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let cp = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0)
                    .map_or(pt, |(sm, sub)| sm.clipped_point(*sub, si as usize, pt));
                vm.ret_v(cp);
            }
            "getsurfacepointattribute" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let pi = vm.g_f(p(2)) as i32;
                let attr = vm.g_f(p(3)) as u32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let v = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0 && pi >= 0)
                    .map_or([0.0; 3], |(sm, sub)| sm.point_attribute(*sub, si as usize, pi as usize, attr));
                vm.ret_v(v);
            }
            "getsurfacenumtriangles" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let n = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0)
                    .and_then(|(sm, sub)| sm.face(*sub, si as usize))
                    .map_or(0, |f| f.verts.len().saturating_sub(2));
                vm.ret_f(n as f32);
            }
            "getsurfacetriangle" => {
                let e = ent_arg(vm, 0)?;
                let si = vm.g_f(p(1)) as i32;
                let ti = vm.g_f(p(2)) as i32;
                let mi = vm.ent_f(e, self.c.sv.fields.modelindex) as usize;
                let tri = self.c.sv.surface_models.get(mi).and_then(|s| s.as_ref())
                    .filter(|_| si >= 0 && ti >= 0)
                    .and_then(|(sm, sub)| sm.face(*sub, si as usize))
                    .filter(|f| (ti as usize) < f.verts.len().saturating_sub(2))
                    .map_or([0.0; 3], |_| [0.0, (ti + 1) as f32, (ti + 2) as f32]);
                vm.ret_v(tri);
            }
            "cvar_string" => {
                let n = lossy(&str_arg(vm, 0)?);
                let s = self.c.cvars.by_name(&n).map_or("", |c| c.string.as_str()).to_string();
                vm.ret_temp(s.as_bytes());
            }
            "registercvar" => {
                let n = lossy(&str_arg(vm, 0)?);
                let v = lossy(&str_arg(vm, 1)?);
                let existed = self.c.cvars.find(&n).is_some();
                if !existed {
                    self.c.cvars.register(&n, &v, 0);
                }
                vm.ret_f(i32::from(!existed) as f32);
            }
            "print" => {
                let s = lossy(&vm.var_string(0)?);
                self.c.print(&s);
            }
            "setmodelindex" => {
                let e = ent_arg(vm, 0)?;
                let idx = vm.g_f(p(1)) as usize;
                let m = self.c.sv.model_precache.get(idx).map(|s| s.as_bytes().to_vec()).unwrap_or_default();
                let h = if m.is_empty() { 0 } else { vm.strings.engine(&m) };
                let f = &self.c.sv.fields;
                let (fm, fmi) = (f.model, f.modelindex);
                vm.set_ent_i(e, fm, h);
                vm.set_ent_f(e, fmi, idx as f32);
                let (mn, mx) = match self.c.sv.models.get(idx).and_then(|m| m.as_ref()) {
                    Some(m) => (m.mins.to_array(), m.maxs.to_array()),
                    None => ([0.0; 3], [0.0; 3]),
                };
                self.set_min_max_size(vm, e, mn, mx)?;
            }
            "modelnameforindex" => {
                let idx = vm.g_f(p(0)) as usize;
                match self.c.sv.model_precache.get(idx) {
                    Some(s) if !s.is_empty() => vm.ret_temp(s.as_bytes()),
                    _ => vm.ret_i(0),
                }
            }
            "checkcommand" => {
                let n = lossy(&str_arg(vm, 0)?).to_ascii_lowercase();
                let r = if self.c.cmd_names.contains(&n) {
                    1.0
                } else if self.c.cvars.find(&n).is_some() {
                    3.0
                } else {
                    0.0
                };
                vm.ret_f(r);
            }
            "clientcommand" => {
                let e = vm.g_i(p(0));
                let s = lossy(&str_arg(vm, 1)?);
                match self.client_of(e) {
                    Some(i) if self.c.clients[i].active => self.c.queued_client_cmds.push((i, s)),
                    _ => self.c.print("PF_clientcommand: not a client\n"),
                }
            }
            "particleeffectnum" => self.c.pf_particleeffectnum(vm)?,
            "trailparticles" => self.c.pf_trailparticles(vm)?,
            "pointparticles" => self.c.pf_pointparticles(vm)?,
            "te_particlerain" => self.c.pf_te_weather(vm, false)?,
            "te_particlesnow" => self.c.pf_te_weather(vm, true)?,
            "ex_CheckPlayerEXFlags" | "ex_walkpathtogoal" => vm.ret_f(0.0),
            _ if self.c.pf_te_effect(vm, name)? => {}
            _ => return Err(vm.run_error(format!("unimplemented builtin {name}"))),
        }
        Ok(())
    }

    fn particle_effect_index(&mut self, name: &str) -> Option<f32> {
        self.c.fte_particles().then(|| self.c.particle_precache(name) as f32)
    }

    fn extensions_enabled(&self) -> bool {
        self.c.cvars.value_of("pr_checkextension") != 0.0
    }

    fn has_extension(&self, name: &str) -> bool {
        if !self.extensions_enabled() {
            return false;
        }
        if qvm::EXTENSIONS.contains(&name) {
            return true;
        }
        const SERVER_EXTENSIONS: &[&str] = &[
            "DP_CON_SET",
            "DP_CON_SETA",
            "DP_REGISTERCVAR",
            "DP_QC_CVAR_STRING",
            "DP_QC_GETSURFACE",
            "DP_QC_GETSURFACETRIANGLE",
            "DP_QC_GETSURFACEPOINTATTRIBUTE",
            "DP_SV_PRINT",
            "DP_TE_STANDARDEFFECTBUILTINS",
            "DP_VIEWZOOM",
            "EXT_CSQC",
        ];
        if SERVER_EXTENSIONS.contains(&name) {
            return true;
        }
        let pext2 = self.c.cvars.value_of("sv_protocol_pext2") as u32 & qnet::delta::PEXT2_SUPPORTED;
        match name {
            "DP_ENT_ALPHA" => self.c.sv.protocol != PROTOCOL_NETQUAKE || pext2 & qnet::delta::PEXT2_REPLACEMENTDELTAS != 0,
            "DP_ENT_SCALE" => self.c.sv.protocol == PROTOCOL_RMQ || pext2 & qnet::delta::PEXT2_REPLACEMENTDELTAS != 0,
            "DP_ENT_COLORMOD" => pext2 & qnet::delta::PEXT2_REPLACEMENTDELTAS != 0,
            _ => self.c.fte_particles() && crate::particles::EXTENSIONS.contains(&name),
        }
    }

    fn dprint(&mut self, msg: &str) {
        if self.c.cvars.value_of("developer") != 0.0 {
            self.c.print(msg);
        }
    }

    fn print(&mut self, msg: &str) {
        self.c.print(msg);
    }
}
