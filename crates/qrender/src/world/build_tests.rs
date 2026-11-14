//! CalcSurfaceExtents must round to float before sizing baked lightmaps.
use super::World;
use qformats::bsp::{Bsp, Face, Plane, TexInfo};

#[test]
fn lightmap_bounds_match_vkquake_float_rounding() {
    // Double-precision projections land just outside the 16-unit grid;
    // vkQuake's float assignment rounds them back onto it. Retaining the
    // doubles adds a column and row, corrupting both sample stride and styles.
    let epsilon = 2.0_f32.powi(-24);
    let bsp = Bsp {
        vertexes: vec![[16.0, 0.0, 0.0], [32.0, 0.0, 0.0], [16.0, 16.0, 0.0]],
        edges: vec![[0, 1], [1, 2], [2, 0]],
        surfedges: vec![0, 1, 2],
        planes: vec![Plane {
            normal: [0.0, 0.0, 1.0],
            dist: 0.0,
            kind: 2,
        }],
        texinfo: vec![TexInfo {
            vecs: [[1.0, 0.0, 0.0, -epsilon], [0.0, 1.0, 0.0, epsilon]],
            miptex: 0,
            flags: 0,
        }],
        faces: vec![Face {
            plane: 0,
            side: 0,
            first_edge: 0,
            num_edges: 3,
            texinfo: 0,
            styles: [0, 1, 255, 255],
            lightofs: 0,
        }],
        lighting: vec![10, 20, 30, 40, 50, 60, 70, 80],
        ..Default::default()
    };
    let world = World::build(&bsp, None);
    assert_eq!(world.surfaces[0].texturemins, [16, 0]);
    assert_eq!(world.surfaces[0].extents, [16, 16]);
    assert_eq!(world.lm_surfs[0].size, 2 | (2 << 16));
    assert_eq!(world.surfaces[0].samples, 0);
    let samples: Vec<_> = world.samples.iter().map(|s| s & 255).collect();
    assert_eq!(
        samples,
        bsp.lighting
            .iter()
            .map(|&v| u32::from(v))
            .collect::<Vec<_>>()
    );
}
