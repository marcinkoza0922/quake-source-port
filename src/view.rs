//! V_CalcRefdef (view.c): bob, roll, view height, punch, damage kick,
//! colour shifts and the view model; plus building the render entity list.

use qclient::{
    ClientEntity, ClientState, LERP_FINISH, LERP_MOVESTEP, LERP_RESETANIM, LERP_RESETANIM2,
    LERP_RESETMOVE,
};
use qcore::math::{Vec3, angle_vectors};
use qnet::protocol::*;
use qrender::{DLight, Particle, RenderEntity, View};

use crate::assets::{Kind, Level};
use crate::sbar::{IT_INVISIBILITY, IT_INVULNERABILITY, IT_QUAD, IT_SUIT};

pub mod weapon;
mod damage;
mod settings;
mod speed;
pub mod camera;
pub use settings::{Idle, ViewCvars};

pub const CONTENTS_EMPTY: i32 = -1;
pub const CONTENTS_SOLID: i32 = -2;
pub const CONTENTS_WATER: i32 = -3;
pub const CONTENTS_SLIME: i32 = -4;
pub const CONTENTS_LAVA: i32 = -5;
pub const CONTENTS_SKY: i32 = -6;

#[derive(Clone, Copy, Default)]
struct CShift {
    color: [f32; 3],
    percent: f32,
}

/// Per-entity alias frame lerp state (R_SetupAliasFrame).
#[derive(Clone, Copy, Default)]
struct Anim {
    model: usize,
    frame: i32,
    prev: i32,
    start: f64,
    // R_SetupEntityTransform state
    move_start: f64,
    prev_origin: [f32; 3],
    cur_origin: [f32; 3],
    prev_angles: [f32; 3],
    cur_angles: [f32; 3],
}

#[derive(Default)]
pub struct ViewState {
    /// Model indices not drawn because an `r_effect ... replace` emitter
    /// stands in for them (MOD_EMITREPLACE).
    pub replaced_models: Vec<bool>,
    dmg_time: f32,
    dmg_roll: f32,
    dmg_pitch: f32,
    damage: CShift,
    edges: [f32; 4],
    bonus: CShift,
    /// `cshift_empty`: the contents shift outside liquids, set by `v_cshift`
    /// (QC stuffs it: AD, Alkaline, ...). Survives level changes like C's.
    empty: CShift,
    oldz: f32,
    anims: Vec<Anim>,
    static_anims: Vec<Anim>,
    view_anim: Anim,
    pub face_anim_time: f64,
    /// Add the player's own body as a portal-only entity (portals exist).
    pub with_self: bool,
    /// Third-person camera: draw the player's own body in the main view too.
    pub third_person: bool,
    pub dodge_pose: [f32; 3],
    /// Smoothed third-person crosshair convergence.
    pub aim: camera::Aim,
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

impl ViewState {
    pub fn reset(&mut self) {
        let empty = self.empty;
        *self = ViewState::default();
        self.empty = empty;
    }

    /// V_cshift_f: `v_cshift r g b percent` (atoi like C).
    pub fn set_empty_shift(&mut self, r: i32, g: i32, b: i32, percent: i32) {
        self.empty = CShift { color: [r as f32, g as f32, b as f32], percent: percent as f32 };
    }

    /// V_ResetEffects (CL_Disconnect).
    pub fn reset_effects(&mut self) {
        self.empty.percent = 0.0;
    }

    /// V_ParseDamage.
    pub fn parse_damage(&mut self, cl: &ClientState, cv: &ViewCvars, armor: i32, blood: i32, from: [f32; 3]) {
        let count = (blood as f32 * 0.5 + armor as f32 * 0.5).max(10.0);
        self.face_anim_time = cl.time + 0.2;
        self.damage.percent = (self.damage.percent + 3.0 * count).clamp(0.0, 150.0);
        self.damage.color = if armor > blood {
            [200.0, 100.0, 100.0]
        } else if armor != 0 {
            [220.0, 50.0, 50.0]
        } else {
            [255.0, 0.0, 0.0]
        };
        let Some(ent) = cl.entities.get(cl.viewentity) else { return };
        let mut d = Vec3::from_array(from) - Vec3::from_array(ent.origin);
        let len = dot(d, d).sqrt();
        if len > 0.0 {
            d = d / len;
        }
        let (f, r, _) = angle_vectors(Vec3::from_array(ent.angles));
        let (forward, right, _) = angle_vectors(Vec3::new(0.0, cl.viewangles[1], 0.0));
        let front = dot(d, forward);
        let side = dot(d, right);
        let weights = [front.max(0.0), side.max(0.0), (-front).max(0.0), (-side).max(0.0)];
        let strength = (armor + blood).max(0) as f32 / 80.0;
        for (edge, weight) in self.edges.iter_mut().zip(weights) {
            *edge = (*edge + strength * if len < 1.0 { 1.0 } else { weight }).min(0.85);
        }
        self.dmg_roll = count * dot(d, r) * cv.kickroll;
        self.dmg_pitch = count * dot(d, f) * cv.kickpitch;
        self.dmg_time = cv.kicktime;
    }

