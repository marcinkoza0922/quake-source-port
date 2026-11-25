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

/// Most stickybombs, portal owners and gel splats a level has.
const MAX_STICKIES: usize = 64;
const MAX_OWNERS: f32 = 16.0;
const MAX_GEL: usize = 4096;

/// Numbers no bigger than any map; `None` when one isn't.
fn numbers(args: &str) -> Option<Vec<f32>> {
    let n: Vec<f32> = args.split_whitespace().filter_map(|w| w.parse().ok()).collect();
    n.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.0).then_some(n)
}

impl ClientState {
    /// Keeps `qsp_boom` marks (`m owner ...`) and flights (`t`, `f owner x y z`,
    /// over with `c owner`).
    pub(crate) fn parse_boom(&mut self, line: &str) {
        let mut w = line.split_whitespace();
        let (Some(kind), Some(o)) = (w.next(), w.next().and_then(|w| w.parse::<f32>().ok())) else { return };
        if !(0.0..MAX_OWNERS).contains(&o) || line.len() > 512 {
            return;
        }
        let owner = o as u8;
        let i = self.boom.iter().position(|b| b.0 == owner).unwrap_or_else(|| {
            self.boom.push((owner, [None, None]));
            self.boom.len() - 1
        });
        let [marks, flight] = &mut self.boom[i].1;
        match kind {
            "m" => *marks = w.next().is_some().then(|| line.into()),
            // (without the throwing sound)
            "t" | "f" => *flight = Some(format!("f {}", line.split_whitespace().skip(1).collect::<Vec<_>>().join(" ")).into()),
            "c" => *flight = None,
            _ => {}
        }
    }

    /// Keeps `qsp_gel` splats: `a id ...` adds one, `d id` removes it, `c` all.
    pub(crate) fn parse_gel(&mut self, line: &str) {
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        let id = rest.split_whitespace().next().and_then(|w| w.parse::<f32>().ok()).map(|v| v as u32);
        match (op, id) {
            ("c", _) => self.gel.clear(),
            ("a" | "d", Some(id)) => {
                self.gel.retain(|g| g.0 != id);
                if op == "a" && self.gel.len() < MAX_GEL {
                    self.gel.push((id, line.into()));
                }
            }
            _ => {}
        }
    }

    /// `qsp_sticky id [x y z]` (no position removes it), `qsp_sticky clear`.
    pub(crate) fn parse_sticky(&mut self, args: &str) {
        if args.trim() == "clear" {
            self.stickies.clear();
            return;
        }
        let Some(n) = numbers(args) else { return };
        let Some(&id) = n.first() else { return };
        let id = id as u32;
        self.stickies.retain(|s| s.0 != id);
        if let [_, x, y, z] = n[..]
            && self.stickies.len() < MAX_STICKIES
        {
            self.stickies.push((id, [x, y, z]));
        }
    }

    /// `qsp_portal owner slot [ox oy oz nx ny nz ux uy uz]` (no args after
    /// the slot removes it).
    pub(crate) fn parse_portal(&mut self, args: &str) {
        let Some(n) = numbers(args) else { return };
        let (Some(&o), Some(&s)) = (n.first(), n.get(1)) else { return };
        if !(0.0..MAX_OWNERS).contains(&o) || !(0.0..2.0).contains(&s) {
            return;
        }
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
    use super::*;

    #[test]
    fn states_are_limited() {
        let mut cl = ClientState::new();
        for bad in ["0 0 nan inf 1 0 0 0 0 0 0", "0 2 1 2 3 0 0 1 1 0 0", "16 0 1 2 3 0 0 1 1 0 0", "-1 0 1 2 3 0 0 1 1 0 0", "0 0 2e6 2 3 0 0 1 1 0 0"] {
            cl.parse_portal(bad);
        }
        assert!(cl.portals.is_empty(), "{:?}", cl.portals);
        for i in 0..500 {
            cl.parse_portal(&format!("{} {} 1 2 3 0 0 1 1 0 0", i % 20, i / 20 % 5));
            cl.parse_sticky(&format!("{i} 1 2 3"));
            cl.parse_mag(&format!("{} {} 5 1 0 1 0 1 0 1 0", i % 300, i));
        }
        cl.parse_sticky("5 nan nan nan");
        assert_eq!((cl.portals.len(), cl.stickies.len()), (32, 64));
        assert!(cl.stickies.iter().all(|s| s.1.iter().all(|v| v.is_finite())));
        assert!(cl.mags.is_empty(), "more parts than any gun has");
    }

    #[test]
    fn boomerangs_are_kept_by_owner() {
        let mut cl = ClientState::new();
        for l in ["m 0 5 1.5 7 2", "t 0 1 2 3", "f 0 4 5 6", "t 1 9 9 9", "c 1", "m 99 1 1", "s 5 2 0", "\u{3000}t 2 1 2 3"] {
            cl.parse_boom(l);
        }
        assert_eq!(cl.boom[0], (0, [Some("m 0 5 1.5 7 2".into()), Some("f 0 4 5 6".into())]));
        assert_eq!(cl.boom[1], (1, [None, None]));
        cl.parse_boom("m 0");
        assert_eq!(cl.boom[0].1[0], None);
    }

    #[test]
    fn gel_is_kept_by_id() {
        let mut cl = ClientState::new();
        for l in ["a 1 0 1 2 3 0 0 1 20", "b 0 0 1 2 3 4 5 6", "a 2 1 1 2 3 0 0 1 20", "a 1 1 9 9 9 0 0 1 20", "d 2"] {
            cl.parse_gel(l);
        }
        assert_eq!(cl.gel, [(1, "a 1 1 9 9 9 0 0 1 20".into())]);
        cl.parse_gel("c");
        assert!(cl.gel.is_empty());
    }

    #[test]
    fn rejects_malformed_presentation_messages() {
        assert!(PortalEffect::parse("1 1 0 0 0 0 10 20 30").is_some());
        for bad in ["1 2 0 0 0 0 10 20 30", "1 1 3 0 0 0 10 20 30", "1 1 0 NaN 0 0 10 20 30", "1 1 0 0 0 0 10 20", "1 1 0 0 0 0 10 20 30 extra"] {
            assert!(PortalEffect::parse(bad).is_none());
        }
    }
}
