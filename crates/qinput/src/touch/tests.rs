use super::*;
const LAY: TouchLayout = TouchLayout { aspect: 16.0 / 9.0, size: 1.0, left_handed: false, insets: [0.0; 4], placements: [None; BUTTONS.len()] };

#[test]
fn fire_button_holds_and_looks() {
    let mut t = TouchOverlay::default();
    let (c, _) = LAY.button(0);
    assert_eq!(t.event(1, TouchPhase::Down, c, 0, &LAY, TouchLook::default()), vec!["+attack 1000\n"]);
    assert!(t.pressed(0));
    t.event(1, TouchPhase::Move, [c[0] + 0.01, c[1]], 0, &LAY, TouchLook::default());
    assert!(t.take_look()[0] > 0.0);
    assert_eq!(t.event(1, TouchPhase::Up, c, 0, &LAY, TouchLook::default()), vec!["-attack 1000\n"]);
    assert!(!t.pressed(0));
}

#[test]
fn left_half_is_a_floating_stick() {
    let mut t = TouchOverlay::default();
    t.event(7, TouchPhase::Down, [0.2, 0.6], 0, &LAY, TouchLook::default());
    t.event(7, TouchPhase::Move, [0.2, 0.6 - 0.06], 0, &LAY, TouchLook::default());
    let v = t.move_vec(&LAY);
    assert!(v[0].abs() < 1e-4 && (v[1] + 0.5).abs() < 1e-3, "{v:?}");
    // past the rim: full deflection, base dragged along
    t.event(7, TouchPhase::Move, [0.2, 0.1], 0, &LAY, TouchLook::default());
    assert!((t.move_vec(&LAY)[1] + 1.0).abs() < 1e-4);
    assert!(t.stick().unwrap().0[1] < 0.6);
    t.event(7, TouchPhase::Up, [0.2, 0.1], 0, &LAY, TouchLook::default());
    assert_eq!(t.move_vec(&LAY), [0.0; 2]);
}

#[test]
fn right_half_looks_and_pause_is_one_shot() {
    let mut t = TouchOverlay::default();
    t.event(2, TouchPhase::Down, [0.6, 0.3], 0, &LAY, TouchLook::default());
    t.event(2, TouchPhase::Move, [0.6, 0.35], 0, &LAY, TouchLook::default());
    let l = t.take_look();
    assert!(l[0] == 0.0 && (l[1] - 0.05 * FALLBACK_HEIGHT_CM).abs() < 1e-5);
    assert_eq!(t.take_look(), [0.0; 2]);
    let (c, _) = LAY.button(PAUSE_BUTTON);
    assert_eq!(t.event(3, TouchPhase::Down, c, 0, &LAY, TouchLook::default()), vec!["togglemenu\n"]);
    assert!(t.event(3, TouchPhase::Up, c, 0, &LAY, TouchLook::default()).is_empty());
}

#[test]
fn release_all_lets_go_of_held_buttons() {
    let mut t = TouchOverlay::default();
    let (c, _) = LAY.button(1);
    t.event(4, TouchPhase::Down, c, 0, &LAY, TouchLook::default());
    t.event(5, TouchPhase::Down, [0.1, 0.5], 0, &LAY, TouchLook::default());
    assert_eq!(t.release_all(), vec!["-jump 1001\n"]);
    assert!(!t.any_finger());
}

#[test]
fn buttons_dont_overlap() {
    for lay in [LAY, TouchLayout::new(4.0 / 3.0, 2.0, true, [0.12, 0.06, 0.04, 0.07])] {
        for (i, a) in BUTTONS.iter().enumerate() {
            let ([x, y], r) = lay.button(i);
            let [l, t, right, bot] = lay.insets;
            let r = r * HIT_SCALE;
            assert!(x - r / lay.aspect >= l && x + r / lay.aspect <= 1.0 - right);
            assert!(y - r >= t && y + r <= 1.0 - bot);
            for (j, b) in BUTTONS.iter().enumerate().skip(i + 1) {
                let ((ca, ra), (cb, rb)) = (lay.button(i), lay.button(j));
                assert!(lay.dist(ca, cb) > (ra + rb) * 1.15, "{} / {}", a.label, b.label);
            }
        }
    }
}

