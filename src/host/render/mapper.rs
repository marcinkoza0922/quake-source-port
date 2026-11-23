use super::*;
use crate::blood::Geometry;
use qrender::{MapperSettings, Particle, View};

impl Host {
    pub(super) fn mapper_allowed(&self) -> bool {
        !self.remote() && self.cl.maxclients <= 1
    }

    pub(super) fn mapper_settings(&self) -> MapperSettings {
        let allowed = self.mapper_allowed();
        MapperSettings {
            wire: if allowed {
                self.cv("r_showtris").clamp(0.0, 2.0) as u32
            } else {
                0
            },
            lightmap: allowed && self.cv("r_lightmap") != 0.0,
            lock_pvs: allowed && self.cv("r_lockpvs") != 0.0,
            speeds: self.cv("r_speeds") != 0.0,
        }
    }

    pub(super) fn mapper_geometry(
        &mut self,
        lightmap: bool,
        v: &View,
        parts: &mut Vec<Particle>,
        fte: &mut Geometry,
    ) {
        if lightmap {
            fte.0.clear();
            fte.1.clear();
            fte.2.clear();
            parts.clear();
        }
        let eye = Vec3::from(v.origin);
        if self.mapper_allowed() {
            self.bboxes
                .volumes(&self.server, self.level.bsp.as_ref(), eye, &mut self.r, fte);
        }
        let bb = crate::bboxes::Settings::read(&self.server.core.cvars);
        if bb.mode != 0 && self.cv("r_drawentities") != 0.0 {
            let boxes = self.server.edict_boxes(
                self.cl.viewentity as u32,
                (bb.mode.abs() >= 2).then_some(eye),
            );
            let fwd = qcore::math::angle_vectors(Vec3::from(v.angles)).0;
            self.bboxes.frame(&bb, &boxes, eye, fwd, &mut self.r, fte);
        }
        self.bboxes.pointfile_frame(eye, &mut self.r, fte);
    }

    pub(super) fn draw_mapper_stats(&self, c: &mut Canvas, mapper: MapperSettings) {
        if !mapper.speeds && mapper.wire == 0 {
            return;
        }
        c.reset_origin();
        let s = self.r.mapper_stats();
        let (first, second) = if mapper.wire != 0 && !self.r.supports_wireframe() {
            (
                format!("{} tris", s.triangles),
                Some("Wireframe unavailable".to_owned()),
            )
        } else if mapper.speeds {
            (
                format!(
                    "{} wpoly  {} epoly  {} tris",
                    s.world_polys, s.entity_polys, s.triangles
                ),
                Some(format!("{} leaves  {} draws", s.leaves, s.draws)),
            )
        } else {
            (format!("{} tris", s.triangles), None)
        };
        for (i, line) in [Some(first), second].into_iter().flatten().enumerate() {
            let w = c.ptext_w(&line);
            let x = (c.vw() - w - 8.0).max(0.0);
            let y = 20.0 + i as f32 * 9.0;
            c.fill(x - 3.0, y - 1.0, w + 6.0, 9.0, [0.0, 0.0, 0.0, 0.65]);
            c.ptext(x, y, &line, crate::draw::WHITE);
        }
    }
}