    pub fn bonus_flash(&mut self) {
        self.bonus = CShift { color: [215.0, 186.0, 69.0], percent: 50.0 };
    }

    /// V_UpdateBlend: decays shifts and returns the polyblend.
    /// `flash` scales the damage/bonus/powerup shifts (accessibility).
    /// `cshift` scales each shift (gl_cshiftpercent, gl_polyblend).
    pub fn blend(&mut self, cl: &ClientState, contents: i32, ft: f32, flash: f32, cshift: [f32; 4]) -> [f32; 4] {
        // V_SetContentsColor: empty, solid and sky use cshift_empty;
        // everything else that isn't lava or slime is water.
        let content = match contents {
            CONTENTS_EMPTY | CONTENTS_SOLID | CONTENTS_SKY => self.empty,
            CONTENTS_LAVA => CShift { color: [255.0, 80.0, 0.0], percent: 150.0 },
            CONTENTS_SLIME => CShift { color: [0.0, 25.0, 5.0], percent: 150.0 },
            _ => CShift { color: [130.0, 80.0, 50.0], percent: 128.0 },
        };
        let it = cl.items;
        let power = if it & IT_QUAD != 0 {
            CShift { color: [0.0, 0.0, 255.0], percent: 30.0 }
        } else if it & IT_SUIT != 0 {
            CShift { color: [0.0, 255.0, 0.0], percent: 20.0 }
        } else if it & IT_INVISIBILITY != 0 {
            CShift { color: [100.0, 100.0, 100.0], percent: 100.0 }
        } else if it & IT_INVULNERABILITY != 0 {
            CShift { color: [255.0, 255.0, 0.0], percent: 30.0 }
        } else {
            CShift::default()
        };
        let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
        let flash = flash.clamp(0.0, 1.0);
        let scaled = |s: CShift| CShift { percent: s.percent * flash, ..s };
        let shifts = [content, scaled(self.damage), scaled(self.bonus), scaled(power)];
        for (j, s) in shifts.into_iter().enumerate() {
            // only the contents shift during intermission (johnfitz)
            if cl.intermission != 0 && j != 0 {
                continue;
            }
            let a2 = s.percent * cshift[j] / 255.0;
            if a2 <= 0.0 {
                continue;
            }
            a += a2 * (1.0 - a);
            let f = a2 / a;
            r = r * (1.0 - f) + s.color[0] * f;
            g = g * (1.0 - f) + s.color[1] * f;
            b = b * (1.0 - f) + s.color[2] * f;
        }
        for edge in &mut self.edges {
            *edge = (*edge - ft.max(0.0) * 0.65).max(0.0);
        }
        self.damage.percent = (self.damage.percent - ft * 150.0).max(0.0);
        self.bonus.percent = (self.bonus.percent - ft * 100.0).max(0.0);
        [r / 255.0, g / 255.0, b / 255.0, a.clamp(0.0, 1.0)]
    }

    fn calc_bob(cl: &ClientState, cv: &ViewCvars) -> f32 {
        if cv.bobcycle <= 0.0 {
            return 0.0;
        }
        let t = cl.time as f32;
        let mut cycle = (t - (t / cv.bobcycle).floor() * cv.bobcycle) / cv.bobcycle;
        let pi = std::f32::consts::PI;
        cycle = if cycle < cv.bobup { pi * cycle / cv.bobup } else { pi + pi * (cycle - cv.bobup) / (1.0 - cv.bobup) };
        let v = cl.velocity;
        let mut bob = (v[0] * v[0] + v[1] * v[1]).sqrt() * cv.bob;
        bob = bob * 0.3 + bob * 0.7 * cycle.sin();
        bob.clamp(-7.0, 4.0)
    }

