//! Special controller gestures on the stats and menu buttons:
//! - Double-tapping the controller stats button (`+showscores`, default Select /
//!   View) always takes a screenshot without showing the scoreboard.
//! - On the controller menu button (`Start`, or any gamepad button bound to
//!   `togglemenu` / `menu_main`): double-tapping creates a quick save, and a
//!   long press performs a quick load.

use super::*;
use qinput::GamepadButton;

/// Maximum gap (seconds) between tap 1 release and tap 2 press.
const DOUBLE_TAP_WINDOW: f64 = 0.35;
/// Maximum hold (seconds) for a stats press to count as a tap.
const STATS_TAP_MAX: f64 = 0.35;
/// Hold duration (seconds) on the controller menu button to trigger quick load.
const MENU_LONG_PRESS: f64 = 0.55;

#[derive(Default)]
pub(super) struct PadGestures {
    /// Stats button currently held: `(button, pressed_at)`.
    stats_press: Option<(GamepadButton, f64)>,
    /// Last short tap of the stats button: `(button, released_at)`.
    stats_last_tap: Option<(GamepadButton, f64)>,
    /// Suppress the release of a stats button whose press triggered a screenshot.
    stats_suppress_up: Option<GamepadButton>,
    /// Menu button currently held: `(button, pressed_at, on_death_screen)`.
    menu_hold: Option<(GamepadButton, f64, bool)>,
    /// Main menu was just opened by tap 1 of this button: `(button, opened_at)`.
    menu_tap_armed: Option<(GamepadButton, f64)>,
    /// Suppress the release of a menu button after double-tap or long-press.
    menu_suppress_up: Option<GamepadButton>,
}

impl PadGestures {
    /// Reset transient gesture tracking (e.g. on disconnect).
    pub(super) fn clear_session(&mut self) {
        self.stats_press = None;
        self.stats_last_tap = None;
        self.menu_hold = None;
        self.menu_tap_armed = None;
    }

    /// Full reset when the gamepad disconnects.
    pub(super) fn reset_all(&mut self) {
        *self = Self::default();
    }
}

impl Host {
    fn is_pad_stats_button(&self, button: GamepadButton) -> bool {
        self.input
            .binding(button.key())
            .is_some_and(|b| b.trim().eq_ignore_ascii_case("+showscores"))
    }

    fn is_pad_menu_button(&self, button: GamepadButton) -> bool {
        button == GamepadButton::Start
            || self.input.binding(button.key()).is_some_and(|b| {
                let b = b.trim();
                b.eq_ignore_ascii_case("togglemenu") || b.eq_ignore_ascii_case("menu_main")
            })
    }

    fn note_pad_device(&mut self) {
        if self.input.last_device == Device::Touch {
            qinput::release_touch(self);
        }
        self.input.last_device = Device::Gamepad;
    }

    /// Check controller gestures before normal input routing. Returns `true`
    /// when the event was consumed and should not be forwarded to `qinput`.
    pub(super) fn handle_pad_gesture(&mut self, ie: &InputEvent) -> bool {
        if let InputEvent::GamepadConnected(false) = ie {
            self.pad_gestures.reset_all();
            return false;
        }
        // Any other deliberate input cancels an armed menu double-tap so
        // navigating the menu and pressing Start later closes it normally.
        if let Some((armed_btn, _)) = self.pad_gestures.menu_tap_armed {
            let cancel = match *ie {
                InputEvent::Key { down: true, .. } | InputEvent::MouseButton { down: true, .. } => true,
                InputEvent::GamepadButton { button, down: true } if button != armed_btn => true,
                InputEvent::GamepadAxis { value, .. } if value.abs() > 0.5 => true,
                _ => false,
            };
            if cancel {
                self.pad_gestures.menu_tap_armed = None;
            }
        }
        let InputEvent::GamepadButton { button, down } = *ie else {
            return false;
        };
        if self.handle_pad_menu_gesture(button, down) {
            return true;
        }
        self.handle_pad_stats_gesture(button, down)
    }

