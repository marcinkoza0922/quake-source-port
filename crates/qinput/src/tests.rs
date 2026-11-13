use super::*;
use aim::{AimTarget, NoWorld, TargetKind};
use qcore::cmd::{Cbuf, Commands, execute};

struct Host {
    cbuf: Cbuf,
    cvars: Cvars,
    cmds: Commands<Host>,
    input: Input,
    out: String,
}
impl CmdContext for Host {
    fn cbuf(&mut self) -> &mut Cbuf {
        &mut self.cbuf
    }
    fn cvars(&mut self) -> &mut Cvars {
        &mut self.cvars
    }
    fn commands(&mut self) -> &mut Commands<Self> {
        &mut self.cmds
    }
    fn print(&mut self, t: &str) {
        self.out.push_str(t);
    }
}
impl InputHost for Host {
    fn input(&mut self) -> &mut Input {
        &mut self.input
    }
}
fn host() -> Host {
    let mut cvars = Cvars::new();
    let input = Input::new(&mut cvars);
    let mut cmds = Commands::with_builtins();
    register_commands(&mut cmds);
    Host { cbuf: Cbuf::new(), cvars, cmds, input, out: String::new() }
}
fn run(h: &mut Host, s: &str) {
    h.cbuf.add_text(s);
    execute(h);
}
fn frame(h: &mut Host, aim: Option<&AimContext>) -> InputState {
    h.input.frame(&h.cvars, [0.0; 3], 1.0 / 60.0, aim)
}

#[test]
fn weapon_slots_are_local_and_leave_mod_impulses_available() {
    let mut h = host();
    for (arg, expected) in [("1",1),("9",9),("0",10)] {
        run(&mut h, &format!("weapon_slot {arg}\nimpulse 240\n"));
        let state = frame(&mut h, None);
        assert_eq!((state.weapon_slot,state.impulse),(expected,240));
        assert_eq!(frame(&mut h,None).weapon_slot,0);
    }
    run(&mut h,"weapon_slot 11\n");
    assert_eq!(frame(&mut h,None).weapon_slot,0);
}

#[test]
fn bind_and_buttons() {
    let mut h = host();
    run(&mut h, "bind w +forward\nbind MOUSE1 +attack\nbind SPACE \"impulse 7\"\n");
    assert_eq!(h.input.binding(b'w' as i32), Some("+forward"));
    key_event(&mut h, b'w' as i32, true);
    execute(&mut h);
    assert!(h.input.buttons[btn::FORWARD].is_down());
    assert_eq!(frame(&mut h, None).forward, 0.5);
    assert_eq!(frame(&mut h, None).forward, 1.0);
    // autorepeat ignored
    key_event(&mut h, b'w' as i32, true);
    key_event(&mut h, b'w' as i32, false);
    execute(&mut h);
    assert!(!h.input.buttons[btn::FORWARD].is_down());
    key_event(&mut h, K_SPACE, true);
    execute(&mut h);
    assert_eq!(frame(&mut h, None).impulse, 7);
    assert_eq!(frame(&mut h, None).impulse, 0);
    // a tap shorter than a frame still fires attack
    handle_event(&mut h, InputEvent::MouseButton { button: 1, down: true });
    handle_event(&mut h, InputEvent::MouseButton { button: 1, down: false });
    execute(&mut h);
    assert!(frame(&mut h, None).attack);
    assert!(!frame(&mut h, None).attack);
    run(&mut h, "bindlist\n");
    assert!(h.out.contains("3 bindings"));
    run(&mut h, "unbind w\n");
    assert_eq!(h.input.binding(b'w' as i32), None);
    run(&mut h, "bind ABUTTON +jump\nunbindall\n");
    assert!(h.input.bindings.iter().all(Option::is_none));
}

#[test]
fn dest_routing_and_release() {
    let mut h = host();
    run(&mut h, "bind w +forward\n");
    key_event(&mut h, b'w' as i32, true);
    execute(&mut h);
    set_key_dest(&mut h, KeyDest::Console);
    execute(&mut h);
    assert!(!h.input.buttons[btn::FORWARD].is_down(), "released on dest change");
    assert_eq!(key_event(&mut h, b'a' as i32, true), Some(Routed::Key { key: b'a' as i32, dest: KeyDest::Console }));
    assert_eq!(handle_event(&mut h, InputEvent::Text('a')), Some(Routed::Text { ch: 'a', dest: KeyDest::Console }));
    set_key_dest(&mut h, KeyDest::Game);
    assert_eq!(key_event(&mut h, K_ESCAPE, true), Some(Routed::Escape));
    // binding pressed in console is not released as game command
    assert!(h.cbuf.is_empty());
}

