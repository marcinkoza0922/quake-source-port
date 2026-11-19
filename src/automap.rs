//! Automap / 3D wireframe map (`cl_automap*` cvars and `automap` command).
//!
//! Builds a clean architectural wireframe from the loaded BSP world model
//! (removing coplanar BSP split seams and sky ceilings), tracks explored PVS
//! areas as the player moves, and renders on the statistics screen (Tab/Select),
//! as an optional always-on HUD editor widget, or as a full-screen interactive
//! 3D orbitable wireframe map (`M`).

use qcore::cvar::Cvars;
use qcore::math::Vec3;
use qformats::Bsp;

use crate::draw::Canvas;
use crate::glyphs::Glyphs;

pub mod draw;
pub mod input;
pub mod mesh;
mod widget;

pub use draw::HeaderInfo;
use draw::Settings;
pub use mesh::MapMesh;

/// Maximum player breadcrumb positions kept per level.
const MAX_TRAIL: usize = 256;
/// Distance between consecutive breadcrumb points.
const TRAIL_STEP: f32 = 48.0;
/// Distance moved before re-checking PVS leaf exploration.
const EXPLORE_STEP: f32 = 16.0;

#[derive(Default)]
pub struct Automap {
    pub mesh: Option<MapMesh>,
    pub explored: Vec<bool>,
    last_leaf: Option<usize>,
    last_explore_pos: Vec3,
    pub trail: Vec<Vec3>,
    /// Interactive fullscreen 3D map overlay is currently open.
    pub fullscreen: bool,
    pub cam_focus: Vec3,
    pub cam_yaw: f32,
    pub cam_pitch: f32,
    pub cam_zoom: f32,
    pub fs_top_down: bool,
    pub fs_reveal: bool,
    drag_orbit: bool,
    drag_pan: bool,
    last_mouse: Option<(f32, f32)>,
    /// Held navigation keys while fullscreen automap is open (W, S, A, D, Q, E, Left, Right, Up, Down, LT, RT).
    held: [bool; 12],
}

impl Automap {
    /// Reads HUD automap settings from cvars.
    pub fn settings(cv: &Cvars) -> Settings {
        Settings {
            enabled: cv.value_of("cl_automap") != 0.0,
            top_down: cv.value_of("cl_automap_view").round() as i32 == 1,
            overlay: cv.value_of("cl_automap_display").round() as i32 == 1,
            rotate: cv.value_of("cl_automap_rotate") != 0.0,
            reveal: cv.value_of("cl_automap_reveal") != 0.0,
            zoom: cv.value_of("cl_automap_zoom").clamp(0.5, 3.0),
            alpha: cv.value_of("cl_automap_alpha").clamp(0.2, 1.0),
            markers: cv.value_of("cl_automap_markers") != 0.0,
        }
    }

    /// Rebuilds the cached wireframe mesh when the loaded map changes.
    pub fn sync_level(&mut self, mapname: &str, bsp: Option<&Bsp>) {
        if mapname.is_empty() || bsp.is_none() {
            if self.mesh.is_some() {
                *self = Self::default();
            }
            return;
        }
        if self.mesh.as_ref().is_some_and(|m| m.mapname == mapname) {
            return;
        }
        let bsp = bsp.unwrap();
        let m = mesh::build(mapname, bsp);
        let n = m.leaf_count;
        self.mesh = Some(m);
        self.explored = vec![false; n];
        self.last_leaf = None;
        self.trail.clear();
        self.close_fullscreen();
    }

    /// Updates PVS exploration and breadcrumb trail for the player's position.
    pub fn track_player(&mut self, bsp: Option<&Bsp>, pos: Vec3) {
        let Some(bsp) = bsp else { return };
        if self.mesh.is_none() {
            return;
        }
        let moved = (pos - self.last_explore_pos).length();
        let leaf = mesh::point_in_leaf(bsp, pos);
        if self.last_leaf != Some(leaf) || moved >= EXPLORE_STEP {
            mesh::mark_visible_leafs(bsp, pos, &mut self.explored);
            self.last_leaf = Some(leaf);
            self.last_explore_pos = pos;
        }
        match self.trail.last() {
            Some(&last) if (pos - last).length() < TRAIL_STEP => {}
            Some(&last) if (pos - last).length() > 512.0 => {
                // Teleported: push new point without long line across the map? Keep trail fresh.
                self.trail.push(pos);
                if self.trail.len() > MAX_TRAIL {
                    self.trail.remove(0);
                }
            }
            _ => {
                self.trail.push(pos);
                if self.trail.len() > MAX_TRAIL {
                    self.trail.remove(0);
                }
            }
        }
    }

