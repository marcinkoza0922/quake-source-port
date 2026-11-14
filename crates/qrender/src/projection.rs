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

pub(crate) fn half_fov_tangents(fov: f32, scene: (u32, u32)) -> [f32; 2] {
    // fov_x is specified at 4:3; keep vertical FOV and adapt horizontally.
    let aspect = scene.0.max(1) as f32 / scene.1.max(1) as f32;
    let ty = (fov.clamp(1.0, 179.0).to_radians() * 0.5).tan() * 0.75;
    [ty * aspect, ty]
}

/// Half-FOV tangents for a single full-output view, including status-bar rows
/// and the exact render-scale rounding used by the scene projection.
pub fn view_fov_tangents(view: &View, output: (u32, u32), scale: f32) -> [f32; 2] {
    let scene = scene_extent(output, scale);
    let extent = view_extent([0, 0, output.0, output.1], view.reserve_bottom, output, scene);
    half_fov_tangents(view.fov_x, extent)
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
}
