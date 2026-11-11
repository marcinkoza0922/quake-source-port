//! kbutton_t (cl_input.c).

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct KButton {
    /// Keys holding it; -1 = typed from console.
    pub down: [i32; 2],
    /// bit 0 down, bit 1 impulse down, bit 2 impulse up.
    pub state: u8,
}

impl KButton {
    /// KeyDown; `key` is the argv(1) key number or -1.
    pub fn press(&mut self, key: i32) {
        if key == self.down[0] || key == self.down[1] {
            return;
        }
        if self.down[0] == 0 {
            self.down[0] = key;
        } else if self.down[1] == 0 {
            self.down[1] = key;
        } else {
            return;
        }
        if self.state & 1 != 0 {
            return;
        }
        self.state |= 1 + 2;
    }
    /// KeyUp; -1 releases everything.
    pub fn release(&mut self, key: i32) {
        if key == -1 {
            self.down = [0, 0];
            self.state = 4;
            return;
        }
        if self.down[0] == key {
            self.down[0] = 0;
        } else if self.down[1] == key {
            self.down[1] = 0;
        } else {
            return;
        }
        if self.down[0] != 0 || self.down[1] != 0 {
            return;
        }
        if self.state & 1 == 0 {
            return;
        }
        self.state &= !1;
        self.state |= 4;
    }
    pub fn is_down(&self) -> bool {
        self.state & 1 != 0
    }
    /// CL_KeyState: fraction of the frame held; clears impulses.
    pub fn key_state(&mut self) -> f32 {
        let impulsedown = self.state & 2 != 0;
        let impulseup = self.state & 4 != 0;
        let down = self.state & 1 != 0;
        let v = match (impulsedown, impulseup) {
            (true, false) => if down { 0.5 } else { 0.0 },
            (false, true) => 0.0,
            (false, false) => if down { 1.0 } else { 0.0 },
            (true, true) => if down { 0.75 } else { 0.25 },
        };
        self.state &= 1;
        v
    }
}

/// Parse the key-number argument of `+cmd <key>`.
pub fn key_arg(a: &qcore::cmd::Args) -> i32 {
    if a.argc() > 1 { a.argv(1).parse().unwrap_or(-1) } else { -1 }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn kbutton_semantics() {
        let mut b = KButton::default();
        b.press(10);
        assert_eq!(b.key_state(), 0.5);
        assert_eq!(b.key_state(), 1.0);
        b.press(11);
        b.release(10);
        assert!(b.is_down());
        b.release(11);
        assert_eq!(b.key_state(), 0.0);
        b.press(10);
        b.release(10);
        assert_eq!(b.key_state(), 0.25);
        b.press(-1);
        b.release(-1);
        assert!(!b.is_down());
        b.release(99);
        assert_eq!(b.key_state(), 0.0);
    }
}
