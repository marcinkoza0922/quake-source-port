//! Always-visible HUD widget rendering for the `Automap` HUD editor element
//! (and its synthetic preview when editing the HUD from the title screen).

use qcore::math::Vec3;

use crate::draw::Canvas;

use super::draw::{BORDER, GOLD, Projector, Settings, draw_compass, draw_wireframe};
use super::mesh::{EdgeKind, MapEdge, MapMesh};

/// Draws the automap inside a HUD element's local `(0..w, 0..h)` box.
/// The HUD renderer applies element position, scale, and opacity (`e.opacity`).
#[allow(clippy::too_many_arguments)]
pub fn draw_widget(
    c: &mut Canvas,
    mesh: &MapMesh,
    explored: &[bool],
    trail: &[Vec3],
    (w, h): (f32, f32),
    player_pos: Vec3,
    player_yaw: f32,
    set: Settings,
    framed: bool,
) {
    if mesh.edges.is_empty() {
        return;
    }
    let yaw = if set.rotate { player_yaw } else { 90.0 };
    let line_w = (c.scale * 0.45).clamp(1.0, 2.0);
    let rect = if framed {
        c.round_box(0.0, 0.0, w, h, 3.0, [0.03, 0.04, 0.06, 0.78], Some([BORDER[0], BORDER[1], BORDER[2], 0.75]));
        [2.0, 2.0, (w - 4.0).max(1.0), (h - 4.0).max(1.0)]
    } else {
        [0.0, 0.0, w, h]
    };
    let proj = Projector::new(rect, player_pos, yaw, 38.0, set.zoom, set.top_down);
    draw_wireframe(c, mesh, explored, &proj, set.reveal, 1.0, line_w, set.markers, false, trail, player_pos, player_yaw);
    if framed {
        draw_compass(c, w - 13.0, 13.0, 8.0, yaw, 1.0);
        let mode_tag = if set.top_down { "2D" } else { "3D" };
        c.text_raw(4.0, 3.0, mode_tag, 6.0, [GOLD[0], GOLD[1], GOLD[2], 0.9]);
    }
}

/// Draws a stylized wireframe preview when no level is loaded in the HUD editor.
pub fn draw_sample_widget(c: &mut Canvas, (w, h): (f32, f32), set: Settings, framed: bool) {
    let mesh = sample_mesh();
    let explored = [true];
    let trail = [Vec3::new(-180.0, -40.0, 0.0), Vec3::new(-90.0, -40.0, 0.0), Vec3::new(0.0, 0.0, 0.0)];
    draw_widget(c, &mesh, &explored, &trail, (w, h), Vec3::ZERO, 60.0, Settings { reveal: true, ..set }, framed);
}

fn sample_mesh() -> MapMesh {
    let mut edges = Vec::new();
    let mut box_room = |x0: f32, y0: f32, x1: f32, y1: f32, z0: f32, z1: f32, kind: EdgeKind| {
        let pts = [
            Vec3::new(x0, y0, z0), Vec3::new(x1, y0, z0), Vec3::new(x1, y1, z0), Vec3::new(x0, y1, z0),
            Vec3::new(x0, y0, z1), Vec3::new(x1, y0, z1), Vec3::new(x1, y1, z1), Vec3::new(x0, y1, z1),
        ];
        for (i, j) in [(0, 1), (1, 2), (2, 3), (3, 0), (4, 5), (5, 6), (6, 7), (7, 4), (0, 4), (1, 5), (2, 6), (3, 7)] {
            edges.push(MapEdge { a: pts[i], b: pts[j], mid_z: (pts[i].z + pts[j].z) * 0.5, kind, leafs: [0, 0] });
        }
    };
    // Central chamber, north corridor, raised balcony and lava pit.
    box_room(-140.0, -120.0, 140.0, 120.0, -24.0, 96.0, EdgeKind::Solid);
    box_room(-64.0, 120.0, 64.0, 320.0, -24.0, 80.0, EdgeKind::Solid);
    box_room(-280.0, -96.0, -140.0, 48.0, -24.0, 72.0, EdgeKind::Solid);
    box_room(140.0, -80.0, 260.0, 80.0, 48.0, 144.0, EdgeKind::Solid);
    box_room(-64.0, -88.0, 64.0, -24.0, -40.0, -24.0, EdgeKind::Water);
    MapMesh { mapname: String::new(), edges, markers: Vec::new(), leaf_count: 1, walkable_leafs: 1 }
}
