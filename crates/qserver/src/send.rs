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

/// SV_WriteClientdataToMessage.
pub(crate) fn write_clientdata(vm: &mut Vm, c: &mut Core, w: &mut World, e: u32, msg: &mut SizeBuf) {
    let f = c.sv.fields.clone();
    let g = c.sv.globals.clone();
    let pf = c.sv.protocolflags;
    let fi = |vm: &Vm, o: usize| vm.ent_f(e, o) as i32;
    if vm.ent_f(e, f.dmg_take) != 0.0 || vm.ent_f(e, f.dmg_save) != 0.0 {
        let mut other = vm.ent_i(e, f.dmg_inflictor);
        if other < 0 || other as usize >= vm.num_edicts() {
            other = 0;
        }
        let o = other as u32;
        msg.byte(SVC_DAMAGE);
        msg.byte(fi(vm, f.dmg_save));
        msg.byte(fi(vm, f.dmg_take));
        let (org, mn, mx) = (vm.ent_v(o, f.origin), vm.ent_v(o, f.mins), vm.ent_v(o, f.maxs));
        for i in 0..3 {
            msg.coord(org[i] + 0.5 * (mn[i] + mx[i]), pf);
        }
        vm.set_ent_f(e, f.dmg_take, 0.0);
        vm.set_ent_f(e, f.dmg_save, 0.0);
    }
    let p = c.sv.phys;
    let p = qworld::PhysParams { idealpitchscale: c.cvars.value_of("sv_idealpitchscale"), ..p };
    qworld::user::set_ideal_pitch(w, &mut Ents { vm: &mut *vm, c: &mut *c }, &p, e);

    if vm.ent_f(e, f.fixangle) != 0.0 {
        msg.byte(SVC_SETANGLE);
        let a = vm.ent_v(e, f.angles);
        for x in a {
            msg.angle(x, pf);
        }
        vm.set_ent_f(e, f.fixangle, 0.0);
    }
    let mut bits = 0u32;
    if vm.ent_v(e, f.view_ofs)[2] != DEFAULT_VIEWHEIGHT {
        bits |= SU_VIEWHEIGHT;
    }
    if vm.ent_f(e, f.idealpitch) != 0.0 {
        bits |= SU_IDEALPITCH;
    }
    let items = match f.items2 {
        Some(o) => fi(vm, f.items) | ((vm.ent_f(e, o) as i32) << 23),
        None => fi(vm, f.items) | ((vm.g_f(g.serverflags) as i32) << 28),
    };
    bits |= SU_ITEMS;
    if fi(vm, f.flags) & FL_ONGROUND != 0 {
        bits |= SU_ONGROUND;
    }
    if vm.ent_f(e, f.waterlevel) >= 2.0 {
        bits |= SU_INWATER;
    }
    let (punch, vel) = (vm.ent_v(e, f.punchangle), vm.ent_v(e, f.velocity));
    for i in 0..3 {
        if punch[i] != 0.0 {
            bits |= SU_PUNCH1 << i;
        }
        if vel[i] != 0.0 {
            bits |= SU_VELOCITY1 << i;
        }
    }
    if vm.ent_f(e, f.weaponframe) != 0.0 {
        bits |= SU_WEAPONFRAME;
    }
    if vm.ent_f(e, f.armorvalue) != 0.0 {
        bits |= SU_ARMOR;
    }
    bits |= SU_WEAPON;
    let wmodel = c.model_index(vm.ent_str(e, f.weaponmodel).unwrap_or(b""));
    let alpha = c.ext(e).alpha;
    if c.sv.protocol != PROTOCOL_NETQUAKE {
        if wmodel & 0xff00 != 0 {
            bits |= SU_WEAPON2;
        }
        for (o, b) in [
            (f.armorvalue, SU_ARMOR2),
            (f.currentammo, SU_AMMO2),
            (f.ammo_shells, SU_SHELLS2),
            (f.ammo_nails, SU_NAILS2),
            (f.ammo_rockets, SU_ROCKETS2),
            (f.ammo_cells, SU_CELLS2),
        ] {
            if fi(vm, o) & 0xff00 != 0 {
                bits |= b;
            }
        }
        if bits & SU_WEAPONFRAME != 0 && fi(vm, f.weaponframe) & 0xff00 != 0 {
            bits |= SU_WEAPONFRAME2;
        }
        if alpha != ENTALPHA_DEFAULT {
            bits |= SU_WEAPONALPHA;
        }
        if bits >= 65536 {
            bits |= SU_EXTEND1;
        }
        if bits >= 16777216 {
            bits |= SU_EXTEND2;
        }
    }
    msg.byte(SVC_CLIENTDATA);
    msg.short(bits as i32);
    if bits & SU_EXTEND1 != 0 {
        msg.byte((bits >> 16) as i32);
    }
    if bits & SU_EXTEND2 != 0 {
        msg.byte((bits >> 24) as i32);
    }
    if bits & SU_VIEWHEIGHT != 0 {
        msg.char(vm.ent_v(e, f.view_ofs)[2] as i32);
    }
    if bits & SU_IDEALPITCH != 0 {
        msg.char(fi(vm, f.idealpitch));
    }
    for i in 0..3 {
        if bits & (SU_PUNCH1 << i) != 0 {
            msg.char(punch[i] as i32);
        }
        if bits & (SU_VELOCITY1 << i) != 0 {
            msg.char((vel[i] / 16.0) as i32);
        }
    }
    msg.long(items);
    if bits & SU_WEAPONFRAME != 0 {
        msg.byte(fi(vm, f.weaponframe));
    }
    if bits & SU_ARMOR != 0 {
        msg.byte(fi(vm, f.armorvalue));
    }
    msg.byte(wmodel);
    msg.short(fi(vm, f.health));
    for o in [f.currentammo, f.ammo_shells, f.ammo_nails, f.ammo_rockets, f.ammo_cells] {
        msg.byte(fi(vm, o));
    }
    let weapon = fi(vm, f.weapon);
    if c.standard_quake {
        msg.byte(weapon);
    } else if let Some(i) = (0..32).find(|i| weapon & (1 << i) != 0) {
        msg.byte(i);
    }
    if bits & SU_WEAPON2 != 0 {
        msg.byte(wmodel >> 8);
    }
    for (o, b) in [
        (f.armorvalue, SU_ARMOR2),
        (f.currentammo, SU_AMMO2),
        (f.ammo_shells, SU_SHELLS2),
        (f.ammo_nails, SU_NAILS2),
        (f.ammo_rockets, SU_ROCKETS2),
        (f.ammo_cells, SU_CELLS2),
    ] {
        if bits & b != 0 {
            msg.byte(fi(vm, o) >> 8);
        }
    }
    if bits & SU_WEAPONFRAME2 != 0 {
        msg.byte(fi(vm, f.weaponframe) >> 8);
    }
    if bits & SU_WEAPONALPHA != 0 {
        msg.byte(alpha as i32);
    }
    if weapon as u8 as i32 != weapon && msg.len() + 6 <= msg.maxsize {
        msg.byte(SVC_UPDATESTAT);
        msg.byte(STAT_ACTIVEWEAPON);
        msg.long(weapon);
    }
}

