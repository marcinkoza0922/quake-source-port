//! Outgoing messages: sounds, client data, entity updates, signon flow
//! (SV_StartSound, SV_WriteClientdataToMessage, SV_WriteEntitiesToClient,
//! SV_SendClientMessages).

use qvm::Vm;
use qworld::{FL_ONGROUND, MOVETYPE_STEP, Vec3, World};

use qnet::delta::EntState;

use crate::client::{MAX_QUEUED_DATAGRAMS, Signon};
use crate::ents::Ents;
use crate::msg::*;
use crate::server::{Core, Server};
use crate::vis::leafs_in_pvs;
mod entities;
use entities::*;
mod clientdata;
pub(crate) use clientdata::*;

/// SV_StartSound into the server datagram.
pub(crate) fn start_sound(vm: &Vm, c: &mut Core, e: u32, channel: i32, sample: &[u8], volume: i32, att: f32) {
    if !(0..=255).contains(&volume) || !(0.0..=4.0).contains(&att) || !(0..=7).contains(&channel) {
        c.print(&format!("SV_StartSound: bad volume/attenuation/channel {volume} {att} {channel}\n"));
        return;
    }
    let sv = &c.sv;
    if sv.datagram.len() > MAX_DATAGRAM - 21 {
        return;
    }
    let Some(snum) = sv.sound_precache.iter().skip(1).position(|s| s.as_bytes() == sample).map(|i| i + 1) else {
        let s = format!("SV_StartSound: {} not precached\n", String::from_utf8_lossy(sample));
        c.print(&s);
        return;
    };
    if (e as usize) >= vm.num_edicts() {
        return;
    }
    let mut mask = 0;
    if volume != 255 {
        mask |= SND_VOLUME;
    }
    if att != 1.0 {
        mask |= SND_ATTENUATION;
    }
    if e >= 8192 {
        if sv.protocol == PROTOCOL_NETQUAKE {
            return;
        }
        mask |= SND_LARGEENTITY;
    }
    if snum >= 256 {
        if sv.protocol == PROTOCOL_NETQUAKE {
            return;
        }
        mask |= SND_LARGESOUND;
    }
    let f = &sv.fields;
    let (o, mn, mx) = (vm.ent_v(e, f.origin), vm.ent_v(e, f.mins), vm.ent_v(e, f.maxs));
    let pf = sv.protocolflags;
    let m = &mut c.sv.datagram;
    m.byte(SVC_SOUND);
    m.byte(mask);
    if mask & SND_VOLUME != 0 {
        m.byte(volume);
    }
    if mask & SND_ATTENUATION != 0 {
        m.byte((att * 64.0) as i32);
    }
    if mask & SND_LARGEENTITY != 0 {
        m.short(e as i32);
        m.byte(channel);
    } else {
        m.short(((e as i32) << 3) | channel);
    }
    if mask & SND_LARGESOUND != 0 {
        m.short(snum as i32);
    } else {
        m.byte(snum as i32);
    }
    for i in 0..3 {
        m.coord(o[i] + 0.5 * (mn[i] + mx[i]), pf);
    }
}

impl Server {
    /// SV_UpdateToReliableMessages + SV_SendClientMessages + SV_CleanupEnts.
    pub(crate) fn send_client_messages(&mut self) {
        let Some(l) = &mut self.level else { return };
        let c = &mut self.core;
        let f = c.sv.fields.clone();
        // frags
        for i in 0..c.maxclients() {
            let frags = l.vm.ent_f(i as u32 + 1, f.frags) as i32;
            if c.clients[i].old_frags != frags {
                for cl in c.clients.iter_mut().filter(|c| c.active) {
                    cl.message.byte(SVC_UPDATEFRAGS);
                    cl.message.byte(i as i32);
                    cl.message.short(frags);
                }
                c.clients[i].old_frags = frags;
            }
        }
        let rd = std::mem::take(&mut c.sv.reliable_datagram.data);
        for cl in c.clients.iter_mut().filter(|c| c.active) {
            cl.message.write(&rd);
        }
        let mut pvs = Vec::new();
        let mut drop = Vec::new();
        for i in 0..c.maxclients() {
            if !c.clients[i].active {
                continue;
            }
            if c.clients[i].spawned && c.clients[i].delta.is_some() && !c.skip_entity_updates {
                send_deltas(&mut l.vm, c, &mut l.world, i, &mut pvs);
            } else if c.clients[i].spawned {
                let mut msg = SizeBuf::new(c.clients[i].max_datagram);
                msg.byte(SVC_TIME);
                msg.float(l.vm.time as f32);
                let e = i as u32 + 1;
                write_clientdata(&mut l.vm, c, &mut l.world, e, &mut msg);
                if !c.skip_entity_updates {
                    write_entities(&mut l.vm, c, e, &mut msg, &mut pvs);
                }
                if msg.len() + c.sv.datagram.len() < msg.maxsize {
                    msg.write(&c.sv.datagram.data);
                }
                push_datagram(c, i, msg.data);
            }
            if flush(c, i) {
                drop.push(i);
            }
        }
        for e in 1..l.vm.num_edicts() as u32 {
            let fx = l.vm.ent_f(e, f.effects) as i32;
            if fx & EF_MUZZLEFLASH != 0 {
                l.vm.set_ent_f(e, f.effects, (fx & !EF_MUZZLEFLASH) as f32);
            }
        }
        for i in drop {
            self.core.print(&format!("client {i} overflowed\n"));
            self.drop_client(i, true);
        }
    }

    /// Deliver client `i`'s pending reliable message (signon progression included).
    pub(crate) fn flush_client(&mut self, i: usize) {
        if flush(&mut self.core, i) {
            self.drop_client(i, true);
        }
    }
}

/// Signon progression + reliable delivery. Returns true if the client overflowed.
fn flush(c: &mut Core, i: usize) -> bool {
    let cl = &mut c.clients[i];
    if !cl.spawned {
        if cl.sendsignon == Signon::SignonBufs {
            while cl.signonidx < c.sv.signon.len() {
                let s = &c.sv.signon[cl.signonidx];
                if cl.message.len() + s.len() > cl.message.maxsize {
                    break;
                }
                cl.message.write(&s.data);
                cl.signonidx += 1;
            }
            if cl.signonidx == c.sv.signon.len() {
                cl.sendsignon = Signon::SignonMsg;
            }
        }
        if cl.sendsignon == Signon::SignonMsg && cl.message.len() + 2 < cl.message.maxsize {
            cl.message.byte(SVC_SIGNONNUM);
            cl.message.byte(2);
            cl.sendsignon = Signon::Flush;
        }
    }
    if cl.message.overflowed {
        cl.message.overflowed = false;
        cl.message.clear();
        return true;
    }
    if !cl.message.is_empty() {
        let d = std::mem::take(&mut cl.message.data);
        cl.reliable_out.push(d);
        if cl.sendsignon == Signon::Flush {
            cl.sendsignon = Signon::Done;
        }
    }
    false
}
