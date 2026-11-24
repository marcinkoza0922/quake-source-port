//! DP_* / FTE_* builtins that need the server (vkQuake `pr_ext.c`): traces,
//! multicast, client queries, cvar queries. `DP_QC_GETLIGHT` (no lightmaps
//! here) and bot clients (`spawnclient` finds no slot) are left out.

use super::*;

/// multicast_t
const PVS_U: i32 = 2;
const ALL_R: i32 = 3;
const PVS_R: i32 = 5;
const ONE_U: i32 = 6;
const ONE_R: i32 = 7;
const INIT: i32 = 8;
/// DATAGRAM_MTU
const BACKBUFFERED: usize = 1400;

/// Extensions these add.
pub(super) const EXTENSIONS: &[&str] = &[
    "DP_QC_CVAR_DEFSTRING",
    "DP_QC_CVAR_TYPE",
    "DP_QC_TRACEBOX",
    "DP_QC_TRACETOSS",
    "DP_QC_WHICHPACK",
    "DP_SV_DRAWONLYTOCLIENT",
    "DP_SV_DROPCLIENT",
    "DP_SV_NODRAWTOCLIENT",
    "DP_SV_POINTSOUND",
    "DP_SV_SPAWNFUNC_PREFIX",
    "DP_SV_WRITEUNTERMINATEDSTRING",
    "FTE_QC_INFOKEY",
    "FTE_QC_MULTICAST",
];

