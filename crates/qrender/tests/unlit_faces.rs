//! R_BuildLightMap: an unlit face in a lit map is black, not fullbright.
use qformats::bsp::{Bsp, Face, Leaf, MipTex, Model, Plane, TexInfo, TextureKind};
use qrender::{RenderFrame, Renderer, View};

fn wall() -> Bsp {
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
        planes: vec![Plane {
            normal: [-1.0, 0.0, 0.0],
            dist: -64.0,
            kind: 0,
        }],
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
            styles: [255; 4],
            lightofs: -1,
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
fn missing_face_samples_are_black_only_when_map_has_lighting() {
    let mut palette = [[0; 3]; 256];
    palette[1] = [160; 3];
    let mut renderer = match Renderer::headless(64, 64, &palette) {
        Ok(renderer) => renderer,
        Err(e) => {
            eprintln!("skip: no Vulkan device: {e}");
            return;
        }
    };
    let frame = RenderFrame {
        view: Some(View {
            origin: [0.0; 3],
            angles: [0.0; 3],
            fov_x: 90.0,
            ..Default::default()
        }),
        ..Default::default()
    };
    let mut bsp = wall();
    // The map has lighting, but this particular face has neither samples
    // nor styles (qbj3 start's bright strips and triangles).
    bsp.lighting = vec![128; 4];
    renderer.load_world(&bsp, None).unwrap();
    renderer.render(&frame).unwrap();
    renderer.render(&frame).unwrap();
    let missing = renderer.read_pixels().unwrap();
    let center = (32 * 64 + 32) * 4;
    assert!(
        missing[center..center + 3].iter().all(|&c| c < 8),
        "unlit face glows: {:?}",
        &missing[center..center + 3]
    );

    // A properly lit face still receives its baked samples.
    bsp.faces[0].lightofs = 0;
    bsp.faces[0].styles = [0, 255, 255, 255];
    renderer.load_world(&bsp, None).unwrap();
    renderer.render(&frame).unwrap();
    renderer.render(&frame).unwrap();
    let lit = renderer.read_pixels().unwrap();
    assert!(lit[center] > 100, "lit wall disappeared: {}", lit[center]);

    // A map compiled without any lighting remains fullbright.
    bsp.lighting.clear();
    bsp.faces[0].lightofs = -1;
    bsp.faces[0].styles = [255; 4];
    renderer.load_world(&bsp, None).unwrap();
    renderer.render(&frame).unwrap();
    renderer.render(&frame).unwrap();
    let unlit_map = renderer.read_pixels().unwrap();
    assert!(
        unlit_map[center] > 100,
        "unlit map went dark: {}",
        unlit_map[center]
    );
}
