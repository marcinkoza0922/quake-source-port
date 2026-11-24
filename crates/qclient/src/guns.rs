//! Add-on guns: `//qsp_gun` events and `//qsp_mag` magazine state (see
//! `qserver::addons::guns`).

use crate::{ClientEvent, ClientState};

/// `GunEvent::kind` values.
pub const DRY: u8 = 0;
pub const FIRE: u8 = 1;
pub const RELOAD: u8 = 2;
pub const CANCEL: u8 = 3;
pub const DONE: u8 = 4;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunEvent {
    pub owner: u8,
    /// `Toy::id`.
    pub toy: u8,
    /// Which of the toy's guns (akimbo: 0 left, 1 right).
    pub part: u8,
    pub kind: u8,
    /// Reload duration.
    pub secs: f32,
    /// The shot hit something damageable.
    pub flesh: bool,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub normal: [f32; 3],
}

/// One magazine: rounds loaded and, while reloading, the client time the
/// reload ends.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MagPart {
    pub rounds: u32,
    pub reload_end: Option<f64>,
    /// How long that reload takes in all.
    pub reload_secs: f32,
}

/// A player's gun magazines.
#[derive(Clone, Debug, PartialEq)]
pub struct MagView {
    pub owner: u8,
    pub toy: u8,
    pub cap: u32,
    pub parts: Vec<MagPart>,
}

impl GunEvent {
    pub(crate) fn parse(args: &str) -> Option<Self> {
        let w: Vec<f32> = args.split_whitespace().map(|w| w.parse().ok()).collect::<Option<_>>()?;
        if w.len() != 15 || w.iter().any(|x| !x.is_finite() || x.abs() > 1_000_000.0) || w[2] > 1.0 || w[3] > 4.0 {
            return None;
        }
        let p = |i: usize| [w[i], w[i + 1], w[i + 2]];
        Some(Self {
            owner: w[0] as u8, toy: w[1] as u8, part: w[2] as u8, kind: w[3] as u8, secs: w[4], flesh: w[5] != 0.0,
            start: p(6), end: p(9), normal: p(12),
        })
    }
}

impl ClientState {
    pub(crate) fn parse_gun(&mut self, args: &str) {
        if let Some(e) = GunEvent::parse(args) {
            self.events.push(ClientEvent::Gun(e));
        }
    }

    /// `qsp_mag owner toy cap rounds left [rounds left]` (nothing after the toy forgets it).
    pub(crate) fn parse_mag(&mut self, args: &str) {
        let w: Vec<f32> = args.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        let (Some(&owner), Some(&toy)) = (w.first(), w.get(1)) else { return };
        let (owner, toy) = (owner as u8, toy as u8);
        self.mags.retain(|m| (m.owner, m.toy) != (owner, toy));
        if w.len() < 5 || w.len().is_multiple_of(2) {
            return;
        }
        let parts = w[3..].as_chunks::<2>().0.iter()
            .map(|&[rounds, left]| MagPart { rounds: rounds as u32, reload_end: (left > 0.0).then(|| self.mtime[0] + left as f64), reload_secs: left })
            .collect();
        self.mags.push(MagView { owner, toy, cap: w[2] as u32, parts });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_gun_events_and_magazines() {
        let mut cl = ClientState::new();
        cl.mtime[0] = 10.0;
        cl.parse_gun("1 9 1 1 0.00 1 1.0 2.0 3.0 4.0 5.0 6.0 0 0 1");
        let Some(ClientEvent::Gun(e)) = cl.events.last() else { panic!() };
        assert_eq!((e.owner, e.toy, e.part, e.kind, e.flesh, e.end), (1, 9, 1, FIRE, true, [4.0, 5.0, 6.0]));
        for bad in ["1 9 1 1 0 1", "1 9 1 9 0 1 1 2 3 4 5 6 0 0 1", "1 9 1 1 0 1 NaN 2 3 4 5 6 0 0 1"] {
            assert!(GunEvent::parse(bad).is_none(), "{bad}");
        }
        cl.parse_mag("0 9 12 12 0.00 3 1.50");
        assert_eq!(cl.mags[0].parts, vec![MagPart { rounds: 12, reload_end: None, reload_secs: 0.0 }, MagPart { rounds: 3, reload_end: Some(11.5), reload_secs: 1.5 }]);
        cl.parse_mag("0 10 30 30 0");
        assert_eq!(cl.mags.len(), 2);
        cl.parse_mag("0 10 30 29 0");
        assert_eq!((cl.mags.len(), cl.mags[1].parts[0].rounds), (2, 29));
        cl.parse_mag("0 9");
        assert_eq!((cl.mags.len(), cl.mags[0].toy), (1, 10));
    }
}
