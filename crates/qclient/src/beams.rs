//! Lightning beams (cl_tent.c CL_UpdateTEnts), including bolts that go
//! through gun portals (`//qsp_beam`).

use qcore::math::Vec3;

use crate::PortalView;
use crate::effects::{TempEntity, normalize};
use crate::state::{ClientState, MAX_TEMP_ENTITIES};

/// Length of one bolt model.
const PIECE: f32 = 30.0;
/// `qserver::addons::portal` oval half axes.
const HALF_W: f32 = 28.0;
const HALF_H: f32 = 48.0;
const MAX_HOPS: usize = 4;

/// beam_t.
#[derive(Debug, Clone, PartialEq)]
pub struct Beam {
    pub entity: i32,
    pub model: &'static str,
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub starttime: f64,
    pub endtime: f64,
    /// Gun portals it goes through; `None` for a straight beam.
    pub path: Option<Box<PortalPath>>,
}

/// Portals a beam goes through.
#[derive(Debug, Clone, PartialEq)]
pub struct PortalPath {
    /// Start as sent (`Beam::start` follows the viewer).
    start: Vec3,
    /// Where the beam would end without portals.
    unfolded: Vec3,
    hops: Vec<Hop>,
}

/// In through `a`, out of `b`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Hop {
    a: PortalView,
    b: PortalView,
}

/// Coordinates of `d` along a portal's normal, right and up.
fn local(p: &PortalView, d: Vec3) -> [f32; 3] {
    let (n, u) = (Vec3::from(p.normal), Vec3::from(p.up));
    [d.dot(n), d.dot(u.cross(n)), d.dot(u)]
}

impl Hop {
    /// The point `p` as carried through (`portal_move::carry`).
    fn carry(&self, p: Vec3) -> Vec3 {
        let [n, r, u] = local(&self.a, p - Vec3::from(self.a.origin));
        let (bn, bu) = (Vec3::from(self.b.normal), Vec3::from(self.b.up));
        Vec3::from(self.b.origin) - bn * n - bu.cross(bn) * r + bu * u
    }

    /// Where `from -> to` goes in, kept inside the oval (the viewer may have
    /// moved since the shot); None if it doesn't go in.
    fn entry(&self, from: Vec3, to: Vec3) -> Option<Vec3> {
        let o = Vec3::from(self.a.origin);
        let (f, t) = (local(&self.a, from - o), local(&self.a, to - o));
        if f[0] < 0.0 || t[0] >= 0.0 {
            return None;
        }
        let k = f[0] / (f[0] - t[0]);
        let (r, u) = (f[1] + (t[1] - f[1]) * k, f[2] + (t[2] - f[2]) * k);
        let out = ((r / HALF_W).powi(2) + (u / HALF_H).powi(2)).sqrt().max(1.0);
        let (n, up) = (Vec3::from(self.a.normal), Vec3::from(self.a.up));
        Some(o + up.cross(n) * (r / out) + up * (u / out))
    }
}

impl PortalPath {
    /// Straight legs from `from` to `end`, in order; `true` on those that
    /// end in a portal.
    fn legs(&self, mut from: Vec3, end: Vec3) -> Vec<(Vec3, Vec3, bool)> {
        // The viewer may have gone through some of the portals since: start
        // at the hop where the shot's own start is closest.
        let (mut skip, mut best) = (0, (self.start - from).length());
        let mut s = self.start;
        for (i, h) in self.hops.iter().enumerate() {
            s = h.carry(s);
            let d = (s - from).length();
            if d < best {
                (skip, best) = (i + 1, d);
            }
        }
        let mut to = self.hops[..skip].iter().fold(self.unfolded, |p, h| h.carry(p));
        let mut out = Vec::with_capacity(self.hops.len() + 1);
        for h in &self.hops[skip..] {
            if let Some(hit) = h.entry(from, to) {
                out.push((from, hit, true));
                from = h.carry(hit);
            }
            to = h.carry(to);
        }
        out.push((from, end, false));
        out
    }
}

