//! Small matrix/vector helpers for the wgpu backend (column-major 4x4).

pub(super) type Mat4 = [f32; 16];
pub(super) type V3 = [f32; 3];


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
pub(super) fn affine(r: [[f32; 3]; 3], t: V3) -> Mat4 {
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

pub(super) fn xform(m: &Mat4, p: V3) -> V3 {
    std::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r])
}

pub(super) fn xform_dir(m: &Mat4, p: V3) -> V3 {
    std::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2])
}

fn mul3(a: [[f32; 3]; 3], b: [[f32; 3]; 3]) -> [[f32; 3]; 3] {
    let mut c = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            c[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    c
}

/// R_RotateForEntity: Rz(yaw) * Ry(-pitch) * Rx(roll).
pub(super) fn entity_rotation(angles: V3) -> [[f32; 3]; 3] {
    let (sy, cy) = angles[1].to_radians().sin_cos();
    let (sp, cp) = (-angles[0]).to_radians().sin_cos();
    let (sr, cr) = angles[2].to_radians().sin_cos();
    let rz = [[cy, -sy, 0.0], [sy, cy, 0.0], [0.0, 0.0, 1.0]];
    let ry = [[cp, 0.0, sp], [0.0, 1.0, 0.0], [-sp, 0.0, cp]];
    let rx = [[1.0, 0.0, 0.0], [0.0, cr, -sr], [0.0, sr, cr]];
    mul3(mul3(rz, ry), rx)
}

/// AngleVectors: (forward, right, up).
pub(super) fn angle_vectors(a: V3) -> (V3, V3, V3) {
    let (sy, cy) = a[1].to_radians().sin_cos();
    let (sp, cp) = a[0].to_radians().sin_cos();
    let (sr, cr) = a[2].to_radians().sin_cos();
    (
        [cp * cy, cp * sy, -sp],
        [-sr * sp * cy + cr * sy, -sr * sp * sy - cr * cy, -sr * cp],
        [cr * sp * cy + sr * sy, cr * sp * sy - sr * cy, cr * cp],
    )
}

pub(super) fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

pub(super) fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

pub(super) fn norm(a: V3) -> V3 {
    let l = dot(a, a).sqrt();
    if l > 0.0 { a.map(|x| x / l) } else { a }
}

pub(super) struct ViewSetup {
    pub(super) vp: Mat4,
    pub(super) eye: V3,
    pub(super) fwd: V3,
    pub(super) right: V3,
    pub(super) up: V3,
    pub(super) frustum: [[f32; 4]; 4],
}

/// Like the Vulkan backend's setup_view, but for WebGPU clip space (y up).
pub(super) fn setup_view(v: &crate::View, w: u32, h: u32) -> ViewSetup {
    let (fwd, right, up) = angle_vectors(v.angles);
    let [tx, ty] = crate::projection::half_fov_tangents(v, (w, h));
    let (n, f) = (4.0f32, 65536.0f32);
    let rows = [right, up, fwd];
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
    ViewSetup {
        vp: mul(&proj, &view),
        eye: e,
        fwd,
        right,
        up,
        frustum: crate::projection::ViewFrustum::for_extent(v, w, h).planes,
    }
}

pub(super) fn sphere_in_frustum(f: &[[f32; 4]; 4], c: V3, r: f32) -> bool {
    crate::projection::ViewFrustum { planes: *f }.contains_sphere(c, r)
}

/// Quake vertex normals, parsed once from the shared GLSL table.
pub(super) fn anorms() -> &'static [V3] {
    static T: std::sync::OnceLock<Vec<V3>> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let src = include_str!("../../shaders/anorms.glsl");
        src.split("vec3(")
            .skip(1)
            .filter_map(|s| {
                let body = s.split(')').next()?;
                let v: Vec<f32> = body.split(',').filter_map(|x| x.trim().parse().ok()).collect();
                (v.len() == 3).then(|| [v[0], v[1], v[2]])
            })
            .collect()
    })
}

/// Inverse of `affine(r, t)` for a rotation `r`: `affine(rᵀ, -rᵀt)`.
pub(super) fn affine_inv(r: [[f32; 3]; 3], t: V3) -> Mat4 {
    let rt: [[f32; 3]; 3] = std::array::from_fn(|i| std::array::from_fn(|j| r[j][i]));
    affine(rt, std::array::from_fn(|i| -dot(rt[i], t)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn affine_inv_undoes_affine() {
        let (r, t) = (entity_rotation([10.0, 70.0, -25.0]), [12.0, -40.0, 7.5]);
        let p = [3.0, -8.0, 21.0];
        let q = xform(&affine_inv(r, t), xform(&affine(r, t), p));
        assert!(p.iter().zip(q).all(|(a, b)| (a - b).abs() < 1e-4), "{q:?}");
    }
}
