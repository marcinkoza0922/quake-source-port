//! 3D wireframe and top-down automap rendering (corner minimap, center overlay,
//! and interactive fullscreen map).

use qcore::math::Vec3;
use qinput::keys::*;

use crate::draw::Canvas;
use crate::glyphs::Glyphs;

use super::mesh::{EdgeKind, MapEdge, MapMesh, MarkerKind};

/// Rendering parameters copied from cvars by the host each frame.
#[derive(Clone, Copy, Debug)]
pub struct Settings {
    pub enabled: bool,
    pub top_down: bool,
    pub overlay: bool,
    pub rotate: bool,
    pub reveal: bool,
    pub zoom: f32,
    pub alpha: f32,
    pub markers: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            top_down: false,
            overlay: false,
            rotate: true,
            reveal: false,
            zoom: 1.0,
            alpha: 0.85,
            markers: true,
        }
    }
}

/// Level metadata shown in the fullscreen automap header.
#[derive(Clone, Copy, Debug, Default)]
pub struct HeaderInfo<'a> {
    pub title: &'a str,
    pub kills: i32,
    pub total_kills: i32,
    pub secrets: i32,
    pub total_secrets: i32,
}

/// Camera and projection state for one automap viewport.
pub struct Projector {
    rect: [f32; 4],
    center: [f32; 2],
    focus: Vec3,
    eye: Vec3,
    right: Vec3,
    up: Vec3,
    dir: Vec3,
    focal: f32,
    ortho_scale: f32,
    top_down: bool,
    max_dist: f32,
}

const NEAR_Z: f32 = 24.0;
pub(super) const BORDER: [f32; 4] = [0.75, 0.58, 0.30, 0.85];
pub(super) const GOLD: [f32; 4] = [0.98, 0.80, 0.35, 1.0];
const FG: [f32; 4] = [0.92, 0.90, 0.85, 1.0];
const MUTED: [f32; 4] = [0.68, 0.66, 0.60, 0.95];

impl Projector {
    pub fn new(rect: [f32; 4], focus: Vec3, yaw_deg: f32, pitch_deg: f32, zoom: f32, top_down: bool) -> Self {
        let zoom = zoom.clamp(0.25, 6.0);
        let yaw = yaw_deg.to_radians();
        let pitch = if top_down { std::f32::consts::FRAC_PI_2 } else { pitch_deg.clamp(8.0, 89.0).to_radians() };
        let f_xy = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let right = Vec3::new(yaw.sin(), -yaw.cos(), 0.0);
        let z_up = Vec3::new(0.0, 0.0, 1.0);
        let dir = f_xy * pitch.cos() - z_up * pitch.sin();
        let up = f_xy * pitch.sin() + z_up * pitch.cos();
        let cam_dist = 960.0 / zoom;
        let eye = focus - dir * cam_dist;
        let dim = rect[2].min(rect[3]).max(1.0);
        Self {
            rect,
            center: [rect[0] + rect[2] * 0.5, rect[1] + rect[3] * 0.5],
            focus,
            eye,
            right,
            up,
            dir,
            focal: dim * 0.90,
            ortho_scale: dim / (1450.0 / zoom),
            top_down,
            max_dist: 2400.0 / zoom.sqrt(),
        }
    }

    /// Projects a single world point to virtual screen coordinates if in front of near plane.
    pub fn project_point(&self, p: Vec3) -> Option<[f32; 2]> {
        if self.top_down {
            let rel = p - self.focus;
            return Some([
                self.center[0] + rel.dot(self.right) * self.ortho_scale,
                self.center[1] - rel.dot(self.up) * self.ortho_scale,
            ]);
        }
        let rel = p - self.eye;
        let cz = rel.dot(self.dir);
        if cz < NEAR_Z {
            return None;
        }
        let cx = rel.dot(self.right);
        let cy = rel.dot(self.up);
        Some([self.center[0] + (cx / cz) * self.focal, self.center[1] - (cy / cz) * self.focal])
    }

