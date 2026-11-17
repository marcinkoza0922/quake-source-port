//! Add-on male-to-male cables from `//qsp_cable owner state fire sx sy sz
//! nx ny nz ex ey ez length [px py pz]...` stufftext (this engine only;
//! other engines skip `//` lines).

use crate::{ClientEvent, ClientState};

/// One player's cable: state 1 plugged into `socket` (facing `normal`,
/// wrapped round `pivots`), 0 yanked out, 2 a missed plug attempt at
/// `socket`. `fire`: the bolt reaches `end` this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct CableView {
    pub owner: u8,
    pub state: u8,
    pub fire: bool,
    pub socket: [f32; 3],
    pub normal: [f32; 3],
    pub end: [f32; 3],
    pub length: f32,
    pub pivots: Vec<[f32; 3]>,
}

impl CableView {
    pub(crate) fn parse(args: &str) -> Option<Self> {
        let w: Vec<&str> = args.split_whitespace().collect();
        if w.len() < 13 || !(w.len() - 13).is_multiple_of(3) || w.len() > 13 + 3 * 32 {
            return None;
        }
        let num = |s: &str| s.parse::<f32>().ok().filter(|v| v.is_finite() && v.abs() <= 1_000_000.0);
        let v3 = |k: usize| Some([num(w[k])?, num(w[k + 1])?, num(w[k + 2])?]);
        let state: u8 = w[1].parse().ok()?;
        let pivots = (13..w.len()).step_by(3).map(v3).collect::<Option<Vec<_>>>()?;
        (state <= 2).then_some(())?;
        Some(CableView {
            owner: w[0].parse().ok()?,
            state,
            fire: w[2] == "1",
            socket: v3(3)?,
            normal: v3(6)?,
            end: v3(9)?,
            length: num(w[12])?,
            pivots,
        })
    }
}

impl ClientState {
    /// Updates `cables` (plugged ones only). Plugging, yanking, misses and
    /// the bolt starting queue a `ClientEvent::Cable` (for sounds).
    pub(crate) fn parse_cable(&mut self, args: &str) {
        let Some(c) = CableView::parse(args) else { return };
        let old = self.cables.iter().position(|o| o.owner == c.owner);
        let was = old.map(|i| self.cables[i].fire);
        if let Some(i) = old {
            self.cables.remove(i);
        }
        let event = match c.state {
            1 => was.is_none() || (c.fire && was == Some(false)),
            0 => old.is_some(),
            _ => true,
        };
        if event {
            self.events.push(ClientEvent::Cable(c.clone()));
        }
        if c.state == 1 {
            self.cables.push(c);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cable_lines_parse_and_track_plugs() {
        let plugged = "0 1 0 1 2 3 0 0 1 0 0 0 512";
        assert!(CableView::parse(plugged).is_some());
        let c = CableView::parse("1 1 1 1 2 3 0 0 1 4 5 6 512 7 8 9 10 11 12").unwrap();
        assert_eq!(c.pivots, vec![[7.0, 8.0, 9.0], [10.0, 11.0, 12.0]]);
        assert!(c.fire);
        for bad in ["0 3 0 1 2 3 0 0 1 0 0 0 512", "0 1 0 1 2 3 0 0 1 0 0 0", "0 1 0 1 2 3 0 0 1 0 0 0 512 1", "0 1 0 NaN 2 3 0 0 1 0 0 0 512"] {
            assert!(CableView::parse(bad).is_none(), "{bad}");
        }
        let mut cl = ClientState::new();
        cl.parse_cable(plugged);
        cl.parse_cable(plugged);
        cl.parse_cable("0 1 1 1 2 3 0 0 1 9 9 9 512");
        assert_eq!(cl.cables.len(), 1);
        cl.parse_cable("0 0 0 1 2 3 0 0 0 0 0 0 0");
        assert!(cl.cables.is_empty());
        let n = cl.events.iter().filter(|e| matches!(e, ClientEvent::Cable(_))).count();
        assert_eq!(n, 3, "plug, bolt start, yank");
    }
}
