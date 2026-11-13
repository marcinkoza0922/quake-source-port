//! Add-on portal and stickybomb state from `//qsp_portal`, `//qsp_portal_fx`
//! and `//qsp_sticky` comment stufftext (this engine only).

use crate::ClientState;

/// One add-on portal: owner client slot, slot 0/1, centre, normal, up.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortalView {
    pub owner: u8,
    pub slot: u8,
    pub origin: [f32; 3],
    pub normal: [f32; 3],
    pub up: [f32; 3],
}

impl ClientState {
    /// `qsp_sticky id [x y z]` (no position removes it), `qsp_sticky clear`.
    pub(crate) fn parse_sticky(&mut self, args: &str) {
        if args.trim() == "clear" {
            self.stickies.clear();
            return;
        }
        let n: Vec<f32> = args.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        let Some(&id) = n.first() else { return };
        let id = id as u32;
        self.stickies.retain(|s| s.0 != id);
        if let [_, x, y, z] = n[..] {
            self.stickies.push((id, [x, y, z]));
        }
    }

    /// `qsp_portal owner slot [ox oy oz nx ny nz ux uy uz]` (no args after
    /// the slot removes it).
    pub(crate) fn parse_portal(&mut self, args: &str) {
        let n: Vec<f32> = args.split_whitespace().filter_map(|w| w.parse().ok()).collect();
        let (Some(&o), Some(&s)) = (n.first(), n.get(1)) else { return };
        let (owner, slot) = (o as u8, s as u8);
        self.portals.retain(|p| (p.owner, p.slot) != (owner, slot));
        if let [_, _, ox, oy, oz, nx, ny, nz, ux, uy, uz] = n[..] {
            self.portals.push(PortalView { owner, slot, origin: [ox, oy, oz], normal: [nx, ny, nz], up: [ux, uy, uz] });
        }
    }
}

/// Server-confirmed shot (0 rejected, 1 placed) or crossing (2).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PortalEffect {
    pub owner: u8,
    pub slot: u8,
    pub kind: u8,
    pub start: [f32; 3],
    pub end: [f32; 3],
}

impl PortalEffect {
    pub(crate) fn parse(args: &str) -> Option<Self> {
        let words: Vec<_> = args.split_whitespace().collect();
        if words.len() != 9 { return None; }
        let owner = words[0].parse().ok()?;
        let slot = words[1].parse::<u8>().ok()?;
        let kind = words[2].parse::<u8>().ok()?;
        if slot > 1 || kind > 2 { return None; }
        let mut xyz = [0.0f32; 6];
        for (v, w) in xyz.iter_mut().zip(&words[3..]) {
            *v = w.parse().ok()?;
            if !v.is_finite() || v.abs() > 1_000_000.0 { return None; }
        }
        Some(Self { owner, slot, kind, start: xyz[..3].try_into().ok()?, end: xyz[3..].try_into().ok()? })
    }
}

#[cfg(test)]
mod portal_effect_tests {
    use super::PortalEffect;
    #[test]
    fn rejects_malformed_presentation_messages() {
        assert!(PortalEffect::parse("1 1 0 0 0 0 10 20 30").is_some());
        for bad in ["1 2 0 0 0 0 10 20 30", "1 1 3 0 0 0 10 20 30", "1 1 0 NaN 0 0 10 20 30", "1 1 0 0 0 0 10 20", "1 1 0 0 0 0 10 20 30 extra"] {
            assert!(PortalEffect::parse(bad).is_none());
        }
    }
}
