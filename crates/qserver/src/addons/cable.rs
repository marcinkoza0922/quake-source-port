//! Male-to-male cable: the player holds one plug in each hand. Alt-fire on
//! an electronics surface within reach (`hook_surface::electronic`, or any
//! metal with `addon_cable_metal`) plugs one end in; the other end then
//! fires a lightning bolt with fire (infinite ammo), but the player is
//! tethered to the socket by a rope that wraps round corners
//! (`cable_rope`) and is `addon_cable_length` long. Alt-fire, putting the
//! cable away, dying or being torn away (a teleporter) yanks it out.
//!
//! The tether only changes the player's velocity (no outward speed at full
//! stretch, plus a pull back proportional to the overshoot), so ordinary
//! player physics does all movement and collision.
//!
//! Clients get `//qsp_cable owner state fire sx sy sz nx ny nz ex ey ez
//! length [px py pz]...` stufftext: state 1 plugged into socket `s` facing
//! `n` (sent every frame, with the rope's pivots), 0 yanked out, 2 a plug
//! attempt missed at `s`; `fire` 1 while the bolt reaches `e`. Other engines
//! skip `//` lines. Cables are transient: saves keep only `toy cable`.

use qcore::math::{Vec3, angle_vectors};
use qvm::OFS_PARM0;
use qworld::{Edicts, Ent, MoveType, SOLID_BSP, V};

use super::cable_rope::Rope;
use super::hook_surface::{Material, Surfaces};
use crate::ents::Ents;
use crate::server::{Core, Level, exec};

/// How far from the eye a socket may be (arm's reach).
pub const REACH: f32 = 112.0;
/// Seconds between bolt damage ticks, bolt range and default damage per
/// tick (the stock lightning gun's).
const REFIRE: f64 = 0.1;
pub const RANGE: f32 = 600.0;
/// Seconds between `lhit` sounds while firing (stock `t_width`).
const HUM: f64 = 0.6;
/// Rope overshoot past which the cable tears out (teleporters, pushers).
const TEAR: f32 = 160.0;
/// Pull back per unit of overshoot (u/s), capped.
const PULL: f32 = 10.0;
const MAX_PULL: f32 = 500.0;

/// What may take the plug, how long the cable is and how hard it hits.
#[derive(Clone, Copy, Debug)]
pub struct Rules {
    pub length: f32,
    pub metal: bool,
    pub damage: f32,
}

impl Rules {
    pub fn read(cv: &qcore::cvar::Cvars) -> Self {
        Rules {
            length: cv.value_of("addon_cable_length").clamp(128.0, 4096.0),
            metal: cv.value_of("addon_cable_metal") != 0.0,
            damage: cv.value_of("addon_cable_damage").clamp(1.0, 500.0),
        }
    }
    /// A face with this texture class takes the plug.
    pub fn socket(&self, electronic: bool, m: Material) -> bool {
        electronic || (self.metal && m == Material::Metal)
    }
}

#[derive(Clone, Debug, Default)]
pub enum State {
    #[default]
    Idle,
    Plugged(Box<Plug>),
}

#[derive(Clone, Debug)]
pub struct Plug {
    /// Socket point, relative to `ent`'s origin when plugged into a brush
    /// entity.
    pub point: Vec3,
    pub ent: Option<Ent>,
    pub normal: Vec3,
    pub rope: Rope,
    next_fire: f64,
    next_hum: f64,
    /// Bolt end this frame, while firing.
    bolt: Option<Vec3>,
}

/// Every player's cable; lives on `Level`.
#[derive(Default)]
pub struct Cables {
    pub state: Vec<State>,
    prev_buttons: Vec<u16>,
    /// One-shot lines (misses, yanks) for the next send.
    events: String,
    sent: Vec<bool>,
}

impl Cables {
    fn ensure(&mut self, n: usize) {
        if self.state.len() < n {
            self.state.resize(n, State::Idle);
            self.prev_buttons.resize(n, 0);
            self.sent.resize(n, false);
        }
    }
    /// Unplug every cable (toy off, add-ons shut down, level change).
    pub fn reset(&mut self) {
        self.state.iter_mut().for_each(|s| *s = State::Idle);
        self.events.clear();
    }
}

/// Where the rope meets player `e` (his hull centre: always clear of walls).
pub fn body<D: Edicts + ?Sized>(ents: &D, e: Ent) -> Vec3 {
    ents.v(e, V::Origin)
}

fn socket_pos<D: Edicts + ?Sized>(p: &Plug, ents: &D) -> Vec3 {
    p.ent.map_or(p.point, |m| ents.v(m, V::Origin) + p.point)
}

/// A world trace that only stops at the world (movers don't snag the rope).
fn world_hit<D: Edicts + ?Sized>(w: &qworld::World, ents: &D, a: Vec3, b: Vec3) -> Option<(Vec3, Vec3)> {
    let tr = w.trace(ents, a, Vec3::ZERO, Vec3::ZERO, b, MoveType::NoMonsters, None);
    (tr.fraction < 1.0 && tr.ent == Some(0) && !tr.startsolid).then_some((tr.endpos, tr.plane_normal))
}