    /// Projects and clips a 3D world line segment against the near plane and viewport rect.
    pub fn project_segment(&self, a: Vec3, b: Vec3) -> Option<([f32; 2], [f32; 2])> {
        let (p0, p1) = if self.top_down {
            let ra = a - self.focus;
            let rb = b - self.focus;
            (
                [self.center[0] + ra.dot(self.right) * self.ortho_scale, self.center[1] - ra.dot(self.up) * self.ortho_scale],
                [self.center[0] + rb.dot(self.right) * self.ortho_scale, self.center[1] - rb.dot(self.up) * self.ortho_scale],
            )
        } else {
            let ra = a - self.eye;
            let rb = b - self.eye;
            let mut ca = [ra.dot(self.right), ra.dot(self.up), ra.dot(self.dir)];
            let mut cb = [rb.dot(self.right), rb.dot(self.up), rb.dot(self.dir)];
            if ca[2] < NEAR_Z && cb[2] < NEAR_Z {
                return None;
            }
            if ca[2] < NEAR_Z {
                let t = (NEAR_Z - ca[2]) / (cb[2] - ca[2]);
                ca = [ca[0] + (cb[0] - ca[0]) * t, ca[1] + (cb[1] - ca[1]) * t, NEAR_Z];
            } else if cb[2] < NEAR_Z {
                let t = (NEAR_Z - cb[2]) / (ca[2] - cb[2]);
                cb = [cb[0] + (ca[0] - cb[0]) * t, cb[1] + (ca[1] - cb[1]) * t, NEAR_Z];
            }
            (
                [self.center[0] + (ca[0] / ca[2]) * self.focal, self.center[1] - (ca[1] / ca[2]) * self.focal],
                [self.center[0] + (cb[0] / cb[2]) * self.focal, self.center[1] - (cb[1] / cb[2]) * self.focal],
            )
        };
        clip_rect(p0, p1, self.rect)
    }

    pub fn in_rect(&self, p: [f32; 2]) -> bool {
        p[0] >= self.rect[0]
            && p[0] <= self.rect[0] + self.rect[2]
            && p[1] >= self.rect[1]
            && p[1] <= self.rect[1] + self.rect[3]
    }
}

/// Liang-Barsky 2D line clipping against `[x, y, w, h]`.
pub fn clip_rect(a: [f32; 2], b: [f32; 2], rect: [f32; 4]) -> Option<([f32; 2], [f32; 2])> {
    let (xmin, ymin, xmax, ymax) = (rect[0], rect[1], rect[0] + rect[2], rect[1] + rect[3]);
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;
    for (p, q) in [(-dx, a[0] - xmin), (dx, xmax - a[0]), (-dy, a[1] - ymin), (dy, ymax - a[1])] {
        if p.abs() < 1e-6 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                if r > t1 { return None; }
                t0 = t0.max(r);
            } else {
                if r < t0 { return None; }
                t1 = t1.min(r);
            }
        }
    }
    Some(([a[0] + dx * t0, a[1] + dy * t0], [a[0] + dx * t1, a[1] + dy * t1]))
}

fn edge_color(edge: &MapEdge, focus: Vec3, max_dist: f32, base_alpha: f32) -> Option<[f32; 4]> {
    let mid = (edge.a + edge.b) * 0.5;
    let dist = (mid - focus).length();
    if dist > max_dist {
        return None;
    }
    let dist_fade = (1.0 - (dist / max_dist).powf(1.6)).clamp(0.12, 1.0);
    let dz = edge.mid_z - focus.z;
    let z_fade = (1.0 - (dz.abs() / 640.0).min(0.75)).clamp(0.22, 1.0);
    let a = (base_alpha * dist_fade * z_fade).clamp(0.06, 1.0);
    let rgb = match edge.kind {
        EdgeKind::Solid => {
            if dz.abs() < 72.0 {
                [0.48, 0.92, 0.96]
            } else if dz >= 72.0 {
                [0.48, 0.58, 0.95]
            } else {
                [0.24, 0.56, 0.64]
            }
        }
        EdgeKind::Water => [0.24, 0.62, 1.0],
        EdgeKind::Slime => [0.36, 0.92, 0.36],
        EdgeKind::Lava => [1.0, 0.42, 0.16],
        EdgeKind::Teleporter => [0.88, 0.42, 1.0],
        EdgeKind::DoorSilver => [0.80, 0.95, 1.0],
        EdgeKind::DoorGold => [1.0, 0.82, 0.24],
    };
    Some([rgb[0], rgb[1], rgb[2], a])
}