/// One line of bolt models; a beam into a portal ends on it, others may
/// stick out past their end (as in C). False when the list is full.
fn pieces(out: &mut Vec<TempEntity>, rng: &mut crate::QRand, model: &'static str, (from, to, portal): (Vec3, Vec3, bool)) -> bool {
    let (from, to) = (from.to_array(), to.to_array());
    let dist = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let (yaw, pitch) = if dist[0] == 0.0 && dist[1] == 0.0 {
        (0.0, if dist[2] > 0.0 { 90.0 } else { 270.0 })
    } else {
        let mut yaw = (dist[1].atan2(dist[0]).to_degrees()) as i32 as f32;
        if yaw < 0.0 {
            yaw += 360.0;
        }
        let fwd = (dist[0] * dist[0] + dist[1] * dist[1]).sqrt();
        let mut pitch = (dist[2].atan2(fwd).to_degrees()) as i32 as f32;
        if pitch < 0.0 {
            pitch += 360.0;
        }
        (yaw, pitch)
    };
    let (dir, mut d) = normalize(dist);
    let mut org = from;
    while d > 0.0 {
        if out.len() >= MAX_TEMP_ENTITIES {
            return false;
        }
        if portal && d < PIECE {
            for j in 0..3 {
                org[j] -= dir[j] * (PIECE - d);
            }
        }
        let roll = (rng.rand() % 360) as f32;
        out.push(TempEntity { model, origin: org, angles: [pitch, yaw, roll] });
        for j in 0..3 {
            org[j] += dir[j] * PIECE;
        }
        d -= PIECE;
    }
    true
}

impl ClientState {
    /// `qsp_beam entity ux uy uz (ox oy oz)..`, right after that entity's
    /// beam: its unfolded end and the centres of the portals it enters.
    pub(crate) fn parse_portal_beam(&mut self, args: &str) {
        let mut words = args.split_whitespace();
        let Some(entity) = words.next().and_then(|w| w.parse::<i32>().ok()) else { return };
        let Ok(n) = words.map(|w| w.parse::<f32>()).collect::<Result<Vec<_>, _>>() else { return };
        if n.len() < 6 || n.len() % 3 != 0 || n.len() / 3 > MAX_HOPS + 1 || n.iter().any(|v| !v.is_finite()) {
            return;
        }
        let mut at = n.as_chunks::<3>().0.iter().copied();
        let Some(unfolded) = at.next() else { return };
        let hops: Option<Vec<Hop>> = at
            .map(|o| {
                let a = *self.portals.iter().find(|p| p.origin == o)?;
                let b = *self.portals.iter().find(|p| p.owner == a.owner && p.slot != a.slot)?;
                Some(Hop { a, b })
            })
            .collect();
        let Some(hops) = hops else { return };
        if let Some(b) = self.beams.as_mut_slice().iter_mut().find(|b| b.entity == entity) {
            b.path = Some(Box::new(PortalPath { start: b.start.into(), unfolded: unfolded.into(), hops }));
        }
    }

    /// CL_UpdateTEnts: beam segments into [`ClientState::temp_entities`].
    pub fn update_tents(&mut self) {
        self.temp_entities.clear();
        self.rng.srand((self.time * 1000.0) as i32 as u32);
        let t = self.time;
        let view = self.viewentity as i32;
        let view_origin = self.entities.get(self.viewentity).map(|e| e.origin);
        let mut out = std::mem::take(&mut self.temp_entities);
        'outer: for b in self.beams.as_mut_slice().iter_mut().filter(|b| b.starttime <= t && b.endtime >= t) {
            if b.entity == view && let Some(o) = view_origin {
                b.start = o;
            }
            let (start, end) = (b.start.into(), b.end.into());
            let Some(path) = &b.path else {
                if !pieces(&mut out, &mut self.rng, b.model, (start, end, false)) {
                    break;
                }
                continue;
            };
            for leg in path.legs(start, end) {
                if !pieces(&mut out, &mut self.rng, b.model, leg) {
                    break 'outer;
                }
            }
        }
        self.temp_entities = out;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Z: [f32; 3] = [0.0, 0.0, 1.0];

