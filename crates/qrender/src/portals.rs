//! Strictly identified, intra-level teleporter windows. No gameplay changes.
use crate::{Bsp, TextureKind, world};

#[derive(Clone, Debug)]
pub struct Portal {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub normal: [f32; 3],
    pub center: [f32; 3],
    pub destination: [f32; 3],
    pub yaw: f32,
}

pub(crate) fn discover(bsp: &Bsp, world: &world::World) -> Vec<Portal> {
    let Ok(entities) = qformats::entities::parse_entities(&bsp.entities) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for trigger in entities
        .iter()
        .filter(|e| e.classname() == Some("trigger_teleport"))
    {
        // Named/silent triggers can be inactive or have custom behaviour.
        if trigger.get("targetname").is_some()
            || trigger
                .get("spawnflags")
                .and_then(|s| s.parse::<u32>().ok())
                .unwrap_or(0)
                & 2
                != 0
        {
            continue;
        }
        let Some(target) = trigger.get("target").filter(|t| !t.is_empty()) else {
            continue;
        };
        let targets: Vec<_> = entities
            .iter()
            .filter(|e| e.get("targetname") == Some(target))
            .collect();
        if targets.len() != 1 || targets[0].classname() != Some("info_teleport_destination") {
            continue;
        }
        let dest = targets[0];
        let Some(origin) = dest.get("origin").and_then(vector) else {
            continue;
        };
        let Some(model) = trigger
            .get("model")
            .and_then(|s| s.strip_prefix('*'))
            .and_then(|s| s.parse::<usize>().ok())
            .and_then(|i| bsp.models.get(i))
        else {
            continue;
        };
        // Only the world's actual *tele* sheets, touching this trigger volume.
        let Some(wm) = world.submodels.first() else {
            continue;
        };
        for s in &world.surfaces
            [wm.first_surface as usize..(wm.first_surface + wm.num_surfaces) as usize]
        {
            let Some(Some(tex)) = bsp.textures.get(s.texture as usize) else {
                continue;
            };
            if s.kind != TextureKind::Liquid || world::material_flags(&tex.name, tex.kind) & 15 != 4
            {
                continue;
            }
            if (0..3).any(|k| s.maxs[k] < model.mins[k] - 2.0 || s.mins[k] > model.maxs[k] + 2.0) {
                continue;
            }
            let normal = if s.flipped {
                s.normal.map(|x| -x)
            } else {
                s.normal
            };
            if normal[2].abs() > 0.1 {
                continue;
            } // classic vertical teleporter sheets
            let center = std::array::from_fn(|k| (s.mins[k] + s.maxs[k]) * 0.5);
            let angle = dest
                .get("angle")
                .and_then(|s| s.parse::<f32>().ok())
                .unwrap_or(0.0);
            // Vanilla QC raises destination markers 27 units; player eyes add 22.
            out.push(Portal {
                mins: s.mins,
                maxs: s.maxs,
                normal,
                center,
                destination: [origin[0], origin[1], origin[2] + 49.0],
                yaw: angle.to_radians() - (-normal[1]).atan2(-normal[0]),
            });
        }
    }
    out
}
fn vector(s: &str) -> Option<[f32; 3]> {
    let v: Vec<f32> = s
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    v.try_into().ok()
}
impl Portal {
    pub fn rotate(&self, v: [f32; 3]) -> [f32; 3] {
        let (s, c) = self.yaw.sin_cos();
        [c * v[0] - s * v[1], s * v[0] + c * v[1], v[2]]
    }
    pub fn to_destination(&self, p: [f32; 3]) -> [f32; 3] {
        let v = self.rotate(std::array::from_fn(|k| p[k] - self.center[k]));
        std::array::from_fn(|k| v[k] + self.destination[k])
    }
    pub fn to_source(&self) -> [f32; 16] {
        let (s, c) = self.yaw.sin_cos();
        let d = self.destination;
        [
            c,
            -s,
            0.0,
            0.0,
            s,
            c,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            self.center[0] - c * d[0] - s * d[1],
            self.center[1] + s * d[0] - c * d[1],
            self.center[2] - d[2],
            1.0,
        ]
    }
    pub fn plane(&self) -> [f32; 4] {
        [
            self.normal[0],
            self.normal[1],
            self.normal[2],
            -world::dot(self.normal, self.center),
        ]
    }
    pub fn clip(&self) -> [f32; 4] {
        let n = self.rotate(self.normal).map(|x| -x);
        [n[0], n[1], n[2], -world::dot(n, self.destination)]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(class: &str, destination: &str, extra: &str) -> (Bsp, world::World) {
        use qformats::bsp::{Face, MipTex, Model};
        let model = Model {
            mins: [-16.0; 3],
            maxs: [16.0; 3],
            origin: [0.0; 3],
            headnode: [0; 4],
            visleafs: 0,
            first_face: 0,
            num_faces: 1,
        };
        let bsp = Bsp {
            entities: format!(
                "{{\"classname\" \"{class}\" \"model\" \"*1\" \"target\" \"exit\" {extra}}} {{\"classname\" \"{destination}\" \"targetname\" \"exit\" \"origin\" \"100 200 300\"}}"
            ),
            models: vec![model, model],
            textures: vec![Some(MipTex {
                name: "*teleport".into(),
                width: 16,
                height: 16,
                kind: TextureKind::Liquid,
                mips: vec![],
                shift: 0,
            })],
            faces: vec![Face {
                plane: 0,
                side: 0,
                first_edge: 0,
                num_edges: 0,
                texinfo: 0,
                styles: [255; 4],
                lightofs: -1,
            }],
            ..Default::default()
        };
        let mut w = world::World::build(&bsp, None);
        let s = &mut w.surfaces[0];
        s.kind = TextureKind::Liquid;
        s.normal = [1.0, 0.0, 0.0];
        s.mins = [0.0, -16.0, -16.0];
        s.maxs = [0.0, 16.0, 16.0];
        (bsp, w)
    }
    #[test]
    fn only_explicit_unambiguous_visible_intra_level_links() {
        for (class, dest, extra, expected) in [
            ("trigger_teleport", "info_teleport_destination", "", 1),
            (
                "trigger_teleport",
                "info_teleport_destination",
                "\"spawnflags\" \"1\"",
                1,
            ),
            ("trigger_changelevel", "info_teleport_destination", "", 0),
            ("trigger_teleport", "info_intermission", "", 0),
            (
                "trigger_teleport",
                "info_teleport_destination",
                "\"spawnflags\" \"2\"",
                0,
            ),
            (
                "trigger_teleport",
                "info_teleport_destination",
                "\"targetname\" \"activate\"",
                0,
            ),
        ] {
            let (b, w) = fixture(class, dest, extra);
            assert_eq!(discover(&b, &w).len(), expected, "{class} {dest} {extra}");
        }
        let (mut b, w) = fixture("trigger_teleport", "info_teleport_destination", "");
        b.entities
            .push_str("{\"targetname\" \"exit\" \"classname\" \"info_teleport_destination\"}");
        assert!(discover(&b, &w).is_empty());
    }
    #[test]
    fn camera_mapping_preserves_offsets_and_has_an_inverse() {
        let (b, w) = fixture("trigger_teleport", "info_teleport_destination", "");
        let p = &discover(&b, &w)[0];
        let eye = [84.0, 17.0, 10.0];
        let mapped = p.to_destination(eye);
        let m = p.to_source();
        for k in 0..3 {
            let back = m[k] * mapped[0] + m[4 + k] * mapped[1] + m[8 + k] * mapped[2] + m[12 + k];
            assert!((back - eye[k]).abs() < 0.001);
        }
        let clip = p.clip();
        assert!(world::dot([clip[0], clip[1], clip[2]], mapped) + clip[3] < 0.0);
        assert_eq!(p.to_destination(p.center), p.destination);
    }
}

/// Enumerate the exact teleporter windows used by the renderer, without a GPU.
pub fn inspect(bsp: &Bsp) -> Vec<Portal> {
    discover(bsp, &world::World::build(bsp, None))
}