#[allow(clippy::too_many_arguments)]
pub(super) fn draw_wireframe(
    c: &mut Canvas,
    mesh: &MapMesh,
    explored: &[bool],
    proj: &Projector,
    reveal: bool,
    alpha: f32,
    line_w: f32,
    markers: bool,
    labels: bool,
    trail: &[Vec3],
    player_pos: Vec3,
    player_yaw: f32,
) {
    // Draw geometry edges.
    for edge in &mesh.edges {
        if !reveal && !mesh.edge_explored(edge, explored) {
            continue;
        }
        let Some(col) = edge_color(edge, proj.focus, proj.max_dist, alpha) else { continue };
        let Some((p0, p1)) = proj.project_segment(edge.a, edge.b) else { continue };
        let w = if matches!(edge.kind, EdgeKind::DoorSilver | EdgeKind::DoorGold) { line_w * 1.4 } else { line_w };
        c.line(p0[0], p0[1], p1[0], p1[1], w, col);
    }

    // Player breadcrumb trail.
    if markers && trail.len() >= 2 {
        let total = trail.len() as f32;
        for (i, w) in trail.windows(2).enumerate() {
            if (w[1] - w[0]).length() > 512.0 {
                continue;
            }
            let fade = ((i + 1) as f32 / total).powf(1.2);
            if let Some((p0, p1)) = proj.project_segment(w[0], w[1]) {
                c.line(p0[0], p0[1], p1[0], p1[1], line_w * 1.2, [1.0, 0.78, 0.28, alpha * 0.55 * fade]);
            }
        }
    }

    // Static landmarks (keys, exits, teleporters).
    if markers {
        for m in &mesh.markers {
            if !reveal && !mesh.marker_explored(m, explored) {
                continue;
            }
            let Some(p) = proj.project_point(m.pos) else { continue };
            if !proj.in_rect(p) {
                continue;
            }
            let (col, tag) = match m.kind {
                MarkerKind::Exit => ([0.35, 1.0, 0.45, alpha], "EXIT"),
                MarkerKind::KeySilver => ([0.78, 0.94, 1.0, alpha], "KEY"),
                MarkerKind::KeyGold => ([1.0, 0.84, 0.20, alpha], "KEY"),
                MarkerKind::Teleporter => ([0.88, 0.45, 1.0, alpha], "TELE"),
            };
            let r = if labels { 3.5 } else { 2.5 };
            c.line(p[0], p[1] - r, p[0] + r, p[1], line_w * 1.3, col);
            c.line(p[0] + r, p[1], p[0], p[1] + r, line_w * 1.3, col);
            c.line(p[0], p[1] + r, p[0] - r, p[1], line_w * 1.3, col);
            c.line(p[0] - r, p[1], p[0], p[1] - r, line_w * 1.3, col);
            if labels {
                c.text_raw(p[0] + r + 2.0, p[1] - 3.0, tag, 6.0, col);
            }
        }
    }

    // Player directional chevron arrow in world space.
    let yaw = player_yaw.to_radians();
    let fwd = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
    let side = Vec3::new(yaw.sin(), -yaw.cos(), 0.0);
    let sz = if labels { 34.0 } else { 28.0 };
    let tip = player_pos + fwd * sz;
    let left = player_pos - fwd * (sz * 0.65) - side * (sz * 0.55);
    let right = player_pos - fwd * (sz * 0.65) + side * (sz * 0.55);
    let tail = player_pos - fwd * (sz * 0.25);
    let pcol = [1.0, 0.88, 0.24, 1.0];
    for (a, b) in [(tip, left), (tip, right), (left, tail), (right, tail)] {
        if let Some((p0, p1)) = proj.project_segment(a, b) {
            c.line(p0[0], p0[1], p1[0], p1[1], (line_w * 1.6).max(1.5), pcol);
        }
    }
}

pub(super) fn draw_compass(c: &mut Canvas, cx: f32, cy: f32, r: f32, yaw_deg: f32, alpha: f32) {
    // World north is +Y (yaw = 90°). Screen offset for world north relative to camera yaw:
    let a = (90.0 - yaw_deg).to_radians();
    let tip = (r - 4.0).max(3.0);
    let nx = cx + a.sin() * tip;
    let ny = cy - a.cos() * tip;
    c.line(cx, cy, nx, ny, 1.3, [1.0, 0.42, 0.32, alpha]);
    let lx = cx + a.sin() * (r + 1.5);
    let ly = cy - a.cos() * (r + 1.5);
    c.text_raw(lx - 2.5, ly - 3.5, "N", 6.5, [1.0, 0.78, 0.38, alpha]);
}

