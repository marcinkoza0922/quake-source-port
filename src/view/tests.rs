use super::*;
use qclient::{ClientEntity, LERP_RESETANIM, LERP_RESETANIM2, LERP_RESETMOVE};

#[test]
fn appearing_monster_snaps_then_smooths_normal_steps() {
    let mut cl = ClientState::new();
    cl.protocol = PROTOCOL_NETQUAKE;
    cl.model_precache = vec![String::new(), "progs/ogre.mdl".into()];
    cl.entities.resize(3, ClientEntity::default());
    cl.entities[2].baseline = qclient::EntityState {
        modelindex: 1, origin: [256.0, 128.0, 24.0], ..Default::default()
    };
    // Exercise both a first appearance and reuse of old renderer history.
    for stale in [[0.0; 3], [-128.0, 64.0, 24.0]] {
        cl.mtime = [1.0, 0.9];
        cl.entities[2].msgtime = 0.5;
        let mut anim = Anim { cur_origin: stale, ..Default::default() };
        cl.parse_server_message(&[(U_SIGNAL | U_STEP) as u8, 2]).unwrap();
        cl.relink_entities();
        let e = &mut cl.entities[2];
        let start = e.origin;
        assert_eq!(ViewState::move_lerp(&mut anim, e, 1.0, true).0, start);
        assert_eq!(e.lerpflags & LERP_RESETMOVE, 0);

        // U_STEP is also sent for normal movement; it must not reset smoothing.
        cl.mtime = [1.1, 1.0];
        cl.entities[2].baseline.origin[0] += 20.0;
        cl.parse_server_message(&[(U_SIGNAL | U_STEP) as u8, 2]).unwrap();
        cl.relink_entities();
        let e = &mut cl.entities[2];
        assert_eq!(ViewState::move_lerp(&mut anim, e, 1.1, true).0, start);
        let halfway = ViewState::move_lerp(&mut anim, e, 1.15, true).0;
        assert!((halfway[0] - start[0] - 10.0).abs() < 0.001);
    }
}

#[test]
fn muzzleflash_resetanim_and_resetanim2_skip_lerp_both_ways() {
    let mut anim = Anim::default();
    let mut flags = 0u32;
    // Idle at frame 10.
    assert_eq!(ViewState::alias_frame(&mut anim, 1, 10, &mut flags, 1.0, true, false, 0.0), (10, 10, 1.0));

    // Muzzleflash fires: server sets EF_MUZZLEFLASH -> relink sets LERP_RESETANIM | LERP_RESETANIM2.
    flags |= LERP_RESETANIM | LERP_RESETANIM2;
    let (f, prev, _) = ViewState::alias_frame(&mut anim, 1, 11, &mut flags, 1.02, true, false, 0.0);
    assert_eq!((f, prev), (11, 11), "flash frame must snap, not lerp from idle");
    assert_eq!(flags & LERP_RESETANIM, 0);
    assert_ne!(flags & LERP_RESETANIM2, 0, "RESETANIM2 stays until next pose change");

    // Next render tick on the same flash frame keeps RESETANIM2 armed.
    let (f, prev, _) = ViewState::alias_frame(&mut anim, 1, 11, &mut flags, 1.03, true, false, 0.0);
    assert_eq!((f, prev), (11, 11));
    assert_ne!(flags & LERP_RESETANIM2, 0);

    // Next animation frame (flash disappears): RESETANIM2 prevents lerping out of flash.
    let (f, prev, _) = ViewState::alias_frame(&mut anim, 1, 12, &mut flags, 1.05, true, false, 0.0);
    assert_eq!((f, prev), (12, 12), "post-flash frame must snap, not morph flash back in");
    assert_eq!(flags & LERP_RESETANIM2, 0);

    // Subsequent recoil frames lerp smoothly.
    let (f, prev, blend) = ViewState::alias_frame(&mut anim, 1, 13, &mut flags, 1.075, true, false, 0.0);
    assert_eq!((f, prev, blend), (13, 12, 0.0));
    let (f, prev, blend) = ViewState::alias_frame(&mut anim, 1, 13, &mut flags, 1.125, true, false, 0.0);
    assert_eq!((f, prev), (13, 12));
    assert!((blend - 0.5).abs() < 0.001);
}

