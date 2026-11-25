//! CSQC `qvm::Host` implementation: stats, 2D draw, 3D scene, `read*` net events,
//! cvars, commands, player/server keys, world traces, models, sounds, particles.

use qcore::math::Vec3;
use qsound::SoundEvent;
use qvm::{Builtin, Host as VmHost, Result, Vm, ofs_parm};

use super::draw::DrawSpace;

mod draw_cmds;
mod scene_cmds;
use super::{CsqcEnv, CsqcState};

pub struct CsqcHost<'a, 'b> {
    pub state: &'a mut CsqcState,
    pub env: &'a mut CsqcEnv<'b>,
}

const CSQC_EXTENSIONS: &[&str] = &[
    "EXT_CSQC",
    "FTE_STRINGS",
    "DP_QC_SPRINTF",
    "DP_CON_SET",
    "DP_REGISTERCVAR",
    "DP_QC_CVAR_STRING",
    "DP_QC_TRACEBOX",
    "DP_CSQC_ENTITYNOCULL",
    "FTE_CSQC_BASEFRAME",
    "DP_QC_GETSURFACE",
];

fn p(i: usize) -> usize {
    ofs_parm(i)
}

fn s_arg(vm: &Vm, i: usize) -> Result<String> {
    Ok(String::from_utf8_lossy(vm.g_str(p(i))?).into_owned())
}

fn b_arg(vm: &Vm, i: usize) -> Result<Vec<u8>> {
    Ok(vm.g_str(p(i))?.to_vec())
}

impl CsqcHost<'_, '_> {
    fn draw_space(&self) -> DrawSpace {
        DrawSpace {
            fb_w: self.env.width.max(1) as f32,
            fb_h: self.env.height.max(1) as f32,
            virt_w: self.env.virt[0].max(1.0),
            virt_h: self.env.virt[1].max(1.0),
        }
    }

    /// Virtual-display cvar values matching vkQuake's `PR_GetCSQCCvarValue`:
    /// reports a ~1080p reference display at the current aspect ratio so mods
    /// that compute pixel offsets from `scr_sbarscale`/`vid_width` (e.g. AD)
    /// lay out identically at 720p, 1080p, 1440p, and 4K.
    fn csqc_display_cvar(&self, name: &str) -> Option<f32> {
        let sp = self.draw_space();
        let aspect = sp.fb_w / sp.fb_h.max(1.0);
        let ref_w = (2_000_000.0 * aspect).sqrt().round();
        let ref_h = (2_000_000.0 / aspect).sqrt().round();
        if name.eq_ignore_ascii_case("scr_sbarscale") {
            Some((ref_w / sp.virt_w.max(1.0)).max(1.0))
        } else if name.eq_ignore_ascii_case("vid_width") {
            Some(ref_w)
        } else if name.eq_ignore_ascii_case("vid_height") {
            Some(ref_h)
        } else {
            None
        }
    }

    fn resolve_model(&mut self, name: &str) -> Option<qrender::ModelId> {
        if name.is_empty() {
            return None;
        }
        if let Some(m) = self.env.level.named(self.env.fs, self.env.r, name) {
            return Some(m.id);
        }
        None
    }

    fn model_name_for_index(&self, idx: i32) -> &str {
        if idx > 0 {
            self.env.cl.model_precache.get(idx as usize).map_or("", String::as_str)
        } else if idx < 0 {
            self.state
                .extra_models
                .get((-idx - 1) as usize)
                .map_or("", String::as_str)
        } else {
            ""
        }
    }

    fn player_key(&self, slot: i32, key: &str) -> String {
        let idx = if slot < 0 {
            let mut order: Vec<usize> = (0..self.env.cl.scores.len())
                .filter(|&i| !self.env.cl.scores[i].name.is_empty())
                .collect();
            order.sort_by(|&a, &b| self.env.cl.scores[b].frags.cmp(&self.env.cl.scores[a].frags));
            let rank = (-slot - 1) as usize;
            let Some(&i) = order.get(rank) else { return String::new() };
            i
        } else {
            slot as usize
        };
        let Some(sc) = self.env.cl.scores.get(idx) else {
            return String::new();
        };
        if sc.name.is_empty() {
            return String::new();
        }
        match key.to_ascii_lowercase().as_str() {
            "name" => sc.name.clone(),
            "frags" => sc.frags.to_string(),
            "ping" => sc.ping.to_string(),
            "pl" => "0".to_string(),
            "topcolor" => ((sc.colors >> 4) & 15).to_string(),
            "bottomcolor" => (sc.colors & 15).to_string(),
            "colors" => sc.colors.to_string(),
            "userid" => (idx + 1).to_string(),
            "viewentity" | "entnum" => (idx + 1).to_string(),
            "team" => ((sc.colors & 15) + 1).to_string(),
            _ => String::new(),
        }
    }

