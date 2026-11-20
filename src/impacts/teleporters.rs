//! Weapon impacts on the rendered teleporter sheet, which hull traces can
//! pass through on their way to the solid wall behind it.
use super::*;
use qformats::Bsp;

type Sheet = Vec<Vec3>;

#[derive(Default)]
pub(super) struct TeleportImpacts {
    // Indexed by BSP submodel. Only *tele faces are copied.
    sheets: Option<Vec<Vec<Sheet>>>,
    has_sheets: bool,
    ripples: Vec<[f32; 4]>,
    explosions: VecDeque<(Vec3, f64)>,
}

impl TeleportImpacts {
    fn update(&mut self, windows: bool, time: f64) {
        self.ripples
            .retain(|r| windows && time - f64::from(r[3]) < qrender::TELEPORT_RIPPLE_LIFE);
        self.explosions.retain(|&(_, born)| time - born < 0.6);
    }

    pub(super) fn ripples(&self) -> &[[f32; 4]] {
        &self.ripples
    }

    fn hit(&mut self, world: &BspWorld<'_>, from: Vec3, impact: Vec3) -> Option<Vec3> {
        let dir = (impact - from).normalize();
        // The temp entity can be offset back from the actual collision. Look
        // a little beyond it, and include the recess behind a typical sheet.
        let start = impact - dir * (impact - from).length().min(128.0);
        self.trace(world, start, impact + dir * 16.0)
    }

    fn has_sheets(&mut self, bsp: &Bsp) -> bool {
        if self.sheets.is_none() {
            let sheets = collect_sheets(bsp);
            self.has_sheets = sheets.iter().any(|model| !model.is_empty());
            self.sheets = Some(sheets);
        }
        self.has_sheets
    }

    fn trace(&mut self, world: &BspWorld<'_>, start: Vec3, end: Vec3) -> Option<Vec3> {
        if !self.has_sheets(world.bsp) {
            return None;
        }
        let sheets = self.sheets.as_ref()?;
        let mut first = intersect(sheets.first()?, start, end);
        for brush in world.ents {
            if let Some(polys) = sheets.get(brush.submodel)
                && let Some(f) = intersect(polys, brush.local(start), brush.local(end))
                && first.is_none_or(|best| f < best)
            {
                first = Some(f);
            }
        }
        let point = start.lerp(end, first?);
        if world.cm.point_contents(start) == qworld::CONTENTS_SOLID {
            return None;
        }
        let solid = world.trace_line(start, point);
        // Hull traces stop 1/32 unit short of a coincident solid face.
        ((solid.endpos - point).length_sq() <= 0.25).then_some(point)
    }

    fn ripple(&mut self, point: Vec3, time: f64) {
        // Cluster pellets landing together so they generate one wave.
        if self.ripples.iter().any(|r| {
            time - f64::from(r[3]) < 0.025
                && (Vec3::new(r[0], r[1], r[2]) - point).length_sq() < 16.0
        }) {
            return;
        }
        if self.ripples.len() == qrender::MAX_TELEPORT_RIPPLES {
            self.ripples.remove(0);
        }
        self.ripples.push([point.x, point.y, point.z, time as f32]);
    }
}

impl Impacts {
    /// Route impacts before any of the particle, decal or water systems run.
    /// Returns whether this map has sheets for which we should track shooters.
    pub(crate) fn prepare(
        &mut self,
        cv: &qcore::cvar::Cvars,
        cl: &mut qclient::ClientState,
        lv: &crate::assets::Level,
        brushes: &mut Option<Vec<BrushEnt>>,
        view: Vec3,
        debris: &crate::debris::Debris,
    ) -> bool {
        if cl.time < self.time {
            self.clear();
        }
        let windows = cv.value_of("r_teleportwindows") != 0.0;
        self.teleporters.update(windows, cl.time);
        let has_sheets = lv
            .bsp
            .as_ref()
            .is_some_and(|bsp| self.teleporters.has_sheets(bsp));
        if cl.impact_effects.is_empty() && cl.weapon_impacts.is_empty() {
            return has_sheets;
        }
        let enabled = mark_settings(cv);
        let surface_fx = cv.value_of("r_impact_fx") >= 1.0 && impacts_on(cv);
        let world = if has_sheets {
            cached_world(cl, lv, brushes)
        } else {
            None
        };
        let time = cl.time;
        let mut contact = |mark: Option<u8>, pos| {
            let point = Vec3::from(pos);
            let Some(world) = &world else { return false };
            let from = debris.shot_origin(view, point);
            let Some(hit) = self.teleporters.hit(world, from, point) else {
                return false;
            };
            if windows && (surface_fx || mark.is_some_and(|k| enabled[k as usize])) {
                self.teleporters.ripple(hit, time);
            }
            if mark == Some(2) {
                let explosions = &mut self.teleporters.explosions;
                if explosions.len() == 64 {
                    explosions.pop_front();
                }
                explosions.push_back((point, time));
            }
            true
        };
        // The impact_test console command also feeds the mark path directly.
        cl.weapon_impacts
            .retain(|&(mark, pos)| !contact(Some(mark), pos));
        let mut impacts = std::mem::take(&mut cl.impact_effects);
        for impact in impacts.drain(..) {
            if !contact(impact.kind.mark(), impact.pos) {
                cl.emit_impact_effect(impact);
            }
        }
        cl.impact_effects = impacts;
        has_sheets
    }

