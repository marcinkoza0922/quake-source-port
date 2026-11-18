//! External replacement textures cover a face like the miptex they replace,
//! whatever their size (texel coords stay in miptex units).
use qformats::bsp::{Bsp, Face, Leaf, MipTex, Model, Plane, TexInfo, TextureKind};
use qformats::image::Rgba;
use qrender::{ExternalTexture, LevelBsp, RenderFrame, Renderer, View};

const RED: [u8; 3] = [200, 0, 0];
const BLUE: [u8; 3] = [0, 0, 200];

/// A 64-unit wall in front of the origin with one 16×16 texture: index 1
/// on the left half of each row, 2 on the right. Unlit map, so fullbright.
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
    let pixels: Vec<u8> = (0..256).map(|i| if i % 16 < 8 { 1 } else { 2 }).collect();
    Bsp {
        vertexes: vec![[64.0, -32.0, -32.0], [64.0, -32.0, 32.0], [64.0, 32.0, 32.0], [64.0, 32.0, -32.0]],
        edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
        surfedges: vec![0, 1, 2, 3],
        planes: vec![Plane { normal: [-1.0, 0.0, 0.0], dist: -64.0, kind: 0 }],
        // One texel every 4 units, so the wall shows the texture once.
        texinfo: vec![TexInfo { vecs: [[0.0, 0.25, 0.0, 8.0], [0.0, 0.0, 0.25, 8.0]], miptex: 0, flags: 0 }],
        textures: vec![Some(MipTex {
            name: "wall".into(),
            width: 16,
            height: 16,
            kind: TextureKind::Normal,
            mips: vec![pixels],
            shift: 0,
        })],
        faces: vec![Face { plane: 0, side: 0, first_edge: 0, num_edges: 4, texinfo: 0, styles: [255; 4], lightofs: -1 }],
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

/// The wall's texture at `scale`× its size, as an external image.
fn upscaled(scale: u32) -> ExternalTexture {
    let n = 16 * scale;
    let pixels = (0..n * n)
        .flat_map(|i| {
            let c = if (i % n) / scale < 8 { RED } else { BLUE };
            [c[0], c[1], c[2], 255]
        })
        .collect();
    ExternalTexture { base: Rgba { width: n, height: n, pixels }, glow: None }
}

#[test]
fn larger_replacement_covers_the_face_like_the_miptex() {
    let mut palette = [[0; 3]; 256];
    palette[1] = RED;
    palette[2] = BLUE;
    let mut r = match Renderer::headless(64, 64, &palette) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("skip: no Vulkan device: {e}");
            return;
        }
    };
    let frame = RenderFrame {
        view: Some(View { origin: [0.0; 3], angles: [0.0; 3], fov_x: 90.0, ..Default::default() }),
        ..Default::default()
    };
    let bsp = wall();
    let mut shoot = |external: &[Option<ExternalTexture>]| {
        r.load_level(LevelBsp { bsp: &bsp, lit: None, external }, &[]).unwrap();
        r.render(&frame).unwrap();
        r.render(&frame).unwrap();
        let px = r.read_pixels().unwrap();
        // Inside the wall, a quarter in from each side, half way up.
        [22usize, 42].map(|x| {
            let i = (32 * 64 + x) * 4;
            [px[i], px[i + 1], px[i + 2]]
        })
    };
    let miptex = shoot(&[]);
    assert_ne!(miptex[0], miptex[1], "the wall should show both halves: {miptex:?}");
    let replaced = shoot(&[Some(upscaled(4))]);
    assert_eq!(replaced, miptex, "a 64×64 replacement must cover the face like the 16×16 miptex");
}
