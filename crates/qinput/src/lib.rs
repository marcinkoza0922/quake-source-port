//! qinput — binds, +/- buttons, mouse, gamepad curves, aim assist. No SDL;
//! the binary maps platform events to [`InputEvent`]. See docs/DESIGN.md.

pub mod aim;
pub mod lockon;
pub mod button;
pub mod gamepad;
pub mod keys;
pub mod touch;

use aim::{AimContext, AimParams, AimResult};
use button::{KButton, key_arg};
use gamepad::{AccelRamp, FlickParams, FlickStick};
use keys::*;
use qclient::InputState;
use qcore::cmd::{Args, CmdContext};
use qcore::cvar::{CVAR_ARCHIVE, CvarId, Cvars};
#[path = "lib/events.rs"]
mod events;
pub use events::*;
#[path = "lib/frame.rs"]
mod frame;

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
    /// Key number (Start/Guide map to ESCAPE like ironwail; Back is its own
    /// bindable key).
    pub fn key(self) -> i32 {
        use GamepadButton::*;
        match self {
            South => K_ABUTTON,
            East => K_BBUTTON,
            West => K_XBUTTON,
            North => K_YBUTTON,
            Back => K_BACK,
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
    /// Finger on the touchscreen: window-normalised position, window
    /// width/height.
    Touch { id: u64, phase: touch::TouchPhase, x: f32, y: f32, aspect: f32 },
}

/// Events the console/menu must handle (not routed to binds).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Routed {
    Key { key: i32, dest: KeyDest },
    Text { ch: char, dest: KeyDest },
    /// Escape pressed in game: toggle menu.
    Escape,
}