fn idle_cvars() -> ViewCvars {
    ViewCvars {
        bob: 0.0,
        bobcycle: 0.6,
        bobup: 0.5,
        rollangle: 0.0,
        rollspeed: 200.0,
        kicktime: 0.5,
        kickroll: 0.0,
        kickpitch: 0.0,
        fov: 90.0,
        gunkick: false,
        speed_blur: false,
        viewmodel: weapon::Alignment::Center,
        lerp_models: false,
        lerp_move: false,
        flash: 1.0,
        idle: Idle::VANILLA,
        cshift: [1.0; 4],
    }
}

fn idle_client() -> ClientState {
    let mut cl = ClientState::new();
    cl.entities.resize(2, Default::default());
    cl.viewentity = 1;
    cl.stats[STAT_HEALTH] = 100;
    cl.viewangles = [10.0, 90.0, 0.0];
    cl.time = 1.25;
    cl
}

/// V_AddIdle sways the view by scale * sin(time * cycle) * level; the gun
/// takes it back out (CalcGunAngle), and intermissions always sway.
#[test]
fn idle_sway() {
    let mut cl = idle_client();
    let lv = Level::default();
    let mut cv = idle_cvars();
    let (still, _) = ViewState::default().calc_refdef(&mut cl, &cv, &lv, 0.0);
    assert_eq!(still.angles, [10.0, 90.0, 0.0]);
    cv.idle.scale = 2.0;
    let (v, _) = ViewState::default().calc_refdef(&mut cl, &cv, &lv, 0.0);
    let want = |cycle: f64, level: f32| 2.0 * (1.25 * cycle).sin() as f32 * level;
    assert!((v.angles[0] - 10.0 - want(1.0, 0.3)).abs() < 1e-5);
    assert!((v.angles[1] - 90.0 - want(2.0, 0.3)).abs() < 1e-5);
    assert!((v.angles[2] - want(0.5, 0.1)).abs() < 1e-5);
    // reduce motion turns it off
    let (v, _) = ViewState::default().calc_refdef(&mut cl, &cv.reduce_motion(true), &lv, 0.0);
    assert_eq!(v.angles, [10.0, 90.0, 0.0]);
    // intermission: scale 1 whatever v_idlescale says
    cl.intermission = 1;
    cl.entities[1].angles = [0.0, 45.0, 0.0];
    let (v, gun) = ViewState::default().calc_refdef(&mut cl, &idle_cvars(), &lv, 0.0);
    assert!(gun.is_none());
    assert!((v.angles[1] - 45.0 - want(2.0, 0.3) / 2.0).abs() < 1e-5);
}

/// V_CalcBlend: gl_cshiftpercent scales the tints, intermissions keep only
/// the contents shift.
#[test]
fn blend_percentages() {
    let mut cl = idle_client();
    let mut vs = ViewState::default();
    vs.bonus_flash();
    let full = vs.blend(&cl, CONTENTS_EMPTY, 0.0, 1.0, [1.0; 4]);
    assert!((full[3] - 50.0 / 255.0).abs() < 1e-5);
    let half = vs.blend(&cl, CONTENTS_EMPTY, 0.0, 1.0, [1.0, 1.0, 0.5, 1.0]);
    assert!((half[3] - 25.0 / 255.0).abs() < 1e-5);
    assert_eq!(vs.blend(&cl, CONTENTS_EMPTY, 0.0, 1.0, [0.0; 4])[3], 0.0);
    cl.intermission = 1;
    assert_eq!(vs.blend(&cl, CONTENTS_EMPTY, 0.0, 1.0, [1.0; 4])[3], 0.0);
    assert!(vs.blend(&cl, CONTENTS_LAVA, 0.0, 1.0, [1.0; 4])[3] > 0.5);
}
