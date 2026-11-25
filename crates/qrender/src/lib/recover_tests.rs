//! Creations that fail in the middle leave nothing behind.
use super::*;
use qformats::bsp::MipTex;

fn headless() -> Option<Renderer> {
    Renderer::headless(64, 64, &[[0; 3]; 256]).ok()
}

/// Memory handed out and texture slots in use.
fn usage(r: &Renderer) -> (u64, usize) {
    (r.gpu.alloc.used(), r.textures.iter().flatten().count())
}

/// Fails every creation `make` does, one run each, then lets it succeed.
/// `reset` puts back what it replaces before it starts. Returns the failures.
fn fail_each<T>(r: &mut Renderer, reset: impl Fn(&mut Renderer), make: impl Fn(&mut Renderer) -> Result<T>) -> u32 {
    for n in 0.. {
        r.idle();
        reset(r);
        let before = usage(r);
        let (models, pics, poses) = (r.models.len(), r.pics.len(), r.alias_cpu.len());
        r.gpu.journal.fail_in.set(Some(n));
        let made = make(r);
        let pending = r.gpu.journal.fail_in.take().is_some();
        match made {
            Ok(_) => {
                assert!(pending, "creation {n} failed unnoticed");
                return n;
            }
            Err(e) => assert_eq!(e.message, "injected"),
        }
        assert_eq!(usage(r), before, "creation {n} of a failed run leaked");
        assert_eq!((r.models.len(), r.pics.len(), r.alias_cpu.len()), (models, pics, poses), "creation {n}");
        assert_eq!(r.render(&RenderFrame::default()).err().map(|e| e.message), None, "still draws");
    }
    unreachable!()
}

fn texture(name: &str) -> Option<MipTex> {
    Some(MipTex { name: name.into(), width: 16, height: 16, kind: TextureKind::Normal, mips: vec![vec![1; 256]], shift: 0 })
}

#[test]
fn failed_targets_leak_nothing() {
    let Some(mut r) = headless() else { return };
    r.msaa = 4.min(r.gpu.max_samples);
    r.pipes.build(&r.gpu, COLOR_FORMAT, r.msaa).unwrap();
    r.bloom = Some(1);
    r.gun_targets = true;
    let drop_targets = |r: &mut Renderer| {
        r.idle();
        r.destroy_targets();
    };
    for n in 0.. {
        drop_targets(&mut r);
        let before = usage(&r);
        r.gpu.journal.fail_in.set(Some(n));
        let made = r.create_targets();
        let pending = r.gpu.journal.fail_in.take().is_some();
        if made.is_ok() {
            assert!(pending && n > 10, "{n}");
            break;
        }
        assert!(r.targets.is_none());
        assert_eq!(usage(&r), before, "creation {n}");
    }
    // the parts made on first use
    for (ensure, reset) in [
        (Renderer::ensure_ao_source as fn(&mut Renderer) -> Result<()>, 4u32),
        (Renderer::ensure_droplets, 1),
        (Renderer::ensure_rt_refl, 1),
    ] {
        for n in 0..reset {
            let before = usage(&r);
            r.gpu.journal.fail_in.set(Some(n));
            assert_eq!(ensure(&mut r).unwrap_err().message, "injected");
            assert_eq!(usage(&r), before, "creation {n}");
        }
        ensure(&mut r).unwrap();
    }
}

#[test]
fn failed_shadow_maps_leak_nothing() {
    let Some(mut r) = headless() else { return };
    for n in 0..14 {
        r.idle();
        r.destroy_shadow_maps();
        let before = usage(&r);
        r.gpu.journal.fail_in.set(Some(n));
        let made = r.create_shadow_maps(2, 64);
        if r.gpu.journal.fail_in.take().is_some() {
            assert!(made.is_ok());
            assert_eq!(n, 13, "one image, twelve faces");
            return;
        }
        assert!(made.is_err() && r.shadow_maps.is_none());
        assert_eq!(usage(&r), before, "creation {n}");
    }
    panic!("never succeeded");
}

#[test]
fn failed_loads_leak_nothing() {
    let Some(mut r) = headless() else { return };
    r.render(&RenderFrame::default()).unwrap();
    let keep = |_: &mut Renderer| {};
    assert!(fail_each(&mut r, keep, |r| r.register_pic_rgba(8, 8, &[9; 256])) >= 2);
    let face = qformats::image::Rgba { width: 8, height: 8, pixels: vec![7; 256] };
    let faces: [_; 6] = std::array::from_fn(|_| face.clone());
    assert!(fail_each(&mut r, |r| r.set_skybox(None).unwrap(), |r| r.set_skybox(Some(&faces))) >= 12);

    // (needs the pose buffer to grow: its replacement may fail too)
    let verts = vec![qformats::mdl::TriVert { v: [1, 2, 3], normal: 0 }; 3];
    let corner = verts[0];
    let pose = qformats::mdl::Pose { name: String::new(), bboxmin: corner, bboxmax: corner, verts };
    let mdl = Mdl {
        scale: [1.0; 3],
        scale_origin: [0.0; 3],
        bounding_radius: 8.0,
        eye_position: [0.0; 3],
        skin_width: 4,
        skin_height: 4,
        num_verts: 3,
        synctype: 0,
        flags: 0,
        size: 1.0,
        skins: vec![qformats::mdl::Skin { intervals: Vec::new(), images: vec![vec![255; 16]] }],
        stverts: vec![qformats::mdl::StVert { onseam: 0, s: 0, t: 0 }; 3],
        triangles: vec![qformats::mdl::Triangle { faces_front: 1, verts: [0, 1, 2] }],
        frames: vec![qformats::mdl::Frame { bboxmin: corner, bboxmax: corner, intervals: Vec::new(), poses: vec![pose] }],
    };
    assert!(fail_each(&mut r, keep, |r| r.load_alias(&mdl)) >= 6);
    r.alias_cpu.resize((r.alias_buf.size / 4) as usize, 0);
    assert!(fail_each(&mut r, keep, |r| r.load_alias(&mdl)) >= 7);

    // a picture that changes size keeps its old image
    let pic = r.register_pic_rgba(8, 8, &[9; 256]).unwrap();
    let bigger = |r: &mut Renderer| {
        r.resize_pic(pic, 8, 8).unwrap();
        r.update_pic(pic, &[9; 256]).unwrap();
        r.render(&RenderFrame::default()).unwrap();
        r.render(&RenderFrame::default()).unwrap();
        r.idle();
        r.resize_pic(pic, 16, 16).unwrap();
    };
    assert!(fail_each(&mut r, bigger, |r| r.update_pic(pic, &[9; 1024])) >= 2);
}

