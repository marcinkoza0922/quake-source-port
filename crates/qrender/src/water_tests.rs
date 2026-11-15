use super::*;
use crate::world::World;
use qformats::bsp::{Bsp, CONTENTS_EMPTY, CONTENTS_SOLID, CONTENTS_WATER, Leaf, Node, Plane};

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

/// 4-unit-deep pool (`-4 <= z < 0`, `x >= 0`) over a solid floor (`z < -4`).
fn shallow_pool_world() -> World {
    let leaf = |contents| Leaf {
        contents,
        visofs: -1,
        mins: [-128.0; 3],
        maxs: [128.0; 3],
        first_marksurface: 0,
        num_marksurfaces: 0,
        ambient_level: [0; 4],
    };
    let bsp = Bsp {
        planes: vec![
            Plane { normal: [0.0, 0.0, 1.0], dist: 0.0, kind: 2 },
            Plane { normal: [0.0, 0.0, 1.0], dist: -4.0, kind: 2 },
            Plane { normal: [1.0, 0.0, 0.0], dist: 0.0, kind: 0 },
        ],
        nodes: vec![
            Node { plane: 0, children: [-2, 1], mins: [-128.0; 3], maxs: [128.0; 3], first_face: 0, num_faces: 0 },
            Node { plane: 1, children: [2, -3], mins: [-128.0; 3], maxs: [128.0; 3], first_face: 0, num_faces: 0 },
            Node { plane: 2, children: [-4, -2], mins: [-128.0; 3], maxs: [128.0; 3], first_face: 0, num_faces: 0 },
        ],
        leafs: vec![leaf(CONTENTS_EMPTY), leaf(CONTENTS_EMPTY), leaf(CONTENTS_SOLID), leaf(CONTENTS_WATER)],
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

#[test]
fn shallow_pool_ripples_on_move_and_splashes_on_jump_or_shot() {
    let world = shallow_pool_world();
    let mut fx = WaterFx::default();

    // Player standing on dry floor (feet at z = -4, origin at z = 20).
    let drops = fx.update(&world, 1.0, std::iter::empty(), Some([-16.0, 0.0, 20.0]), None, &[], true, true, 2);
    assert!(drops.is_empty());
    assert!(fx.ripples.is_empty());

    // Walking horizontally into the 4-unit pool causes a ripple, but no splash particles.
    let drops = fx.update(&world, 1.05, std::iter::empty(), Some([16.0, 0.0, 20.0]), None, &[], true, true, 2);
    assert_eq!(fx.ripples.len(), 1);
    assert!(drops.is_empty());

    // Continuing to move inside the shallow pool causes wake ripples, still no splash particles.
    let drops = fx.update(&world, 1.4, std::iter::empty(), Some([48.0, 0.0, 20.0]), None, &[], true, true, 2);
    assert_eq!(fx.ripples.len(), 2);
    assert!(drops.is_empty());

    // Jumping out of the shallow pool causes both a ripple and splash particles.
    let drops = fx.update(&world, 1.45, std::iter::empty(), Some([48.0, 0.0, 34.0]), None, &[], true, true, 2);
    assert_eq!(fx.ripples.len(), 3);
    assert!(!drops.is_empty());

    // Shooting into the shallow pool (hit on the solid floor just below z = -4) splashes and ripples.
    let mut fx_shot = WaterFx::default();
    fx_shot.update(&world, 1.0, std::iter::empty(), None, Some([16.0, 0.0, 32.0]), &[], true, true, 2);
    let drops = fx_shot.update(
        &world,
        1.05,
        std::iter::empty(),
        None,
        Some([16.0, 0.0, 32.0]),
        &[([16.0, 0.0, -4.05], false)],
        true,
        true,
        2,
    );
    assert_eq!(fx_shot.ripples.len(), 1);
    assert!(!drops.is_empty());
}
