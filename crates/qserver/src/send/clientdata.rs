use super::*;

/// Damage, ideal pitch and fixangle messages that precede both `svc_clientdata`
/// and `PEXT2_PREDINFO` stat updates.
pub(crate) fn write_damage_and_angle(vm: &mut Vm, c: &mut Core, w: &mut World, e: u32, msg: &mut SizeBuf) {
    let f = c.sv.fields.clone();
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
}

/// SV_CalcStats (vkQuake sv_main.c) for `PEXT2_PREDINFO` and custom QC stats.
pub(crate) fn calc_stats(
    vm: &Vm,
    c: &Core,
    e: u32,
    predinfo: bool,
) -> ([i32; qnet::protocol::MAX_CL_STATS], [f32; qnet::protocol::MAX_CL_STATS], Vec<Option<Vec<u8>>>) {
    use qnet::protocol::*;
    let f = &c.sv.fields;
    let g = &c.sv.globals;
    let mut si = [0i32; MAX_CL_STATS];
    let mut sf = [0.0f32; MAX_CL_STATS];
    let mut ss: Vec<Option<Vec<u8>>> = vec![None; MAX_CL_STATS];
    let items = match f.items2 {
        Some(o) => (vm.ent_f(e, f.items) as i32) | ((vm.ent_f(e, o) as i32) << 23),
        None => (vm.ent_f(e, f.items) as i32) | ((vm.g_f(g.serverflags) as i32) << 28),
    };
    sf[STAT_HEALTH] = vm.ent_f(e, f.health);
    si[STAT_WEAPON] = c.model_index(vm.ent_str(e, f.weaponmodel).unwrap_or(b""));
    sf[STAT_AMMO] = vm.ent_f(e, f.currentammo);
    sf[STAT_ARMOR] = vm.ent_f(e, f.armorvalue);
    sf[STAT_WEAPONFRAME] = vm.ent_f(e, f.weaponframe);
    sf[STAT_SHELLS] = vm.ent_f(e, f.ammo_shells);
    sf[STAT_NAILS] = vm.ent_f(e, f.ammo_nails);
    sf[STAT_ROCKETS] = vm.ent_f(e, f.ammo_rockets);
    sf[STAT_CELLS] = vm.ent_f(e, f.ammo_cells);
    sf[STAT_ACTIVEWEAPON] = vm.ent_f(e, f.weapon);
    if let Some(vz) = f.viewzoom {
        let v = vm.ent_f(e, vz);
        if v != 0.0 {
            sf[STAT_VIEWZOOM] = (v * 255.0).max(1.0);
        }
    }
    if predinfo {
        si[STAT_ITEMS] = items;
        sf[STAT_VIEWHEIGHT] = vm.ent_v(e, f.view_ofs)[2];
        sf[STAT_IDEALPITCH] = vm.ent_f(e, f.idealpitch);
        let punch = vm.ent_v(e, f.punchangle);
        sf[STAT_PUNCHANGLE_X] = punch[0];
        sf[STAT_PUNCHANGLE_Y] = punch[1];
        sf[STAT_PUNCHANGLE_Z] = punch[2];
    }
    // Custom stats from clientstat (#232) / globalstat (#233).
    for cs in &c.sv.custom_stats {
        if cs.idx >= MAX_CL_STATS {
            continue;
        }
        match (cs.field, cs.global) {
            (Some(ofs), _) => match cs.ty {
                1 => ss[cs.idx] = vm.ent_str(e, ofs).ok().map(|b| b.to_vec()),
                2 => sf[cs.idx] = vm.ent_f(e, ofs),
                3 if cs.idx + 2 < MAX_CL_STATS => {
                    let v = vm.ent_v(e, ofs);
                    sf[cs.idx] = v[0];
                    sf[cs.idx + 1] = v[1];
                    sf[cs.idx + 2] = v[2];
                }
                4 | 10 => si[cs.idx] = vm.ent_i(e, ofs),
                _ => {}
            },
            (None, Some(ofs)) => match cs.ty {
                1 => ss[cs.idx] = vm.g_str(ofs).ok().map(|b| b.to_vec()),
                2 => sf[cs.idx] = vm.g_f(ofs),
                3 if cs.idx + 2 < MAX_CL_STATS => {
                    let v = vm.g_v(ofs);
                    sf[cs.idx] = v[0];
                    sf[cs.idx + 1] = v[1];
                    sf[cs.idx + 2] = v[2];
                }
                4 | 10 => si[cs.idx] = vm.g_i(ofs),
                _ => {}
            },
            _ => {}
        }
    }
    (si, sf, ss)
}

/// SV_WriteClientdataToMessage.
pub(crate) fn write_clientdata(vm: &mut Vm, c: &mut Core, w: &mut World, e: u32, msg: &mut SizeBuf) {
    let f = c.sv.fields.clone();
    let g = c.sv.globals.clone();
    let fi = |vm: &Vm, o: usize| vm.ent_f(e, o) as i32;
    write_damage_and_angle(vm, c, w, e, msg);
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
