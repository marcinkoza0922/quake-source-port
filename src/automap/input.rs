//! Interactive fullscreen 3D automap input handling (keyboard, mouse, controller).

use qcore::math::Vec3;
use qinput::keys::*;

use super::Automap;

fn held_slot(key: i32) -> Option<usize> {
    Some(match key {
        k if k == b'w' as i32 || k == b'W' as i32 => 0,
        k if k == b's' as i32 || k == b'S' as i32 => 1,
        k if k == b'a' as i32 || k == b'A' as i32 => 2,
        k if k == b'd' as i32 || k == b'D' as i32 => 3,
        k if k == b'q' as i32 || k == b'Q' as i32 => 4,
        k if k == b'e' as i32 || k == b'E' as i32 => 5,
        K_LEFTARROW => 6,
        K_RIGHTARROW => 7,
        K_UPARROW => 8,
        K_DOWNARROW => 9,
        K_LTRIGGER => 10,
        K_RTRIGGER => 11,
        _ => return None,
    })
}

impl Automap {
    /// Handles a key or gamepad button event while the fullscreen automap is open.
    /// Returns `true` if the event was consumed.
    pub fn key(&mut self, key: i32, down: bool, player_pos: Vec3, player_yaw: f32) -> bool {
        if !self.fullscreen {
            return false;
        }
        if let Some(slot) = held_slot(key) {
            self.held[slot] = down;
            return true;
        }
        if !down {
            return true;
        }
        match key {
            K_ESCAPE | K_BBUTTON | K_START => {
                self.close_fullscreen();
            }
            K_TAB | K_YBUTTON => {
                self.fs_top_down = !self.fs_top_down;
            }
            k if k == b'f' as i32 || k == b'F' as i32 || k == K_XBUTTON => {
                self.fs_reveal = !self.fs_reveal;
            }
            k if k == b'r' as i32 || k == b'R' as i32 || k == K_LSHOULDER => {
                self.recenter(player_pos, player_yaw);
            }
            k if k == b'+' as i32 || k == b'=' as i32 || k == K_KP_PLUS || k == K_MWHEELUP => {
                self.zoom_by(1.18);
            }
            k if k == b'-' as i32 || k == b'_' as i32 || k == K_KP_MINUS || k == K_MWHEELDOWN => {
                self.zoom_by(1.0 / 1.18);
            }
            K_PGUP | K_DPAD_UP => {
                self.cam_focus.z += 64.0;
            }
            K_PGDN | K_DPAD_DOWN => {
                self.cam_focus.z -= 64.0;
            }
            _ => {}
        }
        true
    }

    /// Updates analog trigger zoom hold state (0 = LT zoom out, 1 = RT zoom in).
    pub fn set_trigger(&mut self, right: bool, active: bool) {
        self.held[if right { 11 } else { 10 }] = active;
    }

    pub fn mouse_button(&mut self, button: u8, down: bool) {
        if !self.fullscreen {
            return;
        }
        match button {
            1 => self.drag_orbit = down,
            2 | 3 => self.drag_pan = down,
            _ => {}
        }
        if !down && !self.drag_orbit && !self.drag_pan {
            self.last_mouse = None;
        }
    }

    pub fn mouse_move(&mut self, pos: (f32, f32)) {
        if !self.fullscreen {
            return;
        }
        if let Some(prev) = self.last_mouse {
            let dx = pos.0 - prev.0;
            let dy = pos.1 - prev.1;
            if self.drag_orbit {
                self.cam_yaw = (self.cam_yaw - dx * 0.45).rem_euclid(360.0);
                if !self.fs_top_down {
                    self.cam_pitch = (self.cam_pitch + dy * 0.35).clamp(10.0, 88.0);
                }
            } else if self.drag_pan {
                let speed = 3.2 / self.cam_zoom.max(0.25);
                self.pan_screen(-dx * speed, dy * speed);
            }
        }
        if self.drag_orbit || self.drag_pan {
            self.last_mouse = Some(pos);
        }
    }

    pub fn zoom_by(&mut self, factor: f32) {
        self.cam_zoom = (self.cam_zoom * factor).clamp(0.25, 6.0);
    }

    pub fn pan_screen(&mut self, right_units: f32, forward_units: f32) {
        let yaw = self.cam_yaw.to_radians();
        let fwd = Vec3::new(yaw.cos(), yaw.sin(), 0.0);
        let right = Vec3::new(yaw.sin(), -yaw.cos(), 0.0);
        self.cam_focus = self.cam_focus + right * right_units + fwd * forward_units;
    }

    /// Continuous pan / orbit / elevation / zoom from held keys and gamepad sticks.
    pub fn update_controls(&mut self, dt: f32, inp: &qinput::Input) {
        if !self.fullscreen {
            return;
        }
        let pan_speed = 650.0 / self.cam_zoom.max(0.25);
        let mut right = 0.0f32;
        let mut fwd = 0.0f32;
        let mut dz = 0.0f32;
        let mut dyaw = 0.0f32;
        let mut dpitch = 0.0f32;
        let mut zoom_dir = 0.0f32;

        if self.held[0] { fwd += 1.0; }
        if self.held[1] { fwd -= 1.0; }
        if self.held[2] { right -= 1.0; }
        if self.held[3] { right += 1.0; }
        if self.held[4] { dz -= 1.0; }
        if self.held[5] { dz += 1.0; }
        if self.held[6] { dyaw += 1.0; }
        if self.held[7] { dyaw -= 1.0; }
        if self.held[8] { dpitch += 1.0; }
        if self.held[9] { dpitch -= 1.0; }
        if self.held[10] { zoom_dir -= 1.0; }
        if self.held[11] { zoom_dir += 1.0; }

        if inp.gamepad_connected {
            let (ls, rs) = inp.sticks();
            if ls[0].abs() > 0.15 { right += ls[0]; }
            if ls[1].abs() > 0.15 { fwd -= ls[1]; }
            if rs[0].abs() > 0.15 { dyaw -= rs[0]; }
            if rs[1].abs() > 0.15 { dpitch += rs[1]; }
        }

        if right != 0.0 || fwd != 0.0 {
            self.pan_screen(right * pan_speed * dt, fwd * pan_speed * dt);
        }
        if dz != 0.0 {
            self.cam_focus.z += dz * 420.0 * dt;
        }
        if dyaw != 0.0 {
            self.cam_yaw = (self.cam_yaw + dyaw * 110.0 * dt).rem_euclid(360.0);
        }
        if dpitch != 0.0 && !self.fs_top_down {
            self.cam_pitch = (self.cam_pitch + dpitch * 75.0 * dt).clamp(10.0, 88.0);
        }
        if zoom_dir != 0.0 {
            self.zoom_by((1.0 + zoom_dir * 1.4 * dt).max(0.5));
        }
    }
}