    fn server_key(&self, key: &str) -> String {
        match key.to_ascii_lowercase().as_str() {
            "mapname" => self.env.cl.mapname.clone(),
            "levelname" => self.env.cl.levelname.clone(),
            "maxclients" => self.env.cl.maxclients.to_string(),
            "protocol" => format!("QSP-{}", self.env.cl.protocol),
            "gamedir" => self.env.cl.server_gamedir.clone(),
            "hostname" => self.env.cvars.by_name("hostname").map_or("", |c| c.string.as_str()).to_string(),
            other => self.env.cvars.by_name(other).map_or("", |c| c.string.as_str()).to_string(),
        }
    }

    fn trace_world(&self, vm: &mut Vm, start: [f32; 3], end: [f32; 3]) {
        let g = self.state.globals;
        let (s, e) = (Vec3::from_array(start), Vec3::from_array(end));
        let tr = self
            .env
            .level
            .collision_model()
            .map(|cm| cm.hull(0, 0).trace(s, e))
            .unwrap_or_else(|| qworld::Trace::new(e));
        let set_f = |vm: &mut Vm, ofs: usize, v: f32| if ofs != 0 { vm.set_g_f(ofs, v); };
        let set_v = |vm: &mut Vm, ofs: usize, v: [f32; 3]| if ofs != 0 { vm.set_g_v(ofs, v); };
        set_f(vm, g.trace_allsolid, if tr.allsolid { 1.0 } else { 0.0 });
        set_f(vm, g.trace_startsolid, if tr.startsolid { 1.0 } else { 0.0 });
        set_f(vm, g.trace_fraction, tr.fraction);
        set_v(vm, g.trace_endpos, tr.endpos.to_array());
        set_v(vm, g.trace_plane_normal, tr.plane_normal.to_array());
        set_f(vm, g.trace_plane_dist, tr.plane_dist);
        if g.trace_ent != 0 {
            vm.set_g_i(g.trace_ent, 0);
        }
        set_f(vm, g.trace_inopen, if tr.inopen { 1.0 } else { 0.0 });
        set_f(vm, g.trace_inwater, if tr.inwater { 1.0 } else { 0.0 });
    }
}

impl VmHost for CsqcHost<'_, '_> {
    fn has_extension(&self, name: &str) -> bool {
        CSQC_EXTENSIONS.iter().any(|&e| e.eq_ignore_ascii_case(name))
    }

    fn extensions_enabled(&self) -> bool {
        self.env.cvars.value_of("pr_checkextension") != 0.0
    }

    fn dprint(&mut self, msg: &str) {
        if self.env.cvars.value_of("developer") != 0.0 {
            self.env.con.print(msg);
        }
    }

    fn print(&mut self, msg: &str) {
        self.env.con.print(msg);
    }

