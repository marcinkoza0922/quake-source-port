//! Real-time dynamic lights (r_shadow_realtime_dlight), shared by both
//! backends: which of the frame's dynamic lights are lit per pixel and which
//! get cube shadow maps, the cube faces' matrices, and the light list the
//! shaders read (`Lights` in common.glsl / common.wgsl).

use crate::{DLight, Settings};

/// Most dynamic lights lit per pixel.
pub(crate) const MAX_RT_LIGHTS: usize = 32;
/// Shadow map layers the light list has matrices for (8 lights × 6 faces).
pub(crate) const MAX_SHADOW_LAYERS: usize = 48;
/// Near plane of the cube faces; with 16-bit depth this keeps about one unit
/// of precision at a 500-unit radius.
const SHADOW_NEAR: f32 = 4.0;

type Mat4 = [f32; 16];

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RtLightGpu {
    pos_radius: [f32; 4],
    color_minlight: [f32; 4],
    /// x: first shadow map layer, -1 = unshadowed.
    info: [i32; 4],
}

/// The shaders' `Lights` (std430 in Vulkan, a uniform in wgpu; the same
/// bytes, since every member is 16-byte aligned).
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct LightsGpu {
    /// Lights, shadow filter quality (0..=2), shadow face size.
    count: [u32; 4],
    lights: [RtLightGpu; MAX_RT_LIGHTS],
    vp: [Mat4; MAX_SHADOW_LAYERS],
}

/// A selected light: lit per pixel, shadowed from `layer` (six faces) or not.
#[derive(Clone, Copy)]
pub(crate) struct RtLight {
    pub(crate) light: DLight,
    pub(crate) layer: Option<u32>,
}

/// Shadowed lights and face size for `st`; (1, 16), a placeholder, while off.
pub(crate) fn shadow_budget(st: &Settings) -> (usize, u32) {
    if !(st.rt_dlights && st.dlight_shadows && st.dynamic) {
        return (1, 16);
    }
    match st.shadow_quality {
        0 => (2, 256),
        1 => (4, 512),
        _ => (8, 512),
    }
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut c = [0.0; 16];
    for col in 0..4 {
        for row in 0..4 {
            c[col * 4 + row] = (0..4).map(|k| a[k * 4 + row] * b[col * 4 + k]).sum();
        }
    }
    c
}

/// This frame's per-pixel lights: those whose sphere touches a view
/// (`views`: eye, frustum planes), most important first (big and near), at
/// most [`MAX_RT_LIGHTS`]; the first `shadows` get shadow map layers.
pub(crate) fn select(dlights: &[DLight], views: &[([f32; 3], [[f32; 4]; 4])], st: &Settings, shadows: usize) -> Vec<RtLight> {
    if !(st.rt_dlights && st.dynamic && !st.fullbright) || views.is_empty() {
        return Vec::new();
    }
    let in_view = |d: &DLight| {
        views.iter().any(|(_, f)| crate::projection::ViewFrustum { planes: *f }.contains_sphere(d.origin, d.radius))
    };
    let mut v: Vec<(f32, DLight)> = dlights
        .iter()
        .filter(|d| d.radius > d.minlight.max(0.0) + 1.0 && in_view(d))
        .map(|d| {
            let near = views
                .iter()
                .map(|(e, _)| {
                    let x = [d.origin[0] - e[0], d.origin[1] - e[1], d.origin[2] - e[2]];
                    dot(x, x).sqrt()
                })
                .fold(f32::MAX, f32::min);
            (d.radius / (near + d.radius), *d)
        })
        .collect();
    v.sort_by(|a, b| b.0.total_cmp(&a.0));
    v.truncate(MAX_RT_LIGHTS);
    let shadows = shadows.min(MAX_SHADOW_LAYERS / 6);
    v.into_iter()
        .enumerate()
        .map(|(i, (_, light))| RtLight { light, layer: (i < shadows).then_some(i as u32 * 6) })
        .collect()
}

/// Cube face view-projections of a light, in the shaders' face order (+X,
/// -X, +Y, -Y, +Z, -Z): 90° frusta from the near plane to the radius, with
/// the main view's conventions (rows right, -up, forward; depth 0..1).
pub(crate) fn face_vps(o: [f32; 3], radius: f32) -> [Mat4; 6] {
    const Z: [f32; 3] = [0.0, 0.0, 1.0];
    const X: [f32; 3] = [1.0, 0.0, 0.0];
    let axes = [
        ([1.0, 0.0, 0.0], Z),
        ([-1.0, 0.0, 0.0], Z),
        ([0.0, 1.0, 0.0], Z),
        ([0.0, -1.0, 0.0], Z),
        ([0.0, 0.0, 1.0], X),
        ([0.0, 0.0, -1.0], X),
    ];
    let (n, f) = (SHADOW_NEAR, radius.max(SHADOW_NEAR * 2.0));
    axes.map(|(fwd, up)| {
        let rows = [cross(fwd, up), [-up[0], -up[1], -up[2]], fwd];
        let mut view = [0.0; 16];
        view[15] = 1.0;
        for (r, row) in rows.iter().enumerate() {
            for c in 0..3 {
                view[c * 4 + r] = row[c];
            }
            view[12 + r] = -dot(*row, o);
        }
        let mut proj = [0.0; 16];
        proj[0] = 1.0;
        proj[5] = 1.0;
        proj[10] = f / (f - n);
        proj[11] = 1.0;
        proj[14] = -f * n / (f - n);
        mul(&proj, &view)
    })
}

