use super::*;

pub(super) fn ident() -> Mat4 {
    let mut m = [0.0; 16];
    m[0] = 1.0;
    m[5] = 1.0;
    m[10] = 1.0;
    m[15] = 1.0;
    m
}

pub(super) fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut c = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            c[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    c
}

/// From a row-major 3x3 rotation and translation.
pub(super) fn affine(r: [[f32; 3]; 3], t: [f32; 3]) -> Mat4 {
    let mut m = ident();
    for (row, rr) in r.iter().enumerate() {
        for (col, v) in rr.iter().enumerate() {
            m[col * 4 + row] = *v;
        }
    }
    m[12] = t[0];
    m[13] = t[1];
    m[14] = t[2];
    m
}

pub(super) fn mul3(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut c = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    c
}

/// R_RotateForEntity: Rz(yaw) * Ry(-pitch) * Rx(roll).
pub(super) fn entity_rotation(angles: [f32; 3]) -> [[f32; 3]; 3] {
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sp, cp) = (-angles[0]).to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    let rz = [[cy, -sy, 0.0], [sy, cy, 0.0], [0.0, 0.0, 1.0]];
    let ry = [[cp, 0.0, sp], [0.0, 1.0, 0.0], [-sp, 0.0, cp]];
    let rx = [[1.0, 0.0, 0.0], [0.0, cr, -sr], [0.0, sr, cr]];
    mul3(mul3(rz, ry), rx)
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
    // fov_adapt: fov_x is for 4:3, widened for the actual aspect
    let aspect = w.max(1) as f32 / h.max(1) as f32;
    let tx43 = (v.fov_x.clamp(1.0, 179.0).to_radians() * 0.5).tan();
    let ty = tx43 * 0.75;
    let tx = ty * aspect;
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
    let row = |i: usize| [vp[i], vp[4 + i], vp[8 + i], vp[12 + i]];
    let (r0, r1, r3) = (row(0), row(1), row(3));
    let mut frustum = [[0.0; 4]; 4];
    for (k, (a, s)) in [(r0, 1.0), (r0, -1.0), (r1, 1.0), (r1, -1.0)]
        .iter()
        .enumerate()
    {
        let p: [f32; 4] = std::array::from_fn(|i| r3[i] + s * a[i]);
        let l = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
        frustum[k] = p.map(|x| x / l);
    }
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
    f.iter()
        .all(|p| p[0] * c[0] + p[1] * c[1] + p[2] * c[2] + p[3] >= -r)
}