/// Entities `clent` may see: itself, then every entity with a model that
/// touches its fat PVS (the SV_WriteEntitiesToClient filter).
fn visible_entities(vm: &Vm, c: &mut Core, clent: u32, pvs: &mut Vec<u8>) -> Vec<u32> {
    let f = c.sv.fields.clone();
    let org = Vec3::from_array(vm.ent_v(clent, f.origin)) + Vec3::from_array(vm.ent_v(clent, f.view_ofs));
    c.sv.vis.fat_pvs(org, pvs);
    let protocol = c.sv.protocol;
    let mut list = vec![clent];
    for e in 1..vm.num_edicts() as u32 {
        if e == clent || vm.is_free(e) {
            continue;
        }
        let mi = vm.ent_f(e, f.modelindex) as i32;
        if mi == 0 || vm.ent_str(e, f.model).unwrap_or(b"").is_empty() {
            continue;
        }
        if protocol == PROTOCOL_NETQUAKE && mi & 0xff00 != 0 {
            continue;
        }
        let x = c.ext(e);
        if x.leafs.len() < qworld::world::MAX_ENT_LEAFS && !leafs_in_pvs(&x.leafs, pvs) {
            continue;
        }
        list.push(e);
    }
    list
}

/// Baseline as the client holds it (an entity without svc_spawnbaseline
/// has the default state, scale 16).
fn baseline_state(b: &crate::server::Baseline) -> EntState {
    EntState {
        origin: b.origin,
        angles: b.angles,
        modelindex: b.modelindex as u32,
        frame: b.frame as u32,
        skin: b.skin as u32,
        colormap: b.colormap as u8,
        effects: b.effects as u32,
        alpha: b.alpha,
        scale: if b.scale == 0 { ENTSCALE_DEFAULT } else { b.scale },
        ..EntState::default()
    }
}

