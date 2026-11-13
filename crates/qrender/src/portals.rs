//! Strictly identified, intra-level teleporter windows. No gameplay changes.
use crate::{Bsp, TextureKind, world};

#[derive(Clone, Debug)]
pub struct Portal {
    pub mins: [f32; 3],
    pub maxs: [f32; 3],
    pub normal: [f32; 3],
    pub center: [f32; 3],
    pub destination: [f32; 3],
    /// Maps directions at the source to directions at the destination
    /// (`rot[i][j]`: row i, column j).
    pub rot: [[f32; 3]; 3],
    /// Point whose PVS is drawn (inside the destination's empty space).
    pub pvs: [f32; 3],
    /// Index into [`crate::RenderFrame::gun_portals`] for add-on portals;
    /// `None` for teleporter windows.
    pub gun: Option<usize>,
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

/// The window through add-on portal `index` out of its linked exit: in
/// through the face, out of the exit's face (normal and right flip, up
/// kept), matching `qserver::addons::portal_move::carry`.
pub fn gun(index: usize, g: &crate::GunPortal, (eo, en, eu): ([f32; 3], [f32; 3], [f32; 3])) -> Portal {
    let (n, u) = (g.normal, g.up);
    let r = cross(u, n);
    let er = cross(eu, en);
    let rot = std::array::from_fn(|i| std::array::from_fn(|j| -en[i] * n[j] - er[i] * r[j] + eu[i] * u[j]));
    let [hw, hh] = g.half;
    let (mut mins, mut maxs) = ([f32::MAX; 3], [f32::MIN; 3]);
    for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
        for k in 0..3 {
            let c = g.origin[k] + r[k] * a * hw + u[k] * b * hh;
            mins[k] = mins[k].min(c - 1.0);
            maxs[k] = maxs[k].max(c + 1.0);
        }
    }
    Portal {
        mins,
        maxs,
        normal: n,
        center: g.origin,
        destination: eo,
        rot,
        pvs: std::array::from_fn(|k| eo[k] + en[k] * 8.0),
        gun: Some(index),
    }
}

