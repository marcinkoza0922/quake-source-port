//! V_CalcRefdef (view.c): bob, roll, view height, punch, damage kick,
//! colour shifts and the view model; plus building the render entity list.

use qclient::{ClientEntity, ClientState, LERP_FINISH, LERP_MOVESTEP, LERP_RESETANIM, LERP_RESETMOVE};
use qcore::math::{Vec3, angle_vectors};
use qnet::protocol::*;
use qrender::{DLight, Particle, RenderEntity, View};

use crate::assets::{Kind, Level};
use crate::sbar::{IT_INVISIBILITY, IT_INVULNERABILITY, IT_QUAD, IT_SUIT};

pub const CONTENTS_WATER: i32 = -3;
pub const CONTENTS_SLIME: i32 = -4;
pub const CONTENTS_LAVA: i32 = -5;

/// View cvars read once per frame.
#[derive(Clone, Copy, Debug)]
pub struct ViewCvars {
    pub bob: f32,
    pub bobcycle: f32,
    pub bobup: f32,
    pub rollangle: f32,
    pub rollspeed: f32,
    pub kicktime: f32,
    pub kickroll: f32,
    pub kickpitch: f32,
    pub fov: f32,
    pub gunkick: bool,
    pub drawviewmodel: bool,
    /// r_lerpmodels: blend alias frames.
    pub lerp_models: bool,
    /// r_lerpmove: smooth MOVESTEP (monster) movement over 0.1s.
    pub lerp_move: bool,
    /// v_flashscale: strength of damage, pickup and powerup screen flashes.
    pub flash: f32,
}

impl ViewCvars {
    /// a11y_reducemotion: no bob, strafe roll, damage kick or gun kick.
    pub fn reduce_motion(mut self, on: bool) -> Self {
        if on {
            self.bob = 0.0;
            self.rollangle = 0.0;
            self.kickroll = 0.0;
            self.kickpitch = 0.0;
            self.gunkick = false;
        }
        self
    }
}

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
    bonus: CShift,
    oldz: f32,
    anims: Vec<Anim>,
    static_anims: Vec<Anim>,
    view_anim: Anim,
    pub face_anim_time: f64,
}

fn dot(a: Vec3, b: Vec3) -> f32 {
    a.x * b.x + a.y * b.y + a.z * b.z
}

impl ViewState {
    pub fn reset(&mut self) {
        *self = ViewState::default();
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
        self.dmg_roll = count * dot(d, r) * cv.kickroll;
        self.dmg_pitch = count * dot(d, f) * cv.kickpitch;
        self.dmg_time = cv.kicktime;
    }

    pub fn bonus_flash(&mut self) {
        self.bonus = CShift { color: [215.0, 186.0, 69.0], percent: 50.0 };
    }