/// Which device the player used last (prompts and glyphs follow it).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Device {
    #[default]
    KeyboardMouse,
    Gamepad,
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
    pub const WHEEL: usize = 18;
    pub const LOCKON: usize = 19;
    /// Add-ons: secondary fire and the toy-slot toggle.
    pub const ATTACK2: usize = 20;
    pub const TOY: usize = 21;
    pub const COUNT: usize = 22;
    pub const NAMES: [&str; COUNT] = [
        "forward", "back", "moveleft", "moveright", "moveup", "movedown", "left", "right", "lookup", "lookdown",
        "speed", "strafe", "attack", "jump", "mlook", "klook", "use", "gyroaction", "weaponwheel", "lockon",
        "attack2", "toy",
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
    joy_rumble = "joy_rumble", "1";
    joy_glyphs = "joy_glyphs", "0";
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
    autoaim = "assist_autoaim", "0";
    autoaim_mode = "assist_autoaim_mode", "0";
    autoaim_cone = "assist_autoaim_cone", "8";
    autoaim_indicator = "assist_autoaim_indicator", "1";
    autoaim_reacquire = "assist_autoaim_reacquire", "0";
    autoaim_range = "assist_autoaim_range", "2048";
    aim_assist = "aim_assist", "1";
    aim_cone = "aim_cone", "6";
    aim_range = "aim_range", "2048";
    aim_slowdown = "aim_slowdown", "0.4";
    aim_rotation = "aim_rotation", "0.5";
    aim_snap = "aim_snap", "0";
    aim_weight_dist = "aim_weight_dist", "0.3";
    aim_target_players = "aim_target_players", "1";
    in_toggleattack = "in_toggleattack", "0";
    in_togglerun = "in_togglerun", "0";
    touch_enable = "touch_enable", "1";
    touch_sensitivity = "touch_sensitivity", "1";
    touch_size = "touch_size", "1";
    touch_opacity = "touch_opacity", "0.5";
    touch_invert = "touch_invert", "0";
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
    /// Device of the last deliberate input (buttons, sticks, keys, mouse).
    pub last_device: Device,
    /// A menu is waiting for a key to bind: sticks don't emulate arrows.
    pub capture: bool,
    /// Weapon wheel direction this frame (unit vector, +y down), or zero
    /// while the stick/mouse is centred. Only set while the wheel is held.
    pub wheel_aim: [f32; 2],
    wheel_mouse: [f32; 2],
    accel: AccelRamp,
    flick: FlickStick,
    /// Last aim assist result (for HUD/debug).
    pub last_aim: AimResult,
    pub lockon: lockon::LockOn,
    /// Host permits auto-aim only in local play.
    pub autoaim_allowed: bool,
    pub autoaim_fov: [f32; 2],
    pub autoaim_players: bool,
    pub touch: touch::TouchOverlay,
    /// On-screen controls showing (`touch_enable 1`: since the last touch,
    /// until another device is used).
    pub touch_shown: bool,
    /// Window width/height from the last touch, for the stick.
    pub touch_aspect: f32,
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
            last_device: Device::KeyboardMouse,
            capture: false,
            wheel_aim: [0.0; 2],
            wheel_mouse: [0.0; 2],
            accel: AccelRamp::default(),
            flick: FlickStick::default(),
            last_aim: AimResult::NONE,
            lockon: lockon::LockOn::default(),
            autoaim_allowed: false,
            autoaim_fov: [90.0, 73.74],
            autoaim_players: false,
            touch: touch::TouchOverlay::default(),
            touch_shown: false,
            touch_aspect: 16.0 / 9.0,
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
    /// Weapon wheel button held (and in game).
    pub fn wheel_held(&self) -> bool {
        self.buttons[btn::WHEEL].is_down() && self.dest == KeyDest::Game
    }
    /// Layout of the on-screen controls.
    pub fn touch_layout(&self, cvars: &Cvars) -> touch::TouchLayout {
        touch::TouchLayout { aspect: self.touch_aspect, size: cvars.value(self.cv.touch_size) }
    }
    /// Whether the on-screen controls should be drawn and used.
    pub fn touch_visible(&self, cvars: &Cvars) -> bool {
        match cvars.value(self.cv.touch_enable) as i32 {
            0 => false,
            1 => self.touch_shown,
            _ => true,
        }
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

/// `+name`/`-name` for a button that latches on each press while its toggle
/// cvar is set (accessibility: no need to hold attack or run).
macro_rules! toggle_button_cmds {
    ($c:expr, $i:expr, $cvar:ident) => {{
        $c.add(&format!("+{}", btn::NAMES[$i]), |ctx, a| {
            let on = { let id = ctx.input().cv.$cvar; ctx.cvars().value(id) != 0.0 };
            toggle_press(&mut ctx.input().buttons[$i], key_arg(a), on);
        });
        $c.add(&format!("-{}", btn::NAMES[$i]), |ctx, a| {
            let on = { let id = ctx.input().cv.$cvar; ctx.cvars().value(id) != 0.0 };
            if !on {
                ctx.input().buttons[$i].release(key_arg(a));
            }
        });
    }};
}


macro_rules! button_cmds {
    ($c:expr, $($i:expr),*) => {$(
        $c.add(&format!("+{}", btn::NAMES[$i]), |ctx, a| ctx.input().buttons[$i].press(key_arg(a)));
        $c.add(&format!("-{}", btn::NAMES[$i]), |ctx, a| ctx.input().buttons[$i].release(key_arg(a)));
    )*};
}

/// Register bind/unbind/unbindall/bindlist, +/- buttons and impulse.
pub fn register_commands<C: InputHost>(c: &mut Commands<C>) {
    button_cmds!(c, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 13, 14, 15, 16, 17, 18, 19, 20, 21);
    toggle_button_cmds!(c, btn::ATTACK, in_toggleattack);
    toggle_button_cmds!(c, btn::SPEED, in_togglerun);
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


#[cfg(test)]
mod tests;

#[cfg(test)]
mod toggle_tests {
    use super::*;
    #[test]
    fn toggle_press_latches() {
        let mut b = KButton::default();
        toggle_press(&mut b, 10, true);
        b.release(10); // key up is ignored while latched
        assert!(b.is_down());
        toggle_press(&mut b, 10, true);
        assert!(!b.is_down());
        toggle_press(&mut b, 10, false);
        b.release(10);
        assert!(!b.is_down());
    }
}
