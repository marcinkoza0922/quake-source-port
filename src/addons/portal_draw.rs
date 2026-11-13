//! Portal-gun portals for the renderer: each portal in the owner's colour
//! pair, linked to its partner so qrender can draw the view out of it.

use qrender::GunPortal;
use qserver::addons::portal::{HALF_H, HALF_W};

/// (fire, alt-fire) colours per player, cycling after the fourth.
const COLOURS: [[[u8; 3]; 2]; 4] = [
    [[40, 130, 255], [255, 140, 20]],
    [[60, 230, 90], [180, 70, 255]],
    [[40, 230, 230], [240, 50, 50]],
    [[250, 230, 40], [240, 60, 200]],
];

pub fn colour(owner: usize, slot: usize) -> [u8; 3] {
    COLOURS[owner % COLOURS.len()][slot & 1]
}

/// The client's portals as renderer portals. `live`: draw the view through
/// linked portals (`addon_portal_view`).
pub fn gun_portals(portals: &[qclient::PortalView], live: bool) -> Vec<GunPortal> {
    portals
        .iter()
        .map(|p| {
            let exit = portals.iter().find(|q| q.owner == p.owner && q.slot != p.slot).map(|q| (q.origin, q.normal, q.up));
            GunPortal {
                origin: p.origin,
                normal: p.normal,
                up: p.up,
                half: [HALF_W, HALF_H],
                color: colour(p.owner as usize, p.slot as usize),
                exit,
                live,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partners_link_by_owner() {
        let v = |owner, slot| qclient::PortalView { owner, slot, origin: [slot as f32; 3], normal: [0.0, 0.0, 1.0], up: [1.0, 0.0, 0.0] };
        let g = gun_portals(&[v(0, 0), v(1, 0), v(0, 1)], true);
        assert_eq!(g[0].exit.map(|e| e.0), Some([1.0; 3]));
        assert_eq!(g[1].exit, None);
        assert_ne!(g[0].color, g[1].color);
    }
}
