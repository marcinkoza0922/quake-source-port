//! `qvm::Host`: the world/network builtins of pr_cmds.c.

use std::sync::Arc;

use qvm::{Builtin, Host, Result, Vm, VmError, ofs_parm};
use qworld::world::MoveType;
use qworld::{CollisionModel, FL_FLY, FL_ONGROUND, FL_SWIM, SOLID_NOT, Vec3, World};

use crate::ents::Ents;
use crate::msg::*;
use crate::server::{Core, ModelInfo, ed_free};
use crate::vis::leafs_in_pvs;
mod model;
mod trace;
mod entities;
mod feedback;

const FL_NOTARGET: i32 = 128;
const DAMAGE_AIM: f32 = 2.0;
const MSG_BROADCAST: i32 = 0;
const MSG_ONE: i32 = 1;
const MSG_ALL: i32 = 2;
const MSG_INIT: i32 = 3;

pub(crate) struct Ctx<'a> {
    pub c: &'a mut Core,
    pub w: &'a mut World,
    damage_calls: Vec<Option<feedback::DamageCall>>,
}

impl<'a> Ctx<'a> {
    pub(crate) fn new(c: &'a mut Core, w: &'a mut World) -> Self {
        Self { c, w, damage_calls: Vec::new() }
    }
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

mod builtins;

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

}