#[test]
fn a_level_that_fails_to_load_leaks_nothing() {
    let Some(mut r) = headless() else { return };
    let bsp = Bsp { textures: vec![texture("a"), texture("b")], ..Default::default() };
    for n in 0.. {
        // the failed level's own objects and the last good one's are gone
        r.load_world(&Bsp::default(), None).unwrap();
        r.idle();
        r.gpu.journal.fail_in.set(Some(0));
        assert!(r.load_world(&bsp, None).is_err(), "nothing was freed for it yet");
        assert!(r.world.is_some());
        r.gpu.journal.fail_in.set(Some(n));
        let made = r.load_world(&bsp, None);
        if r.gpu.journal.fail_in.take().is_some() {
            made.unwrap();
            assert!(n > 15, "{n}");
            break;
        }
        assert!(made.is_err());
        assert!(n == 0 || r.world.is_none());
        assert_eq!(r.sky_layers, [NO_TEX; 2]);
        // twice in a row costs no more than once
        let once = usage(&r);
        r.gpu.journal.fail_in.set(Some(n.max(1)));
        assert!(r.load_world(&bsp, None).is_err());
        r.idle();
        if n > 0 {
            assert_eq!(usage(&r), once, "creation {n}");
        }
        r.render(&RenderFrame::default()).unwrap();
    }
    r.render(&RenderFrame { view: Some(View::default()), ..Default::default() }).unwrap();
}

#[test]
fn settings_that_do_not_fit_are_reduced_until_they_change() {
    let Some(mut r) = headless() else { return };
    if r.gpu.max_samples < 4 {
        return;
    }
    let frame = |msaa| RenderFrame { settings: Settings { msaa, render_scale: 2.0, ..Default::default() }, ..Default::default() };
    // creations it takes to go from the default settings to these
    r.render(&RenderFrame::default()).unwrap();
    r.gpu.journal.fail_in.set(Some(1000));
    r.render(&frame(4)).unwrap();
    let all = 1000 - r.gpu.journal.fail_in.take().unwrap();
    for fail in [0, 20, all / 2, all - 3, all - 1] {
        r.render(&RenderFrame::default()).unwrap();
        r.gpu.journal.fail_in.set(Some(fail));
        let e = r.render(&frame(4)).unwrap_err();
        assert!(e.message.starts_with("injected"), "{e}");
        assert!(r.gpu.journal.fail_in.take().is_none());
        for _ in 0..3 {
            r.render(&frame(4)).unwrap();
            assert_eq!((r.msaa, r.pipes.samples, r.render_scale), (1, 1, 1.0));
            assert_eq!(r.targets.as_ref().unwrap().scene_w, 64);
        }
        // any change is tried in full
        r.render(&frame(2)).unwrap();
        assert_eq!((r.msaa, r.render_scale), (2, 2.0));
    }
}

#[test]
fn a_failed_pipeline_build_leaves_no_handles() {
    let Some(mut r) = headless() else { return };
    r.render(&RenderFrame::default()).unwrap();
    r.idle();
    for n in [0, 1, 25, 60] {
        r.gpu.journal.fail_in.set(Some(n));
        assert!(r.pipes.build(&r.gpu, COLOR_FORMAT, 1).is_err());
        assert!(r.pipes.graphics_pipes().iter().all(|&p| p == vk::Pipeline::null()), "{n}");
        assert_eq!(r.pipes.samples, 0);
        // noticed although the settings are the same
        r.render(&RenderFrame::default()).unwrap();
        assert_eq!(r.pipes.samples, 1);
        r.idle();
    }
    r.gpu.journal.fail_in.set(Some(3));
    assert!(r.pipes.build(&r.gpu, COLOR_FORMAT, 1).is_err());
    // dropped without any
}

#[test]
fn wild_lod_biases_are_bounded() {
    let Some(mut r) = headless() else { return };
    let mut sampler = |bias: f32| {
        let fr = RenderFrame { settings: Settings { lod_bias: Some(bias), ..Default::default() }, ..Default::default() };
        r.render(&fr).unwrap();
        let (first, set) = (r.sampler_world, r.world_filter.2);
        r.render(&fr).unwrap();
        assert_eq!(r.sampler_world, first, "made again every frame");
        (set, r.gpu.max_lod_bias)
    };
    assert_eq!(sampler(f32::NAN).0, 0.0);
    assert_eq!(sampler(f32::INFINITY).0, 0.0);
    let (set, max) = sampler(1e9);
    assert_eq!(set, max);
    assert_eq!(sampler(-1e9).0, -max);
    assert_eq!(sampler(-0.5).0, -0.5);
}
