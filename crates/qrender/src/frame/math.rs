use super::*;

pub(super) fn ident() -> Mat4 {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

#[inline]
pub(super) fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut c = [0.0; 16];
    for col in 0..4 {
        let bc = col * 4;
        let b0 = b[bc];
        let b1 = b[bc + 1];
        let b2 = b[bc + 2];
        let b3 = b[bc + 3];
        for row in 0..4 {
            c[bc + row] = a[row] * b0 + a[4 + row] * b1 + a[8 + row] * b2 + a[12 + row] * b3;
        }
    }
    c
}

/// From a row-major 3x3 rotation and translation.
#[inline]
pub(super) fn affine(r: [[f32; 3]; 3], t: [f32; 3]) -> Mat4 {
    [
        r[0][0], r[1][0], r[2][0], 0.0,
        r[0][1], r[1][1], r[2][1], 0.0,
        r[0][2], r[1][2], r[2][2], 0.0,
        t[0], t[1], t[2], 1.0,
    ]
}

/// `affine(r, origin) * affine(diag(scale), scale_origin)` without a 4x4 matmul.
#[inline]
pub(super) fn alias_model_matrix(
    r: [[f32; 3]; 3],
    origin: [f32; 3],
    scale: [f32; 3],
    scale_origin: [f32; 3],
) -> Mat4 {
    let [sx, sy, sz] = scale;
    let [ox, oy, oz] = scale_origin;
    [
        r[0][0] * sx, r[1][0] * sx, r[2][0] * sx, 0.0,
        r[0][1] * sy, r[1][1] * sy, r[2][1] * sy, 0.0,
        r[0][2] * sz, r[1][2] * sz, r[2][2] * sz, 0.0,
        r[0][0] * ox + r[0][1] * oy + r[0][2] * oz + origin[0],
        r[1][0] * ox + r[1][1] * oy + r[1][2] * oz + origin[1],
        r[2][0] * ox + r[2][1] * oy + r[2][2] * oz + origin[2],
        1.0,
    ]
}

/// Quantized 16-angle Quake alias shade vector (`norm([cos(-ang), sin(-ang), 1])`).
#[inline]
pub(super) fn shade_vec(yaw: f32) -> [f32; 3] {
    static TABLE: std::sync::OnceLock<[[f32; 3]; 16]> = std::sync::OnceLock::new();
    let tbl = TABLE.get_or_init(|| {
        std::array::from_fn(|q| {
            let ang = q as f32 / 16.0 * std::f32::consts::TAU;
            norm([(-ang).cos(), (-ang).sin(), 1.0])
        })
    });
    tbl[((yaw * (16.0 / 360.0)) as i32 & 15) as usize]
}