/// SVFTE_BuildSnapshotForClient + SV_BuildEntityState: the visible
/// entities' states, sorted by number.
fn snapshot(vm: &Vm, c: &mut Core, clent: u32, pvs: &mut Vec<u8>) -> Vec<(u32, EntState)> {
    let f = c.sv.fields.clone();
    let mask = c.sv.effects_mask;
    let mut list = visible_entities(vm, c, clent, pvs);
    list.sort_unstable();
    let effect = |o: Option<usize>, e: u32| {
        o.map_or(0, |o| vm.ent_f(e, o) as i32).clamp(0, qnet::delta::MAX_PARTICLETYPES as i32 - 1) as u16
    };
    let mut out = Vec::with_capacity(list.len());
    for e in list {
        let effects = vm.ent_f(e, f.effects) as i32 & mask;
        let x = c.ext(e);
        if let Some(o) = f.alpha {
            x.alpha = entalpha_encode(vm.ent_f(e, o));
        }
        let alpha = x.alpha;
        if e != clent && alpha == ENTALPHA_ZERO && effects == 0 {
            continue;
        }
        out.push((
            e,
            EntState {
                origin: vm.ent_v(e, f.origin),
                angles: vm.ent_v(e, f.angles),
                modelindex: vm.ent_f(e, f.modelindex) as u32,
                frame: vm.ent_f(e, f.frame) as u32,
                skin: vm.ent_f(e, f.skin) as u32,
                colormap: vm.ent_f(e, f.colormap) as u8,
                effects: effects as u32,
                eflags: if vm.ent_f(e, f.movetype) as i32 == MOVETYPE_STEP { qnet::delta::EFLAGS_STEP } else { 0 },
                alpha,
                scale: f.scale.map_or(ENTSCALE_DEFAULT, |o| entscale_encode(vm.ent_f(e, o))),
                traileffect: effect(f.traileffectnum, e),
                emiteffect: effect(f.emiteffectnum, e),
                ..EntState::default()
            },
        ));
    }
    out
}

/// One frame of replacement deltas for client `i` (SV_SendClientDatagram
/// with PEXT2_REPLACEMENTDELTAS): client data, then svcfte_updateentities
/// (more datagrams when the snapshot doesn't fit), then the server datagram.
/// The local client is lossless, so its frames are acked as written.
fn send_deltas(vm: &mut Vm, c: &mut Core, w: &mut World, i: usize, pvs: &mut Vec<u8>) {
    let Some(mut d) = c.clients[i].delta.take() else { return };
    let e = i as u32 + 1;
    let max = c.clients[i].max_datagram;
    let mut msg = SizeBuf::new(max);
    write_clientdata(vm, c, w, e, &mut msg);
    let snap = snapshot(vm, c, e, pvs);
    d.calc(snap, vm.num_edicts());
    let (time, pf, local) = (vm.time as f32, c.sv.protocolflags, c.clients[i].local);
    loop {
        let ext = &c.sv.ext;
        let base = |n: u32| ext.get(n as usize).map_or_else(EntState::default, |x| baseline_state(&x.baseline));
        let mut q = qnet::SizeBuf::new(max);
        d.write(&mut q, max.saturating_sub(msg.len()).max(64), time, pf, &base);
        msg.write(&q.data);
        if local {
            d.ack(d.next_sequence.wrapping_sub(1));
        }
        if !d.more() {
            break;
        }
        push_datagram(c, i, std::mem::replace(&mut msg, SizeBuf::new(max)).data);
    }
    if msg.len() + c.sv.datagram.len() < msg.maxsize {
        msg.write(&c.sv.datagram.data);
    }
    push_datagram(c, i, msg.data);
    c.clients[i].delta = Some(d);
}

fn push_datagram(c: &mut Core, i: usize, data: Vec<u8>) {
    let q = &mut c.clients[i].datagrams;
    if q.len() >= MAX_QUEUED_DATAGRAMS {
        q.pop_front();
    }
    q.push_back(data);
}