#[test]
fn left_handed_layout_swaps_stick_and_look_zones() {
    let lay = TouchLayout { left_handed: true, ..LAY };
    let mut t = TouchOverlay::default();
    t.event(1, TouchPhase::Down, [0.15, 0.5], 0, &lay, TouchLook::default());
    t.event(1, TouchPhase::Move, [0.2, 0.5], 0, &lay, TouchLook::default());
    t.event(2, TouchPhase::Down, [0.8, 0.6], 0, &lay, TouchLook::default());
    t.event(2, TouchPhase::Move, [0.8, 0.55], 0, &lay, TouchLook::default());
    assert!(t.take_look()[0] > 0.0);
    assert!(t.move_vec(&lay)[1] < 0.0);
}

#[test]
fn wheel_keeps_final_swipe_and_cancels_without_selection() {
    let mut t = TouchOverlay::default();
    let (p, _) = LAY.button(WHEEL);
    t.event(1, TouchPhase::Down, p, 0, &LAY, TouchLook::default());
    assert_eq!(t.take_wheel(&LAY), Some(WheelGesture::Held([0.0; 2])));
    let end = [p[0], p[1] - 0.1];
    t.event(1, TouchPhase::Move, end, 0, &LAY, TouchLook::default());
    assert_eq!(t.take_look(), [0.0; 2]);
    t.event(1, TouchPhase::Up, end, 0, &LAY, TouchLook::default());
    assert_eq!(t.take_wheel(&LAY), Some(WheelGesture::Released([0.0, -1.0])));
    assert_eq!(t.take_wheel(&LAY), None);
    t.event(2, TouchPhase::Down, p, 0, &LAY, TouchLook::default());
    t.event(2, TouchPhase::Cancel, end, 0, &LAY, TouchLook::default());
    assert_eq!(t.take_wheel(&LAY), None);
}

#[test]
fn physical_drag_distance_is_independent_of_screen_size() {
    let drag = |size_cm, distance: f32| {
        let mut touch = TouchOverlay::default();
        let look = TouchLook { size_cm, acceleration: 0.0 };
        touch.event(1, TouchPhase::Down, [0.6, 0.2], 0, &LAY, look);
        touch.event(1, TouchPhase::Move, [0.6, 0.2 + distance], 10_000_000, &LAY, look);
        touch.take_look()[1]
    };
    for (height, fraction) in [(6.5, 1.0 / 6.5), (13.0, 1.0 / 13.0)] {
        assert!((drag(Some([height * LAY.aspect, height]), fraction) - 1.0).abs() < 1e-5);
    }
    assert!((drag(None, 0.5) * 28.0 - 90.0).abs() < 1e-4);
}

#[test]
fn acceleration_uses_finger_sample_time_and_leaves_slow_drags_linear() {
    let drag = |elapsed: u64, acceleration| {
        let mut touch = TouchOverlay::default();
        let look = TouchLook { size_cm: Some([20.0, 10.0]), acceleration };
        touch.event(1, TouchPhase::Down, [0.6, 0.2], 1_000_000, &LAY, look);
        touch.event(1, TouchPhase::Move, [0.6, 0.3], 1_000_000 + elapsed, &LAY, look);
        touch.take_look()[1]
    };
    assert!((drag(100_000_000, 0.5) - 1.0).abs() < 1e-5);
    assert!((drag(10_000_000, 0.5) - 1.5).abs() < 1e-5);
    assert!((drag(10_000_000, 0.0) - 1.0).abs() < 1e-5);
}

#[test]
fn custom_button_moves_hit_area_and_mirrors_inside_safe_edges() {
    let mut layout = TouchLayout::new(1.8, 1.0, false, [0.12, 0.03, 0.04, 0.06]);
    let (old, radius) = layout.button(0);
    layout.placements[0] = Some(layout.placement_at([0.3, 0.35], 1.5));
    let (center, resized) = layout.button(0);
    assert!((resized / radius - 1.5).abs() < 1e-6);
    assert_eq!(layout.button_at(old), None);
    let mut touch = TouchOverlay::default();
    assert_eq!(touch.event(1, TouchPhase::Down, center, 0, &layout, TouchLook::default()), ["+attack 1000\n"]);
    layout.left_handed = true;
    let ([mirrored, y], _) = layout.button(0);
    assert!((mirrored + center[0] - 1.08).abs() < 1e-6);
    assert_eq!(y, center[1]);
    layout.placements[0] = Some(layout.placement_at([0.99, 0.99], 2.0));
    let ([x, y], radius) = layout.button(0);
    assert!(x + radius / layout.aspect < 0.96 && y + radius < 0.94);
}
