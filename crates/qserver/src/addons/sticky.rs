//! Stickybomb launcher (TF2 homage): fire lobs a bomb on a grenade arc
//! (rockets ammo, like the grenade launcher), it sticks to whatever it hits
//! — world, brush entities (riding doors and plats) and monsters (riding
//! them; a dead one drops it to the floor) — and alt-fire detonates every
//! bomb the player has out with a rocket explosion through QC's own
//! `T_RadiusDamage`. Bombs are engine-side, not edicts, so they can't
//! exhaust the edict table; [`MAX_BOMBS`] caps them per level and the
//! oldest of the shooter's bombs goes off when the cap is hit.
//!
//! Clients learn about bombs from `//qsp_sticky id [x y z]` stufftext
//! (no position = gone); saves keep them as `sticky ...` header lines.

use qcore::math::{Vec3, angle_vectors};
use qvm::{OFS_PARM0, Vm};
use qworld::{CONTENTS_SKY, Edicts, Ent, F, MoveType, V, World};

use crate::client::Client;
use crate::ents::Ents;
use crate::server::{Core, exec};

/// Bombs out at once, all players together.
pub const MAX_BOMBS: usize = 64;
/// Seconds between shots while fire is held (the grenade launcher's rate).
const REFIRE: f64 = 0.6;
/// Explosion damage (a rocket's 120).
const DAMAGE: f32 = 120.0;
const SOLID_BSP: f32 = 4.0;
const SOLID_NOT: f32 = 0.0;
const IT_GRENADE_LAUNCHER: f32 = 32.0;
const IT_ROCKET_LAUNCHER: f32 = 64.0;
const TE_EXPLOSION: i32 = 3;
const SVC_TEMP_ENTITY: i32 = 23;

/// A bomb: flying (`stuck` None) or riding entity `stuck.0` at offset `.1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bomb {
    pub id: u32,
    pub owner: usize,
    pub origin: Vec3,
    pub vel: Vec3,
    pub stuck: Option<(Ent, Vec3)>,
    /// Last origin sent to clients.
    sent: Option<Vec3>,
}

#[derive(Clone, Debug, Default)]
pub struct Stickies {
    pub bombs: Vec<Bomb>,
    next_id: u32,
    prev_buttons: Vec<u8>,
    next_fire: Vec<f64>,
    synced: Vec<bool>,
    outbox: Vec<String>,
}

impl Stickies {
    fn ensure(&mut self, n: usize) {
        if self.prev_buttons.len() < n {
            self.prev_buttons.resize(n, 0);
            self.next_fire.resize(n, 0.0);
            self.synced.resize(n, false);
        }
    }

    pub fn add(&mut self, owner: usize, origin: Vec3, vel: Vec3, stuck: Option<(Ent, Vec3)>) {
        self.next_id = self.next_id.wrapping_add(1);
        self.bombs.push(Bomb { id: self.next_id, owner, origin, vel, stuck, sent: None });
    }

    fn remove(&mut self, k: usize) -> Bomb {
        let b = self.bombs.remove(k);
        if b.sent.is_some() {
            self.outbox.push(format!("//qsp_sticky {}\n", b.id));
        }
        b
    }

    /// Header lines: `sticky owner x y z vx vy vz ent ox oy oz` (ent -1:
    /// flying).
    pub fn save_text(&self) -> String {
        let mut s = String::new();
        for b in &self.bombs {
            let (e, o) = b.stuck.map_or((-1, Vec3::ZERO), |(e, o)| (e as i64, o));
            s += &format!(
                "sticky {} {} {} {} {} {} {} {} {} {} {}\n",
                b.owner, b.origin.x, b.origin.y, b.origin.z, b.vel.x, b.vel.y, b.vel.z, e, o.x, o.y, o.z
            );
        }
        s
    }

    /// Reads the `sticky` lines of a save header (other lines are skipped).
    pub fn load_text(&mut self, text: &str) {
        for line in text.lines() {
            let Some(rest) = line.strip_prefix("sticky ") else { continue };
            let n: Vec<f32> = rest.split_whitespace().filter_map(|w| w.parse().ok()).collect();
            if let [o, x, y, z, vx, vy, vz, e, ox, oy, oz] = n[..]
                && self.bombs.len() < MAX_BOMBS
            {
                let stuck = (e >= 0.0).then(|| (e as Ent, Vec3::new(ox, oy, oz)));
                self.add(o as usize, Vec3::new(x, y, z), Vec3::new(vx, vy, vz), stuck);
            }
        }
    }

