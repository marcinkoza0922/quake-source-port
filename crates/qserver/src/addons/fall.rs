//! Fall damage for monsters while the portal gun is on: portals make it
//! easy to drop or fling a monster far, and vanilla QC only hurts players.
//! Damage goes through the progs' own `T_Damage`, so kills count, pain
//! and death animations play and mods keep their rules.

use qvm::{OFS_PARM0, Vm};
use qworld::{Ent, V, World};

use crate::ents::Ents;
use crate::server::{Core, exec};

const FL_ONGROUND: i32 = 512;
const FL_MONSTER: i32 = 32;
const FL_FLY: i32 = 1;
const FL_SWIM: i32 = 2;
/// Landing speed that starts to hurt (the player's QC threshold).
pub const HURT_SPEED: f32 = 650.0;

/// Damage for landing at `speed` (positive, downwards): 5 at the
/// threshold, +1 per 10 units/s beyond, so a long portal drop or fling
/// kills a grunt while a plain ledge drop barely scratches.
pub fn damage(speed: f32) -> f32 {
    if speed < HURT_SPEED { 0.0 } else { 5.0 + (speed - HURT_SPEED) / 10.0 }
}

/// Airborne monsters and their vertical speed at the end of last frame.
#[derive(Default)]
pub struct Falls {
    air: Vec<(Ent, f32)>,
}

impl Falls {
    pub fn clear(&mut self) {
        self.air.clear();
    }
}

/// After physics: hurt monsters that landed since last frame, then record
/// who is airborne now.
pub(crate) fn frame(st: &mut Falls, vm: &mut Vm, c: &mut Core, w: &mut World) {
    let landed: Vec<(Ent, f32)> = {
        let ents = Ents { vm: &mut *vm, c: &mut *c };
        st.air
            .iter()
            .filter(|&&(e, vz)| !qworld::Edicts::is_free(&ents, e) && qworld::Edicts::flags(&ents, e) & FL_ONGROUND != 0 && damage(-vz) > 0.0)
            .map(|&(e, vz)| (e, damage(-vz)))
            .collect()
    };
    let f = vm.progs.find_function("T_Damage").unwrap_or(0) as i32;
    for (e, dmg) in landed {
        if f <= 0 || vm.ent_f(e, c.sv.fields.health) <= 0.0 {
            continue;
        }
        // T_Damage(targ, inflictor, attacker, damage): the world did it
        vm.set_g_i(OFS_PARM0, e as i32);
        vm.set_g_i(OFS_PARM0 + 3, 0);
        vm.set_g_i(OFS_PARM0 + 6, 0);
        vm.set_g_f(OFS_PARM0 + 9, dmg);
        exec(vm, c, w, f);
    }
    st.air.clear();
    let ents = Ents { vm: &mut *vm, c: &mut *c };
    use qworld::Edicts;
    for e in 1..ents.num_edicts() {
        let fl = ents.flags(e);
        if !ents.is_free(e) && fl & FL_MONSTER != 0 && fl & (FL_ONGROUND | FL_FLY | FL_SWIM) == 0 {
            st.air.push((e, ents.v(e, V::Velocity).z));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn damage_starts_at_the_player_threshold() {
        assert_eq!(damage(600.0), 0.0);
        assert_eq!(damage(650.0), 5.0);
        assert_eq!(damage(1000.0), 40.0);
    }
}