/// Draws the automap on the statistics screen (while holding Tab / Select).
#[allow(clippy::too_many_arguments)]
pub fn draw_scores(
    c: &mut Canvas,
    mesh: &MapMesh,
    explored: &[bool],
    trail: &[Vec3],
    player_pos: Vec3,
    player_yaw: f32,
    set: Settings,
    top_reserve: f32,
    reserve_bottom: f32,
) {
    if !set.enabled || mesh.edges.is_empty() {
        return;
    }
    let vw = c.vw();
    let y0 = top_reserve;
    let avail_h = (c.vh() - reserve_bottom - y0 - 8.0).max(60.0);
    let yaw = if set.rotate { player_yaw } else { 90.0 };
    let line_w = (c.scale * 0.45).clamp(1.0, 2.0);

    if set.overlay {
        let w = (vw * 0.72).min(460.0);
        let h = (avail_h * 0.88).min(260.0);
        let rect = [(vw - w) * 0.5, y0 + (avail_h - h) * 0.5, w, h];
        let proj = Projector::new(rect, player_pos, yaw, 38.0, set.zoom, set.top_down);
        draw_wireframe(c, mesh, explored, &proj, set.reveal, set.alpha * 0.9, line_w, set.markers, false, trail, player_pos, player_yaw);
        return;
    }

    // Framed centered panel on the statistics screen.
    let w = (vw * 0.68).clamp(160.0, 340.0);
    let h = (avail_h * 0.84).clamp(96.0, 180.0);
    let x = (vw - w) * 0.5;
    let y = y0 + (avail_h - h) * 0.5;
    let rect = [x + 2.0, y + 2.0, w - 4.0, h - 4.0];
    c.round_box(x, y, w, h, 3.0, [0.03, 0.04, 0.06, set.alpha * 0.78], Some([BORDER[0], BORDER[1], BORDER[2], set.alpha * 0.75]));
    let proj = Projector::new(rect, player_pos, yaw, 38.0, set.zoom, set.top_down);
    draw_wireframe(c, mesh, explored, &proj, set.reveal, set.alpha, line_w, set.markers, false, trail, player_pos, player_yaw);
    draw_compass(c, x + w - 13.0, y + 13.0, 8.0, yaw, set.alpha);
    let mode_tag = if set.top_down { "2D" } else { "3D" };
    c.text_raw(x + 4.0, y + 3.0, mode_tag, 6.0, [GOLD[0], GOLD[1], GOLD[2], set.alpha * 0.9]);
    let pct = if set.reveal { 100 } else { mesh.explored_percent(explored) };
    let pct_str = format!("{pct}%");
    let pw = c.text_w_raw(&pct_str, 6.0);
    c.text_raw(x + w - pw - 4.0, y + h - 8.5, &pct_str, 6.0, [MUTED[0], MUTED[1], MUTED[2], set.alpha * 0.9]);
}

