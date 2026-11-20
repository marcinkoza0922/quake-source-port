//! Offscreen Vulkan tests for modern map/mod compatibility:
//! - HDR (`E5BGR9`) lightmaps + `.dlit` directional deluxemaps (normal/specular)
//! - IQM v2 GPU skeletal skinning across animation frames
//! - `Draw2D::Clip` scissor clipping for CSQC

use qformats::bsp::{Bsp, Face, Leaf, MipTex, Model, Plane, TexInfo, TextureKind, encode_e5bgr9};
use qformats::iqm::{Iqm, IqmJoint, IqmMesh, IqmPose};
use qrender::{Draw2D, LevelBsp, Md3SkinImage, Md3SurfaceSkins, RenderEntity, RenderFrame, Renderer, Settings, View};

fn lit_wall(hdr: Vec<u32>, deluxe: Vec<u8>) -> Bsp {
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
        vertexes: vec![[64.0, -32.0, -32.0], [64.0, -32.0, 32.0], [64.0, 32.0, 32.0], [64.0, 32.0, -32.0]],
        edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
        surfedges: vec![0, 1, 2, 3],
        planes: vec![Plane { normal: [-1.0, 0.0, 0.0], dist: -64.0, kind: 0 }],
        texinfo: vec![TexInfo { vecs: [[0.0, 0.25, 0.0, 8.0], [0.0, 0.0, 0.25, 8.0]], miptex: 0, flags: 0 }],
        textures: vec![Some(MipTex {
            name: "wall".into(),
            width: 16,
            height: 16,
            kind: TextureKind::Normal,
            mips: vec![vec![1u8; 256]],
            shift: 0,
        })],
        faces: vec![Face { plane: 0, side: 0, first_edge: 0, num_edges: 4, texinfo: 0, styles: [0, 255, 255, 255], lightofs: 0 }],
        lighting: vec![128; 4],
        hdr_lighting: Some(hdr),
        deluxemap: Some(deluxe),
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
fn hdr_lightmaps_preserve_hue_and_deluxemap_adds_specular() {
    let mut pal = [[0u8; 3]; 256];
    pal[1] = [180, 180, 180]; // neutral grey wall
    let Ok(mut r) = Renderer::headless(64, 64, &pal) else { return };

    // Strong orange HDR light [4.0, 1.0, 0.0]: LDR hard-clamping at 2.0/1.0 would
    // saturate differently than HDR soft rolloff, and deluxemap pointing back toward
    // the camera (-X -> [0, 128, 128]) should add a bright specular highlight.
    let hdr = vec![encode_e5bgr9([4.0, 1.0, 0.0]); 4];
    let bsp = lit_wall(hdr, [0u8, 128, 128].repeat(4));
    r.load_level(LevelBsp::new(&bsp, None), &[]).unwrap();

    let view = Some(View { origin: [0.0; 3], angles: [0.0; 3], fov_x: 90.0, ..Default::default() });
    let render_px = |r: &mut Renderer, settings: Settings| {
        let fr = RenderFrame { view, settings, ..Default::default() };
        r.render(&fr).unwrap();
        r.render(&fr).unwrap();
        let px = r.read_pixels().unwrap();
        let i = (32 * 64 + 32) * 4;
        [px[i], px[i + 1], px[i + 2]]
    };

    let base_hdr = render_px(&mut r, Settings { hdr_lightmaps: true, deluxemapping: false, ..Default::default() });
    // Orange hue preserved under the soft shoulder: R > G > B, R not hard-clipped flat.
    assert!(base_hdr[0] > base_hdr[1] && base_hdr[1] > base_hdr[2], "base_hdr={base_hdr:?}");

    // Below the tonemap shoulder the deluxemap (light straight from the camera) adds
    // normal shading + a specular highlight; the hue-preserving shoulder would hide it.
    let dim = vec![encode_e5bgr9([0.4, 0.3, 0.2]); 4];
    let bsp = lit_wall(dim, [0u8, 128, 128].repeat(4));
    r.load_level(LevelBsp::new(&bsp, None), &[]).unwrap();
    let plain = render_px(&mut r, Settings { hdr_lightmaps: true, deluxemapping: false, ..Default::default() });
    let with_dlx = render_px(&mut r, Settings { hdr_lightmaps: true, deluxemapping: true, deluxemapping_specular: 0.6, ..Default::default() });
    assert!(
        with_dlx.iter().zip(plain).all(|(&d, b)| d >= b) && with_dlx[0] > plain[0] + 4,
        "expected deluxemap specular boost: plain={plain:?} dlx={with_dlx:?}"
    );
}

#[test]
fn iqm_gpu_skinning_moves_bone_across_frames_and_clip_scissors_2d() {
    let pal = [[0u8; 3]; 256];
    let Ok(mut r) = Renderer::headless(64, 64, &pal) else { return };

    // A square quad in the YZ plane at x=64, skinned to bone 0 which shifts +Y by 30 on frame 1.
    let iqm = Iqm {
        flags: 0,
        meshes: vec![IqmMesh {
            name: "quad".into(),
            material: String::new(),
            first_vertex: 0,
            num_vertexes: 4,
            first_triangle: 0,
            num_triangles: 2,
        }],
        positions: vec![
            [0.0, -24.0, -10.0],
            [0.0, -24.0, 10.0],
            [0.0, -4.0, 10.0],
            [0.0, -4.0, -10.0],
        ],
        texcoords: vec![[0.0, 0.0], [0.0, 1.0], [1.0, 1.0], [1.0, 0.0]],
        normals: vec![[-1.0, 0.0, 0.0]; 4],
        blend_indexes: vec![[0, 0, 0, 0]; 4],
        blend_weights: vec![[255, 0, 0, 0]; 4],
        // Clockwise as seen from the camera (Quake alias front faces).
        triangles: vec![[0, 2, 1], [0, 3, 2]],
        joints: vec![IqmJoint {
            name: "root".into(),
            parent: -1,
            translate: [0.0; 3],
            rotate: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }],
        poses: vec![IqmPose {
            parent: -1,
            mask: 1 << 1, // channel 1 = translate Y
            channel_offset: [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
            channel_scale: [0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        }],
        anims: vec![],
        num_frames: 2,
        num_framechannels: 1,
        frames: vec![0, 28], // frame 0: Y in [-24,-4] (right of screen); frame 1: Y in [+4,+24] (left of screen)
        bounds: vec![],
    };
    let skin = Md3SurfaceSkins {
        skins: vec![(
            vec![Md3SkinImage { width: 1, height: 1, base: vec![0, 255, 0, 255], fb: None, cmap: None }],
            vec![],
        )],
        holey: false,
    };
    let mid = r.load_iqm(&iqm, &[skin]).unwrap();
    let view = Some(View { origin: [0.0; 3], angles: [0.0; 3], fov_x: 90.0, ..Default::default() });
    let sample = |r: &mut Renderer, frame: u32| {
        let ent = RenderEntity {
            model: mid,
            origin: [64.0, 0.0, 0.0],
            frame,
            prev_frame: frame,
            lerp: 1.0,
            fullbright: true,
            ..Default::default()
        };
        let fr = RenderFrame { view, entities: &[ent], ..Default::default() };
        r.render(&fr).unwrap();
        r.render(&fr).unwrap();
        let px = r.read_pixels().unwrap();
        let left_g = px[(32 * 64 + 25) * 4 + 1];
        let right_g = px[(32 * 64 + 39) * 4 + 1];
        (left_g, right_g)
    };

    let (l0, r0) = sample(&mut r, 0);
    let (l1, r1) = sample(&mut r, 1);
    assert!(r0 > 180 && l0 < 30, "frame 0 should be on right: left={l0} right={r0}");
    assert!(l1 > 180 && r1 < 30, "frame 1 should skin to left: left={l1} right={r1}");

    // Also verify Draw2D::Clip restricts a full-screen white fill to the left half [0..32]
    let draw2d = [
        Draw2D::Clip(Some([0.0, 0.0, 32.0, 64.0])),
        Draw2D::Fill { x: 0.0, y: 0.0, w: 64.0, h: 64.0, color: [1.0, 1.0, 1.0, 1.0] },
        Draw2D::Clip(None),
    ];
    let fr = RenderFrame { draw2d: &draw2d, ..Default::default() };
    r.render(&fr).unwrap();
    r.render(&fr).unwrap();
    let px = r.read_pixels().unwrap();
    assert!(px[(32 * 64 + 16) * 4] > 200, "inside clip should be white");
    assert!(px[(32 * 64 + 48) * 4] < 20, "outside clip should remain black");
}
