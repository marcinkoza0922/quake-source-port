//! Candidate visibility reuse must not share the per-camera frustum result.
use super::*;
use qformats::bsp::{Leaf, Node, Plane};

fn world() -> World {
    let mut w = World::build(&Bsp::default(), None);
    let leaf = |x, visofs, first, count| Leaf {
        contents: -1, visofs, mins: [x, -1.0, -1.0], maxs: [x + 1.0, 1.0, 1.0],
        first_marksurface: first, num_marksurfaces: count, ambient_level: [0; 4],
    };
    w.leafs = vec![leaf(0.0, -1, 0, 0), leaf(2.0, 0, 0, 1),
        leaf(-3.0, 1, 1, 1), leaf(0.0, -1, 2, 0)];
    w.numleafs = 3;
    w.visdata = vec![1, 2]; // leaf 1 and leaf 2 each see only themselves.
    w.bsp_nodes = vec![Node {
        plane: 0, children: [-2, -3], mins: [-4.0; 3], maxs: [4.0; 3],
        first_face: 0, num_faces: 0,
    }];
    w.planes = vec![Plane { normal: [1.0, 0.0, 0.0], dist: 0.0, kind: 0 }];
    w.marksurfaces = vec![0, 0];
    w.surfaces.push(Surface {
        first_index: 0, num_indices: 0, texture: 0, kind: TextureKind::Normal,
        normal: [0.0; 3], dist: 0.0, flipped: false, lm: NO_LM,
        vecs: [[0.0; 4]; 2], lm_vecs: [[0.0; 4]; 2], lm_size: [0; 2],
        texturemins: [0; 2], extents: [0; 2],
        samples: NO_LM, hdr_samples: false, has_deluxe: false, styles: [255; 4], mins: [0.0; 3], maxs: [0.0; 3],
    });
    w
}

fn candidates(w: &World) -> Vec<u32> {
    w.vis_leaves.iter().map(|l| l.leaf).collect()
}

#[test]
fn all_visible_candidates_share_one_key_across_views_and_toggle_back_to_pvs() {
    let mut w = world();
    w.mark_leaves(1);
    assert_eq!(candidates(&w), [1]);
    w.mark_leaves(2);
    assert_eq!(candidates(&w), [2]);
    w.portal_surfaces([1.0, 0.0, 0.0]);
    assert!(!w.portal_cache.is_empty());

    for _ in 0..2 {
        w.set_transparent_liquids(true);
        assert!(w.portal_cache.is_empty());
        for leaf in [1, 2, 3, 0, 1] {
            w.mark_leaves(leaf);
            assert_eq!(w.vis_leaf, 0, "all-visible candidates must share the cache key");
            assert_eq!(candidates(&w), [1, 2], "empty leaves must stay excluded");
        }
        w.set_transparent_liquids(false);
        w.mark_leaves(2);
        assert_eq!(w.vis_leaf, 2);
        assert_eq!(candidates(&w), [2], "opaque mode must restore leaf-specific PVS");
    }
}

#[test]
fn missing_vis_and_solid_leaf_fallbacks_share_candidates_but_not_normal_pvs() {
    let mut w = world();
    for leaf in [0, 3, 99] {
        w.mark_leaves(leaf);
        assert_eq!(w.vis_leaf, 0);
        assert_eq!(candidates(&w), [1, 2]);
    }
    w.mark_leaves(1);
    assert_eq!(candidates(&w), [1]);
    w.visdata.clear();
    for leaf in [1, 2] {
        w.mark_leaves(leaf);
        assert_eq!(w.vis_leaf, 0);
        assert_eq!(candidates(&w), [1, 2]);
    }
}