    /// V_UpdateBlend: decays shifts and returns the polyblend.
    /// `flash` scales the damage/bonus/powerup shifts (accessibility).
    pub fn blend(&mut self, cl: &ClientState, contents: i32, ft: f32, flash: f32) -> [f32; 4] {
        let content = match contents {
            CONTENTS_LAVA => CShift { color: [255.0, 80.0, 0.0], percent: 150.0 },
            CONTENTS_SLIME => CShift { color: [0.0, 25.0, 5.0], percent: 150.0 },
            c if c <= CONTENTS_WATER => CShift { color: [130.0, 80.0, 50.0], percent: 128.0 },
            _ => CShift::default(),
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
        for s in [content, scaled(self.damage), scaled(self.bonus), scaled(power)] {
            let a2 = s.percent / 255.0;
            if a2 <= 0.0 {
                continue;
            }
            a += a2 * (1.0 - a);
            let f = a2 / a;
            r = r * (1.0 - f) + s.color[0] * f;
            g = g * (1.0 - f) + s.color[1] * f;
            b = b * (1.0 - f) + s.color[2] * f;
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
    pub fn calc_refdef(&mut self, cl: &ClientState, cv: &ViewCvars, lv: &Level, ft: f32) -> (View, Option<RenderEntity>) {
        let ent = cl.entities.get(cl.viewentity).cloned().unwrap_or_default();
        let mut view = View { origin: ent.origin, angles: cl.viewangles, fov_x: cv.fov, reserve_bottom: 0 };
        if cl.intermission != 0 {
            view.angles = ent.angles;
            return (view, None);
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
        // view model
        // CalcGunAngle: yaw/pitch follow the view, roll comes from
        // cl.viewangles (normally 0), so strafe/damage roll doesn't tilt the gun.
        let mut gun_angles = view.angles;
        gun_angles[0] = -view.angles[0];
        gun_angles[2] = cl.viewangles[2];
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
        let show = cv.drawviewmodel && !dead && cl.items & IT_INVISIBILITY == 0 && wmodel != 0;
        let gun = lv.models.get(wmodel).copied().flatten().filter(|_| show).map(|m| {
            let frame = cl.stats[STAT_WEAPONFRAME];
            let a = &mut self.view_anim;
            if a.model != wmodel || cl.viewent.lerpflags & LERP_RESETANIM != 0 {
                if a.model != wmodel {
                    a.prev = frame;
                } else {
                    a.prev = a.frame;
                }
                a.model = wmodel;
                a.frame = frame;
                a.start = cl.time;
            } else if a.frame != frame {
                a.prev = a.frame;
                a.frame = frame;
                a.start = cl.time;
            }
            // view->lerpfinish = ent->lerpfinish when the server sent one
            let lerp = if !cv.lerp_models {
                1.0
            } else if ent.lerpflags & LERP_FINISH != 0 && ent.lerpfinish > a.start {
                (((cl.time - a.start) / (ent.lerpfinish - a.start)) as f32).clamp(0.0, 1.0)
            } else {
                (((cl.time - a.start) / 0.1) as f32).clamp(0.0, 1.0)
            };
            RenderEntity {
                model: m.id,
                origin: gorg,
                angles: gun_angles,
                frame: frame.max(0) as u32,
                prev_frame: a.prev.max(0) as u32,
                lerp,
                viewmodel: true,
                alpha: 1.0,
                ..Default::default()
            }
        });
        (view, gun)
    }

    /// R_SetupAliasFrame: (frame, prev_frame, blend). Consumes LERP_RESETANIM.
    fn anim_lerp(a: &mut Anim, e: &mut ClientEntity, time: f64, enabled: bool) -> (u32, u32, f32) {
        if a.model != e.model || e.lerpflags & LERP_RESETANIM != 0 {
            a.model = e.model;
            a.frame = e.frame;
            a.prev = e.frame;
            a.start = time - 1.0;
            e.lerpflags &= !LERP_RESETANIM;
        } else if a.frame != e.frame {
            a.prev = a.frame;
            a.frame = e.frame;
            a.start = time;
        }
        if !enabled {
            return (a.frame.max(0) as u32, a.frame.max(0) as u32, 1.0);
        }
        let blend = if e.lerpflags & LERP_FINISH != 0 && e.lerpfinish > a.start {
            ((time - a.start) / (e.lerpfinish - a.start)) as f32
        } else {
            ((time - a.start) / 0.1) as f32
        };
        (a.frame.max(0) as u32, a.prev.max(0) as u32, blend.clamp(0.0, 1.0))
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
    for (_, d) in cl.dlights.iter() {
        if d.die < cl.time || d.radius <= 0.0 {
            continue;
        }
        out.push(DLight { origin: d.origin, radius: d.radius, minlight: d.minlight, color: d.color });
    }
}

#[cfg(test)]
mod movement_tests {
    use super::*;

    #[test]
    fn appearing_monster_snaps_then_smooths_normal_steps() {
        let mut cl = ClientState::new();
        cl.protocol = PROTOCOL_NETQUAKE;
        cl.model_precache = vec![String::new(), "progs/ogre.mdl".into()];
        cl.entities.resize(3, ClientEntity::default());
        cl.entities[2].baseline = qclient::EntityState {
            modelindex: 1, origin: [256.0, 128.0, 24.0], ..Default::default()
        };
        // Exercise both a first appearance and reuse of old renderer history.
        for stale in [[0.0; 3], [-128.0, 64.0, 24.0]] {
            cl.mtime = [1.0, 0.9];
            cl.entities[2].msgtime = 0.5;
            let mut anim = Anim { cur_origin: stale, ..Default::default() };
            cl.parse_server_message(&[(U_SIGNAL | U_STEP) as u8, 2]).unwrap();
            cl.relink_entities();
            let e = &mut cl.entities[2];
            let start = e.origin;
            assert_eq!(ViewState::move_lerp(&mut anim, e, 1.0, true).0, start);
            assert_eq!(e.lerpflags & LERP_RESETMOVE, 0);

            // U_STEP is also sent for normal movement; it must not reset smoothing.
            cl.mtime = [1.1, 1.0];
            cl.entities[2].baseline.origin[0] += 20.0;
            cl.parse_server_message(&[(U_SIGNAL | U_STEP) as u8, 2]).unwrap();
            cl.relink_entities();
            let e = &mut cl.entities[2];
            assert_eq!(ViewState::move_lerp(&mut anim, e, 1.1, true).0, start);
            let halfway = ViewState::move_lerp(&mut anim, e, 1.15, true).0;
            assert!((halfway[0] - start[0] - 10.0).abs() < 0.001);
        }
    }
}
