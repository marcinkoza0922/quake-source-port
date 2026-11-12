use super::*;

/// Entities `clent` may see: itself, then every entity with a model that
/// touches its fat PVS (the SV_WriteEntitiesToClient filter).
pub(super) fn visible_entities(vm: &Vm, c: &mut Core, clent: u32, pvs: &mut Vec<u8>) -> Vec<u32> {
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
pub(super) fn baseline_state(b: &crate::server::Baseline) -> EntState {
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
pub(super) fn snapshot(vm: &Vm, c: &mut Core, clent: u32, pvs: &mut Vec<u8>) -> Vec<(u32, EntState)> {
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
pub(super) fn send_deltas(vm: &mut Vm, c: &mut Core, w: &mut World, i: usize, pvs: &mut Vec<u8>) {
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

pub(super) fn push_datagram(c: &mut Core, i: usize, data: Vec<u8>) {
    let q = &mut c.clients[i].datagrams;
    if q.len() >= MAX_QUEUED_DATAGRAMS {
        q.pop_front();
    }
    q.push_back(data);
}

/// SV_WriteEntitiesToClient.
pub(super) fn write_entities(vm: &mut Vm, c: &mut Core, clent: u32, msg: &mut SizeBuf, pvs: &mut Vec<u8>) {
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
