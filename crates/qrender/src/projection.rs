//! Shared scene sizing and FOV adaptation for rendering and CPU geometry culling.
use crate::View;

pub(crate) fn scene_extent(output: (u32, u32), scale: f32) -> (u32, u32) {
    let scale = scale.clamp(0.25, 2.0);
    let dim = |n: u32| ((n.max(1) as f32 * scale).round() as u32).clamp(1, 16384);
    (dim(output.0), dim(output.1))
}

pub(crate) fn view_extent(rect: [u32; 4], reserve: u32, output: (u32, u32), scene: (u32, u32)) -> (u32, u32) {
    let w = ((rect[2] as f32 * scene.0 as f32 / output.0.max(1) as f32).round() as u32).clamp(1, scene.0);
    let h = ((rect[3].saturating_sub(reserve) as f32 * scene.1 as f32 / output.1.max(1) as f32).round() as u32).clamp(1, scene.1);
    (w, h)
}

pub(crate) fn half_fov_tangents(view: &View, scene: (u32, u32)) -> [f32; 2] {
    // fov_x is specified at 4:3; keep vertical FOV and adapt horizontally.
    let aspect = scene.0.max(1) as f32 / scene.1.max(1) as f32;
    let ty = (view.fov_x.clamp(1.0, 179.0).to_radians() * 0.5).tan() * 0.75;
    let [sx, sy] = fov_warp(view.waterwarp);
    [ty * aspect * sx, ty * sy]
}

/// vkQuake r_waterwarp 2 (R_SetupView): scales the half-FOV tangents so the
/// pulse looks the same at any FOV.
fn fov_warp(w: crate::WaterWarp) -> [f32; 2] {
    match w {
        crate::WaterWarp::Fov(t) => {
            let s = (t * 1.5).sin() * 0.03;
            [0.97 + s, 1.03 - s]
        }
        _ => [1.0, 1.0],
    }
}

/// Half-FOV tangents for a single full-output view, including status-bar rows
/// and the exact render-scale rounding used by the scene projection.
pub fn view_fov_tangents(view: &View, output: (u32, u32), scale: f32) -> [f32; 2] {
    let scene = scene_extent(output, scale);
    let extent = view_extent([0, 0, output.0, output.1], view.reserve_bottom, output, scene);
    half_fov_tangents(view, extent)
}

/// Normalized side frustum planes (left, right, top, bottom) for a camera view.
#[derive(Clone, Copy, Debug)]
pub struct ViewFrustum {
    pub planes: [[f32; 4]; 4],
}

impl ViewFrustum {
    /// Frustum planes for a single full-output view, matching the renderer's
    /// scene sizing, status-bar reservation, and 4:3 FOV adaptation.
    pub fn from_view(v: &View, output: (u32, u32), scale: f32) -> Self {
        let scene = scene_extent(output, scale);
        let extent = view_extent([0, 0, output.0, output.1], v.reserve_bottom, output, scene);
        Self::for_extent(v, extent.0, extent.1)
    }

    pub(crate) fn for_extent(v: &View, w: u32, h: u32) -> Self {
        let [tx, ty] = half_fov_tangents(v, (w, h));
        let (fwd, right, up) = qcore::math::angle_vectors(v.angles.into());
        let e = qcore::math::Vec3::from(v.origin);
        let dirs = [
            fwd + right / tx,
            fwd - right / tx,
            fwd - up / ty,
            fwd + up / ty,
        ];
        let mut planes = [[0.0; 4]; 4];
        for (slot, d) in planes.iter_mut().zip(dirs) {
            let l = d.length().max(1e-6);
            let n = d / l;
            *slot = [n.x, n.y, n.z, -n.dot(e)];
        }
        Self { planes }
    }

    pub fn contains_sphere(&self, center: [f32; 3], radius: f32) -> bool {
        self.planes
            .iter()
            .all(|p| p[0] * center[0] + p[1] * center[1] + p[2] * center[2] + p[3] >= -radius)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fov_uses_scaled_viewport_and_reserved_rows() {
        for output in [(1280, 720), (640, 480), (720, 1280), (3440, 1440), (853, 479)] {
            for scale in [0.25, 0.73, 1.0, 2.0] {
                for reserve in [0, 48, output.1, output.1 + 10] {
                    let view = View { fov_x: 90.0, reserve_bottom: reserve, ..View::default() };
                    let (sw, sh) = scene_extent(output, scale);
                    let h = (((output.1.saturating_sub(reserve)) as f32 * sh as f32 / output.1 as f32).round() as u32).clamp(1, sh);
                    let [tx, ty] = view_fov_tangents(&view, output, scale);
                    assert!((ty - 0.75).abs() < 1e-6);
                    assert!((tx - 0.75 * sw as f32 / h as f32).abs() < 1e-3);
                }
            }
        }
        assert_eq!(scene_extent((0, 0), 0.0), (1, 1));
    }

    #[test]
    fn frustum_accepts_forward_and_rejects_behind() {
        let view = View { origin: [10.0, 20.0, 30.0], angles: [0.0, 0.0, 0.0], fov_x: 90.0, ..View::default() };
        let f = ViewFrustum::from_view(&view, (1280, 720), 1.0);
        assert!(f.contains_sphere([110.0, 20.0, 30.0], 8.0));
        assert!(!f.contains_sphere([-90.0, 20.0, 30.0], 8.0));
        assert!(!f.contains_sphere([50.0, 500.0, 30.0], 8.0));
    }
}
