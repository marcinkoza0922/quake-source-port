//! Convert standard QC FireBullets traces into swept nail-sized missiles.
//! QC retains spread, shot count, ammunition and attack timing. Ordinary
//! edicts and QC touch/think callbacks also make pellets save naturally.
use qcore::math::Vec3;
use qvm::{Vm, Result, ofs_parm};
use qworld::World;
use crate::{ents::Ents, server::Core};

const CLASS: &[u8] = b"qsp_shotgun_pellet";
/// Matches `qclient::EF_QSP_PELLET`; not in the non-QEX effects mask.
const EF_QSP_PELLET: f32 = 128.0;

pub(crate) fn launch(vm: &mut Vm, c: &mut Core, w: &mut World,
    start: Vec3, end: Vec3, nomonsters: i32, owner: u32) -> Result<bool> {
    if !super::active(c) || c.cvars.value_of("addon_projectile_shotguns") == 0.0
        || nomonsters != 0 || vm.current_function_name() != "FireBullets" {
        return Ok(false);
    }
    // The standard helper is shared by both player shotguns and grunts.
    // Unknown mod implementations retain their own behavior.
    let Some(touch) = vm.progs.find_function("spike_touch") else { return Ok(false) };
    let Some(think) = vm.progs.find_function("SUB_Remove") else { return Ok(false) };
    let Some(model) = c.sv.model_precache.iter().position(|s| s == "progs/spike.mdl") else { return Ok(false) };
    let dir = end - start;
    if !dir.length().is_finite() || dir.length() < 1.0 { return Ok(false); }
    let e = vm.ed_alloc()?;
    *c.ext(e) = Default::default();
    let class = vm.strings.engine(CLASS);
    let f = &c.sv.fields;
    vm.set_ent_i(e, f.classname, class);
    // The nail model carries the entity to clients; ours hide it for a tracer
    // (EF_QSP_PELLET), other engines show a nail.
    let model_name = vm.strings.engine(b"progs/spike.mdl");
    vm.set_ent_i(e, f.model, model_name);
    vm.set_ent_f(e, f.modelindex, model as f32);
    vm.set_ent_f(e, f.effects, EF_QSP_PELLET);
    vm.set_ent_i(e, f.owner, owner as i32);
    vm.set_ent_f(e, f.movetype, 5.0); // MOVETYPE_FLY: swept point collision, no gravity
    vm.set_ent_f(e, f.solid, 2.0);
    vm.set_ent_v(e, f.origin, start.to_array());
    let velocity = dir / dir.length() * c.cvars.value_of("addon_shotgun_speed").clamp(100.0, 30000.0);
    vm.set_ent_v(e, f.velocity, velocity.to_array());
    let yaw = velocity.y.atan2(velocity.x).to_degrees();
    let pitch = velocity.z.atan2((velocity.x * velocity.x + velocity.y * velocity.y).sqrt()).to_degrees();
    vm.set_ent_v(e, f.angles, [pitch, yaw, 0.0]);
    vm.set_ent_i(e, f.touch, touch as i32);
    vm.set_ent_i(e, f.think, think as i32);
    vm.set_ent_f(e, f.nextthink, vm.time as f32 + 2048.0 / velocity.length());
    w.link_edict(&mut Ents { vm, c }, e, false);
    Ok(true)
}

/// spike_touch announces a wall hit as TE_SPIKE (nail tink, nail-sized
/// hit effects); a pellet reports it as TE_GUNSHOT, like hitscan bullets.
/// spike_touch only writes svc_temp_entity (23) and TE_SPIKE (0) as bytes.
pub(crate) fn temp_entity(vm: &Vm, c: &Core, v: i32) -> i32 {
    const TE_SPIKE: i32 = 0;
    const TE_GUNSHOT: i32 = 2;
    let me = vm.self_ent();
    if v == TE_SPIKE && me > 0 && (me as usize) < vm.num_edicts()
        && vm.ent_str(me, c.sv.fields.classname).ok() == Some(CLASS) {
        TE_GUNSHOT
    } else {
        v
    }
}

/// spike_touch's base 9 damage becomes the shotgun's 4 before QC applies
/// quad, armor, pain, kills, knockback and attacker attribution.
pub(crate) fn damage(vm: &mut Vm, c: &Core) {
    let e = vm.g_i(ofs_parm(1));
    if e > 0 && (e as usize) < vm.num_edicts()
        && vm.ent_str(e as u32, c.sv.fields.classname).ok() == Some(CLASS) {
        vm.set_g_f(ofs_parm(3), vm.g_f(ofs_parm(3)) * (4.0 / 9.0));
    }
}

#[cfg(test)]
#[path = "shotgun_tests.rs"]
mod tests;