/// Rotation by `yaw` radians about +z.
pub fn yaw_rot(yaw: f32) -> [[f32; 3]; 3] {
    let (s, c) = yaw.sin_cos();
    [[c, -s, 0.0], [s, c, 0.0], [0.0, 0.0, 1.0]]
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
            let destination = [origin[0], origin[1], origin[2] + 49.0];
            out.push(Portal {
                mins: s.mins,
                maxs: s.maxs,
                normal,
                center,
                destination,
                rot: yaw_rot(angle.to_radians() - (-normal[1]).atan2(-normal[0])),
                pvs: destination,
                gun: None,
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
        std::array::from_fn(|i| world::dot(self.rot[i], v))
    }
    pub fn to_destination(&self, p: [f32; 3]) -> [f32; 3] {
        let v = self.rotate(std::array::from_fn(|k| p[k] - self.center[k]));
        std::array::from_fn(|k| v[k] + self.destination[k])
    }
    /// Column-major destination→source transform (inverse of `to_destination`).
    pub fn to_source(&self) -> [f32; 16] {
        let (r, d) = (self.rot, self.destination);
        let mut m = [0.0; 16];
        for j in 0..3 {
            for i in 0..3 {
                m[j * 4 + i] = r[j][i];
            }
        }
        for i in 0..3 {
            m[12 + i] = self.center[i] - (0..3).map(|j| r[j][i] * d[j]).sum::<f32>();
        }
        m[15] = 1.0;
        m
    }
    /// Destination point back to the source side (inverse of `to_destination`).
    pub fn from_destination(&self, p: [f32; 3]) -> [f32; 3] {
        let d: [f32; 3] = std::array::from_fn(|k| p[k] - self.destination[k]);
        std::array::from_fn(|i| (0..3).map(|j| self.rot[j][i] * d[j]).sum::<f32>() + self.center[i])
    }
    /// `inner` (a portal on this one's destination side) as seen through
    /// this one: a window from the source side whose mapping is this
    /// portal's followed by `inner`'s, its clip plane `inner`'s.
    pub fn through(&self, inner: &Portal) -> Portal {
        let (mut mins, mut maxs) = ([f32::MAX; 3], [f32::MIN; 3]);
        for c in 0..8 {
            let p = std::array::from_fn(|k| if c >> k & 1 == 0 { inner.mins[k] } else { inner.maxs[k] });
            let q = self.from_destination(p);
            for k in 0..3 {
                mins[k] = mins[k].min(q[k]);
                maxs[k] = maxs[k].max(q[k]);
            }
        }
        let (a, b) = (self.rot, inner.rot);
        Portal {
            mins,
            maxs,
            normal: std::array::from_fn(|i| (0..3).map(|j| a[j][i] * inner.normal[j]).sum()),
            center: self.from_destination(inner.center),
            destination: inner.destination,
            rot: std::array::from_fn(|i| std::array::from_fn(|j| (0..3).map(|k| b[i][k] * a[k][j]).sum())),
            pvs: inner.pvs,
            gun: inner.gun,
        }
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
        // gun portals sit on a wall: clip just in front of it so the wall
        // itself (coplanar with the plane) is gone
        let lift = if self.gun.is_some() { 0.5 } else { 0.0 };
        [n[0], n[1], n[2], -world::dot(n, self.destination) - lift]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gun_window_matches_teleport_rotation() {
        let g = crate::GunPortal {
            origin: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
            up: [1.0, 0.0, 0.0],
            half: [28.0, 48.0],
            color: [0; 3],
            exit: None,
            live: true,
        };
        let exit = ([500.0, 0.0, 64.0], [-1.0, 0.0, 0.0], [0.0, 0.0, 1.0]);
        let p = gun(0, &g, exit);
        // falling into the floor comes out of the wall, heading -x
        assert_eq!(p.rotate([0.0, 0.0, -1.0]), [-1.0, 0.0, 0.0]);
        assert_eq!(p.to_destination(p.center), exit.0);
        // to_source undoes to_destination
        let q = p.to_destination([3.0, -7.0, 20.0]);
        let m = p.to_source();
        let back: [f32; 3] = std::array::from_fn(|i| m[i] * q[0] + m[4 + i] * q[1] + m[8 + i] * q[2] + m[12 + i]);
        assert!((0..3).all(|k| (back[k] - [3.0, -7.0, 20.0][k]).abs() < 1e-3), "{back:?}");
    }

    #[test]
    fn nested_window_composes_both_mappings() {
        let g = |o: [f32; 3], n: [f32; 3]| crate::GunPortal {
            origin: o,
            normal: n,
            up: [0.0, 0.0, 1.0],
            half: [28.0, 48.0],
            color: [0; 3],
            exit: None,
            live: true,
        };
        let (a, b) = (g([0.0; 3], [1.0, 0.0, 0.0]), g([300.0, 40.0, 0.0], [0.0, 1.0, 0.0]));
        let outer = gun(0, &a, (b.origin, b.normal, b.up));
        let inner = gun(1, &b, (a.origin, a.normal, a.up));
        let both = outer.through(&inner);
        let p = [50.0, 7.0, -3.0];
        let want = inner.to_destination(outer.to_destination(p));
        let got = both.to_destination(p);
        assert!((0..3).all(|k| (got[k] - want[k]).abs() < 1e-3), "{got:?} {want:?}");
        assert!((0..3).all(|k| (both.from_destination(got)[k] - p[k]).abs() < 1e-3));
        let (c1, c2) = (both.clip(), inner.clip());
        assert!((0..4).all(|k| (c1[k] - c2[k]).abs() < 1e-3), "{c1:?} {c2:?}");
    }

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
