//! qinput — binds, +/- buttons, mouse, gamepad curves, aim assist. No SDL;
//! the binary maps platform events to [`InputEvent`]. See docs/DESIGN.md.

pub mod aim;
pub mod button;
pub mod gamepad;
pub mod keys;

use aim::{AimContext, AimParams, AimResult};
use button::{KButton, key_arg};
use gamepad::{AccelRamp, FlickParams, FlickStick};
use keys::*;
use qclient::InputState;
use qcore::cmd::{Args, CmdContext};
use qcore::cvar::{CVAR_ARCHIVE, CvarId, Cvars};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadButton {
    South,
    East,
    West,
    North,
    Back,
    Guide,
    Start,
    LeftStick,
    RightStick,
    LeftShoulder,
    RightShoulder,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    Misc1,
    Paddle1,
    Paddle2,
    Paddle3,
    Paddle4,
    Touchpad,
}

impl GamepadButton {
    /// Key number (Start/Back map to ESCAPE/TAB like ironwail).
    pub fn key(self) -> i32 {
        use GamepadButton::*;
        match self {
            South => K_ABUTTON,
            East => K_BBUTTON,
            West => K_XBUTTON,
            North => K_YBUTTON,
            Back => K_TAB,
            Guide | Start => K_ESCAPE,
            LeftStick => K_LTHUMB,
            RightStick => K_RTHUMB,
            LeftShoulder => K_LSHOULDER,
            RightShoulder => K_RSHOULDER,
            DpadUp => K_DPAD_UP,
            DpadDown => K_DPAD_DOWN,
            DpadLeft => K_DPAD_LEFT,
            DpadRight => K_DPAD_RIGHT,
            Misc1 => K_MISC1,
            Paddle1 => K_PADDLE1,
            Paddle2 => K_PADDLE2,
            Paddle3 => K_PADDLE3,
            Paddle4 => K_PADDLE4,
            Touchpad => K_TOUCHPAD,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GamepadAxis {
    LeftX,
    LeftY,
    RightX,
    RightY,
    LeftTrigger,
    RightTrigger,
}

/// Platform-neutral input event (mirrored from qplatform by the binary).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum InputEvent {
    /// Key number from [`keys`] (lowercase ASCII for printable keys).
    Key { key: i32, down: bool },
    Text(char),
    MouseMotion { dx: f32, dy: f32 },
    /// + = up.
    MouseWheel { y: f32 },
    MouseButton { button: u8, down: bool },
    GamepadButton { button: GamepadButton, down: bool },
    /// Sticks -1..1 (+y = down), triggers 0..1.
    GamepadAxis { axis: GamepadAxis, value: f32 },
    /// Angular rates in rad/s: pitch (+ = tilt up), yaw (+ = turn left).
    Gyro { pitch: f32, yaw: f32 },
    GamepadConnected(bool),
}

/// Events the console/menu must handle (not routed to binds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Routed {
    Key { key: i32, dest: KeyDest },
    Text { ch: char, dest: KeyDest },
    /// Escape pressed in game: toggle menu.
    Escape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LookDevice {
    #[default]
    Mouse,
    Gamepad,
}

/// Button indices into [`Input::buttons`].
pub mod btn {
    pub const FORWARD: usize = 0;
    pub const BACK: usize = 1;
    pub const MOVELEFT: usize = 2;
    pub const MOVERIGHT: usize = 3;
    pub const MOVEUP: usize = 4;
    pub const MOVEDOWN: usize = 5;
    pub const LEFT: usize = 6;
    pub const RIGHT: usize = 7;
    pub const LOOKUP: usize = 8;
    pub const LOOKDOWN: usize = 9;
    pub const SPEED: usize = 10;
    pub const STRAFE: usize = 11;
    pub const ATTACK: usize = 12;
    pub const JUMP: usize = 13;
    pub const MLOOK: usize = 14;
    pub const KLOOK: usize = 15;
    pub const USE: usize = 16;
    pub const GYRO: usize = 17;
    pub const COUNT: usize = 18;
    pub const NAMES: [&str; COUNT] = [
        "forward", "back", "moveleft", "moveright", "moveup", "movedown", "left", "right", "lookup", "lookdown",
        "speed", "strafe", "attack", "jump", "mlook", "klook", "use", "gyroaction",
    ];
}

macro_rules! cvars {
    ($($field:ident = $name:literal, $def:literal;)*) => {
        /// Registered cvar handles.
        #[derive(Clone, Copy, Debug)]
        pub struct InputCvars { $(pub $field: CvarId,)* }
        impl InputCvars {
            pub fn register(cv: &mut Cvars) -> Self {
                Self { $($field: cv.register($name, $def, CVAR_ARCHIVE),)* }
            }
        }
        /// (name, default) of every cvar qinput registers.
        pub const CVAR_LIST: &[(&str, &str)] = &[$(($name, $def),)*];
    };
}

cvars! {
    sensitivity = "sensitivity", "3";
    m_pitch = "m_pitch", "0.022";
    m_yaw = "m_yaw", "0.022";
    m_forward = "m_forward", "1";
    m_side = "m_side", "0.8";
    m_filter = "m_filter", "0";
    m_invert = "m_invert", "0";
    freelook = "freelook", "1";
    lookstrafe = "lookstrafe", "0";
    joy_enable = "joy_enable", "1";
    joy_deadzone_look = "joy_deadzone_look", "0.175";
    joy_deadzone_move = "joy_deadzone_move", "0.175";
    joy_outer_threshold_look = "joy_outer_threshold_look", "0.02";
    joy_outer_threshold_move = "joy_outer_threshold_move", "0.02";
    joy_deadzone_trigger = "joy_deadzone_trigger", "0.2";
    joy_sensitivity_yaw = "joy_sensitivity_yaw", "240";
    joy_sensitivity_pitch = "joy_sensitivity_pitch", "130";
    joy_invert = "joy_invert", "0";
    joy_exponent = "joy_exponent", "2";
    joy_exponent_move = "joy_exponent_move", "2";
    joy_swapmovelook = "joy_swapmovelook", "0";
    joy_accel = "joy_accel", "1.6";
    joy_accel_threshold = "joy_accel_threshold", "0.95";
    joy_accel_delay = "joy_accel_delay", "0.15";
    joy_accel_ramp = "joy_accel_ramp", "0.35";
    joy_flick = "joy_flick", "0";
    joy_flick_time = "joy_flick_time", "0.125";
    joy_flick_deadzone = "joy_flick_deadzone", "0.9";
    joy_flick_noise_thresh = "joy_flick_noise_thresh", "2.0";
    joy_flick_adjust_speed = "joy_flick_adjust_speed", "30.0";
    gyro_enable = "gyro_enable", "0";
    gyro_mode = "gyro_mode", "0";
    gyro_yawsensitivity = "gyro_yawsensitivity", "2.5";
    gyro_pitchsensitivity = "gyro_pitchsensitivity", "2.5";
    gyro_noise_thresh = "gyro_noise_thresh", "1.5";
    aim_assist = "aim_assist", "1";
    aim_cone = "aim_cone", "6";
    aim_range = "aim_range", "2048";
    aim_slowdown = "aim_slowdown", "0.4";
    aim_rotation = "aim_rotation", "0.5";
    aim_snap = "aim_snap", "0";
    aim_weight_dist = "aim_weight_dist", "0.3";
    aim_target_players = "aim_target_players", "1";
}

/// Host context for qinput commands.
pub trait InputHost: CmdContext {
    fn input(&mut self) -> &mut Input;
}

#[derive(Debug)]
pub struct Input {
    pub cv: InputCvars,
    pub bindings: Vec<Option<String>>,
    pub buttons: [KButton; btn::COUNT],
    pub dest: KeyDest,
    /// Key held, and whether its press ran a game binding.
    keydown: [bool; MAX_KEYS],
    game_down: [bool; MAX_KEYS],
    impulse: u8,
    mouse: [f32; 2],
    old_mouse: [f32; 2],
    axes: [f32; 6],
    gyro: [f32; 2],
    pub gamepad_connected: bool,
    pub last_look: LookDevice,
    accel: AccelRamp,
    flick: FlickStick,
    /// Last aim assist result (for HUD/debug).
    pub last_aim: AimResult,
}

impl Input {
    pub fn new(cvars: &mut Cvars) -> Self {
        Self {
            cv: InputCvars::register(cvars),
            bindings: vec![None; MAX_KEYS],
            buttons: [KButton::default(); btn::COUNT],
            dest: KeyDest::Game,
            keydown: [false; MAX_KEYS],
            game_down: [false; MAX_KEYS],
            impulse: 0,
            mouse: [0.0; 2],
            old_mouse: [0.0; 2],
            axes: [0.0; 6],
            gyro: [0.0; 2],
            gamepad_connected: false,
            last_look: LookDevice::Mouse,
            accel: AccelRamp::default(),
            flick: FlickStick::default(),
            last_aim: AimResult::NONE,
        }
    }

    pub fn set_binding(&mut self, key: i32, cmd: Option<&str>) {
        if let Some(b) = self.bindings.get_mut(key as usize) {
            *b = cmd.filter(|s| !s.is_empty()).map(str::to_string);
        }
    }
    pub fn binding(&self, key: i32) -> Option<&str> {
        self.bindings.get(key as usize)?.as_deref()
    }
    pub fn key_down(&self, key: i32) -> bool {
        self.keydown.get(key as usize).copied().unwrap_or(false)
    }
    /// Key_WriteBindings lines.
    pub fn archive_bindings(&self) -> Vec<String> {
        let mut v = vec!["unbindall".to_string()];
        for (k, b) in self.bindings.iter().enumerate() {
            if let Some(b) = b {
                v.push(format!("bind \"{}\" \"{}\"", keynum_to_string(k as i32), b));
            }
        }
        v
    }
}

use qcore::cmd::Commands;

macro_rules! button_cmds {
    ($c:expr, $($i:expr),*) => {$(
        $c.add(&format!("+{}", btn::NAMES[$i]), |ctx, a| ctx.input().buttons[$i].press(key_arg(a)));
        $c.add(&format!("-{}", btn::NAMES[$i]), |ctx, a| ctx.input().buttons[$i].release(key_arg(a)));
    )*};
}

/// Register bind/unbind/unbindall/bindlist, +/- buttons and impulse.
pub fn register_commands<C: InputHost>(c: &mut Commands<C>) {
    button_cmds!(c, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17);
    c.add("impulse", |ctx, a| ctx.input().impulse = a.argv(1).parse().unwrap_or(0));
    c.add("bind", cmd_bind);
    c.add("unbind", |ctx, a| {
        if a.argc() != 2 {
            ctx.print("unbind <key> : remove commands from a key\n");
            return;
        }
        let k = string_to_keynum(a.argv(1));
        if k == -1 {
            ctx.print(&format!("\"{}\" isn't a valid key\n", a.argv(1)));
            return;
        }
        ctx.input().set_binding(k, None);
    });
    c.add("unbindall", |ctx, _| ctx.input().bindings.iter_mut().for_each(|b| *b = None));
    c.add("bindlist", |ctx, _| {
        let lines: Vec<String> = ctx
            .input()
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(k, b)| b.as_ref().map(|b| format!("   {} \"{}\"\n", keynum_to_string(k as i32), b)))
            .collect();
        for l in &lines {
            ctx.print(l);
        }
        ctx.print(&format!("{} bindings\n", lines.len()));
    });
}

fn cmd_bind<C: InputHost>(ctx: &mut C, a: &Args) {
    if a.argc() < 2 {
        ctx.print("bind <key> [command] : attach a command to a key\n");
        return;
    }
    let k = string_to_keynum(a.argv(1));
    if k == -1 {
        ctx.print(&format!("\"{}\" isn't a valid key\n", a.argv(1)));
        return;
    }
    if a.argc() == 2 {
        let s = match ctx.input().binding(k) {
            Some(b) => format!("\"{}\" = \"{}\"\n", a.argv(1), b),
            None => format!("\"{}\" is not bound\n", a.argv(1)),
        };
        ctx.print(&s);
        return;
    }
    let cmd = a.all()[2..].join(" ");
    ctx.input().set_binding(k, Some(&cmd));
}

/// Key_Event. Returns what the console/menu should handle, if anything.
pub fn key_event<C: InputHost>(ctx: &mut C, key: i32, down: bool) -> Option<Routed> {
    let Ok(ku) = usize::try_from(key) else { return None };
    if ku >= MAX_KEYS {
        return None;
    }
    let inp = ctx.input();
    let repeat = down && inp.keydown[ku];
    inp.keydown[ku] = down;
    if !down {
        if std::mem::take(&mut inp.game_down[ku])
            && let Some(b) = inp.binding(key)
            && let Some(rest) = b.strip_prefix('+')
        {
            let s = format!("-{rest} {key}\n");
            ctx.cbuf().add_text(&s);
        }
        return None;
    }
    let dest = inp.dest;
    if key == K_ESCAPE {
        return Some(if dest == KeyDest::Game { Routed::Escape } else { Routed::Key { key, dest } });
    }
    if dest != KeyDest::Game {
        return Some(Routed::Key { key, dest });
    }
    if repeat {
        return None;
    }
    if let Some(b) = inp.binding(key).map(str::to_string) {
        inp.game_down[ku] = true;
        let s = if b.starts_with('+') { format!("{b} {key}\n") } else { format!("{b}\n") };
        ctx.cbuf().add_text(&s);
    }
    None
}

/// Change key destination; releases held game bindings (Key_ClearStates).
pub fn set_key_dest<C: InputHost>(ctx: &mut C, dest: KeyDest) {
    if ctx.input().dest == dest {
        return;
    }
    if dest != KeyDest::Game {
        for k in 0..MAX_KEYS as i32 {
            if ctx.input().keydown[k as usize] {
                key_event(ctx, k, false);
            }
        }
        let inp = ctx.input();
        inp.mouse = [0.0; 2];
        inp.accel.reset();
        inp.flick.reset();
    }
    ctx.input().dest = dest;
}

const TRIGGER_KEYS: [i32; 2] = [K_LTRIGGER, K_RTRIGGER];
const MENU_ARROWS: [i32; 4] = [K_LEFTARROW, K_RIGHTARROW, K_UPARROW, K_DOWNARROW];
const STICK_KEY_THRESH: f32 = 0.5;

/// Feed one platform event.
pub fn handle_event<C: InputHost>(ctx: &mut C, ev: InputEvent) -> Option<Routed> {
    match ev {
        InputEvent::Key { key, down } => key_event(ctx, key, down),
        InputEvent::Text(ch) => {
            let d = ctx.input().dest;
            (d != KeyDest::Game).then_some(Routed::Text { ch, dest: d })
        }
        InputEvent::MouseMotion { dx, dy } => {
            let inp = ctx.input();
            if inp.dest == KeyDest::Game {
                inp.mouse[0] += dx;
                inp.mouse[1] += dy;
                if dx != 0.0 || dy != 0.0 {
                    inp.last_look = LookDevice::Mouse;
                }
            }
            None
        }
        InputEvent::MouseWheel { y } => {
            let k = if y > 0.0 { K_MWHEELUP } else if y < 0.0 { K_MWHEELDOWN } else { return None };
            let r = key_event(ctx, k, true);
            key_event(ctx, k, false);
            r
        }
        InputEvent::MouseButton { button, down } => {
            if (1..=5).contains(&button) {
                key_event(ctx, K_MOUSE1 + button as i32 - 1, down)
            } else {
                None
            }
        }
        InputEvent::GamepadButton { button, down } => key_event(ctx, button.key(), down),
        InputEvent::GamepadAxis { axis, value } => {
            let i = axis as usize;
            let old = std::mem::replace(&mut ctx.input().axes[i], value);
            let cvs = ctx.input().cv;
            let mut out = None;
            if i >= 4 {
                let th = ctx.cvars().value(cvs.joy_deadzone_trigger);
                let (was, is) = (old > th, value > th);
                if was != is {
                    out = key_event(ctx, TRIGGER_KEYS[i - 4], is);
                }
            } else if i < 2 && ctx.input().dest != KeyDest::Game {
                // left stick emulates arrows in menus/console
                for (j, sign) in [(0usize, -1.0f32), (1, 1.0)] {
                    let k = MENU_ARROWS[i * 2 + j];
                    let (was, is) = (old * sign > STICK_KEY_THRESH, value * sign > STICK_KEY_THRESH);
                    if was != is {
                        out = key_event(ctx, k, is).or(out);
                    }
                }
            }
            out
        }
        InputEvent::Gyro { pitch, yaw } => {
            ctx.input().gyro = [pitch, yaw];
            None
        }
        InputEvent::GamepadConnected(c) => {
            let inp = ctx.input();
            inp.gamepad_connected = c;
            if !c {
                inp.axes = [0.0; 6];
                inp.gyro = [0.0; 2];
            }
            None
        }
    }
}

impl Input {
    /// Build this frame's InputState (CL_BaseMove inputs + IN_Move look).
    /// `viewangles` are current client angles (for flick/aim). `aim` is the
    /// world context for aim assist (None = off).
    pub fn frame(&mut self, cvars: &Cvars, viewangles: [f32; 3], ft: f32, aim: Option<&AimContext>) -> InputState {
        let v = |id: CvarId| cvars.value(id);
        let c = self.cv;
        let b = &mut self.buttons;
        let mut s = InputState::default();
        let klook = b[btn::KLOOK].is_down();
        let fwd = b[btn::FORWARD].key_state();
        let back = b[btn::BACK].key_state();
        if klook {
            s.lookup = fwd;
            s.lookdown = back;
        } else {
            s.forward = fwd;
            s.back = back;
        }
        s.moveleft = b[btn::MOVELEFT].key_state();
        s.moveright = b[btn::MOVERIGHT].key_state();
        s.moveup = b[btn::MOVEUP].key_state();
        s.movedown = b[btn::MOVEDOWN].key_state();
        s.left = b[btn::LEFT].key_state();
        s.right = b[btn::RIGHT].key_state();
        s.lookup += b[btn::LOOKUP].key_state();
        s.lookdown += b[btn::LOOKDOWN].key_state();
        s.speed = b[btn::SPEED].is_down();
        s.strafe = b[btn::STRAFE].is_down();
        s.attack = b[btn::ATTACK].state & 3 != 0;
        b[btn::ATTACK].state &= !2;
        s.jump = b[btn::JUMP].state & 3 != 0;
        b[btn::JUMP].state &= !2;
        s.impulse = std::mem::take(&mut self.impulse);
        let gyro_btn = b[btn::GYRO].is_down();
        let mlook = b[btn::MLOOK].is_down() || v(c.freelook) != 0.0;

        if self.dest != KeyDest::Game {
            self.mouse = [0.0; 2];
            return s;
        }

        // gamepad
        let mut pad_look = [0.0f32; 2];
        let mut pad_input = false;
        let mut aim_ok = false;
        if self.gamepad_connected && v(c.joy_enable) != 0.0 {
            let (mut mv, mut lk) = ([self.axes[0], self.axes[1]], [self.axes[2], self.axes[3]]);
            if v(c.joy_swapmovelook) != 0.0 {
                std::mem::swap(&mut mv, &mut lk);
            }
            let mv = gamepad::apply_easing(
                gamepad::apply_deadzone(mv, v(c.joy_deadzone_move), v(c.joy_outer_threshold_move)),
                v(c.joy_exponent_move),
            );
            let lkd = gamepad::apply_deadzone(lk, v(c.joy_deadzone_look), v(c.joy_outer_threshold_look));
            let lke = gamepad::apply_easing(lkd, v(c.joy_exponent));
            s.moveright += mv[0].max(0.0);
            s.moveleft += (-mv[0]).max(0.0);
            s.back += mv[1].max(0.0);
            s.forward += (-mv[1]).max(0.0);
            pad_input = mv != [0.0; 2] || lkd != [0.0; 2];
            let gyro_on = v(c.gyro_enable) != 0.0
                && match v(c.gyro_mode) as i32 {
                    1 => !gyro_btn,
                    2 => gyro_btn,
                    _ => true,
                };
            if v(c.joy_flick) != 0.0 {
                let p = FlickParams {
                    time: v(c.joy_flick_time),
                    deadzone: v(c.joy_flick_deadzone),
                    noise_thresh: v(c.joy_flick_noise_thresh),
                    adjust_speed: v(c.joy_flick_adjust_speed),
                };
                s.look_delta[0] += self.flick.update(lk, &p, ft);
            } else {
                self.flick.reset();
                let acc = self.accel.update(
                    gamepad::magnitude(lkd),
                    v(c.joy_accel_threshold),
                    v(c.joy_accel_delay),
                    v(c.joy_accel_ramp),
                    v(c.joy_accel),
                    ft,
                );
                let inv = if v(c.joy_invert) != 0.0 { -1.0 } else { 1.0 };
                pad_look[0] = -lke[0] * v(c.joy_sensitivity_yaw) * acc * ft;
                pad_look[1] = lke[1] * v(c.joy_sensitivity_pitch) * inv * ft;
            }
            if gyro_on {
                let th = v(c.gyro_noise_thresh);
                let gy = gamepad::gyro_filter(self.gyro[1].to_degrees(), th);
                let gp = gamepad::gyro_filter(self.gyro[0].to_degrees(), th);
                s.look_delta[0] += gy * v(c.gyro_yawsensitivity) * ft;
                s.look_delta[1] -= gp * v(c.gyro_pitchsensitivity) * ft;
            }
            if pad_input || (gyro_on && self.gyro != [0.0; 2]) {
                self.last_look = LookDevice::Gamepad;
            }
            aim_ok = self.last_look == LookDevice::Gamepad;
        }

        // aim assist: gamepad only, never with mouse look
        self.last_aim = AimResult::NONE;
        let mouse_moved = self.mouse != [0.0; 2];
        if aim_ok && !mouse_moved && v(c.aim_assist) != 0.0
            && let Some(ctx) = aim
        {
            let p = AimParams {
                cone: v(c.aim_cone),
                range: v(c.aim_range),
                slowdown: v(c.aim_slowdown),
                rotation: v(c.aim_rotation),
                snap: v(c.aim_snap),
                weight_dist: v(c.aim_weight_dist),
                target_players: v(c.aim_target_players) != 0.0,
            };
            self.last_aim = aim::aim_assist(&p, ctx, viewangles, ft, pad_input);
        }
        s.look_delta[0] += pad_look[0] * self.last_aim.scale + self.last_aim.delta[0];
        s.look_delta[1] += pad_look[1] * self.last_aim.scale + self.last_aim.delta[1];

        // mouse
        let mut m = std::mem::take(&mut self.mouse);
        if v(c.m_filter) != 0.0 {
            let cur = m;
            m = [(m[0] + self.old_mouse[0]) * 0.5, (m[1] + self.old_mouse[1]) * 0.5];
            self.old_mouse = cur;
        } else {
            self.old_mouse = m;
        }
        let sens = v(c.sensitivity);
        let (dx, dy) = (m[0] * sens, m[1] * sens);
        let fwd_speed = 200.0;
        if s.strafe || (v(c.lookstrafe) != 0.0 && mlook) {
            let side = v(c.m_side) * dx / fwd_speed;
            if side > 0.0 { s.moveright += side } else { s.moveleft -= side }
        } else {
            s.look_delta[0] -= v(c.m_yaw) * dx;
        }
        if mlook && !s.strafe {
            let inv = if v(c.m_invert) != 0.0 { -1.0 } else { 1.0 };
            s.look_delta[1] += v(c.m_pitch) * dy * inv;
        } else {
            let f = v(c.m_forward) * dy / fwd_speed;
            if f > 0.0 { s.back += f } else { s.forward -= f }
        }
        s
    }
}

#[cfg(test)]
mod tests;