    fn calc_roll(angles: [f32; 3], vel: [f32; 3], cv: &ViewCvars) -> f32 {
        let (_, r, _) = angle_vectors(Vec3::from_array(angles));
        let side = dot(Vec3::from_array(vel), r);
        let sign = if side < 0.0 { -1.0 } else { 1.0 };
        let side = side.abs();
        let v = if side < cv.rollspeed { side * cv.rollangle / cv.rollspeed } else { cv.rollangle };
        v * sign
    }

    /// V_CalcRefdef (or V_CalcIntermissionRefdef). Returns the view and the
    /// view model entity, if one should be drawn.
    pub fn calc_refdef(&mut self, cl: &mut ClientState, cv: &ViewCvars, lv: &Level, ft: f32) -> (View, Option<RenderEntity>) {
        let ent = cl.entities.get(cl.viewentity).cloned().unwrap_or_default();
        let mut view = View { origin: ent.origin, angles: cl.viewangles, fov_x: cv.fov, ..Default::default() };
        if cl.intermission != 0 {
            // V_CalcIntermissionRefdef: always idle in intermission
            view.angles = ent.angles;
            for (a, i) in view.angles.iter_mut().zip(cv.idle.offsets(cl.time, 1.0)) {
                *a += i;
            }
            return (view, None);
        }
        if cv.speed_blur && cl.stats[STAT_HEALTH] > 0 {
            view.speed_blur = speed::strength(cl.velocity);
        }
        let bob = Self::calc_bob(cl, cv);
        view.origin[2] += cl.viewheight + bob;
        for k in 0..3 {
            view.origin[k] += 1.0 / 32.0;
        }
        view.angles[2] += Self::calc_roll(ent.angles, cl.velocity, cv);
        if self.dmg_time > 0.0 {
            view.angles[2] += self.dmg_time / cv.kicktime * self.dmg_roll;
            view.angles[0] += self.dmg_time / cv.kicktime * self.dmg_pitch;
            self.dmg_time -= ft;
        }
        let dead = cl.stats[STAT_HEALTH] <= 0;
        if dead {
            view.angles[2] = 80.0;
        }
        // V_AddIdle
        let idle = cv.idle.offsets(cl.time, cv.idle.scale);
        for (a, i) in view.angles.iter_mut().zip(idle) {
            *a += i;
        }
        // view model
        // CalcGunAngle: yaw/pitch follow the view, roll comes from
        // cl.viewangles (normally 0), so strafe/damage roll doesn't tilt the gun.
        // The gun takes the idle sway back out (twice for pitch, as in C).
        let mut gun_angles = view.angles;
        gun_angles[0] = -view.angles[0] - idle[0];
        gun_angles[1] -= idle[1];
        gun_angles[2] = cl.viewangles[2] - idle[2];
        let (fwd, _, _) = angle_vectors(Vec3::from_array([-ent.angles[0], ent.angles[1], 0.0]));
        let mut gorg = ent.origin;
        gorg[2] += cl.viewheight;
        for k in 0..3 {
            gorg[k] += fwd[k] * bob * 0.4;
        }
        gorg[2] += bob;
        // punch
        if cv.gunkick {
            let t = (((cl.time - cl.punchtime) / 0.1) as f32).clamp(0.0, 1.0);
            for k in 0..3 {
                let p = cl.punchangles[1][k] + t * (cl.punchangles[0][k] - cl.punchangles[1][k]);
                view.angles[k] += p;
            }
        }
        // step smoothing
        if cl.onground && ent.origin[2] - self.oldz > 0.0 {
            self.oldz += ft * 80.0;
            if self.oldz > ent.origin[2] {
                self.oldz = ent.origin[2];
            }
            if ent.origin[2] - self.oldz > 12.0 {
                self.oldz = ent.origin[2] - 12.0;
            }
            view.origin[2] += self.oldz - ent.origin[2];
            gorg[2] += self.oldz - ent.origin[2];
        } else {
            self.oldz = ent.origin[2];
        }
        let wmodel = cl.stats[STAT_WEAPON] as usize;
        let show = cv.viewmodel.visible() && !dead && cl.items & IT_INVISIBILITY == 0 && wmodel != 0;
        let gun = lv.models.get(wmodel).copied().flatten().filter(|_| show).map(|m| {
            let frame = cl.stats[STAT_WEAPONFRAME];
            let (f, prev, lerp) = Self::alias_frame(
                &mut self.view_anim,
                wmodel,
                frame,
                &mut cl.viewent.lerpflags,
                cl.time,
                cv.lerp_models,
                ent.lerpflags & LERP_FINISH != 0,
                ent.lerpfinish,
            );
            RenderEntity {
                model: m.id,
                origin: (Vec3::from(gorg) + cv.viewmodel.offset(view.angles)).to_array(),
                angles: gun_angles,
                frame: f,
                prev_frame: prev,
                lerp,
                viewmodel: true,
                alpha: 1.0,
                ..Default::default()
            }
        });
        (view, gun)
    }