    pub fn open_fullscreen(&mut self, player_pos: Vec3, player_yaw: f32, cv: &Cvars) {
        if self.mesh.is_none() {
            return;
        }
        let s = Self::settings(cv);
        self.fullscreen = true;
        self.fs_top_down = s.top_down;
        self.fs_reveal = s.reveal;
        self.cam_zoom = s.zoom;
        self.recenter(player_pos, if s.rotate { player_yaw } else { 90.0 });
    }

    pub fn close_fullscreen(&mut self) {
        self.fullscreen = false;
        self.drag_orbit = false;
        self.drag_pan = false;
        self.last_mouse = None;
        self.held = [false; 12];
    }

    pub fn toggle_fullscreen(&mut self, player_pos: Vec3, player_yaw: f32, cv: &Cvars) {
        if self.fullscreen {
            self.close_fullscreen();
        } else {
            self.open_fullscreen(player_pos, player_yaw, cv);
        }
    }

    pub fn recenter(&mut self, player_pos: Vec3, yaw: f32) {
        self.cam_focus = player_pos;
        self.cam_yaw = yaw;
        self.cam_pitch = 38.0;
    }

    /// Renders either the interactive fullscreen map or the statistics-screen automap.
    #[allow(clippy::too_many_arguments)]
    pub fn draw(
        &self,
        c: &mut Canvas,
        cv: &Cvars,
        player_pos: Vec3,
        player_yaw: f32,
        top_reserve: f32,
        reserve_bottom: f32,
        header: HeaderInfo<'_>,
        pad_ui: bool,
        glyphs: &Glyphs,
    ) {
        let Some(mesh) = &self.mesh else { return };
        let set = Self::settings(cv);
        if self.fullscreen {
            draw::draw_fullscreen(
                c,
                mesh,
                &self.explored,
                &self.trail,
                player_pos,
                player_yaw,
                self.cam_focus,
                self.cam_yaw,
                self.cam_pitch,
                self.cam_zoom,
                self.fs_top_down,
                self.fs_reveal,
                set.markers,
                header,
                pad_ui,
                glyphs,
            );
        } else {
            draw::draw_scores(c, mesh, &self.explored, &self.trail, player_pos, player_yaw, set, top_reserve, reserve_bottom);
        }
    }

    /// Renders the live automap inside a HUD editor widget box `(0..w, 0..h)`.
    pub fn draw_widget(&self, c: &mut Canvas, size: (f32, f32), player_pos: Vec3, player_yaw: f32, set: Settings, framed: bool) {
        if let Some(mesh) = &self.mesh {
            widget::draw_widget(c, mesh, &self.explored, &self.trail, size, player_pos, player_yaw, set, framed);
        }
    }

