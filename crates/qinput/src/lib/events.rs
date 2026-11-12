use super::*;

pub(super) fn toggle_press(b: &mut KButton, key: i32, toggle: bool) {
    if !toggle {
        b.press(key);
    } else if b.is_down() {
        b.release(-1);
    } else {
        b.press(key);
        // Latch: forget the key so its release doesn't let go.
        b.down = [-1, 0];
    }
}

pub(super) fn cmd_bind<C: InputHost>(ctx: &mut C, a: &Args) {
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

pub(super) const TRIGGER_KEYS: [i32; 2] = [K_LTRIGGER, K_RTRIGGER];
pub(super) const MENU_ARROWS: [i32; 4] = [K_LEFTARROW, K_RIGHTARROW, K_UPARROW, K_DOWNARROW];
pub(super) const STICK_KEY_THRESH: f32 = 0.5;

/// Feed one platform event.
pub fn handle_event<C: InputHost>(ctx: &mut C, ev: InputEvent) -> Option<Routed> {
    let dev = match ev {
        InputEvent::Key { down: true, .. } | InputEvent::MouseButton { down: true, .. } | InputEvent::MouseWheel { .. } => {
            Some(Device::KeyboardMouse)
        }
        // small jitter from a resting mouse doesn't switch away from the pad
        InputEvent::MouseMotion { dx, dy } if dx.abs() + dy.abs() > 4.0 => Some(Device::KeyboardMouse),
        InputEvent::GamepadButton { down: true, .. } => Some(Device::Gamepad),
        InputEvent::GamepadAxis { value, .. } if value.abs() > 0.5 => Some(Device::Gamepad),
        _ => None,
    };
    if let Some(d) = dev {
        ctx.input().last_device = d;
    }
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
            } else if i < 2 && ctx.input().dest != KeyDest::Game && !ctx.input().capture {
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
                inp.last_device = Device::KeyboardMouse;
            }
            None
        }
    }
}