/// Draws the interactive full-screen 3D wireframe map.
#[allow(clippy::too_many_arguments)]
pub fn draw_fullscreen(
    c: &mut Canvas,
    mesh: &MapMesh,
    explored: &[bool],
    trail: &[Vec3],
    player_pos: Vec3,
    player_yaw: f32,
    focus: Vec3,
    yaw_deg: f32,
    pitch_deg: f32,
    zoom: f32,
    top_down: bool,
    reveal: bool,
    markers: bool,
    header: HeaderInfo<'_>,
    pad_ui: bool,
    glyphs: &Glyphs,
) {
    let base_scale = c.scale;
    c.menu_layout();
    // Dim whole screen.
    c.fill(-c.ox, -c.oy, c.vw(), c.vh(), [0.02, 0.03, 0.05, 0.88]);

    // Map viewport box.
    let rect = [14.0, 26.0, 612.0, 304.0];
    c.round_box(rect[0], rect[1], rect[2], rect[3], 4.0, [0.03, 0.04, 0.07, 0.82], Some(BORDER));

    let proj = Projector::new([rect[0] + 2.0, rect[1] + 2.0, rect[2] - 4.0, rect[3] - 4.0], focus, yaw_deg, pitch_deg, zoom, top_down);
    let line_w = (c.scale * 0.5).clamp(1.0, 2.2);
    draw_wireframe(c, mesh, explored, &proj, reveal, 0.95, line_w, markers, true, trail, player_pos, player_yaw);

    // Subtle focus crosshair when panned away from the player.
    if (focus - player_pos).length() > 32.0 {
        let (cx, cy) = (proj.center[0], proj.center[1]);
        let col = [1.0, 1.0, 1.0, 0.35];
        c.line(cx - 5.0, cy, cx + 5.0, cy, 1.0, col);
        c.line(cx, cy - 5.0, cx, cy + 5.0, 1.0, col);
    }

    // Header bar.
    let title = if header.title.is_empty() { mesh.mapname.as_str() } else { header.title };
    c.text_raw(16.0, 10.0, title, 10.0, GOLD);
    let pct = if reveal { 100 } else { mesh.explored_percent(explored) };
    let mode = if top_down { qcore::tr!("Top-down") } else { qcore::tr!("3D wireframe") };
    let stats = format!(
        "{}  |  {}: {}/{}  |  {}: {}/{}  |  {}%",
        mode,
        qcore::tr!("Kills"),
        header.kills,
        header.total_kills,
        qcore::tr!("Secrets"),
        header.secrets,
        header.total_secrets,
        pct
    );
    let sw = c.text_w_raw(&stats, 8.0);
    c.text_raw(624.0 - sw, 11.0, &stats, 8.0, FG);

    // Compass in top-right of map box.
    draw_compass(c, rect[0] + rect[2] - 20.0, rect[1] + 20.0, 12.0, yaw_deg, 0.95);

    // Compact legend in top-left of map box.
    let legend = [
        ([0.48, 0.92, 0.96, 0.95], qcore::t!("Current floor")),
        ([0.48, 0.58, 0.95, 0.95], qcore::t!("Above")),
        ([0.24, 0.56, 0.64, 0.95], qcore::t!("Below")),
        ([0.35, 1.0, 0.45, 0.95], qcore::t!("Exit")),
        ([1.0, 0.84, 0.20, 0.95], qcore::t!("Keys")),
    ];
    c.round_box(rect[0] + 6.0, rect[1] + 6.0, 74.0, legend.len() as f32 * 9.0 + 6.0, 3.0, [0.0, 0.0, 0.0, 0.55], Some([BORDER[0], BORDER[1], BORDER[2], 0.45]));
    for (i, &(col, label)) in legend.iter().enumerate() {
        let ly = rect[1] + 10.0 + i as f32 * 9.0;
        c.fill(rect[0] + 10.0, ly + 2.0, 6.0, 3.0, col);
        c.text(rect[0] + 20.0, ly, label, 6.5, MUTED);
    }

    // Bottom control hints.
    let hy = 338.0;
    if pad_ui {
        let items = [
            (K_LTHUMB, qcore::t!("Pan")),
            (K_RTHUMB, qcore::t!("Orbit")),
            (K_LTRIGGER, qcore::t!("Zoom")),
            (K_YBUTTON, qcore::t!("View")),
            (K_XBUTTON, qcore::t!("Reveal")),
            (K_LSHOULDER, qcore::t!("Reset")),
            (K_BBUTTON, qcore::t!("Close")),
        ];
        let w = glyphs.hints(c, 0.0, hy, &items, false);
        c.round_box(320.0 - w * 0.5 - 8.0, hy - 3.0, w + 16.0, crate::glyphs::HINT_H + 6.0, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        glyphs.hints_centered(c, 320.0, hy, &items);
    } else {
        let hint = qcore::t!("Drag: Orbit  |  WASD: Pan  |  Q/E: Floor  |  Wheel: Zoom  |  Tab: 3D/2D  |  F: Reveal  |  R: Reset  |  M/Esc: Close");
        let w = c.text_w(hint, 7.5);
        c.round_box(320.0 - w * 0.5 - 8.0, hy - 2.0, w + 16.0, 14.0, 4.0, [0.0, 0.0, 0.0, 0.7], Some(BORDER));
        c.text(320.0 - w * 0.5, hy + 1.0, hint, 7.5, MUTED);
    }

    c.reset_origin();
    c.scale = base_scale;
}