    /// R_SetupAliasFrame: (frame, prev_frame, blend). Consumes LERP_RESETANIM / LERP_RESETANIM2.
    #[allow(clippy::too_many_arguments)]
    fn alias_frame(
        a: &mut Anim,
        model: usize,
        frame: i32,
        lerpflags: &mut u32,
        time: f64,
        enabled: bool,
        has_finish: bool,
        lerpfinish: f64,
    ) -> (u32, u32, f32) {
        if a.model != model || *lerpflags & LERP_RESETANIM != 0 {
            a.model = model;
            a.frame = frame;
            a.prev = frame;
            a.start = time - 1.0;
            *lerpflags &= !LERP_RESETANIM;
        } else if a.frame != frame {
            if *lerpflags & LERP_RESETANIM2 != 0 {
                a.prev = frame;
                a.frame = frame;
                a.start = time - 1.0;
                *lerpflags &= !LERP_RESETANIM2;
            } else {
                a.prev = a.frame;
                a.frame = frame;
                a.start = time;
            }
        }
        if !enabled {
            return (a.frame.max(0) as u32, a.frame.max(0) as u32, 1.0);
        }
        let blend = if has_finish && lerpfinish > a.start {
            ((time - a.start) / (lerpfinish - a.start)) as f32
        } else {
            ((time - a.start) / 0.1) as f32
        };
        (a.frame.max(0) as u32, a.prev.max(0) as u32, blend.clamp(0.0, 1.0))
    }

    fn anim_lerp(a: &mut Anim, e: &mut ClientEntity, time: f64, enabled: bool) -> (u32, u32, f32) {
        let has_finish = e.lerpflags & LERP_FINISH != 0;
        let lerpfinish = e.lerpfinish;
        Self::alias_frame(a, e.model, e.frame, &mut e.lerpflags, time, enabled, has_finish, lerpfinish)
    }

    /// R_SetupEntityTransform: smoothed (origin, angles). Consumes LERP_RESETMOVE.
    fn move_lerp(a: &mut Anim, e: &mut ClientEntity, time: f64, enabled: bool) -> ([f32; 3], [f32; 3]) {
        if e.lerpflags & LERP_RESETMOVE != 0 {
            a.move_start = 0.0;
            (a.prev_origin, a.cur_origin) = (e.origin, e.origin);
            (a.prev_angles, a.cur_angles) = (e.angles, e.angles);
            e.lerpflags &= !LERP_RESETMOVE;
        } else if e.origin != a.cur_origin || e.angles != a.cur_angles {
            a.move_start = time;
            a.prev_origin = a.cur_origin;
            a.cur_origin = e.origin;
            a.prev_angles = a.cur_angles;
            a.cur_angles = e.angles;
        }
        if !enabled || e.lerpflags & LERP_MOVESTEP == 0 {
            return (e.origin, e.angles);
        }
        let blend = if e.lerpflags & LERP_FINISH != 0 && e.lerpfinish > a.move_start {
            ((time - a.move_start) / (e.lerpfinish - a.move_start)) as f32
        } else {
            ((time - a.move_start) / 0.1) as f32
        }
        .clamp(0.0, 1.0);
        let mut o = [0.0; 3];
        let mut ang = [0.0; 3];
        for i in 0..3 {
            o[i] = a.prev_origin[i] + (a.cur_origin[i] - a.prev_origin[i]) * blend;
            let mut d = a.cur_angles[i] - a.prev_angles[i];
            if d > 180.0 {
                d -= 360.0;
            } else if d < -180.0 {
                d += 360.0;
            }
            ang[i] = a.prev_angles[i] + d * blend;
        }
        (o, ang)
    }