#[test]
fn triggers_are_buttons() {
    let mut h = host();
    run(&mut h, "bind RTRIGGER +attack\n");
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightTrigger, value: 0.1 });
    execute(&mut h);
    assert!(!h.input.buttons[btn::ATTACK].is_down());
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightTrigger, value: 0.6 });
    execute(&mut h);
    assert!(h.input.buttons[btn::ATTACK].is_down());
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightTrigger, value: 0.0 });
    execute(&mut h);
    assert!(!h.input.buttons[btn::ATTACK].is_down());
}

#[test]
fn mouse_look() {
    let mut h = host();
    handle_event(&mut h, InputEvent::MouseMotion { dx: 10.0, dy: 5.0 });
    let s = frame(&mut h, None);
    assert!((s.look_delta[0] + 10.0 * 3.0 * 0.022).abs() < 1e-5);
    assert!((s.look_delta[1] - 5.0 * 3.0 * 0.022).abs() < 1e-5);
    h.cvars.set("m_invert", "1");
    h.cvars.set("m_filter", "1");
    handle_event(&mut h, InputEvent::MouseMotion { dx: 0.0, dy: 10.0 });
    let s = frame(&mut h, None);
    assert!((s.look_delta[1] + 7.5 * 3.0 * 0.022).abs() < 1e-5, "{:?}", s.look_delta);
    h.cvars.set("freelook", "0");
    handle_event(&mut h, InputEvent::MouseMotion { dx: 0.0, dy: -20.0 });
    let s = frame(&mut h, None);
    assert_eq!(s.look_delta[1], 0.0);
    assert!(s.forward > 0.0);
}

#[test]
fn gamepad_move_and_look() {
    let mut h = host();
    handle_event(&mut h, InputEvent::GamepadConnected(true));
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::LeftY, value: -1.0 });
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightX, value: 1.0 });
    let s = frame(&mut h, None);
    assert!((s.forward - 1.0).abs() < 1e-5);
    assert!((s.look_delta[0] + 240.0 / 60.0).abs() < 1e-3);
    // ramp kicks in after sustained deflection
    let mut last = 0.0;
    for _ in 0..60 {
        last = frame(&mut h, None).look_delta[0];
    }
    assert!((last + 1.6 * 240.0 / 60.0).abs() < 1e-3, "{last}");
    // inside deadzone: nothing
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightX, value: 0.1 });
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::LeftY, value: 0.1 });
    let s = frame(&mut h, None);
    assert_eq!((s.forward, s.look_delta), (0.0, [0.0, 0.0]));
}

#[test]
fn gyro_look() {
    let mut h = host();
    h.cvars.set("gyro_enable", "1");
    handle_event(&mut h, InputEvent::GamepadConnected(true));
    handle_event(&mut h, InputEvent::Gyro { pitch: 0.0, yaw: 1.0 });
    let s = frame(&mut h, None);
    assert!((s.look_delta[0] - 1f32.to_degrees() * 2.5 / 60.0).abs() < 1e-3);
    h.cvars.set("gyro_mode", "2"); // needs +gyroaction
    assert_eq!(frame(&mut h, None).look_delta[0], 0.0);
}

fn target() -> AimTarget {
    AimTarget { id: 1, origin: [400.0, 0.0, 0.0], mins: [-16.0; 3], maxs: [16.0; 3], velocity: [0.0, 200.0, 0.0], kind: TargetKind::Monster }
}

#[test]
fn aim_assist_gamepad_only() {
    let mut h = host();
    let t = [target()];
    let ctx = AimContext { eye: [0.0; 3], player_velocity: [0.0; 3], targets: &t, vis: &NoWorld, allowed: true };
    // mouse user: no effect even with pad connected
    handle_event(&mut h, InputEvent::GamepadConnected(true));
    handle_event(&mut h, InputEvent::MouseMotion { dx: 3.0, dy: 0.0 });
    let with = frame(&mut h, Some(&ctx));
    handle_event(&mut h, InputEvent::MouseMotion { dx: 3.0, dy: 0.0 });
    let without = frame(&mut h, None);
    assert_eq!(with.look_delta, without.look_delta);
    assert_eq!(h.input.last_aim, AimResult::NONE);
    // gamepad user moving: slowdown + rotation applied
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::LeftX, value: 1.0 });
    let s = frame(&mut h, Some(&ctx));
    assert_eq!(h.input.last_aim.target, Some(0));
    assert!(h.input.last_aim.scale < 1.0);
    assert!(s.look_delta[0] > 0.0, "tracks target moving left");
    // disabled by cvar
    h.cvars.set("aim_assist", "0");
    frame(&mut h, Some(&ctx));
    assert_eq!(h.input.last_aim, AimResult::NONE);
}

