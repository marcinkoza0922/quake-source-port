use super::*;
use crate::Bsp;

fn bsp_with_bspx(lumps: &[(&str, &[u8])]) -> Vec<u8> {
    // Header with every standard lump empty at offset 0, then 2 pad bytes
    // to exercise alignment.
    let mut d = 29i32.to_le_bytes().to_vec();
    d.extend(std::iter::repeat_n(0u8, HEADER_LUMPS * 8));
    let hdr = d.len() as u32;
    d[4..8].copy_from_slice(&hdr.to_le_bytes());
    d[8..12].copy_from_slice(&2u32.to_le_bytes());
    d.extend([0xaa, 0xbb]);
    while !d.len().is_multiple_of(4) {
        d.push(0);
    }
    d.extend(b"BSPX");
    d.extend((lumps.len() as u32).to_le_bytes());
    let mut ofs = d.len() + lumps.len() * 32;
    for (n, p) in lumps {
        let mut nm = [0u8; 24];
        nm[..n.len()].copy_from_slice(n.as_bytes());
        d.extend(nm);
        d.extend((ofs as u32).to_le_bytes());
        d.extend((p.len() as u32).to_le_bytes());
        ofs += p.len();
    }
    for (_, p) in lumps {
        d.extend(*p);
    }
    d
}

#[test]
fn finds_named_lump() {
    let d = bsp_with_bspx(&[("LMSHIFT", &[4]), ("RGBLIGHTING", &[1, 2, 3])]);
    assert_eq!(find_lump(&d, "RGBLIGHTING"), Some(&[1, 2, 3][..]));
    assert_eq!(find_lump(&d, "LMSHIFT"), Some(&[4][..]));
    assert_eq!(find_lump(&d, "DECOUPLED_LM"), None);
}

#[test]
fn absent_or_truncated() {
    let mut d = bsp_with_bspx(&[("RGBLIGHTING", &[1, 2, 3])]);
    d.truncate(d.len() - 2);
    assert_eq!(find_lump(&d, "RGBLIGHTING"), None);
    assert_eq!(find_lump(&d[..64], "RGBLIGHTING"), None);
}

#[test]
fn decoupled_lm_and_lmshift_roundtrip() {
    let mut dlm = Vec::new();
    dlm.extend(8u16.to_le_bytes());
    dlm.extend(4u16.to_le_bytes());
    dlm.extend(12i32.to_le_bytes());
    for f in [0.125f32, 0.0, 0.0, 1.0, 0.0, 0.125, 0.0, -2.0] {
        dlm.extend(f.to_le_bytes());
    }
    let parsed = parse_decoupled_lm(&dlm, 1).unwrap();
    assert_eq!(parsed.len(), 1);
    assert_eq!((parsed[0].width, parsed[0].height, parsed[0].offset), (8, 4, 12));
    assert_eq!(parsed[0].vecs[0], [0.125, 0.0, 0.0, 1.0]);
    assert_eq!(parsed[0].vecs[1], [0.0, 0.125, 0.0, -2.0]);
    assert!(parse_decoupled_lm(&dlm, 2).is_none());

    assert_eq!(parse_lm_shift(&[3, 4], 2), Some(vec![3, 4]));
    assert_eq!(parse_lm_offset(&[4, 0, 0, 0, 0xff, 0xff, 0xff, 0xff], 2), Some(vec![4, -1]));
    assert_eq!(parse_lm_style(&[0, 1, 255, 255], 1), Some(vec![[0, 1, 255, 255]]));
}

