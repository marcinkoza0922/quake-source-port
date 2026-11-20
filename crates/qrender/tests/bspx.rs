//! End-to-end GPU tests for BSPX `DECOUPLED_LM` and `FACENORMALS` (Vulkan).
use qformats::bsp::{Bsp, Face, Leaf, MipTex, Model, Plane, TexInfo, TextureKind};
use qformats::bspx::{DecoupledLm, FaceNormalIndices, FaceNormals};
use qrender::{DLight, RenderFrame, Renderer, Settings, View};

fn wall_bsp() -> Bsp {
    let leaf = Leaf {
        contents: -1,
        visofs: -1,
        mins: [-128.0; 3],
        maxs: [128.0; 3],
        first_marksurface: 0,
        num_marksurfaces: 1,
        ambient_level: [0; 4],
    };
    Bsp {
        vertexes: vec![
            [64.0, -32.0, -32.0],
            [64.0, -32.0, 32.0],
            [64.0, 32.0, 32.0],
            [64.0, 32.0, -32.0],
        ],
        edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
        surfedges: vec![0, 1, 2, 3],
        planes: vec![Plane { normal: [-1.0, 0.0, 0.0], dist: -64.0, kind: 0 }],
        texinfo: vec![TexInfo {
            vecs: [[0.0, 0.25, 0.0, 8.0], [0.0, 0.0, 0.25, 8.0]],
            miptex: 0,
            flags: 0,
        }],
        textures: vec![Some(MipTex {
            name: "wall".into(),
            width: 16,
            height: 16,
            kind: TextureKind::Normal,
            mips: vec![vec![1; 256]],
            shift: 0,
        })],
        faces: vec![Face {
            plane: 0,
            side: 0,
            first_edge: 0,
            num_edges: 4,
            texinfo: 0,
            styles: [0, 255, 255, 255],
            lightofs: 0,
        }],
        leafs: vec![leaf, leaf],
        marksurfaces: vec![0],
        models: vec![Model {
            mins: [-128.0; 3],
            maxs: [128.0; 3],
            origin: [0.0; 3],
            headnode: [-2; 4],
            visleafs: 1,
            first_face: 0,
            num_faces: 1,
        }],
        ..Default::default()
    }
}

#[test]
fn decoupled_lm_and_facenormals_render_correctly() {
    let mut palette = [[0; 3]; 256];
    palette[1] = [200; 3];
    let mut r = match Renderer::headless(64, 64, &palette) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skip: no Vulkan device: {e}");
            return;
        }
    };
    let view = View { origin: [0.0; 3], angles: [0.0; 3], fov_x: 90.0, ..Default::default() };

    // 1. DECOUPLED_LM: 8x8 luxels across y=-32..32, z=-32..32 (8 units/luxel).
    // Right half (y < 0 -> s in 0..4) dark, left half (y > 0 -> s in 4..8) bright.
    let mut bsp = wall_bsp();
    let mut lm = vec![0u8; 8 * 8];
    for t in 0..8 {
        for s in 0..8 {
            lm[t * 8 + s] = if s < 4 { 20 } else { 220 };
        }
    }
    bsp.lighting = lm;
    bsp.decoupled_lm = Some(vec![DecoupledLm {
        width: 8,
        height: 8,
        offset: 0,
        // Map y in [-32, 32] -> s in [0, 7], z in [-32, 32] -> t in [0, 7]
        vecs: [[0.0, 7.0 / 64.0, 0.0, 3.5], [0.0, 0.0, 7.0 / 64.0, 3.5]],
    }]);
    r.load_world(&bsp, None).unwrap();
    let frame = RenderFrame { view: Some(view), ..Default::default() };
    r.render(&frame).unwrap();
    r.render(&frame).unwrap();
    let px = r.read_pixels().unwrap();
    // Looking along +X: +Y is left of screen (x=20), -Y is right of screen (x=44).
    let left_luma = px[(32 * 64 + 20) * 4];
    let right_luma = px[(32 * 64 + 44) * 4];
    assert!(
        left_luma > right_luma + 80,
        "DECOUPLED_LM left={left_luma} should be much brighter than right={right_luma}"
    );

    // 2. FACENORMALS: uniform dark wall lit by a real-time dlight to the +Y side.
    // Smooth normal tilted toward +Y receives noticeably more light than tilted toward -Y.
    let dlights = [DLight {
        origin: [56.0, 48.0, 0.0],
        radius: 200.0,
        minlight: 0.0,
        color: [1.0, 1.0, 1.0],
        key: 0,
    }];
    let rt_frame = RenderFrame {
        view: Some(view),
        dlights: &dlights,
        settings: Settings { dynamic: true, rt_dlights: true, dlight_shadows: false, ..Default::default() },
        ..Default::default()
    };
    let render_with_normal = |r: &mut Renderer, nrm: [f32; 3]| -> u8 {
        let mut b = wall_bsp();
        b.lighting = vec![0u8; 4 * 4];
        b.face_normals = Some(FaceNormals {
            vecs: vec![nrm],
            per_vert: vec![FaceNormalIndices { normal: 0, tangent: 0, bitangent: 0 }; 4],
            face_offsets: vec![0],
        });
        r.load_world(&b, None).unwrap();
        r.render(&rt_frame).unwrap();
        r.render(&rt_frame).unwrap();
        r.read_pixels().unwrap()[(32 * 64 + 32) * 4]
    };
    let toward = render_with_normal(&mut r, [-0.3, 0.9539, 0.0]);
    let away = render_with_normal(&mut r, [-0.3, -0.9539, 0.0]);
    assert!(
        toward > away + 15,
        "FACENORMALS facing light ({toward}) should exceed facing away ({away})"
    );
}