#[test]
fn cvar_list_registered() {
    let h = host();
    for (n, d) in CVAR_LIST {
        assert_eq!(h.cvars.by_name(n).unwrap().string, *d);
    }
    assert!(h.input.archive_bindings()[0] == "unbindall");
}

#[test]
fn last_device_follows_input() {
    let mut h = host();
    handle_event(&mut h, InputEvent::GamepadConnected(true));
    assert_eq!(h.input.last_device, Device::KeyboardMouse);
    handle_event(&mut h, InputEvent::GamepadButton { button: GamepadButton::South, down: true });
    assert_eq!(h.input.last_device, Device::Gamepad);
    // resting-mouse jitter keeps the pad
    handle_event(&mut h, InputEvent::MouseMotion { dx: 1.0, dy: 0.0 });
    assert_eq!(h.input.last_device, Device::Gamepad);
    handle_event(&mut h, InputEvent::Key { key: b'w' as i32, down: true });
    assert_eq!(h.input.last_device, Device::KeyboardMouse);
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::LeftX, value: 0.9 });
    assert_eq!(h.input.last_device, Device::Gamepad);
    handle_event(&mut h, InputEvent::GamepadConnected(false));
    assert_eq!(h.input.last_device, Device::KeyboardMouse);
}

#[test]
fn wheel_takes_look_stick() {
    let mut h = host();
    handle_event(&mut h, InputEvent::GamepadConnected(true));
    run(&mut h, "bind XBUTTON +weaponwheel\n");
    handle_event(&mut h, InputEvent::GamepadButton { button: GamepadButton::West, down: true });
    execute(&mut h);
    assert!(h.input.wheel_held());
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightX, value: 1.0 });
    let s = h.input.frame(&h.cvars, [0.0; 3], 0.1, None);
    assert_eq!(s.look_delta, [0.0, 0.0]);
    assert_eq!(h.input.wheel_aim, [1.0, 0.0]);
    // the left stick works too, and doesn't move the player
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightX, value: 0.0 });
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::LeftY, value: -1.0 });
    let s = h.input.frame(&h.cvars, [0.0; 3], 0.1, None);
    assert_eq!(h.input.wheel_aim, [0.0, -1.0]);
    assert_eq!(s.forward, 0.0);
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::LeftY, value: 0.0 });
    handle_event(&mut h, InputEvent::GamepadAxis { axis: GamepadAxis::RightX, value: 1.0 });
    handle_event(&mut h, InputEvent::GamepadButton { button: GamepadButton::West, down: false });
    execute(&mut h);
    let s = h.input.frame(&h.cvars, [0.0; 3], 0.1, None);
    assert!(s.look_delta[0] < 0.0);
    assert_eq!(h.input.wheel_aim, [0.0, 0.0]);
}

#[test]
fn back_button_is_bindable() {
    assert_eq!(GamepadButton::Back.key(), keys::K_BACK);
    assert_eq!(keys::string_to_keynum("BACK"), keys::K_BACK);
}

#[test]
fn lockon_binding_overrides_look_and_cycles_without_normal_stick_action() {
    let mut h = host();
    h.cvars.set("assist_autoaim", "1");
    h.input.autoaim_allowed = true;
    run(&mut h, "bind q +lockon\nbind RTHUMB centerview\n");
    key_event(&mut h, b'q' as i32, true);
    execute(&mut h);
    let targets = [
        AimTarget { id: 1, origin: [200.0, 20.0, 0.0], ..target() },
        AimTarget { id: 2, origin: [300.0, -20.0, 0.0], ..target() },
    ];
    let ctx = AimContext { eye: [0.0; 3], player_velocity: [0.0; 3], targets: &targets, vis: &NoWorld, allowed: false };
    h.input.mouse = [100.0, 100.0];
    let s = frame(&mut h, Some(&ctx));
    assert_eq!(h.input.lockon.target, Some(1));
    assert!((s.look_delta[0] - 5.7106).abs() < 0.01);
    key_event(&mut h, K_RTHUMB, true);
    assert!(h.cbuf.next_line().is_none(), "stick binding must not run while switching targets");
    frame(&mut h, Some(&ctx));
    assert_eq!(h.input.lockon.target, Some(2));
    key_event(&mut h, b'q' as i32, false);
    execute(&mut h);
    frame(&mut h, Some(&ctx));
    assert_eq!(h.input.lockon.target, None);
    h.input.autoaim_allowed = false;
    run(&mut h, "+lockon\n");
    frame(&mut h, Some(&ctx));
    assert_eq!(h.input.lockon.target, None);
}