    fn builtin(&mut self, vm: &mut Vm, b: Builtin) -> Result<()> {
        match b {
            Builtin::Setorigin => {
                let e = vm.g_i(p(0)) as u32;
                let org = vm.g_v(p(1));
                if self.state.fields.origin != usize::MAX && !vm.is_free(e) {
                    vm.set_ent_v(e, self.state.fields.origin, org);
                }
            }
            Builtin::Setsize => {
                let e = vm.g_i(p(0)) as u32;
                let (mn, mx) = (vm.g_v(p(1)), vm.g_v(p(2)));
                if !vm.is_free(e) {
                    if self.state.fields.mins != usize::MAX { vm.set_ent_v(e, self.state.fields.mins, mn); }
                    if self.state.fields.maxs != usize::MAX { vm.set_ent_v(e, self.state.fields.maxs, mx); }
                }
            }
            Builtin::Setmodel => {
                let e = vm.g_i(p(0)) as u32;
                let name = s_arg(vm, 1)?;
                if !vm.is_free(e) {
                    if self.state.fields.model != usize::MAX {
                        let h = vm.strings.engine(name.as_bytes());
                        vm.set_ent_i(e, self.state.fields.model, h);
                    }
                    let idx = if let Some(i) = self.env.cl.model_precache.iter().position(|m| m == &name) {
                        i as i32
                    } else if !name.is_empty() {
                        self.resolve_model(&name);
                        let pos = self.state.extra_models.iter().position(|m| m == &name).unwrap_or_else(|| {
                            self.state.extra_models.push(name);
                            self.state.extra_models.len() - 1
                        });
                        -(pos as i32 + 1)
                    } else {
                        0
                    };
                    if self.state.fields.modelindex != usize::MAX {
                        vm.set_ent_f(e, self.state.fields.modelindex, idx as f32);
                    }
                }
            }
            Builtin::PrecacheModel | Builtin::PrecacheModel2 => {
                let h = vm.g_i(p(0));
                let name = s_arg(vm, 0)?;
                if !name.is_empty() {
                    self.resolve_model(&name);
                }
                vm.ret_i(h);
            }
            Builtin::PrecacheSound | Builtin::PrecacheSound2 => {
                let h = vm.g_i(p(0));
                let name = s_arg(vm, 0)?;
                if let Some(snd) = &mut self.env.snd {
                    let _ = snd.precache(&name);
                }
                vm.ret_i(h);
            }
            Builtin::Traceline => {
                let s = vm.g_v(p(0));
                let e = vm.g_v(p(1));
                self.trace_world(vm, s, e);
            }
            Builtin::Pointcontents => {
                let pt = Vec3::from_array(vm.g_v(p(0)));
                let c = self.env.level.collision_model().map_or(qworld::CONTENTS_EMPTY, |cm| cm.point_contents(pt));
                vm.ret_f(c as f32);
            }
            Builtin::Cvar => {
                let name = s_arg(vm, 0)?;
                let val = self.csqc_display_cvar(&name).unwrap_or_else(|| self.env.cvars.value_of(&name));
                vm.ret_f(val);
            }
            Builtin::CvarSet => {
                let (name, val) = (s_arg(vm, 0)?, s_arg(vm, 1)?);
                self.env.cvars.set_script(&name, &val);
            }
            Builtin::Localcmd => {
                let s = s_arg(vm, 0)?;
                self.env.cbuf.add_text(&s);
            }
            Builtin::Bprint | Builtin::Dprint => {
                let s = String::from_utf8_lossy(&vm.var_string(0)?).into_owned();
                if b == Builtin::Bprint || self.env.cvars.value_of("developer") != 0.0 {
                    self.env.con.print(&s);
                }
            }
            Builtin::Centerprint => {
                let s = String::from_utf8_lossy(&vm.var_string(0)?).into_owned();
                let t = self.env.cvars.value_of("scr_centertime").max(0.1) as f64;
                self.env.con.center_print(&s, t, 40);
            }
            Builtin::Sound => {
                let e = vm.g_i(p(0)) as u32;
                let chan = vm.g_f(p(1)) as i32;
                let sample = s_arg(vm, 2)?;
                let vol = vm.g_f(p(3)).clamp(0.0, 1.0);
                let atten = vm.g_f(p(4));
                let origin = if self.state.fields.origin != usize::MAX && !vm.is_free(e) {
                    vm.ent_v(e, self.state.fields.origin)
                } else {
                    self.env.base_view.map_or([0.0; 3], |v| v.origin)
                };
                if let Some(snd) = &mut self.env.snd
                    && let Some(sfx) = snd.precache(&sample)
                {
                    snd.event(SoundEvent::Start { entity: e as i32, channel: chan, sfx, origin, volume: vol, attenuation: atten });
                }
            }
            Builtin::Localsound => {
                let sample = s_arg(vm, 0)?;
                if let Some(snd) = &mut self.env.snd
                    && let Some(sfx) = snd.precache(&sample)
                {
                    snd.event(SoundEvent::Local(sfx));
                }
            }
            Builtin::Error | Builtin::Objerror => {
                let s = String::from_utf8_lossy(&vm.var_string(0)?).into_owned();
                return Err(vm.run_error(format!("CSQC {s}")));
            }
            Builtin::PrecacheFile | Builtin::PrecacheFile2 => {
                let h = vm.g_i(p(0));
                vm.ret_i(h);
            }
            _ => {}
        }
        Ok(())
    }

