//! Confirm damage after QC, rather than guessing from firing or blood effects.
use qvm::{Vm, ofs_parm};

use crate::msg::SVC_STUFFTEXT;
use crate::server::Core;

pub(super) struct DamageCall {
    target: qecs::Entity,
    attacker: usize,
    health: f32,
}

pub(super) fn begin(vm: &Vm, c: &Core) -> Option<DamageCall> {
    let target = u32::try_from(vm.g_i(ofs_parm(0))).ok()?;
    let attacker = usize::try_from(vm.g_i(ofs_parm(2))).ok()?;
    if attacker == 0 || attacker > c.maxclients() || target == attacker as u32
        || target == 0 || target as usize >= vm.num_edicts()
        || !c.clients[attacker - 1].active || !c.clients[attacker - 1].spawned
    {
        return None;
    }
    let fields = &c.sv.fields;
    // Buttons, explosive crates and other damageable scenery are not enemies.
    if vm.ent_f(target, fields.flags) as i32 & (8 | 32) == 0 {
        return None;
    }
    let health = vm.ent_f(target, fields.health);
    if health <= 0.0 || !health.is_finite() {
        return None;
    }
    Some(DamageCall { target: vm.entity(target)?, attacker, health })
}

pub(super) fn finish(vm: &Vm, c: &mut Core, call: DamageCall) {
    let target = call.target.index();
    // Death code may remove a monster, but must never mistake a recycled
    // edict (e.g. a spawned gib) for the original victim.
    if vm.entity(target).is_some_and(|e| e != call.target) {
        return;
    }
    let health = vm.ent_f(target, c.sv.fields.health);
    if !health.is_finite() || health >= call.health {
        return;
    }
    let kill = health <= 0.0;
    let cl = &mut c.clients[call.attacker - 1];
    if !cl.active || !cl.spawned {
        return;
    }
    // Comment-shaped stufftext is harmless on classic clients. This client's
    // parser consumes only the two exact forms as events, never commands.
    cl.message.byte(SVC_STUFFTEXT);
    cl.message.string(if kill { b"//qhit 1\n" } else { b"//qhit 0\n" });
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcore::{cvar::Cvars, fs::Fs};
    use crate::Server;

    #[test]
    fn actual_qc_damage_reports_only_live_enemies_and_actual_health_loss() {
        let base = Fs::default_basedir();
        if !base.join("id1").is_dir() { return; }
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let mut sv = Server::new(fs, Cvars::new(), 1);
        sv.spawn_server("e1m1").unwrap();
        let client = sv.connect_local().unwrap();
        sv.core.clients[client].message.clear();
        let c = &mut sv.core;
        let l = sv.level.as_mut().unwrap();
        let target = l.vm.ed_alloc().unwrap();
        let f = c.sv.fields.clone();
        // A plain living damageable monster without pain/death callbacks is
        // enough to exercise real id1 T_Damage, including invulnerability.
        l.vm.set_ent_f(target, f.flags, 32.0);
        l.vm.set_ent_f(target, f.takedamage, 2.0);
        l.vm.set_ent_f(target, f.health, 100.0);
        let null = l.vm.progs.find_function("SUB_Null").unwrap() as i32;
        for name in ["th_pain", "th_die"] {
            let ofs = l.vm.field_ofs(name).unwrap();
            l.vm.set_ent_i(target, ofs, null);
        }
        fn damage(l: &mut crate::Level, c: &mut Core, client: usize, target: u32, attacker: u32, amount: f32) -> Vec<u8> {
            l.vm.set_g_i(ofs_parm(0), target as i32);
            l.vm.set_g_i(ofs_parm(1), attacker as i32);
            l.vm.set_g_i(ofs_parm(2), attacker as i32);
            l.vm.set_g_f(ofs_parm(3), amount);
            let mut host = super::super::Ctx::new(c, &mut l.world);
            l.vm.execute_by_name(&mut host, "T_Damage").unwrap();
            std::mem::take(&mut host.c.clients[client].message.data)
        }
        let hit = damage(l, c, client, target, 1, 10.0);
        assert_eq!(hit, b"\x09//qhit 0\n\0");
        // World-caused damage and self damage never confirm a player's shot.
        assert!(damage(l, c, client, target, 0, 1.0).is_empty());
        assert!(damage(l, c, client, 1, 1, 1.0).is_empty());
        // A barrel with health is not an enemy.
        l.vm.set_ent_f(target, f.flags, 0.0);
        assert!(damage(l, c, client, target, 1, 1.0).is_empty());
        l.vm.set_ent_f(target, f.flags, 32.0);
        l.vm.set_ent_f(target, f.takedamage, 0.0);
        assert!(damage(l, c, client, target, 1, 10.0).is_empty());
        l.vm.set_ent_f(target, f.takedamage, 2.0);
        assert_eq!(damage(l, c, client, target, 1, 1000.0), b"\x09//qhit 1\n\0");
        assert!(damage(l, c, client, target, 1, 10.0).is_empty());
        // Radius damage calls T_Damage from QC (rather than execute's engine
        // entry point), and must use the same paired hooks.
        l.vm.set_ent_f(target, f.health, 100.0);
        l.vm.set_ent_f(target, f.solid, 3.0);
        l.vm.set_ent_v(target, f.origin, [8000.0, 8000.0, 0.0]);
        l.vm.set_ent_v(1, f.origin, [8000.0, 8000.0, 0.0]);
        l.vm.set_g_i(ofs_parm(0), 1);
        l.vm.set_g_i(ofs_parm(1), 1);
        l.vm.set_g_f(ofs_parm(2), 10.0);
        l.vm.set_g_i(ofs_parm(3), 0);
        let mut host = super::super::Ctx::new(c, &mut l.world);
        l.vm.execute_by_name(&mut host, "T_RadiusDamage").unwrap();
        assert_eq!(host.c.clients[client].message.data, b"\x09//qhit 0\n\0");
        assert!(host.damage_calls.is_empty());
    }
}