    /// Renders the HUD editor preview (real level if loaded, else sample wireframe).
    pub fn draw_widget_preview(&self, c: &mut Canvas, size: (f32, f32), player_pos: Vec3, player_yaw: f32, set: Settings, framed: bool) {
        if let Some(mesh) = &self.mesh {
            widget::draw_widget(c, mesh, &self.explored, &self.trail, size, player_pos, player_yaw, set, framed);
        } else {
            widget::draw_sample_widget(c, size, set, framed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use qformats::bsp::{Face, Leaf, Model, MipTex, Node, Plane, TexInfo, TextureKind};

    fn synthetic_room_bsp() -> Bsp {
        // Simple 128x128x64 box room centered at (0,0,32) with one floor split into two coplanar faces.
        let vertexes = vec![
            [-64.0, -64.0, 0.0],  // 0
            [0.0, -64.0, 0.0],    // 1
            [64.0, -64.0, 0.0],   // 2
            [64.0, 64.0, 0.0],    // 3
            [0.0, 64.0, 0.0],     // 4
            [-64.0, 64.0, 0.0],   // 5
            [-64.0, -64.0, 64.0], // 6
            [64.0, -64.0, 64.0],  // 7
        ];
        // Edges: 1..=9 (edge 0 is dummy in Quake BSP)
        let edges = vec![
            [0, 0],
            [0, 1], // 1: south-west floor edge
            [1, 4], // 2: interior coplanar floor split seam (should be filtered!)
            [4, 5], // 3: north-west floor edge
            [5, 0], // 4: west floor edge
            [1, 2], // 5: south-east floor edge
            [2, 3], // 6: east floor edge
            [3, 4], // 7: north-east floor edge
            [0, 6], // 8: vertical corner
            [2, 7], // 9: vertical corner
            [6, 7], // 10: south top edge
            [0, 2], // 11: south bottom edge for wall
        ];
        let surfedges = vec![
            // Face 0 (west floor): edges 1, 2, 3, 4
            1, 2, 3, 4,
            // Face 1 (east floor): edges 5, 6, 7, -2 (shares edge 2 coplanarly!)
            5, 6, 7, -2,
            // Face 2 (south wall, normal +Y): edges 11, 9, -10, -8
            11, 9, -10, -8,
        ];
        let planes = vec![
            Plane { normal: [0.0, 0.0, 1.0], dist: 0.0, kind: 2 },
            Plane { normal: [0.0, 1.0, 0.0], dist: -64.0, kind: 1 },
        ];
        let textures = vec![Some(MipTex {
            name: "wall".into(),
            width: 64,
            height: 64,
            kind: TextureKind::Normal,
            mips: vec![],
            shift: 0,
        })];
        let texinfo = vec![TexInfo { vecs: [[0.0; 4]; 2], miptex: 0, flags: 0 }];
        let faces = vec![
            Face { plane: 0, side: 0, first_edge: 0, num_edges: 4, texinfo: 0, styles: [0; 4], lightofs: -1 },
            Face { plane: 0, side: 0, first_edge: 4, num_edges: 4, texinfo: 0, styles: [0; 4], lightofs: -1 },
            Face { plane: 1, side: 0, first_edge: 8, num_edges: 4, texinfo: 0, styles: [0; 4], lightofs: -1 },
        ];
        let leafs = vec![
            Leaf { contents: -2, visofs: -1, mins: [0.0; 3], maxs: [0.0; 3], first_marksurface: 0, num_marksurfaces: 0, ambient_level: [0; 4] },
            Leaf { contents: -1, visofs: -1, mins: [-64.0, -64.0, 0.0], maxs: [64.0, 64.0, 64.0], first_marksurface: 0, num_marksurfaces: 3, ambient_level: [0; 4] },
        ];
        let nodes = vec![Node {
            plane: 0,
            children: [-2, -2], // both point to leaf 1
            mins: [-64.0, -64.0, 0.0],
            maxs: [64.0, 64.0, 64.0],
            first_face: 0,
            num_faces: 3,
        }];
        let models = vec![Model {
            mins: [-64.0, -64.0, 0.0],
            maxs: [64.0, 64.0, 64.0],
            origin: [0.0; 3],
            headnode: [0, 0, 0, 0],
            visleafs: 1,
            first_face: 0,
            num_faces: 3,
        }];
        Bsp {
            entities: "{\n\"classname\" \"worldspawn\"\n}\n{\n\"classname\" \"item_key1\"\n\"origin\" \"16 16 16\"\n}\n".into(),
            planes,
            textures,
            vertexes,
            nodes,
            texinfo,
            faces,
            leafs,
            marksurfaces: vec![0, 1, 2],
            edges,
            surfedges,
            models,
            ..Default::default()
        }
    }

    #[test]
    fn coplanar_bsp_split_seam_is_removed_and_markers_parsed() {
        let bsp = synthetic_room_bsp();
        let m = mesh::build("test", &bsp);
        // Edge 2 ([1, 4]: (0,-64,0)->(0,64,0)) is shared by two coplanar floor faces -> filtered out!
        let has_seam = m.edges.iter().any(|e| {
            (e.a == Vec3::new(0.0, -64.0, 0.0) && e.b == Vec3::new(0.0, 64.0, 0.0))
                || (e.b == Vec3::new(0.0, -64.0, 0.0) && e.a == Vec3::new(0.0, 64.0, 0.0))
        });
        assert!(!has_seam, "coplanar interior BSP split edge should be removed");
        // Perimeter and wall edges remain (10 edges: 1,3,4,5,6,7,8,9,10,11).
        assert_eq!(m.edges.len(), 10);
        assert_eq!(m.markers.len(), 1);
        assert_eq!(m.markers[0].kind, mesh::MarkerKind::KeySilver);
    }

    #[test]
    fn liang_barsky_clips_segments_to_rect() {
        let rect = [10.0, 10.0, 100.0, 100.0];
        let (a, b) = draw::clip_rect([0.0, 60.0], [200.0, 60.0], rect).unwrap();
        assert!((a[0] - 10.0).abs() < 1e-3 && (b[0] - 110.0).abs() < 1e-3);
        assert!(draw::clip_rect([0.0, 0.0], [5.0, 5.0], rect).is_none());
    }

    #[test]
    fn exploration_and_fullscreen_controls_work() {
        let bsp = synthetic_room_bsp();
        let mut am = Automap::default();
        let mut cv = Cvars::new();
        cv.register("cl_automap", "1", 0);
        cv.register("cl_automap_view", "0", 0);
        cv.register("cl_automap_display", "0", 0);
        cv.register("cl_automap_rotate", "1", 0);
        cv.register("cl_automap_reveal", "0", 0);
        cv.register("cl_automap_zoom", "1", 0);
        cv.register("cl_automap_alpha", "0.85", 0);
        cv.register("cl_automap_markers", "1", 0);

        am.sync_level("test", Some(&bsp));
        assert_eq!(am.mesh.as_ref().unwrap().explored_percent(&am.explored), 0);
        am.track_player(Some(&bsp), Vec3::new(0.0, 0.0, 16.0));
        assert_eq!(am.mesh.as_ref().unwrap().explored_percent(&am.explored), 100);

        am.toggle_fullscreen(Vec3::new(0.0, 0.0, 16.0), 45.0, &cv);
        assert!(am.fullscreen);
        assert!(!am.fs_top_down);
        am.key(qinput::keys::K_TAB, true, Vec3::ZERO, 0.0);
        assert!(am.fs_top_down);
        am.key(qinput::keys::K_ESCAPE, true, Vec3::ZERO, 0.0);
        assert!(!am.fullscreen);
    }

    #[test]
    fn scores_and_widget_draw_without_overlapping_top_scorebar() {
        let bsp = synthetic_room_bsp();
        let mut am = Automap::default();
        am.sync_level("test", Some(&bsp));
        am.track_player(Some(&bsp), Vec3::new(0.0, 0.0, 16.0));
        let set = Settings { enabled: true, ..Settings::default() };

        // Statistics-screen framed panel stays below the 24px top scorebar (top_reserve = 30).
        let mut c = Canvas::new(1280, 720);
        draw::draw_scores(&mut c, am.mesh.as_ref().unwrap(), &am.explored, &am.trail, Vec3::ZERO, 0.0, set, 30.0, 0.0);
        assert!(!c.list.is_empty());
        let min_y = c.list.iter().filter_map(|d| match *d {
            qrender::Draw2D::Fill { y, .. } => Some(y / c.scale),
            qrender::Draw2D::Line { y0, y1, .. } => Some(y0.min(y1) / c.scale),
            _ => None,
        }).fold(f32::INFINITY, f32::min);
        assert!(min_y >= 29.0, "statistics automap overlapped top scorebar: min_y={min_y}");

        // HUD editor preview works even when no map is loaded.
        let empty = Automap::default();
        let mut c2 = Canvas::new(1280, 720);
        empty.draw_widget_preview(&mut c2, (120.0, 88.0), Vec3::ZERO, 0.0, set, true);
        assert!(c2.list.iter().any(|d| matches!(d, qrender::Draw2D::Line { .. })));
    }
}