    /// Removes every bomb (toy switched off) and tells the clients.
    pub(crate) fn clear(&mut self, clients: &mut [Client]) {
        while !self.bombs.is_empty() {
            self.remove(0);
        }
        self.send(clients);
    }

    /// Full state for newly spawned clients, then moved bombs to everyone.
    pub(crate) fn send(&mut self, clients: &mut [Client]) {
        self.ensure(clients.len());
        for b in &mut self.bombs {
            if b.sent.is_none_or(|s| (s - b.origin).length_sq() > 0.01) {
                b.sent = Some(b.origin);
                let o = b.origin;
                self.outbox.push(format!("//qsp_sticky {} {} {} {}\n", b.id, o.x, o.y, o.z));
            }
        }
        for (i, cl) in clients.iter_mut().enumerate() {
            if !cl.spawned {
                self.synced[i] = false;
                continue;
            }
            if !self.synced[i] {
                self.synced[i] = true;
                write_stuff(cl, "//qsp_sticky clear\n");
                for b in &self.bombs {
                    let o = b.origin;
                    write_stuff(cl, &format!("//qsp_sticky {} {} {} {}\n", b.id, o.x, o.y, o.z));
                }
            } else {
                for s in &self.outbox {
                    write_stuff(cl, s);
                }
            }
        }
        self.outbox.clear();
    }
}

/// A sound the progs precached (mods may not have it: then silence).
fn sound(vm: &Vm, c: &mut Core, e: Ent, channel: i32, name: &str) {
    if c.sv.sound_precache.iter().any(|s| s == name) {
        crate::send::start_sound(vm, c, e, channel, name.as_bytes(), 255, 1.0);
    }
}

fn write_stuff(cl: &mut Client, s: &str) {
    cl.message.byte(crate::msg::SVC_STUFFTEXT);
    cl.message.string(s.as_bytes());
}

/// One server frame: fire / detonate from the buttons, then move bombs.
pub(crate) fn frame(st: &mut Stickies, vm: &mut Vm, c: &mut Core, w: &mut World) {
    let n = c.clients.len();
    st.ensure(n);
    let now = vm.time;
    let mut detonate = Vec::new();
    for i in 0..n {
        let cl = &c.clients[i];
        let b = if cl.spawned && super::Toy::from_buttons(cl.cmd.buttons) == Some(super::Toy::Sticky) { cl.cmd.buttons } else { 0 };
        let new = b & !st.prev_buttons[i];
        st.prev_buttons[i] = b;
        if new & super::BUTTON_TOY_ALT != 0 {
            detonate.push(i);
        }
        if b & super::BUTTON_TOY_FIRE != 0 && now >= st.next_fire[i] {
            let e = i as Ent + 1;
            let f = c.sv.fields.clone();
            if vm.ent_f(e, f.health) <= 0.0 || vm.ent_f(e, f.ammo_rockets) < 1.0 {
                continue;
            }
            st.next_fire[i] = now + REFIRE;
            let rockets = vm.ent_f(e, f.ammo_rockets) - 1.0;
            vm.set_ent_f(e, f.ammo_rockets, rockets);
            let wpn = vm.ent_f(e, f.weapon);
            if wpn == IT_GRENADE_LAUNCHER || wpn == IT_ROCKET_LAUNCHER {
                vm.set_ent_f(e, f.currentammo, rockets);
            }
            if st.bombs.len() >= MAX_BOMBS {
                // the shooter's oldest goes off (anyone's when they own none)
                let k = st.bombs.iter().position(|b| b.owner == i).unwrap_or(0);
                let b = st.remove(k);
                explode(vm, c, w, b);
            }
            let ents = Ents { vm: &mut *vm, c: &mut *c };
            let (fwd, right, up) = angle_vectors(ents.v(e, V::VAngle));
            let vel = fwd * 600.0 + up * 200.0 + right * 0.0;
            st.add(i, ents.v(e, V::Origin), vel, None);
            sound(vm, c, e, 1, "weapons/grenade.wav");
        }
        if !c.clients[i].active {
            detonate.push(i);
        }
    }
    for i in detonate {
        while let Some(k) = st.bombs.iter().position(|b| b.owner == i) {
            let b = st.remove(k);
            explode(vm, c, w, b);
        }
    }
    let dt = vm.g_f(c.sv.globals.frametime);
    let gravity = c.cvars.value_of("sv_gravity");
    let mut k = 0;
    while k < st.bombs.len() {
        let ents = Ents { vm: &mut *vm, c: &mut *c };
        if step(&mut st.bombs[k], w, &ents, dt, gravity) {
            k += 1;
        } else {
            st.remove(k);
        }
    }
}

