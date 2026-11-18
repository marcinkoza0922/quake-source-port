//! The render entity list (R_SetupAliasFrame / R_SetupEntityTransform
//! lerping): visible edicts, the player's own body, statics and temp
//! entities, with visible weapons attached to player bodies.

use qclient::{ClientEntity, ClientState, LERP_FINISH, LERP_MOVESTEP, LERP_RESETANIM, LERP_RESETANIM2, LERP_RESETMOVE};
use qnet::protocol::ENTALPHA_DEFAULT;
use qrender::RenderEntity;

use super::{ViewCvars, ViewState};
use crate::assets::{Kind, Level};

/// Per-entity alias frame lerp state (R_SetupAliasFrame).
#[derive(Clone, Copy, Default)]
pub(super) struct Anim {
    pub(super) model: usize,
    pub(super) frame: i32,
    pub(super) prev: i32,
    pub(super) start: f64,
    // R_SetupEntityTransform state
    pub(super) move_start: f64,
    pub(super) prev_origin: [f32; 3],
    pub(super) cur_origin: [f32; 3],
    pub(super) prev_angles: [f32; 3],
    pub(super) cur_angles: [f32; 3],
}

impl ViewState {
    /// R_SetupAliasFrame: (frame, prev_frame, blend). Consumes LERP_RESETANIM / LERP_RESETANIM2.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn alias_frame(
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
    pub(super) fn move_lerp(a: &mut Anim, e: &mut ClientEntity, time: f64, enabled: bool) -> ([f32; 3], [f32; 3]) {
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
            self.vwep.attach(i, cl, lv, fs, r, out);
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
            self.vwep.attach(me, cl, lv, fs, r, out);
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
