//! `qvm::Host`: the world/network builtins of pr_cmds.c.

use std::sync::Arc;

use qvm::{Builtin, Host, Result, Vm, VmError, ofs_parm};
use qworld::world::MoveType;
use qworld::{CollisionModel, FL_FLY, FL_ONGROUND, FL_SWIM, SOLID_NOT, Vec3, World};

use crate::ents::Ents;
use crate::msg::*;
use crate::server::{Core, ModelInfo, ed_free};
use crate::vis::leafs_in_pvs;

const FL_NOTARGET: i32 = 128;
const DAMAGE_AIM: f32 = 2.0;
const MSG_BROADCAST: i32 = 0;
const MSG_ONE: i32 = 1;
const MSG_ALL: i32 = 2;
const MSG_INIT: i32 = 3;

pub(crate) struct Ctx<'a> {
    pub c: &'a mut Core,
    pub w: &'a mut World,
}

fn p(i: usize) -> usize {
    ofs_parm(i)
}

fn ent_arg(vm: &Vm, i: usize) -> Result<u32> {
    let e = vm.g_i(p(i));
    if e < 0 || e as usize >= vm.num_edicts() {
        return Err(vm.run_error(format!("bad entity {e}")));
    }
    Ok(e as u32)
}

fn str_arg(vm: &Vm, i: usize) -> Result<Vec<u8>> {
    Ok(vm.g_str(p(i))?.to_vec())
}

fn lossy(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}

enum Dest {
    Datagram,
    Client(usize),
    Reliable,
    Signon,
}