    pub(crate) fn hide_explosion_sprites(
        &self,
        cl: &qclient::ClientState,
        ents: &mut Vec<RenderEntity>,
    ) {
        if self.teleporters.explosions.is_empty() {
            return;
        }
        ents.retain(|e| {
            let Some(ent) = cl.entities.get(e.key as usize).filter(|_| e.key != 0) else {
                return true;
            };
            if cl.model_name(ent.model) != Some("progs/s_explod.spr") {
                return true;
            }
            // Match the message position, before render interpolation.
            let origin = Vec3::from(ent.msg_origins[0]);
            !self
                .teleporters
                .explosions
                .iter()
                .any(|&(point, _)| (origin - point).length_sq() <= 4.0 * 4.0)
        });
    }
}

fn collect_sheets(bsp: &Bsp) -> Vec<Vec<Sheet>> {
    bsp.models
        .iter()
        .map(|m| {
            let faces = &bsp.faces[m.first_face as usize..(m.first_face + m.num_faces) as usize];
            faces
                .iter()
                .filter(|f| {
                    let miptex = bsp.texinfo[f.texinfo as usize].miptex;
                    bsp.textures
                        .get(miptex as usize)
                        .and_then(Option::as_ref)
                        .is_some_and(|t| {
                            t.name
                                .as_bytes()
                                .get(..5)
                                .is_some_and(|p| p.eq_ignore_ascii_case(b"*tele"))
                        })
                })
                .map(|f| {
                    (f.first_edge..f.first_edge + f.num_edges)
                        .map(|i| {
                            let se = bsp.surfedges[i as usize];
                            let edge = bsp.edges[se.unsigned_abs() as usize];
                            Vec3::from(bsp.vertexes[edge[usize::from(se < 0)] as usize])
                        })
                        .collect()
                })
                .collect()
        })
        .collect()
}

fn intersect(polys: &[Sheet], start: Vec3, end: Vec3) -> Option<f32> {
    polys
        .iter()
        .filter_map(|p| {
            let (a, rest) = p.split_first()?;
            rest.windows(2)
                .find_map(|edge| mesh::ray(start, end, [*a, edge[0], edge[1]]).map(|(f, _)| f))
        })
        .min_by(f32::total_cmp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use qcore::fs::Fs;
    use qworld::hull::CollisionModel;

    fn load_map(name: &str) -> Option<Bsp> {
        let base = std::env::var_os("QUAKE_BASEDIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("vkQuake directory with maps")
            });
        let mut fs = Fs::new(base);
        fs.add_game_dir("id1");
        let map = fs.open(&format!("maps/{name}.bsp"))?;
        Some(Bsp::parse(&map).unwrap())
    }

    #[test]
    fn impact_uses_sheet_in_front_of_the_solid_wall() {
        let Some(bsp) = load_map("e1m1") else { return };
        let cm = CollisionModel::from_bsp(&bsp);
        let world = BspWorld {
            bsp: &bsp,
            cm: &cm,
            ents: &[],
        };
        let eye = Vec3::new(1312.0, 1016.0, -368.0);
        let forward = Vec3::new(0.0, 1.0, 0.0);
        let wall = world.trace_line(eye, eye + forward * 256.0);
        assert!(wall.frac < 1.0 && wall.endpos.y > 1096.0);
        let mut fx = TeleportImpacts::default();
        let contact = fx.hit(&world, eye, wall.endpos - forward * 4.0).unwrap();
        assert!((contact - Vec3::new(1312.0, 1096.0, -368.0)).length() < 0.01);
        assert!(fx.hit(&world, eye, eye + forward * 30.0).is_none());
        let offset = Vec3::new(44.0, 0.0, 0.0);
        assert!(fx.hit(&world, eye + offset, wall.endpos + offset).is_none());
        assert!(fx.hit(&world, eye + forward * 160.0, contact).is_none());
    }

    #[test]
    fn start_normal_gate_intercepts_the_reported_shot() {
        let Some(bsp) = load_map("start") else { return };
        let cm = CollisionModel::from_bsp(&bsp);
        let world = BspWorld {
            bsp: &bsp,
            cm: &cm,
            ents: &[],
        };
        let eye = Vec3::new(543.25, 1254.375, 14.0);
        let (forward, _, _) = qcore::math::angle_vectors(Vec3::new(-7.59, 90.86, 0.0));
        let wall = world.trace_line(eye, eye + forward * 2048.0);
        let reported_hit = wall.endpos - forward * 4.0;
        assert!(wall.frac < 1.0 && reported_hit.y > 1384.0, "{wall:?}");
        let mut fx = TeleportImpacts::default();
        let contact = fx.hit(&world, eye, reported_hit);
        assert!(
            contact.is_some_and(|p| (p.y - 1384.0).abs() < 0.01),
            "{contact:?}"
        );
    }
}