/// Moves one bomb; false when it is lost (sky, out of the world).
fn step<D: Edicts + ?Sized>(b: &mut Bomb, w: &World, ents: &D, dt: f32, gravity: f32) -> bool {
    if let Some((e, off)) = b.stuck {
        let gone = ents.is_free(e)
            || (e != 0 && ents.f(e, F::Solid) != SOLID_BSP && (ents.f(e, F::Health) <= 0.0 || ents.f(e, F::Solid) == SOLID_NOT));
        if !gone {
            b.origin = if e == 0 { off } else { ents.v(e, V::Origin) + off };
            return true;
        }
        // the host died or vanished: fall to the floor and stick there
        b.stuck = None;
        b.vel = Vec3::ZERO;
    }
    b.vel.z -= gravity * dt;
    let end = b.origin + b.vel * dt;
    let tr = w.trace(ents, b.origin, Vec3::ZERO, Vec3::ZERO, end, MoveType::Missile, Some(b.owner as Ent + 1));
    if tr.allsolid || b.origin.z.abs() > 16384.0 {
        return false;
    }
    let Some(hit) = tr.ent.filter(|_| tr.fraction < 1.0) else {
        b.origin = end;
        return true;
    };
    if w.point_contents(tr.endpos - tr.plane_normal * 2.0) == CONTENTS_SKY {
        return false;
    }
    let pos = tr.endpos + tr.plane_normal;
    b.origin = pos;
    b.vel = Vec3::ZERO;
    b.stuck = Some(if hit == 0 { (0, pos) } else { (hit, pos - ents.v(hit, V::Origin)) });
    true
}

/// Rocket explosion at a bomb: QC `T_RadiusDamage(tmp, owner, 120, world)`
/// from a temporary edict, then TE_EXPLOSION and `weapons/r_exp3.wav`.
fn explode(vm: &mut Vm, c: &mut Core, w: &mut World, b: Bomb) {
    let Ok(tmp) = vm.ed_alloc() else { return };
    let f = c.sv.fields.clone();
    vm.set_ent_v(tmp, f.origin, b.origin.to_array());
    vm.set_ent_i(tmp, f.owner, b.owner as i32 + 1);
    let rd = vm.progs.find_function("T_RadiusDamage").unwrap_or(0) as i32;
    if rd > 0 {
        vm.set_g_i(OFS_PARM0, tmp as i32);
        vm.set_g_i(OFS_PARM0 + 3, b.owner as i32 + 1);
        vm.set_g_f(OFS_PARM0 + 6, DAMAGE);
        vm.set_g_i(OFS_PARM0 + 9, 0);
        exec(vm, c, w, rd);
    }
    let pf = c.sv.protocolflags;
    let m = &mut c.sv.datagram;
    if m.len() < crate::msg::MAX_DATAGRAM - 16 {
        m.byte(SVC_TEMP_ENTITY);
        m.byte(TE_EXPLOSION);
        for v in b.origin.to_array() {
            m.coord(v, pf);
        }
    }
    sound(vm, c, tmp, 0, "weapons/r_exp3.wav");
    crate::server::ed_free(vm, c, w, tmp);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_text_round_trips() {
        let mut a = Stickies::default();
        a.add(0, Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, -5.0), None);
        a.add(1, Vec3::new(4.0, 5.0, 6.0), Vec3::ZERO, Some((7, Vec3::new(0.5, 0.0, 8.0))));
        let mut b = Stickies::default();
        b.load_text(&format!("portal x\n{}", a.save_text()));
        assert_eq!(b.bombs.len(), 2);
        assert_eq!(b.bombs[0].vel, a.bombs[0].vel);
        assert_eq!(b.bombs[1].stuck, a.bombs[1].stuck);
        assert_eq!(b.save_text(), a.save_text());
    }
}