    fn handle_pad_menu_gesture(&mut self, button: GamepadButton, down: bool) -> bool {
        if !down && self.pad_gestures.menu_suppress_up == Some(button) {
            self.pad_gestures.menu_suppress_up = None;
            return true;
        }
        if !self.is_pad_menu_button(button) {
            return false;
        }
        // Never interfere while capturing a keybind or outside a local game.
        if self.menu.capturing() || !self.server.active() || self.demo.is_some() || self.con.open {
            if !down && self.pad_gestures.menu_hold.is_some_and(|(b, _, _)| b == button) {
                self.pad_gestures.menu_hold = None;
            }
            return false;
        }
        let now = self.realtime;
        if down {
            self.note_pad_device();
            // Second tap within the double-tap window right after tap 1 opened Main Menu.
            if let Some((armed_btn, at)) = self.pad_gestures.menu_tap_armed.take()
                && armed_btn == button
                && self.menu.screen == Screen::Main
                && now - at <= DOUBLE_TAP_WINDOW
            {
                self.pad_gestures.menu_suppress_up = Some(button);
                self.menu.screen = Screen::None;
                self.sync_key_dest();
                self.cbuf.add_text("echo Quicksaving...; wait; save quick\n");
                return true;
            }
            // Start holding from gameplay (or Death screen for quick load).
            if self.pad_gestures.menu_hold.is_some_and(|(b, _, _)| b == button) {
                return true;
            }
            if self.key_dest() == KeyDest::Game && !self.automap.fullscreen {
                self.pad_gestures.menu_hold = Some((button, now, false));
                return true;
            }
            if self.menu.screen == Screen::Death {
                self.pad_gestures.menu_hold = Some((button, now, true));
                return true;
            }
            return false;
        }
        // Release of a held menu button before the long-press threshold.
        if let Some((held_btn, _, on_death)) = self.pad_gestures.menu_hold
            && held_btn == button
        {
            self.pad_gestures.menu_hold = None;
            if !on_death && self.key_dest() == KeyDest::Game && !self.automap.fullscreen {
                self.menu.open(Screen::Main);
                self.sync_key_dest();
                self.pad_gestures.menu_tap_armed = Some((button, now));
            }
            return true;
        }
        false
    }

    fn handle_pad_stats_gesture(&mut self, button: GamepadButton, down: bool) -> bool {
        if !down && self.pad_gestures.stats_suppress_up == Some(button) {
            self.pad_gestures.stats_suppress_up = None;
            return true;
        }
        if !self.is_pad_stats_button(button) {
            return false;
        }
        if self.key_dest() != KeyDest::Game || self.demo.is_some() || self.automap.fullscreen {
            if !down && self.pad_gestures.stats_press.is_some_and(|(b, _)| b == button) {
                self.pad_gestures.stats_press = None;
            }
            return false;
        }
        let now = self.realtime;
        if down {
            if let Some((prev_btn, at)) = self.pad_gestures.stats_last_tap.take()
                && prev_btn == button
                && now - at <= DOUBLE_TAP_WINDOW
            {
                self.note_pad_device();
                self.pad_gestures.stats_press = None;
                self.pad_gestures.stats_suppress_up = Some(button);
                self.show_scores = false;
                self.cbuf.add_text("screenshot\n");
                return true;
            }
            self.pad_gestures.stats_press = Some((button, now));
            return false;
        }
        if let Some((prev_btn, pressed_at)) = self.pad_gestures.stats_press.take()
            && prev_btn == button
            && now - pressed_at <= STATS_TAP_MAX
        {
            self.pad_gestures.stats_last_tap = Some((button, now));
        }
        false
    }

    /// Advance held controller gestures once per frame (long-press quick load).
    pub(super) fn poll_pad_gestures(&mut self) {
        let now = self.realtime;
        if let Some((_, at)) = self.pad_gestures.menu_tap_armed
            && (now - at > DOUBLE_TAP_WINDOW || self.menu.screen != Screen::Main)
        {
            self.pad_gestures.menu_tap_armed = None;
        }
        if let Some((_, at)) = self.pad_gestures.stats_last_tap
            && (now - at > DOUBLE_TAP_WINDOW || self.key_dest() != KeyDest::Game)
        {
            self.pad_gestures.stats_last_tap = None;
        }
        let Some((button, pressed_at, on_death)) = self.pad_gestures.menu_hold else {
            return;
        };
        let valid_screen = if on_death {
            self.menu.screen == Screen::Death
        } else {
            self.key_dest() == KeyDest::Game || self.menu.screen == Screen::Death
        };
        if !self.server.active() || self.demo.is_some() || self.con.open || !valid_screen {
            self.pad_gestures.menu_hold = None;
            return;
        }
        if now - pressed_at >= MENU_LONG_PRESS {
            self.pad_gestures.menu_hold = None;
            self.pad_gestures.menu_tap_armed = None;
            self.pad_gestures.menu_suppress_up = Some(button);
            self.cbuf.add_text("echo Quickloading...; wait; load quick\n");
        }
    }
}
