//! Add-on hookshot heads from `//qsp_hook owner state x y z` stufftext
//! (this engine only; other engines skip `//` lines).

use crate::{ClientEvent, ClientState};

/// One player's hook head: state 1 flying, 2 latched, 3 retracting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HookView {
    pub owner: u8,
    pub state: u8,
    pub head: [f32; 3],
}

impl HookView {
    pub(crate) fn parse(args: &str) -> Option<Self> {
        let w: Vec<&str> = args.split_whitespace().collect();
        let [o, s, x, y, z] = w[..] else { return None };
        let state: u8 = s.parse().ok()?;
        let mut head = [0.0f32; 3];
        for (v, t) in head.iter_mut().zip([x, y, z]) {
            *v = t.parse().ok()?;
            if !v.is_finite() || v.abs() > 1_000_000.0 {
                return None;
            }
        }
        (state <= 3).then_some(HookView { owner: o.parse().ok()?, state, head })
    }
}

impl ClientState {
    /// Updates `hooks`; a state change also queues a `ClientEvent::Hook`
    /// (for sounds).
    pub(crate) fn parse_hook(&mut self, args: &str) {
        let Some(h) = HookView::parse(args) else { return };
        let old = self.hooks.iter().position(|o| o.owner == h.owner);
        let prev = old.map_or(0, |i| self.hooks[i].state);
        if let Some(i) = old {
            self.hooks.remove(i);
        }
        if h.state != 0 {
            self.hooks.push(h);
        }
        if prev != h.state {
            self.events.push(ClientEvent::Hook(h));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_lines_parse_and_track_state_changes() {
        assert!(HookView::parse("0 1 1 2 3").is_some());
        for bad in ["0 4 1 2 3", "0 1 1 2", "0 1 NaN 2 3", "x 1 1 2 3"] {
            assert!(HookView::parse(bad).is_none(), "{bad}");
        }
        let mut cl = ClientState::new();
        cl.parse_hook("0 1 1 2 3");
        cl.parse_hook("0 1 4 5 6");
        assert_eq!(cl.hooks.len(), 1);
        assert_eq!(cl.hooks[0].head, [4.0, 5.0, 6.0]);
        cl.parse_hook("0 0 0 0 0");
        assert!(cl.hooks.is_empty());
        let n = cl.events.iter().filter(|e| matches!(e, ClientEvent::Hook(_))).count();
        assert_eq!(n, 2);
    }
}