    /// Portals on the wall x = 100 (facing back at the origin) and on the
    /// wall y = 200 (facing -y); a bolt from the origin through the first.
    fn client() -> ClientState {
        let mut cl = ClientState::new();
        cl.time = 1.0;
        let p = |slot, origin, normal| PortalView { owner: 0, slot, origin, normal, up: Z };
        cl.portals.push(p(0, [100.0, 0.0, 0.0], [-1.0, 0.0, 0.0]));
        cl.portals.push(p(1, [500.0, 200.0, 0.0], [0.0, -1.0, 0.0]));
        let _ = cl.beams.insert(Beam {
            entity: 1,
            model: "progs/bolt2.mdl",
            start: [0.0; 3],
            end: [500.0, 50.0, 0.0],
            starttime: 0.9,
            endtime: 1.2,
            path: None,
        });
        cl
    }

    fn legs(b: &Beam) -> Vec<(Vec3, Vec3, bool)> {
        b.path.as_ref().unwrap().legs(b.start.into(), b.end.into())
    }

    fn close(a: Vec3, b: [f32; 3]) -> bool {
        (a - Vec3::from(b)).length() < 1e-2
    }

    #[test]
    fn bolt_runs_to_one_portal_and_on_from_the_other() {
        let mut cl = client();
        cl.parse_portal_beam("1 250 0 0 100 0 0");
        cl.update_tents();
        assert_eq!(legs(&cl.beams.as_mut_slice()[0]).len(), 2);
        let (first, second): (Vec<TempEntity>, Vec<TempEntity>) = cl.temp_entities.iter().copied().partition(|t| t.origin[0] < 300.0);
        assert_eq!((first.len(), second.len()), (4, 5));
        // 0, 30, 60 and one pulled back to end on the portal
        assert!(close(first[3].origin.into(), [70.0, 0.0, 0.0]), "{:?}", first[3]);
        assert!(close(second[0].origin.into(), [500.0, 200.0, 0.0]), "{:?}", second[0]);
        assert_eq!((first[0].angles[1], second[0].angles[1]), (0.0, 270.0));
    }

    #[test]
    fn bolt_follows_the_viewer() {
        let mut cl = client();
        cl.parse_portal_beam("1 250 0 0 100 0 0");
        let b = &mut cl.beams.as_mut_slice()[0];
        // a step to the side: in at the matching spot, out mirrored through
        b.start = [0.0, 25.0, 0.0];
        let l = legs(b);
        assert!(close(l[0].1, [100.0, 15.0, 0.0]) && close(l[1].0, [515.0, 200.0, 0.0]), "{l:?}");
        // far to the side: kept in the oval
        b.start = [0.0, 100.0, 0.0];
        assert!(close(legs(b)[0].1, [100.0, HALF_W, 0.0]));
        // gone through the portal: straight on from there
        b.start = [500.0, 180.0, 0.0];
        assert_eq!(legs(b), [([500.0, 180.0, 0.0].into(), [500.0, 50.0, 0.0].into(), false)]);
    }

    #[test]
    fn bad_lines_leave_the_beam_straight() {
        let mut cl = client();
        for bad in ["", "1", "1 250 0 0", "1 250 0 0 100 0", "1 250 0 0 7 7 7", "1 NaN 0 0 100 0 0", "2 250 0 0 100 0 0", "1 x 0 0 100 0 0"] {
            cl.parse_portal_beam(bad);
            assert_eq!(cl.beams.as_mut_slice()[0].path, None, "{bad}");
        }
    }
}