/// Does the box `mins`..`maxs` touch the sphere?
pub(crate) fn box_touches_sphere(mins: [f32; 3], maxs: [f32; 3], c: [f32; 3], r: f32) -> bool {
    let d: f32 = (0..3).map(|i| (c[i] - c[i].clamp(mins[i], maxs[i])).powi(2)).sum();
    d <= r * r
}

/// Do two spheres touch?
pub(crate) fn spheres_touch(a: [f32; 3], ra: f32, b: [f32; 3], rb: f32) -> bool {
    let d = [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    dot(d, d) < (ra + rb) * (ra + rb)
}

/// The light list for the shaders.
pub(crate) fn lights_gpu(sel: &[RtLight], quality: u32, size: u32) -> LightsGpu {
    let mut g = LightsGpu {
        count: [sel.len() as u32, quality.min(2), size, 0],
        lights: [RtLightGpu::default(); MAX_RT_LIGHTS],
        vp: [[0.0; 16]; MAX_SHADOW_LAYERS],
    };
    for (i, s) in sel.iter().enumerate() {
        let d = &s.light;
        if let Some(layer) = s.layer {
            for (f, m) in face_vps(d.origin, d.radius).into_iter().enumerate() {
                g.vp[layer as usize + f] = m;
            }
        }
        g.lights[i] = RtLightGpu {
            pos_radius: [d.origin[0], d.origin[1], d.origin[2], d.radius],
            color_minlight: [d.color[0], d.color[1], d.color[2], d.minlight.max(0.0)],
            info: [s.layer.map_or(-1, |l| l as i32), 0, 0, 0],
        };
    }
    g
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(m: &Mat4, p: [f32; 3]) -> [f32; 3] {
        let c: [f32; 4] = std::array::from_fn(|r| m[r] * p[0] + m[4 + r] * p[1] + m[8 + r] * p[2] + m[12 + r]);
        [c[0] / c[3], c[1] / c[3], c[2] / c[3]]
    }

    #[test]
    fn each_face_sees_its_axis_and_the_lookup_picks_it() {
        let o = [10.0, -20.0, 30.0];
        let faces = face_vps(o, 300.0);
        let dirs = [[1.0, 0.2, -0.1], [-1.0, 0.3, 0.2], [0.1, 1.0, 0.4], [-0.3, -1.0, 0.1], [0.2, -0.4, 1.0], [0.3, 0.1, -1.0]];
        for (f, d) in dirs.iter().enumerate() {
            let p = [o[0] + d[0] * 100.0, o[1] + d[1] * 100.0, o[2] + d[2] * 100.0];
            let ndc = project(&faces[f], p);
            assert!(ndc[0].abs() < 1.0 && ndc[1].abs() < 1.0, "face {f}: {ndc:?}");
            assert!(ndc[2] > 0.0 && ndc[2] < 1.0, "face {f} depth {}", ndc[2]);
        }
        // farther along the axis is deeper
        let a = project(&faces[0], [o[0] + 50.0, o[1], o[2]])[2];
        let b = project(&faces[0], [o[0] + 200.0, o[1], o[2]])[2];
        assert!(a < b);
    }

    #[test]
    fn selection_prefers_near_lights_and_shadows_the_first() {
        let st = Settings { rt_dlights: true, ..Default::default() };
        let dl = |x: f32| DLight { origin: [x, 0.0, 0.0], radius: 200.0, minlight: 0.0, color: [1.0; 3], key: 0 };
        // a frustum that accepts everything
        let views = [([0.0; 3], [[0.0, 0.0, 0.0, -1.0]; 4])];
        let sel = select(&[dl(2000.0), dl(100.0)], &views, &st, 1);
        assert_eq!(sel.len(), 2);
        assert_eq!(sel[0].light.origin[0], 100.0);
        assert_eq!(sel[0].layer, Some(0));
        assert_eq!(sel[1].layer, None);
        assert!(select(&[dl(0.0)], &views, &Settings::default(), 1).is_empty());
    }
}
