use super::*;
use crate::world::World;
use qformats::bsp::{Bsp, CONTENTS_EMPTY, CONTENTS_WATER, Leaf, Node, Plane};

/// World with water below z = 0 and empty air above.
fn water_world() -> World {
    let bsp = Bsp {
        planes: vec![Plane { normal: [0.0, 0.0, 1.0], dist: 0.0, kind: 2 }],
        nodes: vec![Node {
            plane: 0,
            children: [-2, -3],
            mins: [-128.0; 3],
            maxs: [128.0; 3],
            first_face: 0,
            num_faces: 0,
        }],
        leafs: vec![
            Leaf { contents: CONTENTS_EMPTY, visofs: -1, mins: [0.0; 3], maxs: [0.0; 3], first_marksurface: 0, num_marksurfaces: 0, ambient_level: [0; 4] },
            Leaf { contents: CONTENTS_EMPTY, visofs: -1, mins: [-128.0, -128.0, 0.0], maxs: [128.0, 128.0, 128.0], first_marksurface: 0, num_marksurfaces: 0, ambient_level: [0; 4] },
            Leaf { contents: CONTENTS_WATER, visofs: -1, mins: [-128.0, -128.0, -128.0], maxs: [128.0, 128.0, 0.0], first_marksurface: 0, num_marksurfaces: 0, ambient_level: [0; 4] },
        ],
        ..Default::default()
    };
    World::build(&bsp, None)
}

#[test]
fn stationary_and_moving_entities_track_splashes_and_wakes() {
    let world = water_world();
    let mut fx = WaterFx::default();

    // First observation establishes track without splashing.
    let p = fx.update(&world, 1.0, [(1, [0.0, 0.0, 8.0])].into_iter(), None, None, &[], true, true, 2);
    assert!(p.is_empty());
    assert!(fx.ripples.is_empty());

    // Stationary entity does not trigger splashes or wakes.
    fx.update(&world, 1.05, [(1, [0.0, 0.0, 8.0])].into_iter(), None, None, &[], true, true, 2);
    assert!(fx.ripples.is_empty());

    // Plunging into water creates a ripple and splash drops.
    let drops = fx.update(&world, 1.1, [(1, [0.0, 0.0, -8.0])].into_iter(), None, None, &[], true, true, 2);
    assert_eq!(fx.ripples.len(), 1);
    assert!(!drops.is_empty());

    // Advance past max drop lifetime (<= 1.4 s) in <= 1.0 s steps so no drops remain.
    fx.update(&world, 2.0, [(1, [0.0, 0.0, -8.0])].into_iter(), None, None, &[], true, false, 2);
    fx.update(&world, 2.9, [(1, [0.0, 0.0, -8.0])].into_iter(), None, None, &[], true, false, 2);
    let before_wake = fx.ripples.len();
    fx.update(&world, 2.95, [(1, [0.0, 0.0, -8.0])].into_iter(), None, None, &[], true, false, 2);
    assert_eq!(fx.ripples.len(), before_wake);

    // Wading fast near the surface creates a wake after cooldown.
    fx.update(&world, 3.0, [(1, [20.0, 0.0, -8.0])].into_iter(), None, None, &[], true, false, 2);
    assert_eq!(fx.ripples.len(), before_wake + 1);
}

#[test]
fn quality_tiers_cap_ripples_drops_and_puffs() {
    let world = water_world();
    let shots: Vec<([f32; 3], bool)> = (0..40)
        .map(|i| ([i as f32 * 2.0 - 40.0, 0.0, -32.0], true))
        .collect();
    for (quality, (max_r, max_d, max_p)) in [(0, (8, 512, 128)), (1, (16, 1024, 256))] {
        let mut fx = WaterFx::default();
        let drops = fx.update(&world, 1.0, std::iter::empty(), None, Some([0.0, 0.0, 32.0]), &[], true, true, quality);
        assert!(drops.is_empty());
        let drops = fx.update(&world, 1.05, std::iter::empty(), None, Some([0.0, 0.0, 32.0]), &shots, true, true, quality);
        assert_eq!(fx.ripples.len(), max_r);
        assert!(drops.len() <= max_d);
        assert!(fx.spray(1.05).count() <= max_p);
    }
    let tex = caustic_texture(64);
    assert_eq!(tex.len(), 64 * 64 * 4);
    assert!(tex.as_chunks::<4>().0.iter().any(|p| p[0] > 100 && p[1] < 200));
}