    fn ext_builtin(&mut self, vm: &mut Vm, name: &str) -> Result<()> {
        if self.ext_draw_stat_net(vm, name)? || self.ext_scene(vm, name) {
            return Ok(());
        }
        match name {

            // ---- Player / Server Keys & Cvars / Commands ----
            "getplayerkeyvalue" => {
                let val = self.player_key(vm.g_f(p(0)) as i32, &s_arg(vm, 1)?);
                vm.ret_temp(val.as_bytes());
            }
            "getplayerkeyfloat" => {
                let val = self.player_key(vm.g_f(p(0)) as i32, &s_arg(vm, 1)?);
                vm.ret_f(qvm::atof(val.as_bytes()));
            }
            "serverkey" => {
                let val = self.server_key(&s_arg(vm, 0)?);
                vm.ret_temp(val.as_bytes());
            }
            "serverkeyfloat" => {
                let val = self.server_key(&s_arg(vm, 0)?);
                vm.ret_f(qvm::atof(val.as_bytes()));
            }
            "cvar_string" => {
                let k = s_arg(vm, 0)?;
                if let Some(v) = self.csqc_display_cvar(&k) {
                    let s = format!("{v}");
                    vm.ret_temp(s.as_bytes());
                } else {
                    let v = self.env.cvars.by_name(&k).map_or("", |c| c.string.as_str()).as_bytes().to_vec();
                    vm.ret_temp(&v);
                }
            }
            "cvar_defstring" => {
                let k = s_arg(vm, 0)?;
                let v = self.env.cvars.by_name(&k).map_or("", |c| c.default.as_str()).as_bytes().to_vec();
                vm.ret_temp(&v);
            }
            "registercvar" => {
                let k = s_arg(vm, 0)?;
                let def = s_arg(vm, 1)?;
                let flags = if vm.argc >= 3 { vm.g_f(p(2)) as u32 } else { 0 };
                let existed = self.env.cvars.find(&k).is_some();
                if !existed {
                    self.env.cvars.register(&k, &def, flags);
                }
                vm.ret_f(if existed { 0.0 } else { 1.0 });
            }
            "registercommand" => {
                let cmd = s_arg(vm, 0)?.to_ascii_lowercase();
                if !cmd.is_empty() {
                    self.state.commands.insert(cmd);
                }
            }
            "checkcommand" => {
                let cmd = s_arg(vm, 0)?.to_ascii_lowercase();
                let found = self.state.commands.contains(&cmd) || self.env.cvars.find(&cmd).is_some();
                vm.ret_f(if found { 1.0 } else { 0.0 });
            }
            "print" => {
                let s = String::from_utf8_lossy(&vm.var_string(0)?).into_owned();
                self.env.con.print(&s);
            }
            "setmodelindex" => {
                let e = vm.g_i(p(0)) as u32;
                let idx = vm.g_f(p(1)) as i32;
                if !vm.is_free(e) {
                    if self.state.fields.modelindex != usize::MAX {
                        vm.set_ent_f(e, self.state.fields.modelindex, idx as f32);
                    }
                    if self.state.fields.model != usize::MAX {
                        let nm = self.model_name_for_index(idx).as_bytes().to_vec();
                        let h = vm.strings.engine(&nm);
                        vm.set_ent_i(e, self.state.fields.model, h);
                    }
                }
            }
            "modelnameforindex" => {
                let nm = self.model_name_for_index(vm.g_f(p(0)) as i32).as_bytes().to_vec();
                vm.ret_temp(&nm);
            }
            "pointsound" => {
                let origin = vm.g_v(p(0));
                let sample = s_arg(vm, 1)?;
                let vol = vm.g_f(p(2)).clamp(0.0, 1.0);
                let atten = vm.g_f(p(3));
                if let Some(snd) = &mut self.env.snd
                    && let Some(sfx) = snd.precache(&sample)
                {
                    snd.event(SoundEvent::Start { entity: -1, channel: 0, sfx, origin, volume: vol, attenuation: atten });
                }
            }
            "cl_localsound" => {
                let sample = s_arg(vm, 0)?;
                if let Some(snd) = &mut self.env.snd
                    && let Some(sfx) = snd.precache(&sample)
                {
                    snd.event(SoundEvent::Local(sfx));
                }
            }
            "isdemo" => vm.ret_f(if self.env.is_demo { 1.0 } else { 0.0 }),
            "gettime" => vm.ret_f(self.env.realtime as f32),
            "setcursormode" => {
                self.state.cursor_mode = vm.g_f(p(0)) != 0.0;
            }
            "getentity" => {
                let entnum = vm.g_f(p(0)) as usize;
                let field = vm.g_f(p(1)) as i32;
                if let Some(e) = self.env.cl.entities.get(entnum) {
                    match field {
                        0 => vm.ret_f(if e.model != 0 { 1.0 } else { 0.0 }), // GE_ACTIVE
                        1 => vm.ret_v(e.origin),                              // GE_ORIGIN
                        4 => vm.ret_f(e.model as f32),                        // GE_MODELINDEX
                        5 => vm.ret_f(e.skin as f32),                         // GE_SKIN
                        6 => vm.ret_f(e.frame as f32),                        // GE_FRAME
                        7 => vm.ret_f(e.effects as f32),                      // GE_EFFECTS
                        8 => vm.ret_v(e.angles),                              // GE_ANGLES
                        9 => vm.ret_f(e.alpha as f32 / 255.0),                // GE_ALPHA
                        _ => vm.ret_v([0.0; 3]),
                    }
                } else {
                    vm.ret_v([0.0; 3]);
                }
            }
            "tracebox" => {
                let s = vm.g_v(p(0));
                let e = vm.g_v(p(3));
                self.trace_world(vm, s, e);
            }
            "particleeffectnum" | "pointparticles" | "trailparticles" | "getinputstate"
            | "frameforname" | "frameduration" | "frametoname" => {
                vm.ret_f(0.0);
            }
            _ => {
                // Unknown optional CSQC extension builtin: return 0 cleanly instead of crashing HUD
                vm.ret_v([0.0; 3]);
            }
        }
        Ok(())
    }
}