pub(super) const IDENT3: [[f32; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// R_RotateForEntity: Rz(yaw) * Ry(-pitch) * Rx(roll).
#[inline]
pub(super) fn entity_rotation(angles: [f32; 3]) -> [[f32; 3]; 3] {
    if angles[0] == 0.0 && angles[2] == 0.0 {
        if angles[1] == 0.0 {
            return IDENT3;
        }
        let (sy, cy) = angles[1].to_radians().sin_cos();
        return [[cy, -sy, 0.0], [sy, cy, 0.0], [0.0, 0.0, 1.0]];
    }
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sp, cp) = (-angles[0]).to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    [
        [cy * cp, -sy * cr + cy * sp * sr, sy * sr + cy * sp * cr],
        [sy * cp, cy * cr + sy * sp * sr, -cy * sr + sy * sp * cr],
        [-sp, cp * sr, cp * cr],
    ]
}

/// AngleVectors: (forward, right, up).
pub fn angle_vectors(a: [f32; 3]) -> ([f32; 3], [f32; 3], [f32; 3]) {
    let (sy, cy) = a[1].to_radians().sin_cos();
    let (sp, cp) = a[0].to_radians().sin_cos();
    let (sr, cr) = a[2].to_radians().sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

pub(super) fn sub(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn add(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

pub(super) fn norm(a: [f32; 3]) -> [f32; 3] {
    let l = dot(a, a).sqrt();
    if l > 0.0 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        a
    }
}

pub(super) fn pack_color(c: [f32; 4]) -> u32 {
    let b = c.map(|v| (v.clamp(0.0, 1.0) * 255.0 + 0.5) as u32);
    b[0] | b[1] << 8 | b[2] << 16 | b[3] << 24
}

pub(super) struct ViewSetup {
    pub(super) vp: Mat4,
    pub(super) eye: [f32; 3],
    pub(super) fwd: [f32; 3],
    pub(super) right: [f32; 3],
    pub(super) up: [f32; 3],
    pub(super) frustum: [[f32; 4]; 4],
    /// Tangents of the half field of view (x, y).
    pub(super) tan: [f32; 2],
}

/// Rows of the 3x4 part of a column-major matrix.
pub(super) fn rows3(m: &Mat4) -> [[f32; 4]; 3] {
    std::array::from_fn(|r| [m[r], m[4 + r], m[8 + r], m[12 + r]])
}

pub(super) fn setup_view(v: &View, w: u32, h: u32) -> ViewSetup {
    let (fwd, right, up) = angle_vectors(v.angles);
    let [tx, ty] = crate::projection::half_fov_tangents(v, (w, h));
    let (n, f) = (4.0f32, 65536.0f32);
    // view: rows right, -up, forward
    let rows = [right, [-up[0], -up[1], -up[2]], fwd];
    let e = v.origin;
    let mut view = ident();
    for (r, row) in rows.iter().enumerate() {
        for c in 0..3 {
            view[c * 4 + r] = row[c];
        }
        view[12 + r] = -dot(*row, e);
    }
    let mut proj = [0.0; 16];
    proj[0] = 1.0 / tx;
    proj[5] = 1.0 / ty;
    proj[10] = f / (f - n);
    proj[11] = 1.0;
    proj[14] = -f * n / (f - n);
    let vp = mul(&proj, &view);
    let frustum = crate::projection::ViewFrustum::for_extent(v, w, h).planes;
    ViewSetup {
        vp,
        eye: e,
        fwd,
        right,
        up,
        frustum,
        tan: [tx, ty],
    }
}

pub(super) fn sphere_in_frustum(f: &[[f32; 4]; 4], c: [f32; 3], r: f32) -> bool {
    crate::projection::ViewFrustum { planes: *f }.contains_sphere(c, r)
}

pub(super) fn rotate3(r: &[[f32; 3]; 3], v: [f32; 3]) -> [f32; 3] {
    [
        r[0][0] * v[0] + r[0][1] * v[1] + r[0][2] * v[2],
        r[1][0] * v[0] + r[1][1] * v[1] + r[1][2] * v[2],
        r[2][0] * v[0] + r[2][1] * v[1] + r[2][2] * v[2],
    ]
}

fn min_angle_diff_rad(a: [f32; 3], b: [f32; 3]) -> f32 {
    let mut sum_deg = 0.0f32;
    for i in 0..3 {
        let d = (a[i] - b[i]).rem_euclid(360.0);
        sum_deg += d.min(360.0 - d);
    }
    sum_deg.to_radians()
}

/// Conservative screen-space visibility and motion check matching `post.frag`'s
/// velocity threshold so sub-pixel or tiny distant moving entities skip `t.vel`.
#[allow(clippy::too_many_arguments)]
pub(super) fn vel_entity_visible(
    vs: &ViewSetup,
    cur_center: [f32; 3],
    prev_center: [f32; 3],
    cur_angles: [f32; 3],
    prev_angles: [f32; 3],
    radius: f32,
    mb_scale: f32,
    view_px: [f32; 2],
    quality: u32,
) -> bool {
    let r = radius.max(0.0);
    let cur_rel = sub(cur_center, vs.eye);
    let prev_rel = sub(prev_center, vs.eye);
    let z_cur = dot(cur_rel, vs.fwd);
    let z_prev = dot(prev_rel, vs.fwd);
    if z_cur.max(z_prev) + r <= 4.0 {
        return false;
    }
    let z_min = (z_cur.min(z_prev) - r).max(4.0);
    let half_x = 0.5 * view_px[0] / vs.tan[0].max(1e-4);
    let half_y = 0.5 * view_px[1] / vs.tan[1].max(1e-4);
    let focal = half_x.max(half_y);
    let r_px = (r / z_min) * focal;
    let rot_disp = (r * min_angle_diff_rad(cur_angles, prev_angles)).min(2.0 * r);
    let d_center = sub(cur_center, prev_center);
    let dx = dot(d_center, vs.right).abs() + rot_disp;
    let dy = dot(d_center, vs.up).abs() + rot_disp;
    let dz = dot(d_center, vs.fwd).abs() + rot_disp;
    let vx = mb_scale * ((dx / z_min) * half_x + 0.5 * view_px[0] * (dz / z_min));
    let vy = mb_scale * ((dy / z_min) * half_y + 0.5 * view_px[1] * (dz / z_min));
    let blur_sq = vx * vx + vy * vy;
    if quality == 0 {
        blur_sq >= 1.0 && r_px >= 2.0
    } else {
        blur_sq >= 0.20 && r_px >= 0.5
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_view() -> ViewSetup {
        setup_view(
            &View {
                origin: [0.0, 0.0, 0.0],
                angles: [0.0, 0.0, 0.0],
                fov_x: 90.0,
                reserve_bottom: 0,
                speed_blur: 0.0,
                waterwarp: crate::WaterWarp::None,
                water_screen: Default::default(),
            },
            1280,
            720,
        )
    }

    #[test]
    fn vel_cull_keeps_visible_fast_entity() {
        let vs = test_view();
        // Monster at x=500 moving 4 units laterally (~5 px blur at 1280px, r ~ 40 px).
        assert!(vel_entity_visible(
            &vs,
            [500.0, 4.0, 0.0],
            [500.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            32.0,
            1.0,
            [1280.0, 720.0],
            1,
        ));
        assert!(vel_entity_visible(
            &vs,
            [500.0, 4.0, 0.0],
            [500.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            32.0,
            1.0,
            [1280.0, 720.0],
            0,
        ));
    }

    #[test]
    fn vel_cull_skips_subpixel_distant_entity() {
        let vs = test_view();
        // Tiny gib at x=2500 drifting 0.25 units (~0.06 px blur).
        assert!(!vel_entity_visible(
            &vs,
            [2500.0, 0.25, 0.0],
            [2500.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            8.0,
            1.0,
            [1280.0, 720.0],
            1,
        ));
    }

    #[test]
    fn vel_cull_tier_difference_on_small_entity() {
        let vs = test_view();
        // Radius ~1.3 px, blur ~0.8 px: kept on high, culled on low.
        let cur = [2500.0, 3.0, 0.0];
        let prev = [2500.0, 0.0, 0.0];
        assert!(vel_entity_visible(&vs, cur, prev, [0.0; 3], [0.0; 3], 5.0, 1.0, [1280.0, 720.0], 1));
        assert!(!vel_entity_visible(&vs, cur, prev, [0.0; 3], [0.0; 3], 5.0, 1.0, [1280.0, 720.0], 0));
    }

    #[test]
    fn vel_cull_keeps_rotating_entity_and_wraps_angles() {
        let vs = test_view();
        // Pure rotation with zero origin delta keeps entity when arc exceeds threshold.
        assert!(vel_entity_visible(
            &vs,
            [300.0, 0.0, 0.0],
            [300.0, 0.0, 0.0],
            [0.0, 5.0, 0.0],
            [0.0, 0.0, 0.0],
            32.0,
            1.0,
            [1280.0, 720.0],
            1,
        ));
        // 359.9 deg -> 0.0 deg is 0.1 deg wrap (~0.056u arc at r=32, ~0.13px), culled.
        assert!(!vel_entity_visible(
            &vs,
            [300.0, 0.0, 0.0],
            [300.0, 0.0, 0.0],
            [0.0, 359.9, 0.0],
            [0.0, 0.0, 0.0],
            32.0,
            1.0,
            [1280.0, 720.0],
            1,
        ));
    }

    #[test]
    fn vel_cull_skips_completely_behind_near_plane() {
        let vs = test_view();
        assert!(!vel_entity_visible(
            &vs,
            [-50.0, 10.0, 0.0],
            [-50.0, 0.0, 0.0],
            [0.0; 3],
            [0.0; 3],
            16.0,
            1.0,
            [1280.0, 720.0],
            1,
        ));
    }
}