impl Ctx<'_> {
    /// PF_VarString with localization: a translated first arg containing
    /// `{N}` placeholders is formatted with the following args, otherwise
    /// all (translated) args are concatenated.
    fn loc_var_string(&self, vm: &Vm, first: usize) -> Result<Vec<u8>> {
        if first >= vm.argc {
            return Ok(Vec::new());
        }
        let loc = &self.c.loc;
        let fmt = loc.get_string(vm.g_str(p(first))?);
        if loc.has_placeholders(fmt) {
            let mut args = Vec::new();
            for i in first + 1..vm.argc {
                args.push(loc.get_string(vm.g_str(p(i))?));
            }
            return Ok(crate::loc::format(fmt, |n| args.get(n).copied().unwrap_or(b"")));
        }
        let mut out = Vec::new();
        for i in first..vm.argc {
            out.extend_from_slice(loc.get_string(vm.g_str(p(i))?));
        }
        Ok(out)
    }

    fn ents<'b>(&'b mut self, vm: &'b mut Vm) -> (Ents<'b>, &'b mut World) {
        (Ents { vm, c: &mut *self.c }, &mut *self.w)
    }

    fn link(&mut self, vm: &mut Vm, e: u32, touch: bool) {
        let (mut ents, w) = self.ents(vm);
        w.link_edict(&mut ents, e, touch);
    }

    fn set_min_max_size(&mut self, vm: &mut Vm, e: u32, mn: [f32; 3], mx: [f32; 3]) -> Result<()> {
        if (0..3).any(|i| mn[i] > mx[i]) {
            return Err(vm.run_error("backwards mins/maxs"));
        }
        let f = &self.c.sv.fields;
        let (fmin, fmax, fsize) = (f.mins, f.maxs, f.size);
        vm.set_ent_v(e, fmin, mn);
        vm.set_ent_v(e, fmax, mx);
        vm.set_ent_v(e, fsize, [mx[0] - mn[0], mx[1] - mn[1], mx[2] - mn[2]]);
        self.link(vm, e, false);
        Ok(())
    }

    fn client_of(&self, e: i32) -> Option<usize> {
        (e >= 1 && e as usize <= self.c.maxclients()).then(|| e as usize - 1)
    }

    fn dest(&mut self, vm: &Vm) -> Result<Dest> {
        Ok(match vm.g_f(p(0)) as i32 {
            MSG_BROADCAST => Dest::Datagram,
            MSG_ONE => {
                let e = vm.g_i(self.c.sv.globals.msg_entity);
                match self.client_of(e) {
                    Some(i) => Dest::Client(i),
                    None => return Err(vm.run_error("WriteDest: not a client")),
                }
            }
            MSG_ALL => Dest::Reliable,
            MSG_INIT => Dest::Signon,
            _ => return Err(vm.run_error("WriteDest: bad destination")),
        })
    }

    fn with_dest(&mut self, vm: &Vm, f: impl FnOnce(&mut SizeBuf, u32)) -> Result<()> {
        let pf = self.c.sv.protocolflags;
        let d = self.dest(vm)?;
        let buf = match d {
            Dest::Datagram => &mut self.c.sv.datagram,
            Dest::Client(i) => &mut self.c.clients[i].message,
            Dest::Reliable => &mut self.c.sv.reliable_datagram,
            Dest::Signon => self.c.signon(64),
        };
        f(buf, pf);
        Ok(())
    }

    fn client_msg(&mut self, i: usize, svc: i32, s: &[u8]) {
        let m = &mut self.c.clients[i].message;
        m.char(svc);
        m.string(s);
    }

    /// Mod_ForName(name, true) for a precache.
    fn load_model(&mut self, vm: &Vm, name: &str, index: usize) -> Result<Option<ModelInfo>> {
        if name.starts_with('*') {
            return Ok(None);
        }
        // Mod_LoadModel dispatches on the file magic, not the extension
        // (some mods ship alias models named b_*.bsp).
        let cached = self.c.sv.brush_cache.get(name).cloned();
        let bsp_ext = name.to_ascii_lowercase().ends_with(".bsp");
        let data = if cached.is_some() || !bsp_ext { None } else { self.c.fs.open(name) };
        let is_brush = cached.is_some()
            || data.as_deref().is_some_and(|d| !matches!(d.get(..4), Some(b"IDPO" | b"IDSP" | b"IDP3")));
        if is_brush {
            let cm = match cached {
                Some(cm) => cm,
                None => {
                    let data = data.unwrap_or_default();
                    let bsp = qformats::Bsp::parse(&data)
                        .map_err(|e| VmError::new(format!("Mod_LoadModel: {name}: {e:?}")))?;
                    let cm = Arc::new(CollisionModel::from_bsp(&bsp));
                    self.c.sv.brush_cache.insert(name.to_string(), cm.clone());
                    cm
                }
            };
            let (mins, maxs) = cm.submodels.first().map_or((Vec3::ZERO, Vec3::ZERO), |m| (m.mins, m.maxs));
            self.w.set_model(index, Some((cm, 0)));
            return Ok(Some(ModelInfo { name: name.into(), brush: true, mins, maxs }));
        }
        if if bsp_ext { data.is_none() } else { !self.c.fs.exists(name) } {
            return Err(vm.run_error(format!("Mod_LoadModel: {name} not found")));
        }
        Ok(Some(ModelInfo { name: name.into(), brush: false, mins: Vec3::splat(-16.0), maxs: Vec3::splat(16.0) }))
    }

    fn precache_model(&mut self, vm: &mut Vm) -> Result<()> {
        if !self.c.sv.loading {
            return Err(vm.run_error("PF_Precache_*: Precache can only be done in spawn functions"));
        }
        let h = vm.g_i(p(0));
        vm.ret_i(h);
        let s = str_arg(vm, 0)?;
        if s.first().is_none_or(|&c| c <= b' ') {
            return Err(vm.run_error("Bad string"));
        }
        let s = lossy(&s);
        if self.c.sv.model_precache.contains(&s) {
            return Ok(());
        }
        let n = self.c.sv.model_precache.len();
        if n >= MAX_MODELS {
            return Err(vm.run_error("PF_precache_model: overflow"));
        }
        let info = self.load_model(vm, &s, n)?;
        self.c.sv.model_precache.push(s);
        self.c.sv.models.push(info);
        Ok(())
    }

    fn precache_sound(&mut self, vm: &mut Vm) -> Result<()> {
        if !self.c.sv.loading {
            return Err(vm.run_error("PF_Precache_*: Precache can only be done in spawn functions"));
        }
        let h = vm.g_i(p(0));
        vm.ret_i(h);
        let s = str_arg(vm, 0)?;
        if s.first().is_none_or(|&c| c <= b' ') {
            return Err(vm.run_error("Bad string"));
        }
        let s = lossy(&s);
        if self.c.sv.sound_precache.contains(&s) {
            return Ok(());
        }
        if self.c.sv.sound_precache.len() >= MAX_SOUNDS {
            return Err(vm.run_error("PF_precache_sound: overflow"));
        }
        self.c.sv.sound_precache.push(s);
        Ok(())
    }

    fn setmodel(&mut self, vm: &mut Vm) -> Result<()> {
        let e = ent_arg(vm, 0)?;
        let m = str_arg(vm, 1)?;
        let Some(i) = self.c.sv.model_precache.iter().position(|s| s.as_bytes() == m.as_slice()) else {
            return Err(vm.run_error(format!("no precache: {}", lossy(&m))));
        };
        let h = vm.strings.engine(&m);
        let f = &self.c.sv.fields;
        let (fm, fmi) = (f.model, f.modelindex);
        vm.set_ent_i(e, fm, h);
        vm.set_ent_f(e, fmi, i as f32);
        let (mn, mx) = match self.c.sv.models.get(i).and_then(|m| m.as_ref()) {
            Some(m) => (m.mins.to_array(), m.maxs.to_array()),
            None => ([0.0; 3], [0.0; 3]),
        };
        self.set_min_max_size(vm, e, mn, mx)
    }

    fn traceline(&mut self, vm: &mut Vm) -> Result<()> {
        let fix = |v: [f32; 3]| if v.iter().any(|x| x.is_nan()) { [0.0; 3] } else { v };
        let v1 = Vec3::from_array(fix(vm.g_v(p(0))));
        let v2 = Vec3::from_array(fix(vm.g_v(p(1))));
        let nomonsters = vm.g_f(p(2)) as i32;
        let pass = ent_arg(vm, 3)?;
        let tr = {
            let (ents, w) = self.ents(vm);
            w.trace(&ents, v1, Vec3::ZERO, Vec3::ZERO, v2, MoveType::from(nomonsters), Some(pass))
        };
        self.set_trace_globals(vm, &tr);
        Ok(())
    }

    fn set_trace_globals(&self, vm: &mut Vm, tr: &qworld::Trace) {
        let g = &self.c.sv.globals;
        vm.set_g_f(g.trace_allsolid, tr.allsolid as i32 as f32);
        vm.set_g_f(g.trace_startsolid, tr.startsolid as i32 as f32);
        vm.set_g_f(g.trace_fraction, tr.fraction);
        vm.set_g_f(g.trace_inwater, tr.inwater as i32 as f32);
        vm.set_g_f(g.trace_inopen, tr.inopen as i32 as f32);
        vm.set_g_v(g.trace_endpos, tr.endpos.to_array());
        vm.set_g_v(g.trace_plane_normal, tr.plane_normal.to_array());
        vm.set_g_f(g.trace_plane_dist, tr.plane_dist);
        vm.set_g_i(g.trace_ent, tr.ent.unwrap_or(0) as i32);
    }

    fn new_check_client(&mut self, vm: &Vm, check: u32) -> u32 {
        let maxc = self.c.maxclients() as u32;
        let f = &self.c.sv.fields;
        let check = check.clamp(1, maxc);
        let mut i = if check == maxc { 1 } else { check + 1 };
        loop {
            if i == maxc + 1 {
                i = 1;
            }
            if i == check {
                break;
            }
            if !vm.is_free(i) && vm.ent_f(i, f.health) > 0.0 && (vm.ent_f(i, f.flags) as i32) & FL_NOTARGET == 0 {
                break;
            }
            i += 1;
        }
        let org = Vec3::from_array(vm.ent_v(i, f.origin)) + Vec3::from_array(vm.ent_v(i, f.view_ofs));
        let leaf = self.c.sv.vis.point_in_leaf(org);
        self.c.sv.checkpvs = self.c.sv.vis.leaf_pvs(leaf);
        i
    }

    fn checkclient(&mut self, vm: &mut Vm) {
        if vm.time - self.c.sv.lastchecktime >= 0.1 {
            let lc = self.c.sv.lastcheck;
            self.c.sv.lastcheck = self.new_check_client(vm, lc);
            self.c.sv.lastchecktime = vm.time;
        }
        let f = &self.c.sv.fields;
        let ent = self.c.sv.lastcheck;
        if (ent as usize) >= vm.num_edicts() || vm.is_free(ent) || vm.ent_f(ent, f.health) <= 0.0 {
            vm.ret_e(0);
            return;
        }
        let s = vm.self_ent().min(vm.num_edicts() as u32 - 1);
        let view = Vec3::from_array(vm.ent_v(s, f.origin)) + Vec3::from_array(vm.ent_v(s, f.view_ofs));
        let l = self.c.sv.vis.point_in_leaf(view) as i64 - 1;
        let vis = l >= 0 && leafs_in_pvs(&[l as u32], &self.c.sv.checkpvs);
        vm.ret_e(if vis { ent } else { 0 });
    }

    fn aim(&mut self, vm: &mut Vm) -> Result<()> {
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

    fn makestatic(&mut self, vm: &mut Vm) -> Result<()> {
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

    fn ambientsound(&mut self, vm: &mut Vm) -> Result<()> {
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

    fn lightstyle(&mut self, vm: &mut Vm) -> Result<()> {
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

    fn findradius(&mut self, vm: &mut Vm) {
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

    fn find(&mut self, vm: &mut Vm) -> Result<()> {
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

    fn give_spawn_parms(&mut self, vm: &mut Vm) -> Result<()> {
        let e = vm.g_i(p(0));
        let Some(i) = self.client_of(e) else { return Err(vm.run_error("Entity is not a client")) };
        let g = self.c.sv.globals.parm1;
        for k in 0..NUM_SPAWN_PARMS {
            vm.set_g_f(g + k, self.c.clients[i].spawn_parms[k]);
        }
        Ok(())
    }
}

impl Host for Ctx<'_> {
    fn before_damage(&mut self, vm: &mut Vm) -> Result<bool> {
        crate::assists::before_damage(vm, self.c)
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
                let mv = Vec3::new(yaw.cos() * dist, yaw.sin() * dist, 0.0);
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
                let v = vm.g_f(p(1)) as i32;
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
            "clientstat" => {}
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
            "checkbuiltin" => vm.ret_f(0.0),
            "ex_CheckPlayerEXFlags" | "ex_walkpathtogoal" => vm.ret_f(0.0),
            _ => return Err(vm.run_error(format!("unimplemented builtin {name}"))),
        }
        Ok(())
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