/// The socket under player `e`'s crosshair: (entity or world, point,
/// normal), when a permitted face is within reach.
pub fn target<D: Edicts + ?Sized>(surf: &Surfaces, w: &qworld::World, ents: &D, e: Ent, rules: Rules) -> Option<(Option<Ent>, Vec3, Vec3)> {
    let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
    let (f, _, _) = angle_vectors(ents.v(e, V::VAngle));
    let tr = w.trace(ents, eye, Vec3::ZERO, Vec3::ZERO, eye + f * REACH, MoveType::NoMonsters, Some(e));
    let m = tr.ent.filter(|_| tr.fraction < 1.0 && !tr.startsolid)?;
    if m != 0 && ents.f(m, qworld::F::Solid) as i32 != SOLID_BSP {
        return None;
    }
    let sub = if m == 0 { 0 } else { w.model(ents.f(m, qworld::F::ModelIndex) as usize).map_or(usize::MAX, |&(_, s)| s) };
    let origin = if m == 0 { Vec3::ZERO } else { ents.v(m, V::Origin) };
    let face = surf.face(sub, tr.endpos - origin, tr.plane_normal)?;
    rules.socket(face.electronic, face.material).then_some(((m != 0).then_some(m), tr.endpos, tr.plane_normal))
}

/// One server frame of every player's cable (after physics).
pub(crate) fn frame(st: &mut Cables, surf: &Surfaces, l: &mut Level, c: &mut Core, enabled: bool) {
    let n = c.clients.len();
    st.ensure(n);
    let rules = Rules::read(&c.cvars);
    let now = l.vm.time;
    let f = c.sv.fields.clone();
    for i in 0..n {
        let e = i as Ent + 1;
        let cl = &c.clients[i];
        let live = cl.active && cl.spawned && l.vm.ent_f(e, f.health) > 0.0;
        let held = enabled && live && super::Toy::from_buttons(cl.cmd.buttons) == Some(super::Toy::Cable);
        let b = if held { cl.cmd.buttons } else { 0 };
        let pressed = b & !st.prev_buttons[i];
        st.prev_buttons[i] = b;
        let fire = b & super::BUTTON_TOY_FIRE != 0;
        let alt = pressed & super::BUTTON_TOY_ALT != 0;
        let mut state = std::mem::take(&mut st.state[i]);
        let mut hit = None;
        {
            let mut ents = Ents { vm: &mut l.vm, c: &mut *c };
            match &mut state {
                State::Idle if alt => match target(surf, &l.world, &ents, e, rules) {
                    Some((ent, point, normal)) => {
                        let local = ent.map_or(point, |m| point - ents.v(m, V::Origin));
                        let rope = Rope::new(point, body(&ents, e));
                        state = State::Plugged(Box::new(Plug { point: local, ent, normal, rope, next_fire: now, next_hum: now, bolt: None }));
                    }
                    None => {
                        let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
                        let (fw, _, _) = angle_vectors(ents.v(e, V::VAngle));
                        let tr = l.world.trace(&ents, eye, Vec3::ZERO, Vec3::ZERO, eye + fw * REACH, MoveType::NoMonsters, Some(e));
                        let at = if tr.fraction < 1.0 { tr.endpos } else { eye + fw * REACH };
                        st.events += &line(i, 2, false, at, tr.plane_normal, Vec3::ZERO, 0.0, &[]);
                    }
                },
                State::Idle => {}
                State::Plugged(p) => {
                    let gone = p.ent.is_some_and(|m| ents.is_free(m));
                    let socket = socket_pos(p, &ents);
                    p.rope.move_socket(socket);
                    let x = body(&ents, e);
                    p.rope.update(x, |a, b| world_hit(&l.world, &ents, a, b));
                    let over = p.rope.length(x) - rules.length;
                    if !held || alt || gone || over > TEAR {
                        st.events += &line(i, 0, false, socket, Vec3::ZERO, Vec3::ZERO, 0.0, &[]);
                        state = State::Idle;
                    } else {
                        if over > 0.0 {
                            tether(&mut ents, e, p.rope.anchor(), x, over);
                        }
                        p.bolt = None;
                        if fire {
                            let eye = ents.v(e, V::Origin) + ents.v(e, V::ViewOfs);
                            let (fw, _, _) = angle_vectors(ents.v(e, V::VAngle));
                            let tr = l.world.trace(&ents, eye, Vec3::ZERO, Vec3::ZERO, eye + fw * RANGE, MoveType::Normal, Some(e));
                            p.bolt = Some(tr.endpos);
                            if pressed & super::BUTTON_TOY_FIRE != 0 {
                                sound(&mut ents, e, "weapons/lstart.wav");
                            }
                            if now >= p.next_hum {
                                p.next_hum = now + HUM;
                                sound(&mut ents, e, "weapons/lhit.wav");
                            }
                            if now >= p.next_fire {
                                p.next_fire = now + REFIRE;
                                hit = tr.ent.filter(|&t| t != 0 && tr.fraction < 1.0).map(|t| (t, tr.endpos));
                            }
                        } else {
                            p.next_fire = p.next_fire.max(now);
                        }
                    }
                }
            }
        }
        st.state[i] = state;
        if let Some((t, at)) = hit {
            zap(l, c, &f, e, t, at, rules.damage);
        }
    }
    send(st, l, c);
}

