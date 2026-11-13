use super::*;

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
        s.attack2 = b[btn::ATTACK2].state & 3 != 0;
        b[btn::ATTACK2].state &= !2;
        s.impulse = std::mem::take(&mut self.impulse);
        s.weapon_slot = std::mem::take(&mut self.weapon_slot);
        let gyro_btn = b[btn::GYRO].is_down();
        let mlook = b[btn::MLOOK].is_down() || v(c.freelook) != 0.0;

        if self.dest != KeyDest::Game {
            self.lockon.reset();
            self.mouse = [0.0; 2];
            self.wheel_aim = [0.0; 2];
            self.wheel_mouse = [0.0; 2];
            return s;
        }
        // both wheels share the aim (stick or mouse cursor)
        let wheel = b[btn::WHEEL].is_down() || b[btn::EQUIPWHEEL].is_down();
        if !wheel {
            self.wheel_aim = [0.0; 2];
            self.wheel_mouse = [0.0; 2];
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
            if wheel {
                // either stick picks a wheel slot (the stronger one) instead
                // of moving or turning
                let pick = if gamepad::magnitude(mv) > gamepad::magnitude(lk) { mv } else { lk };
                let m = gamepad::magnitude(pick);
                if m > 0.5 {
                    self.wheel_aim = [pick[0] / m, pick[1] / m];
                }
                mv = [0.0; 2];
                lk = [0.0; 2];
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

        // touch: stick moves like a gamepad stick, drags turn like a mouse
        if v(c.touch_enable) != 0.0 {
            let lay = self.touch_layout(cvars);
            let mv = self.touch.move_vec(&lay);
            s.moveright += mv[0].max(0.0);
            s.moveleft += (-mv[0]).max(0.0);
            s.back += mv[1].max(0.0);
            s.forward += (-mv[1]).max(0.0);
            // a full screen-height drag turns 180 degrees at sensitivity 1
            let lk = self.touch.take_look();
            let k = 180.0 * v(c.touch_sensitivity);
            let inv = if v(c.touch_invert) != 0.0 { -1.0 } else { 1.0 };
            s.look_delta[0] -= lk[0] * k;
            s.look_delta[1] += lk[1] * k * inv;
        }

        // mouse
        let mut m = std::mem::take(&mut self.mouse);
        if wheel {
            // the mouse steers a virtual cursor on the wheel
            const R: f32 = 120.0;
            let w = [self.wheel_mouse[0] + m[0], self.wheel_mouse[1] + m[1]];
            let mag = gamepad::magnitude(w);
            self.wheel_mouse = if mag > R { [w[0] * R / mag, w[1] * R / mag] } else { w };
            if mag > 30.0 {
                self.wheel_aim = [w[0] / mag, w[1] / mag];
            }
            m = [0.0; 2];
        }
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
        let held = self.buttons[btn::LOCKON].state & 3 != 0;
        self.buttons[btn::LOCKON].state &= !2;
        let cycle = self.key_down(if v(c.joy_swapmovelook) != 0.0 { K_LTHUMB } else { K_RTHUMB });
        let p = lockon::LockOnParams {
            enabled: v(c.autoaim) != 0.0 && !wheel,
            allowed: self.autoaim_allowed,
            toggle: v(c.autoaim_mode) != 0.0,
            reacquire: v(c.autoaim_reacquire) != 0.0,
            cone: v(c.autoaim_cone),
            range: v(c.autoaim_range),
            fov: self.autoaim_fov,
            target_players: self.autoaim_players,
        };
        if let Some(delta) = self.lockon.update(&p, aim, viewangles, held, cycle) {
            s.look_delta = delta;
            if !s.strafe {
                s.left = 0.0;
                s.right = 0.0;
            }
            s.lookup = 0.0;
            s.lookdown = 0.0;
        }
        s
    }
}