#[test]
fn shared_candidates_still_clip_each_view_and_preserve_destination_surfaces() {
    let mut w = world();
    w.set_transparent_liquids(true);
    let mut bits = Vec::new();
    for (eye, plane, expected) in [
        ([1.0, 0.0, 0.0], [1.0, 0.0, 0.0, 0.0], 1 << 1),
        ([-1.0, 0.0, 0.0], [-1.0, 0.0, 0.0, 0.0], 1 << 2),
        ([1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], (1 << 1) | (1 << 2)),
    ] {
        w.leaf_bits(eye, &[plane; 4], &mut bits);
        assert_eq!(bits, [expected]);
        assert_eq!(&*w.portal_surfaces(eye), [0], "destination surfaces must remain deduplicated");
    }
    w.set_transparent_liquids(false);
    w.leaf_bits([1.0, 0.0, 0.0], &[[0.0, 0.0, 0.0, 1.0]; 4], &mut bits);
    assert_eq!(bits, [1 << 1]);
}

#[test]
fn water_vised_skips_novis_only_when_pvs_crosses_liquid_boundaries() {
    let mut w = world();
    // Leaf 1 is air (-1) with a normal surface; leaf 2 is water (-3) with a *water surface.
    w.texture_names = vec!["*water0".into()];
    w.surfaces[0].kind = TextureKind::Liquid;
    w.leafs[2].contents = qformats::bsp::CONTENTS_WATER;
    // visdata: leaf 1 sees only leaf 1; leaf 2 sees only leaf 2 (not water-vised).
    w.visdata = vec![1, 2];
    w.water_vised = w.check_water_vis();
    assert!(w.needs_transparent_novis([0.3, 1.0, 1.0, 1.0]));
    // Vanilla r_wateralpha doesn't apply here (ironwail's contentstransparent).
    assert!(!w.liquid_seethrough(0));
    // Absent liquid types (lava, slime, tele) do not trigger novis.
    assert!(!w.needs_transparent_novis([1.0, 0.3, 0.3, 0.3]));

    // Now leaf 2 sees both leaf 1 and leaf 2 (water-vised).
    w.visdata = vec![1, 3];
    w.water_vised = w.check_water_vis();
    assert!(!w.needs_transparent_novis([0.3, 0.3, 0.3, 0.3]));
    assert!(w.liquid_seethrough(0));

    // Missing visdata still requires novis whenever any liquid is translucent.
    w.visdata.clear();
    w.water_vised = w.check_water_vis();
    assert!(w.needs_transparent_novis([0.3, 1.0, 1.0, 1.0]));
    // ...but with no VIS at all every liquid counts as see-through.
    assert!(w.liquid_seethrough(0));
}

#[test]
fn locked_pvs_retains_its_leaf_with_live_camera_frustum() {
    let mut w = world();
    let east = [1.0, 0.0, 0.0];
    let west = [-1.0, 0.0, 0.0];
    let all = [[0.0, 0.0, 0.0, 1.0]; 4];
    let west_only = [[-1.0, 0.0, 0.0, 0.0]; 4];
    let mut bits = Vec::new();

    w.lock_pvs(true, Some(east));
    w.leaf_bits(west, &all, &mut bits);
    assert_eq!(bits, [1 << 1]);
    w.leaf_bits(west, &west_only, &mut bits);
    assert_eq!(bits, [0], "the live frustum should still clip the frozen leaf");
    w.lock_pvs(false, Some(west));
    w.leaf_bits(west, &all, &mut bits);
    assert_eq!(bits, [1 << 2]);
}

#[test]
fn box_leaves_and_pvs_bits_match_bsp_and_novis() {
    let mut w = world();
    let mut leaves = Vec::new();
    // Box purely on the east (+X) side of split plane x=0 -> leaf 1.
    w.box_leaves([1.0, -1.0, -1.0], [3.0, 1.0, 1.0], &mut leaves);
    assert_eq!(leaves, [1]);
    // Box straddling x=0 -> both leaf 1 and leaf 2.
    leaves.clear();
    w.box_leaves([-2.0, -1.0, -1.0], [2.0, 1.0, 1.0], &mut leaves);
    leaves.sort_unstable();
    assert_eq!(leaves, [1, 2]);
    // PVS bits from east eye include leaf 1, exclude leaf 2.
    let bits = w.pvs_bits([1.0, 0.0, 0.0]).expect("PVS active");
    assert_ne!(bits[0] & (1 << 1), 0);
    assert_eq!(bits[0] & (1 << 2), 0);
    // Transparent-liquid novis returns None (all static entities visible).
    w.set_transparent_liquids(true);
    assert!(w.pvs_bits([1.0, 0.0, 0.0]).is_none());
}

