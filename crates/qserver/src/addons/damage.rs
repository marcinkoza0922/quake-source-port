//! Damage through the progs' own functions, so kills, pain and gibs follow
//! the mod's rules.

use qvm::{Vm, ofs_parm};
use qworld::World;

use crate::server::{Core, exec};

/// `name`'s arguments after the fourth, which some mods added, are what the
/// mod's own calls usually pass.
fn call(vm: &mut Vm, c: &mut Core, w: &mut World, name: &str, set: impl FnOnce(&mut Vm)) {
    let Some(f) = vm.progs.find_function(name) else { return };
    set(vm);
    for (k, v) in vm.usual_late_args(f).into_iter().enumerate() {
        vm.set_g_i(ofs_parm(4 + k), v as i32);
    }
    exec(vm, c, w, f as i32);
}

/// `T_Damage(targ, inflictor, attacker, damage)`
pub(crate) fn damage(vm: &mut Vm, c: &mut Core, w: &mut World, targ: u32, inflictor: u32, attacker: u32, damage: f32) {
    call(vm, c, w, "T_Damage", |vm| {
        vm.set_g_i(ofs_parm(0), targ as i32);
        vm.set_g_i(ofs_parm(1), inflictor as i32);
        vm.set_g_i(ofs_parm(2), attacker as i32);
        vm.set_g_f(ofs_parm(3), damage);
    });
}

/// `T_RadiusDamage(inflictor, attacker, damage, ignore)`
pub(crate) fn radius(vm: &mut Vm, c: &mut Core, w: &mut World, inflictor: u32, attacker: u32, damage: f32, ignore: u32) {
    call(vm, c, w, "T_RadiusDamage", |vm| {
        vm.set_g_i(ofs_parm(0), inflictor as i32);
        vm.set_g_i(ofs_parm(1), attacker as i32);
        vm.set_g_f(ofs_parm(2), damage);
        vm.set_g_i(ofs_parm(3), ignore as i32);
    });
}
