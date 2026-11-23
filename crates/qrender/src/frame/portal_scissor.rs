//! Bound destination rasterization to the source aperture. Destination targets
//! and projection stay full size: recursive views sample screen-space pixels.
use super::{Mat4, vk};

pub(super) fn aperture(
    vp: &Mat4,
    portal: &crate::portals::Portal,
    extent: [u32; 2],
    texture_extent: [u32; 2],
    teleport_ripples: bool,
) -> vk::Rect2D {
    // water.frag accepts points 0.25 units beyond the discovered sheet bounds.
    // Gun bounds already enclose the oval and its normal offset; padding them
    // as well keeps the same conservative path for transformed nested views.
    let mins = portal.mins.map(|x| x - 0.25);
    let maxs = portal.maxs.map(|x| x + 0.25);
    let mut lo = [f32::INFINITY; 2];
    let mut hi = [f32::NEG_INFINITY; 2];
    let full = vk::Rect2D {
        offset: vk::Offset2D::default(),
        extent: vk::Extent2D { width: extent[0], height: extent[1] },
    };
    for corner in 0..8 {
        let p: [f32; 3] = std::array::from_fn(|k| if corner >> k & 1 == 0 { mins[k] } else { maxs[k] });
        let clip: [f32; 4] = std::array::from_fn(|r| vp[r] * p[0] + vp[4 + r] * p[1] + vp[8 + r] * p[2] + vp[12 + r]);
        // A box wholly beyond the near plane has positive w. Its projected
        // extrema occur at corners. Crossing that plane needs clipping; use
        // the full view instead, also covering invalid/degenerate input.
        if clip.iter().any(|x| !x.is_finite()) || clip[3] <= 0.0 || clip[2] <= 0.0 {
            return full;
        }
        for k in 0..2 {
            let pixel = (clip[k] / clip[3] * 0.5 + 0.5) * extent[k] as f32;
            lo[k] = lo[k].min(pixel);
            hi[k] = hi[k].max(pixel);
        }
    }
    // water.glsl warps destination UVs by up to 0.0015 of the *texture*
    // size (plus 0.025 for impact ripples), which can exceed the split viewport.
    // Two extra pixels cover filtering, sample positions and projection rounding.
    // Gun ovals sample without warp (fte.frag).
    let warp = if portal.gun.is_some() {
        0.0
    } else if teleport_ripples && !portal.reflect {
        0.0265
    } else {
        0.0015
    };
    let padding = texture_extent.map(|n| 2.0 + (n as f32 * warp).ceil());
    let start: [u32; 2] = std::array::from_fn(|k| (lo[k].floor() - padding[k]).clamp(0.0, extent[k] as f32) as u32);
    let end: [u32; 2] = std::array::from_fn(|k| (hi[k].ceil() + padding[k]).clamp(0.0, extent[k] as f32) as u32);
    vk::Rect2D {
        offset: vk::Offset2D { x: start[0] as i32, y: start[1] as i32 },
        extent: vk::Extent2D { width: end[0] - start[0], height: end[1] - start[1] },
    }
}

/// Extracts four unit-normal side frustum planes (left, right, top, bottom) for
/// `dest_vp` narrowed to `scissor` inside `extent`. Returns `None` if `scissor`
/// is empty.
pub(super) fn frustum_planes(
    dest_vp: &Mat4,
    scissor: vk::Rect2D,
    extent: [u32; 2],
) -> Option<[[f32; 4]; 4]> {
    if scissor.extent.width == 0 || scissor.extent.height == 0 || extent[0] == 0 || extent[1] == 0 {
        return None;
    }
    let w = extent[0] as f32;
    let h = extent[1] as f32;
    let x0 = scissor.offset.x.max(0) as f32;
    let y0 = scissor.offset.y.max(0) as f32;
    let x1 = (scissor.offset.x.max(0) as u32 + scissor.extent.width).min(extent[0]) as f32;
    let y1 = (scissor.offset.y.max(0) as u32 + scissor.extent.height).min(extent[1]) as f32;
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let lx = 2.0 * (x0 / w) - 1.0;
    let rx = 2.0 * (x1 / w) - 1.0;
    let ty = 2.0 * (y0 / h) - 1.0;
    let by = 2.0 * (y1 / h) - 1.0;
    let row = |r: usize| [dest_vp[r], dest_vp[4 + r], dest_vp[8 + r], dest_vp[12 + r]];
    let (r0, r1, r3) = (row(0), row(1), row(3));
    let norm_plane = |p: [f32; 4]| {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt().max(1e-6);
        [p[0] / len, p[1] / len, p[2] / len, p[3] / len]
    };
    Some([
        norm_plane([r0[0] - lx * r3[0], r0[1] - lx * r3[1], r0[2] - lx * r3[2], r0[3] - lx * r3[3]]),
        norm_plane([rx * r3[0] - r0[0], rx * r3[1] - r0[1], rx * r3[2] - r0[2], rx * r3[3] - r0[3]]),
        norm_plane([r1[0] - ty * r3[0], r1[1] - ty * r3[1], r1[2] - ty * r3[2], r1[3] - ty * r3[3]]),
        norm_plane([by * r3[0] - r1[0], by * r3[1] - r1[1], by * r3[2] - r1[2], by * r3[3] - r1[3]]),
    ])
}