#[test]
fn block_frustum_culling_matches_leaf_culling_across_multiple_blocks() {
    let mut w = world();
    w.leafs.clear();
    w.leafs.push(Leaf {
        contents: -2, visofs: -1, mins: [0.0; 3], maxs: [0.0; 3],
        first_marksurface: 0, num_marksurfaces: 0, ambient_level: [0; 4],
    });
    // 40 leaves spanning x = -20..+20 (3 blocks of 16/16/8: outside, straddling, inside).
    for i in 0..40 {
        let x = i as f32 - 20.0;
        w.leafs.push(Leaf {
            contents: -1, visofs: -1, mins: [x, -1.0, -1.0], maxs: [x + 0.5, 1.0, 1.0],
            first_marksurface: 0, num_marksurfaces: 1, ambient_level: [0; 4],
        });
    }
    w.numleafs = 40;
    w.visdata.clear();
    // Keep only leaves touching x >= 0.
    let frustum = [[1.0, 0.0, 0.0, 0.0]; 4];
    let mut bits = Vec::new();
    w.leaf_bits_for_leaf(1, &frustum, &mut bits);
    for i in 0..40usize {
        let li = i + 1;
        let expected = (i as f32 - 20.0 + 0.5) >= 0.0;
        let got = (bits[li >> 5] & (1 << (li & 31))) != 0;
        assert_eq!(got, expected, "leaf {li}");
    }
}

#[test]
fn begin_light_frame_validates_cached_lightstyles() {
    let mut w = world();
    w.has_light = true;
    w.surfaces[0].lm = 0;
    w.surfaces[0].lm_size = [1, 1];
    w.surfaces[0].lm_vecs = [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]];
    w.surfaces[0].normal = [0.0, 0.0, 1.0];
    w.surfaces[0].dist = 0.0;
    w.surfaces[0].samples = 0;
    w.surfaces[0].styles = [0, 255, 255, 255];
    w.samples = vec![0x0080_8080];
    w.planes = vec![Plane { normal: [0.0, 0.0, 1.0], dist: 0.0, kind: 2 }];
    w.bsp_nodes = vec![Node {
        plane: 0, children: [-1, -2], mins: [-16.0; 3], maxs: [16.0; 3],
        first_face: 0, num_faces: 1,
    }];
    let mut styles = [256i32; 64];
    w.begin_light_frame(&styles);
    let c1 = w.light_point([0.0, 0.0, 8.0], &styles).expect("hit");
    assert!((c1[0] - 128.0).abs() < 1.0);
    // Unchanged frame hits validated_gen fast path.
    w.begin_light_frame(&styles);
    let c2 = w.light_point([0.0, 0.0, 8.0], &styles).expect("hit");
    assert_eq!(c1, c2);
    // Changed style bumps styles_gen and re-evaluates.
    styles[0] = 128;
    w.begin_light_frame(&styles);
    let c3 = w.light_point([0.0, 0.0, 8.0], &styles).expect("hit");
    assert!((c3[0] - 64.0).abs() < 1.0);
}

/// An empty leaf may point anywhere, and a point may be anything.
#[test]
fn bad_leaves_and_points_are_harmless() {
    let mut w = world();
    w.leafs[1].contents = qformats::bsp::CONTENTS_WATER;
    w.leafs[1].first_marksurface = 99;
    w.leafs[1].num_marksurfaces = 0;
    w.check_water_vis();
    for p in [[f32::NAN; 3], [0.0, 0.0, f32::INFINITY], [f32::NEG_INFINITY, 0.0, 0.0]] {
        assert_eq!(w.liquid_surface(p), None);
    }
    // water all the way up, and a height where +16 changes nothing
    for l in &mut w.leafs {
        l.contents = qformats::bsp::CONTENTS_WATER;
    }
    assert!(w.liquid_surface([1.0, 0.0, 0.0]).is_some_and(|(h, _)| h >= 4096.0));
    assert!(w.liquid_surface([1.0, 0.0, 1e20]).is_some());
    assert_eq!(w.liquid_surface([f32::NAN; 3]), None);
}
