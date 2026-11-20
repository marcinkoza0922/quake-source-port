//! scr_showspeed: the player's horizontal speed, a port of ironwail's
//! SCR_DrawSpeed. Shows the peak speed of each 50 ms window so the number
//! stays readable while bunnyhopping.
use super::Host;

/// Length of one peak-hold window (s).
const INTERVAL: f64 = 0.05;

/// Advance the peak-hold state `(peak, shown, window start)` by one frame
/// at `realtime` with horizontal `speed`; returns the value to display.
pub(super) fn step(state: &mut (f32, f32, f64), realtime: f64, speed: f32) -> Option<f32> {
    if state.2 > realtime {
        *state = (0.0, -1.0, 0.0);
    }
    state.0 = state.0.max(speed);
    let shown = (state.1 >= 0.0).then_some(state.1);
    if realtime - state.2 >= INTERVAL {
        *state = (0.0, state.0, realtime);
    }
    shown
}

impl Host {
    /// Advance peak-hold tracking once per frame and return the current
    /// horizontal speed (units/s). Runs every frame so the HUD element and
    /// `scr_showspeed` are ready immediately when shown.
    pub(super) fn update_speed(&mut self) -> Option<f32> {
        let v = self.cl.velocity;
        let speed = (v[0] * v[0] + v[1] * v[1]).sqrt();
        step(&mut self.speed, self.realtime, speed)
    }

    /// Text for the standalone `scr_showspeed` overlay, or None when hidden.
    pub(super) fn speedometer_text(&self, shown: Option<f32>) -> Option<String> {
        let shown = shown?;
        let hidden = self.cl.intermission != 0 || self.cv("viewsize") >= 130.0 || self.cv("scr_showspeed") == 0.0;
        (!hidden).then(|| format!("{}", shown as i32))
    }
}

#[cfg(test)]
mod tests {
    use super::step;

    #[test]
    fn holds_peak_per_interval() {
        let mut s = (0.0, -1.0, 0.0);
        assert_eq!(step(&mut s, 0.01, 300.0), None);
        assert_eq!(step(&mut s, 0.06, 320.0), None);
        // the 0.06 frame closed the window with its peak, 320
        assert_eq!(step(&mut s, 0.07, 100.0), Some(320.0));
        assert_eq!(step(&mut s, 0.12, 50.0), Some(320.0));
        assert_eq!(step(&mut s, 0.13, 0.0), Some(100.0));
        // realtime going backwards (new session) resets
        assert_eq!(step(&mut s, 0.0, 10.0), None);
    }
}