/// SV_WriteEntitiesToClient.
fn write_entities(vm: &mut Vm, c: &mut Core, clent: u32, msg: &mut SizeBuf, pvs: &mut Vec<u8>) {
    let f = c.sv.fields.clone();
    let protocol = c.sv.protocol;
    let pf = c.sv.protocolflags;
    let mask = c.sv.effects_mask;
    let list = visible_entities(vm, c, clent, pvs);
    for e in list {
        if msg.len() + 40 > msg.maxsize {
            break;
        }
        let o = vm.ent_v(e, f.origin);
        let a = vm.ent_v(e, f.angles);
        let mi = vm.ent_f(e, f.modelindex) as i32;
        let frame = vm.ent_f(e, f.frame) as i32;
        let colormap = vm.ent_f(e, f.colormap) as i32;
        let skin = vm.ent_f(e, f.skin) as i32;
        let effects = vm.ent_f(e, f.effects) as i32;
        let movetype = vm.ent_f(e, f.movetype) as i32;
        let nextthink = vm.ent_f(e, f.nextthink);
        let alpha_f = f.alpha.map(|ofs| vm.ent_f(e, ofs));
        let scale = f.scale.map_or(ENTSCALE_DEFAULT, |ofs| entscale_encode(vm.ent_f(e, ofs)));
        let time = vm.time;
        let x = c.ext(e);
        let b = x.baseline;
        let mut bits = 0u32;
        for (i, (x, y)) in o.iter().zip(b.origin).enumerate() {
            let miss = x - y;
            if !(-0.1..=0.1).contains(&miss) {
                bits |= U_ORIGIN1 << i;
            }
        }
        if a[0] != b.angles[0] {
            bits |= U_ANGLE1;
        }
        if a[1] != b.angles[1] {
            bits |= U_ANGLE2;
        }
        if a[2] != b.angles[2] {
            bits |= U_ANGLE3;
        }
        if movetype == MOVETYPE_STEP {
            bits |= U_STEP;
        }
        if b.colormap != colormap {
            bits |= U_COLORMAP;
        }
        if b.skin != skin {
            bits |= U_SKIN;
        }
        if b.frame != frame {
            bits |= U_FRAME;
        }
        if (b.effects ^ effects) & mask != 0 {
            bits |= U_EFFECTS;
        }
        if b.modelindex != mi {
            bits |= U_MODEL;
        }
        if let Some(af) = alpha_f {
            x.alpha = entalpha_encode(af);
        }
        if x.alpha == ENTALPHA_ZERO && effects & mask == 0 {
            continue;
        }
        if protocol != PROTOCOL_NETQUAKE {
            if b.alpha != x.alpha {
                bits |= U_ALPHA;
            }
            if b.scale != scale {
                bits |= U_SCALE;
            }
            if bits & U_FRAME != 0 && frame & 0xff00 != 0 {
                bits |= U_FRAME2;
            }
            if bits & U_MODEL != 0 && mi & 0xff00 != 0 {
                bits |= U_MODEL2;
            }
            if x.sendinterval {
                bits |= U_LERPFINISH;
            }
            if bits >= 65536 {
                bits |= U_EXTEND1;
            }
            if bits >= 16777216 {
                bits |= U_EXTEND2;
            }
        }
        if e >= 256 {
            bits |= U_LONGENTITY;
        }
        if bits >= 256 {
            bits |= U_MOREBITS;
        }
        msg.byte(((bits | U_SIGNAL) & 0xff) as i32);
        if bits & U_MOREBITS != 0 {
            msg.byte(((bits >> 8) & 0xff) as i32);
        }
        if bits & U_EXTEND1 != 0 {
            msg.byte(((bits >> 16) & 0xff) as i32);
        }
        if bits & U_EXTEND2 != 0 {
            msg.byte((bits >> 24) as i32);
        }
        if bits & U_LONGENTITY != 0 {
            msg.short(e as i32);
        } else {
            msg.byte(e as i32);
        }
        if bits & U_MODEL != 0 {
            msg.byte(mi);
        }
        if bits & U_FRAME != 0 {
            msg.byte(frame);
        }
        if bits & U_COLORMAP != 0 {
            msg.byte(colormap);
        }
        if bits & U_SKIN != 0 {
            msg.byte(skin);
        }
        if bits & U_EFFECTS != 0 {
            msg.byte(effects & mask);
        }
        for i in 0..3 {
            if bits & (U_ORIGIN1 << i) != 0 {
                msg.coord(o[i], pf);
            }
            let ab = [U_ANGLE1, U_ANGLE2, U_ANGLE3][i];
            if bits & ab != 0 {
                msg.angle(a[i], pf);
            }
        }
        if bits & U_ALPHA != 0 {
            msg.byte(x.alpha as i32);
        }
        if bits & U_SCALE != 0 {
            msg.byte(scale as i32);
        }
        if bits & U_FRAME2 != 0 {
            msg.byte(frame >> 8);
        }
        if bits & U_MODEL2 != 0 {
            msg.byte(mi >> 8);
        }
        if bits & U_LERPFINISH != 0 {
            msg.byte(q_rint(((nextthink as f64 - time) * 255.0) as f32) as u8 as i32);
        }
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