/// True when any corner of `[mins, maxs]` lies on the positive side of `plane`.
#[inline]
pub(super) fn box_in_plane(plane: [f32; 4], mins: [f32; 3], maxs: [f32; 3]) -> bool {
    let px = if plane[0] >= 0.0 { maxs[0] } else { mins[0] };
    let py = if plane[1] >= 0.0 { maxs[1] } else { mins[1] };
    let pz = if plane[2] >= 0.0 { maxs[2] } else { mins[2] };
    plane[0] * px + plane[1] * py + plane[2] * pz + plane[3] >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::math::{ident, setup_view};

    fn sheet(mins: [f32; 3], maxs: [f32; 3]) -> crate::portals::Portal {
        crate::portals::Portal { mins, maxs, normal: [0.0, 0.0, 1.0], center: [0.0; 3],
            destination: [0.0; 3], rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
            pvs: [0.0; 3], gun: Some(0), reflect: false }
    }

    #[test]
    fn warp_padding_uses_texture_size_in_split_views() {
        let mut p = sheet([-0.01, -0.01, 1.0], [0.01, 0.01, 1.0]);
        let gun = aperture(&ident(), &p, [1000, 500], [4096, 2048], false);
        assert!(gun.extent.width < 1000 && gun.extent.height < 500);
        p.gun = None;
        let water = aperture(&ident(), &p, [1000, 500], [4096, 2048], false);
        assert_eq!(water.offset.x, gun.offset.x - 7);
        assert_eq!(water.offset.y, gun.offset.y - 4);
        assert_eq!(water.extent.width, gun.extent.width + 14);
        assert_eq!(water.extent.height, gun.extent.height + 8);
        let ripple = aperture(&ident(), &p, [1000, 500], [4096, 2048], true);
        assert_eq!(ripple.offset.x, gun.offset.x - 109);
        assert_eq!(ripple.offset.y, gun.offset.y - 55);
        assert_eq!(ripple.extent.width, gun.extent.width + 218);
        assert_eq!(ripple.extent.height, gun.extent.height + 110);
    }

    #[test]
    fn clipping_plane_crossings_and_invalid_projection_use_full_view() {
        let p = sheet([-1.0, -1.0, 3.0], [1.0, 1.0, 5.0]);
        let mut vp = ident();
        vp[11] = 1.0;
        vp[14] = -4.0;
        vp[15] = 0.0;
        let full = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: 320, height: 200 } };
        assert_eq!(aperture(&vp, &p, [320, 200], [640, 400], false), full);
        vp[0] = f32::NAN;
        assert_eq!(aperture(&vp, &p, [320, 200], [640, 400], false), full);
        vp = ident();
        vp[15] = -1.0;
        assert_eq!(aperture(&vp, &p, [320, 200], [640, 400], false), full);
    }

    #[test]
    fn offscreen_bounds_are_clamped_and_may_be_empty() {
        let p = sheet([2.0, 2.0, 1.0], [3.0, 3.0, 2.0]);
        let rect = aperture(&ident(), &p, [320, 200], [640, 400], false);
        assert_eq!(rect.offset, vk::Offset2D { x: 320, y: 200 });
        assert_eq!(rect.extent, vk::Extent2D { width: 0, height: 0 });
    }

    #[test]
    fn transformed_nested_aperture_samples_remain_inside_scissor() {
        // A rotated destination's inner box is transformed back into the
        // source camera's space, exactly as prepare_portals builds recursion.
        let mut outer = sheet([95.0, -8.0, -8.0], [105.0, 8.0, 8.0]);
        outer.center = [100.0, 0.0, 0.0];
        outer.destination = [500.0, 200.0, 40.0];
        outer.rot = crate::portals::yaw_rot(0.7);
        let source = [200.0, 12.0, 8.0];
        let destination = outer.to_destination(source);
        let inner = sheet(destination.map(|x| x - 4.0), destination.map(|x| x + 4.0));
        let nested = outer.through(&inner);
        for extent in [[320, 200], [1024, 768], [720, 1280]] {
            for angles in [[0.0; 3], [4.0, 5.0, 12.0], [-6.0, -8.0, -16.0]] {
                let view = crate::View { origin: [0.0; 3], angles, fov_x: 90.0, ..Default::default() };
                let vp = setup_view(&view, extent[0], extent[1]).vp;
                let rect = aperture(&vp, &nested, extent, extent.map(|n| n * 2), false);
                assert!(rect.extent.width < extent[0], "fixture must exercise a bounded aperture");
                for x in 0..=4 { for y in 0..=4 { for z in 0..=4 {
                    let p = [x, y, z].map(|v| v as f32 * 2.0 - 4.0);
                    let point = std::array::from_fn(|k| destination[k] + p[k]);
                    let point = outer.from_destination(point);
                    let clip: [f32; 4] = std::array::from_fn(|r| vp[r] * point[0] + vp[4 + r] * point[1] + vp[8 + r] * point[2] + vp[12 + r]);
                    let pixel = [0, 1].map(|k| (clip[k] / clip[3] * 0.5 + 0.5) * extent[k] as f32);
                    assert!(pixel[0] >= rect.offset.x as f32 && pixel[0] < (rect.offset.x as u32 + rect.extent.width) as f32);
                    assert!(pixel[1] >= rect.offset.y as f32 && pixel[1] < (rect.offset.y as u32 + rect.extent.height) as f32);
                } } }
            }
        }
    }

    #[test]
    fn narrowed_destination_frustum_covers_aperture_and_rejects_outside() {
        use crate::frame::math::{mul, sphere_in_frustum};
        let view = crate::View { origin: [0.0, 0.0, 48.0], angles: [0.0; 3], fov_x: 90.0, ..Default::default() };
        let extent = [1280, 720];
        let vs = setup_view(&view, extent[0], extent[1]);
        // Teleporter window in front of the camera, shifted right and rotated 90 deg at destination.
        let tele = crate::portals::Portal {
            mins: [196.0, -48.0, 16.0],
            maxs: [204.0, 48.0, 80.0],
            normal: [-1.0, 0.0, 0.0],
            center: [200.0, 0.0, 48.0],
            destination: [1000.0, 500.0, 48.0],
            rot: crate::portals::yaw_rot(std::f32::consts::FRAC_PI_2),
            pvs: [1000.0, 500.0, 48.0],
            gun: None,
            reflect: false,
        };
        let rect = aperture(&vs.vp, &tele, extent, extent, false);
        let dest_vp = mul(&vs.vp, &tele.to_source());
        let f = frustum_planes(&dest_vp, rect, extent).expect("non-empty scissor");
        // Point directly beyond the destination exit (+Y in destination space) is inside.
        let visible = tele.to_destination([400.0, 0.0, 48.0]);
        assert!(sphere_in_frustum(&f, visible, 8.0));
        // Point far outside the narrowed aperture (would be inside a 90 deg full FOV, outside aperture).
        let side = tele.to_destination([220.0, 180.0, 48.0]);
        assert!(!sphere_in_frustum(&f, side, 4.0));
        // Planar reflection (det = -1) also keeps points visible in the reflected water sheet.
        let refl = crate::portals::reflection([0.0, 0.0, 1.0], 0.0, [100.0, -100.0, 0.0], [300.0, 100.0, 0.0], view.origin);
        let r_rect = aperture(&vs.vp, &refl, extent, extent, false);
        let r_vp = mul(&vs.vp, &refl.to_source());
        let rf = frustum_planes(&r_vp, r_rect, extent).expect("non-empty reflection scissor");
        assert!(sphere_in_frustum(&rf, [200.0, 0.0, 48.0], 16.0));
        assert!(!sphere_in_frustum(&rf, [-200.0, 0.0, 48.0], 8.0));
    }
}