impl Ctx<'_> {
    /// True when `name` is one of these builtins.
    pub(super) fn dp_builtin(&mut self, vm: &mut Vm, name: &str) -> Result<bool> {
        match name {
            "tracebox" => {
                let fix = |v: [f32; 3]| Vec3::from_array(if v.iter().any(|x| x.is_nan()) { [0.0; 3] } else { v });
                let (v1, v2) = (fix(vm.g_v(p(0))), fix(vm.g_v(p(3))));
                let (mins, maxs) = (Vec3::from_array(vm.g_v(p(1))), Vec3::from_array(vm.g_v(p(2))));
                let (mt, pass) = (MoveType::from(vm.g_f(p(4)) as i32), ent_arg(vm, 5)?);
                let tr = {
                    let (ents, w) = self.ents(vm);
                    w.trace(&ents, v1, mins, maxs, v2, mt, Some(pass))
                };
                self.set_trace_globals(vm, &tr);
            }
            "tracetoss" => self.trace_toss(vm)?,
            "touchtriggers" => {
                let e = if vm.argc > 0 { ent_arg(vm, 0)? } else { vm.self_ent() };
                if vm.argc > 1 {
                    let o = vm.g_v(p(1));
                    vm.set_ent_v(e, self.c.sv.fields.origin, o);
                }
                self.link(vm, e, true);
            }
            "checkpvs" => {
                let org = Vec3::from_array(vm.g_v(p(0)));
                let e = ent_arg(vm, 1)?;
                let pvs = self.c.sv.vis.leaf_pvs(self.c.sv.vis.point_in_leaf(org));
                let seen = leafs_in_pvs(&self.c.ext(e).leafs, &pvs);
                vm.ret_f(f32::from(seen));
            }
            "pointsound" => {
                let (org, s) = (vm.g_v(p(0)), str_arg(vm, 1)?);
                let (vol, att) = ((vm.g_f(p(2)) * 255.0) as i32, vm.g_f(p(3)));
                crate::send::start_sound_at(vm, self.c, 0, 0, &s, vol, att, Some(org));
            }
            "multicast" => {
                let (org, to) = (vm.g_v(p(0)), vm.g_f(p(1)) as i32);
                self.multicast(vm, org, to);
            }
            "WriteFloat" => {
                let v = vm.g_f(p(1));
                self.with_dest(vm, |m, _| m.float(v))?;
            }
            "WriteInt" | "WriteUInt" => {
                let v = vm.g_i(p(1));
                self.with_dest(vm, |m, _| m.long(v))?;
            }
            "WriteUnterminatedString" => {
                let s = str_arg(vm, 1)?;
                self.with_dest(vm, |m, _| m.write(&s))?;
            }
            "te_blood" => {
                let (org, dir, n) = (vm.g_v(p(0)), vm.g_v(p(1)), vm.g_f(p(2)));
                self.start_particle(org, dir, 73, n as i32);
            }
            "te_bloodqw" => {
                let (org, n) = (vm.g_v(p(0)), vm.g_f(p(1)));
                self.start_particle(org, [0.0; 3], 73, (n * 20.0) as i32);
            }
            "te_lightningblood" => {
                let org = vm.g_v(p(0));
                self.start_particle(org, [0.0, 0.0, -100.0], 20, 225);
            }
            "setattachment" => {
                let (e, to) = (ent_arg(vm, 0)?, vm.g_i(p(1)));
                if let Some(o) = vm.field_ofs("tag_entity") {
                    vm.set_ent_i(e, o, to);
                }
                if let Some(o) = vm.field_ofs("tag_index") {
                    vm.set_ent_f(e, o, 0.0); // no model here has tags
                }
            }
            "isbackbuffered" => {
                let i = self.client_of(vm.g_i(p(0)));
                let clogged = i.is_none_or(|i| !self.c.clients[i].active || self.c.clients[i].message.len() > BACKBUFFERED);
                vm.ret_f(f32::from(clogged));
            }
            "clienttype" => {
                let ty = match self.client_of(vm.g_i(p(0))) {
                    None => 3.0,                                // not a client
                    Some(i) if self.c.clients[i].active => 1.0, // real
                    Some(_) => 0.0,                             // disconnected
                };
                vm.ret_f(ty);
            }
            "spawnclient" => vm.ret_e(0),
            // after the frame's QC: dropping runs ClientDisconnect
            "dropclient" => {
                if let Some(i) = self.client_of(vm.g_i(p(0))) {
                    self.c.queued_drops.push(i);
                }
            }
            "infokey" => match self.info_key(vm)? {
                Some(s) => vm.ret_temp(s.as_bytes()),
                None => vm.ret_i(0),
            },
            "infokeyf" => {
                let v = self.info_key(vm)?.map_or(0.0, |s| qcore::cvar::q_atof(&s));
                vm.ret_f(v);
            }
            "forceinfokey" => self.force_info_key(vm)?,
            // we keep no server info string
            "serverkey" => vm.ret_i(0),
            "serverkeyfloat" => vm.ret_f(0.0),
            "cvar_defstring" => {
                let n = lossy(&str_arg(vm, 0)?);
                match self.c.cvars.by_name(&n).map(|c| c.default.clone()) {
                    Some(d) => vm.ret_temp(d.as_bytes()),
                    None => vm.ret_i(0),
                }
            }
            "cvar_type" => {
                use qcore::cvar::{CVAR_ARCHIVE, CVAR_USERDEFINED};
                let n = lossy(&str_arg(vm, 0)?);
                let ty = self.c.cvars.by_name(&n).map_or(0, |c| {
                    let saved = if c.flags & CVAR_ARCHIVE != 0 { 2 } else { 0 };
                    let engine = if c.flags & CVAR_USERDEFINED == 0 { 8 } else { 0 };
                    1 | saved | engine
                });
                vm.ret_f(ty as f32);
            }
            "gettime" => {
                // one clock: frame time (0) and now (1) differ by under a frame
                let now = qcore::time::SystemTime::now().duration_since(self.c.started).map_or(0.0, |d| d.as_secs_f32());
                vm.ret_f(now);
            }
            "cvar_description" => vm.ret_i(0),
            "buf_cvarlist" => {
                let (pre, anti) = (lossy(&str_arg(vm, 1)?), lossy(&str_arg(vm, 2)?));
                let mut names: Vec<Vec<u8>> = self.c.cvars.iter().map(|c| c.name.as_str())
                    .filter(|n| (pre.is_empty() || matches(&pre, n)) && (anti.is_empty() || !matches(&anti, n)))
                    .map(|n| n.as_bytes().to_vec())
                    .collect();
                names.sort();
                vm.set_strbuf(vm.g_f(p(0)), names);
            }
            "whichpack" => {
                let n = lossy(&str_arg(vm, 0)?);
                match self.c.fs.origin(&n) {
                    Some(o) => vm.ret_temp(o.game.as_bytes()),
                    None => vm.ret_i(0),
                }
            }
            "digest_hex" => {
                let (kind, data) = (str_arg(vm, 0)?, vm.var_string(1)?);
                let digest: Vec<u8> = match kind.as_slice() {
                    b"CRC16" => qvm::crc_block(&data).to_le_bytes().to_vec(),
                    b"MD4" => qnet::qw::md4(&data).iter().flat_map(|w| w.to_le_bytes()).collect(),
                    k => {
                        self.c.print(&format!("PF_digest_hex: Unsupported digest {}\n", lossy(k)));
                        vm.ret_i(0);
                        return Ok(true);
                    }
                };
                let hex: String = digest.iter().map(|b| format!("{b:02X}")).collect();
                vm.ret_temp(hex.as_bytes());
            }
            "frameforname" | "frametoname" | "frameduration" => self.frame_info(vm, name)?,
            // placeholders in vkQuake too (mg3 calls ex_draw_bounds)
            n if n.starts_with("ex_draw_") || n.starts_with("ex_bot_") => vm.ret_f(0.0),
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// PF_TraceToss: where a tossed entity lands (at most ten seconds on).
    fn trace_toss(&mut self, vm: &mut Vm) -> Result<()> {
        let (e, ignore) = (ent_arg(vm, 0)?, ent_arg(vm, 1)?);
        let f = self.c.sv.fields.clone();
        let scale = f.gravity.map_or(0.0, |o| vm.ent_f(e, o));
        let gravity = if scale != 0.0 { scale } else { 1.0 } * self.c.cvars.value_of("sv_gravity") * 0.05;
        let maxvel = self.c.cvars.value_of("sv_maxvelocity");
        let v = |vm: &Vm, o: usize| Vec3::from_array(vm.ent_v(e, o));
        let (mut org, mut vel) = (v(vm, f.origin), v(vm, f.velocity));
        let (mins, maxs) = (v(vm, f.mins), v(vm, f.maxs));
        let mut tr = qworld::Trace::new(org);
        for _ in 0..200 {
            vel.z -= gravity;
            tr = {
                let (ents, w) = self.ents(vm);
                w.trace(&ents, org, mins, maxs, org + vel * 0.05, MoveType::Normal, Some(e))
            };
            org = tr.endpos;
            if tr.fraction < 1.0 && tr.ent.is_some_and(|t| t != ignore) {
                break;
            }
            if vel.length() > maxvel {
                vel *= maxvel / vel.length();
            }
        }
        tr.fraction = 0.0; // not meaningful here
        self.set_trace_globals(vm, &tr);
        Ok(())
    }

    /// SV_Multicast of what QC wrote to MSG_MULTICAST.
    fn multicast(&mut self, vm: &Vm, org: [f32; 3], mut to: i32) {
        let data = std::mem::take(&mut self.c.sv.multicast.data);
        if to == INIT {
            self.c.signon(data.len()).write(&data);
            if self.c.sv.loading {
                return;
            }
            to = ALL_R; // players already on
        }
        let one = |c: &mut Core, i: usize, reliable: bool| {
            let cl = &mut c.clients[i];
            if !cl.active {
                return;
            }
            if reliable {
                cl.message.write(&data);
            } else if cl.spawned && cl.datagrams.len() < crate::client::MAX_QUEUED_DATAGRAMS {
                cl.datagrams.push_back(data.clone());
            }
        };
        match to {
            PVS_U | PVS_R => {
                let pvs = self.c.sv.vis.leaf_pvs(self.c.sv.vis.point_in_leaf(Vec3::from_array(org)));
                for i in 0..self.c.maxclients() {
                    let o = vm.ent_v(i as u32 + 1, self.c.sv.fields.origin);
                    let leaf = self.c.sv.vis.point_in_leaf(Vec3::from_array(o));
                    if leaf == 0 || leafs_in_pvs(&[leaf as u32 - 1], &pvs) {
                        one(self.c, i, to == PVS_R);
                    }
                }
            }
            ONE_U | ONE_R => {
                if let Some(i) = self.client_of(vm.g_i(self.c.sv.globals.msg_entity)) {
                    one(self.c, i, to == ONE_R);
                }
            }
            // no PHS: everyone
            0 | 1 => self.c.sv.datagram.write(&data),
            ALL_R | 4 => self.c.sv.reliable_datagram.write(&data),
            _ => {}
        }
    }

    fn info_key(&mut self, vm: &Vm) -> Result<Option<String>> {
        let (e, key) = (vm.g_i(p(0)), str_arg(vm, 1)?);
        if e == 0 {
            return Ok((key == b"*version").then(|| concat!("quake-source-port ", env!("CARGO_PKG_VERSION")).to_string()));
        }
        let Some(cl) = self.client_of(e).map(|i| &self.c.clients[i]).filter(|c| c.active) else { return Ok(None) };
        Ok(Some(match key.as_slice() {
            b"ip" => if cl.local { "local" } else { "" }.to_string(),
            b"ping" => format!("{:.6}", cl.ping()),
            b"protocol" => match self.c.sv.protocol {
                PROTOCOL_NETQUAKE => "quake",
                PROTOCOL_FITZQUAKE => "fitz666",
                PROTOCOL_RMQ => "rmq999",
                _ => "",
            }
            .to_string(),
            b"name" => cl.name.clone(),
            b"topcolor" => (cl.colors >> 4).to_string(),
            b"bottomcolor" => (cl.colors & 15).to_string(),
            b"team" => format!("t{}", (cl.colors & 15) + 1),
            b"*VIP" | b"*spectator" => String::new(),
            b"csqcactive" => "0".to_string(),
            _ => return Ok(None),
        }))
    }

    /// The keys the engine acts on (SV_DecodeUserInfo); others aren't kept.
    fn force_info_key(&mut self, vm: &Vm) -> Result<()> {
        let Some(i) = self.client_of(vm.g_i(p(0))).filter(|&i| self.c.clients[i].active) else { return Ok(()) };
        let (key, value) = (str_arg(vm, 1)?, lossy(&str_arg(vm, 2)?));
        let colors = self.c.clients[i].colors;
        let n = qcore::cvar::q_atof(&value) as i32 & 15;
        let cmd = match key.as_slice() {
            b"name" => format!("name \"{}\"", value.replace(['"', '\n', ';'], "")),
            b"topcolor" => format!("color {n} {}", colors & 15),
            b"bottomcolor" => format!("color {} {n}", colors >> 4),
            _ => return Ok(()),
        };
        self.c.queued_client_cmds.push((i, cmd));
        Ok(())
    }

    /// Frame names and lengths of the alias model in precache slot parm 0.
    fn frame_info(&mut self, vm: &mut Vm, name: &str) -> Result<()> {
        let index = vm.g_f(p(0)).max(0.0) as usize;
        if !self.c.sv.frame_names.contains_key(&index) {
            let file = self.c.sv.model_precache.get(index).filter(|n| !n.is_empty() && !n.starts_with('*'));
            let frames = file.and_then(|n| self.c.fs.open(n)).filter(|d| d.starts_with(b"IDPO"))
                .and_then(|d| qformats::mdl::Mdl::parse(&d).ok())
                .map(|m| {
                    m.frames.iter().map(|f| {
                        // aliashdr: numposes * the first interval
                        let secs = f.poses.len() as f32 * f.intervals.first().copied().unwrap_or(0.0);
                        (f.poses.first().map_or(String::new(), |p| p.name.clone()), secs)
                    }).collect()
                })
                .unwrap_or_default();
            self.c.sv.frame_names.insert(index, frames);
        }
        let frames = &self.c.sv.frame_names[&index];
        let frame = vm.g_f(p(1));
        let picked = if frame >= 0.0 { frames.get(frame as usize) } else { None };
        match name {
            "frameforname" => {
                let want = lossy(vm.g_str(p(1))?);
                let i = frames.iter().position(|f| f.0 == want);
                vm.ret_f(i.map_or(-1.0, |i| i as f32));
            }
            "frametoname" => match picked {
                Some(f) => vm.ret_temp(f.0.as_bytes()),
                None => vm.ret_i(0),
            },
            _ => vm.ret_f(picked.map_or(0.0, |f| f.1)),
        }
        Ok(())
    }

    /// SV_StartParticle.
    pub(super) fn start_particle(&mut self, org: [f32; 3], dir: [f32; 3], color: i32, count: i32) {
        let pf = self.c.sv.protocolflags;
        let m = &mut self.c.sv.datagram;
        if m.len() > MAX_DATAGRAM - 18 {
            return;
        }
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

/// A prefix, or a pattern when it holds `*` or `?` (wildcmp).
fn matches(pattern: &str, name: &str) -> bool {
    fn wild(p: &[u8], s: &[u8]) -> bool {
        match (p.first(), s.first()) {
            (None, None) => true,
            (Some(b'*'), _) => wild(&p[1..], s) || (!s.is_empty() && wild(p, &s[1..])),
            (Some(b'?'), Some(_)) => wild(&p[1..], &s[1..]),
            (Some(a), Some(b)) if a.eq_ignore_ascii_case(b) => wild(&p[1..], &s[1..]),
            _ => false,
        }
    }
    if pattern.contains(['*', '?']) { wild(pattern.as_bytes(), name.as_bytes()) } else { name.starts_with(pattern) }
}

#[cfg(test)]
mod tests;