#[test]
fn lightgrid_octree_parse_and_trilinear_sample() {
    // 2x2x2 grid split at (1,1,1): octant 0..6 are leaf 0 (covering 2x2x2),
    // with one occluded corner at (1,1,1) to test weight renormalization.
    let mut raw = Vec::new();
    for f in [32.0f32, 32.0, 32.0] {
        raw.extend(f.to_le_bytes());
    }
    for i in [2i32, 2, 2] {
        raw.extend(i.to_le_bytes());
    }
    for f in [0.0f32, 0.0, 0.0] {
        raw.extend(f.to_le_bytes());
    }
    raw.push(1u8); // num_styles (misaligns!)
    raw.extend(LIGHTGRID_LEAF.to_le_bytes()); // root_node = leaf 0
    raw.extend(0u32.to_le_bytes()); // num_nodes
    raw.extend(1u32.to_le_bytes()); // num_leafs
    for i in [0i32, 0, 0, 2, 2, 2] {
        raw.extend(i.to_le_bytes());
    }
    // 8 cells in [z][y][x] order:
    // x=0 cells have red=100, x=1 cells have red=200 (except (1,1,1) occluded).
    for z in 0..2 {
        for y in 0..2 {
            for x in 0..2 {
                if (x, y, z) == (1, 1, 1) {
                    raw.push(0xff);
                } else {
                    raw.push(1); // 1 style
                    raw.push(0); // style 0
                    raw.extend([if x == 0 { 100 } else { 200 }, 50, 0]);
                }
            }
        }
    }
    let grid = parse_lightgrid_octree(&raw).unwrap();
    assert!(grid.lookup_cell([1, 1, 1]).is_none());
    let styles = [256i32; 64];
    // At (0, 0, 0) exact corner -> [100, 50, 0].
    let c0 = grid.sample([0.0, 0.0, 0.0], &styles).unwrap();
    assert!((c0[0] - 100.0).abs() < 1e-4 && (c0[1] - 50.0).abs() < 1e-4);
    // At center (16, 16, 16): 4 cells with red=100 (weight 4/8) and 3 cells with
    // red=200 (weight 3/8), corner (1,1,1) occluded -> weighted mean = (400+600)/7.
    let cm = grid.sample([16.0, 16.0, 16.0], &styles).unwrap();
    assert!((cm[0] - 1000.0 / 7.0).abs() < 1e-3);
    assert!((cm[1] - 50.0).abs() < 1e-3);
    // Scaled lightstyle (512 = 2x).
    let bright = grid.sample([0.0, 0.0, 0.0], &[512]).unwrap();
    assert!((bright[0] - 200.0).abs() < 1e-4);
    // Out of bounds returns None.
    assert!(grid.sample([500.0, 0.0, 0.0], &styles).is_none());
}

#[test]
fn brushlist_and_facenormals_parse() {
    // BRUSHLIST: model 0 with 1 brush and 1 bevel plane.
    let mut bl = Vec::new();
    for i in [1i32, 0, 1, 1] {
        bl.extend(i.to_le_bytes());
    }
    for f in [-16.0f32, -16.0, -16.0, 16.0, 16.0, 16.0] {
        bl.extend(f.to_le_bytes());
    }
    bl.extend((-2i16).to_le_bytes()); // CONTENTS_SOLID
    bl.extend(1u16.to_le_bytes());
    for f in [0.70710677f32, 0.70710677, 0.0, 16.0] {
        bl.extend(f.to_le_bytes());
    }
    let models = parse_brushlist(&bl).unwrap();
    assert_eq!(models.len(), 1);
    assert_eq!(models[0].model_num, 0);
    assert_eq!(models[0].brushes[0].contents, -2);
    assert_eq!(models[0].brushes[0].planes.len(), 1);

    // FACENORMALS: 2 unique vecs, 1 face with 3 edges.
    let face = Face {
        plane: 0,
        side: 0,
        first_edge: 0,
        num_edges: 3,
        texinfo: 0,
        styles: [0, 255, 255, 255],
        lightofs: -1,
    };
    let mut fn_raw = Vec::new();
    fn_raw.extend(2u32.to_le_bytes());
    for f in [0.0f32, 0.0, 1.0, 0.0, 1.0, 0.0] {
        fn_raw.extend(f.to_le_bytes());
    }
    for idx in [0u32, 1, 0, 1, 0, 1, 0, 0, 1] {
        fn_raw.extend(idx.to_le_bytes());
    }
    let fnm = parse_face_normals(&fn_raw, &[face]).unwrap();
    assert_eq!(fnm.vertex_normal(0, 0), Some([0.0, 0.0, 1.0]));
    assert_eq!(fnm.vertex_normal(0, 1), Some([0.0, 1.0, 0.0]));

    // Ensure malformed BSPX lumps don't break Bsp::parse.
    let d = bsp_with_bspx(&[
        ("DECOUPLED_LM", &[1, 2, 3]),
        ("LIGHTGRID_OCTREE", &[0xff; 60]),
        ("BRUSHLIST", &[1, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff]),
        ("FACENORMALS", &[99, 0, 0, 0]),
    ]);
    let b = Bsp::parse(&d).unwrap();
    assert!(b.decoupled_lm.is_none());
    assert!(b.lightgrid.is_none());
    assert!(b.brushlist.is_none());
    assert!(b.face_normals.is_none());
}