    /// Render entities: visedicts, statics, temp entities (beams).
    pub fn entities(&mut self, cl: &mut ClientState, cv: &ViewCvars, lv: &mut Level, fs: &qcore::fs::Fs, r: &mut qrender::Renderer, out: &mut Vec<RenderEntity>) {
        let time = cl.time;
        if self.anims.len() < cl.entities.len() {
            self.anims.resize(cl.entities.len(), Anim::default());
        }
        let mk = |e: &ClientEntity, m: crate::assets::Model, f: (u32, u32, f32), t: ([f32; 3], [f32; 3]), idx: usize, cl: &ClientState| {
            let colormap = (m.kind == Kind::Alias && e.colormap > 0 && (e.colormap as usize) <= cl.scores.len()).then(|| {
                let c = cl.scores[e.colormap as usize - 1].colors;
                [((c >> 4) & 15) as u8, (c & 15) as u8]
            });
            let player = idx >= 1 && idx <= cl.maxclients;
            RenderEntity {
                model: m.id,
                origin: t.0,
                angles: t.1,
                frame: f.0,
                prev_frame: f.1,
                lerp: f.2,
                skin: e.skin.max(0) as u32,
                colormap,
                alpha: if e.alpha == ENTALPHA_DEFAULT { 1.0 } else { (e.alpha as f32 - 1.0) / 254.0 },
                syncbase: e.syncbase,
                viewmodel: false,
                fullbright: false,
                player,
                // motion blur identity; the view entity moves with the camera
                key: if idx > 0 && idx != cl.viewentity { idx as u32 } else { 0 },
                dyn_pose: None,
                views: 0,
                portal_only: false,
                follows_camera: false,
                heat: 0,
            }
        };
        for k in 0..cl.visedicts.len() {
            let i = cl.visedicts[k];
            if self.replaced_models.get(cl.entities[i].model).copied().unwrap_or(false)
                || cl.debris_owned.get(i).copied().unwrap_or(false)
            {
                continue;
            }
            let Some(m) = lv.models.get(cl.entities[i].model).copied().flatten() else { continue };
            let e = &mut cl.entities[i];
            let a = &mut self.anims[i];
            let t = Self::move_lerp(a, e, time, cv.lerp_move);
            let f = if m.kind == Kind::Brush { (e.frame.max(0) as u32, 0, 1.0) } else { Self::anim_lerp(a, e, time, cv.lerp_models) };
            out.push(mk(&cl.entities[i], m, f, t, i, cl));
        }
        let me = cl.viewentity;
        if (self.with_self || self.third_person)
            && me > 0
            && me <= cl.maxclients
            && me < cl.entities.len()
            && let Some(m) = lv.models.get(cl.entities[me].model).copied().flatten().filter(|m| m.kind == Kind::Alias)
        {
            let e = &mut cl.entities[me];
            let a = &mut self.anims[me];
            let t = Self::move_lerp(a, e, time, cv.lerp_move);
            let f = Self::anim_lerp(a, e, time, cv.lerp_models);
            let mut ent = mk(&cl.entities[me], m, f, t, me, cl);
            ent.angles[0] += self.dodge_pose[0];
            ent.angles[2] += self.dodge_pose[1];
            ent.origin[2] += self.dodge_pose[2];
            out.push(RenderEntity { portal_only: !self.third_person, follows_camera: self.third_person, key: me as u32, ..ent });
        }
        if self.static_anims.len() < cl.static_entities.len() {
            self.static_anims.resize(cl.static_entities.len(), Anim::default());
        }
        for i in 0..cl.static_entities.len() {
            let Some(m) = lv.models.get(cl.static_entities[i].model).copied().flatten() else { continue };
            let e = &mut cl.static_entities[i];
            let f = if m.kind == Kind::Brush { (e.frame.max(0) as u32, 0, 1.0) } else { Self::anim_lerp(&mut self.static_anims[i], e, time, cv.lerp_models) };
            let t = (e.origin, e.angles);
            out.push(mk(&cl.static_entities[i], m, f, t, 0, cl));
        }
        for t in &cl.temp_entities {
            if let Some(m) = lv.named(fs, r, t.model) {
                out.push(RenderEntity { model: m.id, origin: t.origin, angles: t.angles, ..Default::default() });
            }
        }
    }
}

pub fn particles(cl: &ClientState, out: &mut Vec<Particle>) {
    let p = &cl.particles;
    for i in 0..p.len() {
        out.push(Particle { origin: [p.x[i], p.y[i], p.z[i]], color: p.color[i], alpha: 1.0 });
    }
}

pub fn dlights(cl: &ClientState, out: &mut Vec<DLight>) {
    for d in cl.dlights.iter() {
        if d.die < cl.time || d.radius <= 0.0 {
            continue;
        }
        out.push(DLight { origin: d.origin, radius: d.radius, minlight: d.minlight, color: d.color });
    }
}

#[cfg(test)]
mod tests;
