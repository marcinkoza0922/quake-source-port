//! World build tests: CalcSurfaceExtents rounding and BSPX lumps
//! (`DECOUPLED_LM`, `LMSHIFT`, `LIGHTGRID_OCTREE`, `FACENORMALS`).
use super::{ATLAS, World};
use qformats::bsp::{Bsp, Face, Plane, TexInfo};
use qformats::bspx::{
    DecoupledLm, FaceNormalIndices, FaceNormals, LIGHTGRID_LEAF, LightGridCell, LightGridLeaf,
    LightGridOctree, LightGridSample,
};

fn base_triangle_bsp() -> Bsp {
    Bsp {
        vertexes: vec![[0.0, 0.0, 0.0], [32.0, 0.0, 0.0], [0.0, 32.0, 0.0]],
        edges: vec![[0, 1], [1, 2], [2, 0]],
        surfedges: vec![0, 1, 2],
        planes: vec![Plane { normal: [0.0, 0.0, 1.0], dist: 0.0, kind: 2 }],
        texinfo: vec![TexInfo {
            vecs: [[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0]],
            miptex: 0,
            flags: 0,
        }],
        faces: vec![Face {
            plane: 0,
            side: 0,
            first_edge: 0,
            num_edges: 3,
            texinfo: 0,
            styles: [0, 255, 255, 255],
            lightofs: 0,
        }],
        ..Default::default()
    }
}

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
        planes: vec![Plane { normal: [0.0, 0.0, 1.0], dist: 0.0, kind: 2 }],
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
    assert_eq!(samples, bsp.lighting.iter().map(|&v| u32::from(v)).collect::<Vec<_>>());
}

#[test]
fn decoupled_lm_sets_custom_size_scale_and_uvs() {
    let mut bsp = base_triangle_bsp();
    // 4 world units per luxel (0.25) -> 9x9 lightmap for 32x32 triangle.
    bsp.lighting = vec![200u8; 9 * 9];
    bsp.decoupled_lm = Some(vec![DecoupledLm {
        width: 9,
        height: 9,
        offset: 0,
        vecs: [[0.25, 0.0, 0.0, 0.0], [0.0, 0.25, 0.0, 0.0]],
    }]);
    let w = World::build(&bsp, None);
    assert_eq!(w.surfaces[0].lm_size, [9, 9]);
    assert_eq!(w.lm_surfs[0].size, 9 | (9 << 16));
    assert!((w.lm_surfs[0].tmins[0] - 4.0).abs() < 1e-4);
    assert!((w.lm_surfs[0].tmins[1] - 4.0).abs() < 1e-4);
    // Vertex 1 is at x=32 -> luxel 8.0 -> (8.5 / ATLAS).
    assert!((w.vertices[1].lm[0] - 8.5 / ATLAS as f32).abs() < 1e-6);
    assert_eq!(w.samples.len(), 81);
}

#[test]
fn lmshift_and_lmoffset_scale_luxels() {
    let mut bsp = base_triangle_bsp();
    // shift=3 -> 8 units/luxel -> 5x5 lightmap for 0..32 triangle, at offset 4.
    bsp.lighting = vec![0; 4 + 25];
    bsp.lighting[4..29].fill(150);
    bsp.lm_shift = Some(vec![3]);
    bsp.lm_offset = Some(vec![4]);
    let w = World::build(&bsp, None);
    assert_eq!(w.surfaces[0].lm_size, [5, 5]);
    assert_eq!(w.lm_surfs[0].size, 5 | (5 << 16));
    assert!((w.lm_surfs[0].tmins[0] - 8.0).abs() < 1e-4);
    assert!((w.vertices[1].lm[0] - 4.5 / ATLAS as f32).abs() < 1e-6);
    assert_eq!(w.samples[0] & 255, 150);
}

#[test]
fn lightgrid_octree_drives_light_point() {
    let mut bsp = base_triangle_bsp();
    bsp.lighting = vec![10; 9];
    let cell = |rgb: [u8; 3]| LightGridCell {
        samples: [
            LightGridSample { style: 0, rgb },
            LightGridSample::default(),
            LightGridSample::default(),
            LightGridSample::default(),
        ],
        count: 1,
    };
    bsp.lightgrid = Some(LightGridOctree {
        grid_dist: [32.0, 32.0, 32.0],
        grid_size: [2, 2, 2],
        grid_mins: [0.0, 0.0, 0.0],
        num_styles: 1,
        root_node: LIGHTGRID_LEAF,
        nodes: vec![],
        leafs: vec![LightGridLeaf {
            mins: [0, 0, 0],
            size: [2, 2, 2],
            cells: vec![cell([40, 120, 200]); 8],
        }],
    });
    let w = World::build(&bsp, None);
    let lp = w.light_point([16.0, 16.0, 16.0], &[256]).unwrap();
    assert!((lp[0] - 40.0).abs() < 1e-3);
    assert!((lp[1] - 120.0).abs() < 1e-3);
    assert!((lp[2] - 200.0).abs() < 1e-3);
}

#[test]
fn facenormals_populate_smooth_world_vertex_normals() {
    let mut bsp = base_triangle_bsp();
    let n0 = [0.0, 0.0, 1.0];
    let n1 = [0.6, 0.0, 0.8];
    let n2 = [0.0, 0.6, 0.8];
    bsp.face_normals = Some(FaceNormals {
        vecs: vec![n0, n1, n2],
        per_vert: vec![
            FaceNormalIndices { normal: 0, tangent: 0, bitangent: 0 },
            FaceNormalIndices { normal: 1, tangent: 0, bitangent: 0 },
            FaceNormalIndices { normal: 2, tangent: 0, bitangent: 0 },
        ],
        face_offsets: vec![0],
    });
    let w = World::build(&bsp, None);
    assert_eq!(w.vertices[0].normal, n0);
    assert!((w.vertices[1].normal[0] - 0.6).abs() < 1e-5);
    assert!((w.vertices[2].normal[1] - 0.6).abs() < 1e-5);
}
