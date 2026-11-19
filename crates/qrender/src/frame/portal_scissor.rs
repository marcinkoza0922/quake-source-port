//! Bound destination rasterization to the source aperture. Destination targets
//! and projection stay full size: recursive views sample screen-space pixels.
use super::{Mat4, vk};

pub(super) fn aperture(
    vp: &Mat4,
    portal: &crate::portals::Portal,
    extent: [u32; 2],
    texture_extent: [u32; 2],
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
    // water.frag warps destination UVs by at most 0.0015 of the *texture*
    // size, which can exceed this split viewport's size. Two extra pixels
    // cover filtering, sample positions and projection rounding. Gun ovals
    // sample without warp (fte.frag). Both use non-mipmapped clamp samplers.
    let padding = texture_extent.map(|n| 2.0 + if portal.gun.is_none() { (n as f32 * 0.0015).ceil() } else { 0.0 });
    let start: [u32; 2] = std::array::from_fn(|k| (lo[k].floor() - padding[k]).clamp(0.0, extent[k] as f32) as u32);
    let end: [u32; 2] = std::array::from_fn(|k| (hi[k].ceil() + padding[k]).clamp(0.0, extent[k] as f32) as u32);
    vk::Rect2D {
        offset: vk::Offset2D { x: start[0] as i32, y: start[1] as i32 },
        extent: vk::Extent2D { width: end[0] - start[0], height: end[1] - start[1] },
    }
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
        let gun = aperture(&ident(), &p, [1000, 500], [4096, 2048]);
        assert!(gun.extent.width < 1000 && gun.extent.height < 500);
        p.gun = None;
        let water = aperture(&ident(), &p, [1000, 500], [4096, 2048]);
        assert_eq!(water.offset.x, gun.offset.x - 7);
        assert_eq!(water.offset.y, gun.offset.y - 4);
        assert_eq!(water.extent.width, gun.extent.width + 14);
        assert_eq!(water.extent.height, gun.extent.height + 8);
    }

    #[test]
    fn clipping_plane_crossings_and_invalid_projection_use_full_view() {
        let p = sheet([-1.0, -1.0, 3.0], [1.0, 1.0, 5.0]);
        let mut vp = ident();
        vp[11] = 1.0;
        vp[14] = -4.0;
        vp[15] = 0.0;
        let full = vk::Rect2D { offset: vk::Offset2D::default(), extent: vk::Extent2D { width: 320, height: 200 } };
        assert_eq!(aperture(&vp, &p, [320, 200], [640, 400]), full);
        vp[0] = f32::NAN;
        assert_eq!(aperture(&vp, &p, [320, 200], [640, 400]), full);
        vp = ident();
        vp[15] = -1.0;
        assert_eq!(aperture(&vp, &p, [320, 200], [640, 400]), full);
    }

    #[test]
    fn offscreen_bounds_are_clamped_and_may_be_empty() {
        let p = sheet([2.0, 2.0, 1.0], [3.0, 3.0, 2.0]);
        let rect = aperture(&ident(), &p, [320, 200], [640, 400]);
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
                let rect = aperture(&vp, &nested, extent, extent.map(|n| n * 2));
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
}
