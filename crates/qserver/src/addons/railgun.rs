//! Railgun toy: a Quake II/III style hitscan slug. Holding fire shoots
//! every [`REFIRE`] seconds while the player has [`AMMO`] cells; the slug
//! pierces every damageable entity on the line (each hit once, through the
//! progs' own `T_Damage`, so kills, pain and gibs follow the mod's rules)
//! and stops at the world or a solid brush entity.
//!
//! Clients see shots as `//qsp_rail owner kind sx sy sz ex ey ez nx ny nz
//! [hx hy hz]...` comment stufftext (kind 1 fired, 0 dry click; `n` is the
//! wall normal, zero when nothing was hit, then the points where entities
//! were pierced). Other engines skip `//` lines, so demos still play.

use qcore::math::{Vec3, angle_vectors};
use qvm::OFS_PARM0;
use qworld::{Edicts, Ent, MoveType, V};

use crate::client::Client;
use crate::ents::Ents;
use crate::server::{Core, Level, exec};

/// Seconds between shots (Quake III: 1.5).
pub const REFIRE: f64 = 1.5;
/// Cells per shot.
pub const AMMO: f32 = 2.0;
/// Seconds between dry clicks with no ammo.
const DRY: f64 = 0.5;
const RANGE: f32 = 8192.0;
/// Most entities one slug passes through.
const MAX_HITS: usize = 16;

/// Per-client refire timers; lives on `Level` (not saved: transient).
#[derive(Clone, Debug, Default)]
pub struct Rails {
    next: Vec<f64>,
}

/// The line a slug follows: the hit points of pierced entities (in order)
/// and the end point with the wall normal (zero if it hit nothing).
#[derive(Clone, Debug, PartialEq)]
pub struct Slug {
    pub hits: Vec<(Ent, Vec3)>,
    pub end: Vec3,
    pub normal: Vec3,
}

/// Trace a slug from `start` along `dir`, piercing damageable entities.
pub fn trace<D: Edicts + ?Sized>(w: &qworld::World, ents: &D, shooter: Ent, start: Vec3, dir: Vec3, damageable: impl Fn(Ent) -> bool) -> Slug {
    let end = start + dir * RANGE;
    let (mut from, mut pass) = (start, shooter);
    let mut hits = Vec::new();
    loop {
        let tr = w.trace(ents, from, Vec3::ZERO, Vec3::ZERO, end, MoveType::Normal, Some(pass));
        match tr.ent {
            Some(e) if e != 0 && damageable(e) && hits.len() < MAX_HITS && !hits.iter().any(|&(h, _)| h == e) => {
                hits.push((e, tr.endpos));
                // continue just past the hit point, ignoring this entity
                from = tr.endpos;
                pass = e;
            }
            Some(_) => return Slug { hits, end: tr.endpos, normal: tr.plane_normal },
            None => return Slug { hits, end: tr.endpos, normal: Vec3::ZERO },
        }
    }
}

/// The stufftext line clients parse (`qclient::RailEffect`).
pub fn line(owner: usize, kind: u8, start: Vec3, slug: &Slug) -> String {
    let v = |v: Vec3| format!(" {} {} {}", v.x, v.y, v.z);
    let mut s = format!("//qsp_rail {owner} {kind}{}{}{}", v(start), v(slug.end), v(slug.normal));
    for &(_, p) in &slug.hits {
        s += &v(p);
    }
    s.push('\n');
    s
}

fn send_all(clients: &mut [Client], s: &str) {
    for cl in clients.iter_mut().filter(|c| c.spawned) {
        cl.message.byte(crate::msg::SVC_STUFFTEXT);
        cl.message.string(s.as_bytes());
    }
}

/// After physics: fire for every client holding the railgun's trigger.
pub(crate) fn frame(st: &mut Rails, l: &mut Level, c: &mut Core) {
    let n = c.clients.len();
    st.next.resize(n, 0.0);
    let now = l.vm.time;
    let damage = c.cvars.value_of("addon_rail_damage").clamp(1.0, 1000.0);
    let f = c.sv.fields.clone();
    for i in 0..n {
        let cl = &c.clients[i];
        if !cl.spawned || super::Toy::from_buttons(cl.cmd.buttons) != Some(super::Toy::Railgun) || cl.cmd.buttons & super::BUTTON_TOY_FIRE == 0 {
            continue;
        }
        if now < st.next[i] {
            continue;
        }
        let e = i as Ent + 1;
        if l.vm.ent_f(e, f.health) <= 0.0 {
            continue;
        }
        let cells = l.vm.ent_f(e, f.ammo_cells);
        let ents = Ents { vm: &mut l.vm, c: &mut *c };
        let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
        let (fwd, right, up) = angle_vectors(ents.v(e, V::VAngle));
        let muzzle = eye + fwd * 16.0 + right * 1.0 - up * 7.0;
        if cells < AMMO {
            st.next[i] = now + DRY;
            let empty = Slug { hits: Vec::new(), end: muzzle, normal: Vec3::ZERO };
            send_all(&mut c.clients, &line(i, 0, muzzle, &empty));
            continue;
        }
        st.next[i] = now + REFIRE;
        let takedamage = f.takedamage;
        let slug = {
            let vm: &qvm::Vm = ents.vm;
            trace(&l.world, &ents, e, eye, fwd, |t| vm.ent_f(t, takedamage) != 0.0)
        };
        l.vm.set_ent_f(e, f.ammo_cells, cells - AMMO);
        // the HUD shows `currentammo` for the held QC weapon (cells for the
        // lightning gun): keep it in step
        if l.vm.ent_f(e, f.currentammo) == cells {
            l.vm.set_ent_f(e, f.currentammo, cells - AMMO);
        }
        send_all(&mut c.clients, &line(i, 1, muzzle, &slug));
        let td = l.vm.progs.find_function("T_Damage").unwrap_or(0) as i32;
        if td <= 0 {
            continue;
        }
        for &(t, _) in &slug.hits {
            if qworld::Edicts::is_free(&Ents { vm: &mut l.vm, c: &mut *c }, t) || l.vm.ent_f(t, f.takedamage) == 0.0 {
                continue;
            }
            // T_Damage(targ, inflictor, attacker, damage)
            l.vm.set_g_i(OFS_PARM0, t as i32);
            l.vm.set_g_i(OFS_PARM0 + 3, e as i32);
            l.vm.set_g_i(OFS_PARM0 + 6, e as i32);
            l.vm.set_g_f(OFS_PARM0 + 9, damage);
            exec(&mut l.vm, c, &mut l.world, td);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_lists_end_normal_and_hits() {
        let slug = Slug { hits: vec![(5, Vec3::new(1.0, 2.0, 3.0))], end: Vec3::new(4.0, 5.0, 6.0), normal: Vec3::new(0.0, 0.0, 1.0) };
        assert_eq!(line(0, 1, Vec3::ZERO, &slug), "//qsp_rail 0 1 0 0 0 4 5 6 0 0 1 1 2 3\n");
    }
}