/// At full stretch: no speed away from the anchor, and a pull back.
fn tether(ents: &mut Ents, e: Ent, anchor: Vec3, x: Vec3, over: f32) {
    let to = x - anchor;
    if to.length_sq() < 1.0 {
        return;
    }
    let d = to.normalize();
    let mut v = ents.v(e, V::Velocity);
    let out = v.dot(d);
    if out > 0.0 {
        v -= d * out;
    }
    v -= d * (over * PULL).min(MAX_PULL);
    ents.set_v(e, V::Velocity, v);
}

/// Play a stock sound on the player if the progs precached it.
fn sound(ents: &mut Ents, e: Ent, name: &str) {
    if ents.c.sv.sound_precache.iter().any(|s| s == name) {
        crate::send::start_sound(ents.vm, ents.c, e, 0, name.as_bytes(), 255, 1.0);
    }
}

/// One damage tick on `t` through the progs' own `T_Damage`, with the
/// stock lightning gun's blood puff.
fn zap(l: &mut Level, c: &mut Core, f: &crate::fields::Fields, e: Ent, t: Ent, at: Vec3, damage: f32) {
    if l.vm.ent_f(t, f.takedamage) == 0.0 {
        return;
    }
    let pf = c.sv.protocolflags;
    let m = &mut c.sv.datagram;
    if m.len() <= crate::msg::MAX_DATAGRAM - 18 {
        m.byte(crate::msg::SVC_PARTICLE);
        for x in at.to_array() {
            m.coord(x, pf);
        }
        // direction '0 0 100', ×16 and clamped as SVC_PARTICLE does
        for x in [0, 0, 127] {
            m.char(x);
        }
        m.byte((damage * 4.0).min(255.0) as i32);
        m.byte(225);
    }
    let td = l.vm.progs.find_function("T_Damage").unwrap_or(0) as i32;
    if td <= 0 {
        return;
    }
    // T_Damage(targ, inflictor, attacker, damage)
    l.vm.set_g_i(OFS_PARM0, t as i32);
    l.vm.set_g_i(OFS_PARM0 + 3, e as i32);
    l.vm.set_g_i(OFS_PARM0 + 6, e as i32);
    l.vm.set_g_f(OFS_PARM0 + 9, damage);
    exec(&mut l.vm, c, &mut l.world, td);
}

/// The stufftext line clients parse (`qclient::CableView`).
#[allow(clippy::too_many_arguments)]
pub fn line(owner: usize, state: u8, fire: bool, socket: Vec3, normal: Vec3, end: Vec3, length: f32, pivots: &[Vec3]) -> String {
    let v = |v: Vec3| format!(" {:.1} {:.1} {:.1}", v.x, v.y, v.z);
    let n = |v: Vec3| format!(" {:.3} {:.3} {:.3}", v.x, v.y, v.z);
    let mut s = format!("//qsp_cable {owner} {state} {}{}{}{} {length:.0}", u8::from(fire), v(socket), n(normal), v(end));
    for &p in pivots {
        s += &v(p);
    }
    s.push('\n');
    s
}

fn send(st: &mut Cables, l: &mut Level, c: &mut Core) {
    let length = Rules::read(&c.cvars).length;
    let mut lines = std::mem::take(&mut st.events);
    let ents = Ents { vm: &mut l.vm, c: &mut *c };
    for (i, s) in st.state.iter().enumerate() {
        let State::Plugged(p) = s else {
            st.sent[i] = false;
            continue;
        };
        st.sent[i] = true;
        let pivots: Vec<Vec3> = p.rope.pivots.iter().map(|q| q.p).collect();
        let end = p.bolt.unwrap_or(Vec3::ZERO);
        lines += &line(i, 1, p.bolt.is_some(), socket_pos(p, &ents), p.normal, end, length, &pivots);
    }
    if lines.is_empty() {
        return;
    }
    for cl in c.clients.iter_mut().filter(|c| c.spawned) {
        cl.message.byte(crate::msg::SVC_STUFFTEXT);
        cl.message.string(lines.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_gate_sockets() {
        let r = Rules { length: 512.0, metal: false, damage: 30.0 };
        assert!(r.socket(true, Material::Metal));
        assert!(!r.socket(false, Material::Metal));
        assert!(Rules { metal: true, ..r }.socket(false, Material::Metal));
        assert!(!Rules { metal: true, ..r }.socket(false, Material::Stone));
    }

    #[test]
    fn line_has_state_socket_and_pivots() {
        let s = line(1, 1, true, Vec3::new(1.0, 2.0, 3.0), Vec3::new(0.0, 0.0, 1.0), Vec3::new(4.0, 5.0, 6.0), 512.0, &[Vec3::new(7.0, 8.0, 9.0)]);
        assert_eq!(s, "//qsp_cable 1 1 1 1.0 2.0 3.0 0.000 0.000 1.000 4.0 5.0 6.0 512 7.0 8.0 9.0\n");
    }
}
