//! Lightning bolts through gun portals. QC traces a bolt, sends one straight
//! beam from the shooter to `trace_endpos` and traces that line again for
//! damage. When the first trace went through portals the straight line is
//! wrong for both, so the traces that crossed are remembered for the frame:
//! the beam is followed by a `//qsp_beam` line with the portals it took, and
//! the damage traces are put back on the bolt's own path.

use qcore::math::Vec3;

use super::portal::Portal;
use crate::msg::SizeBuf;

/// Beam start against its trace's start, damage trace start likewise
/// (id's damage runs from `self.origin`, 16 below the bolt).
const START_SLACK: f32 = 64.0;
/// Beam end against `trace_endpos`.
const END_SLACK: f32 = 1.0;
/// Damage trace end against `trace_endpos`: 4 past it, and 16 to each side.
const DAMAGE_SLACK: f32 = 32.0;
/// Crossing traces kept per frame.
const MAX_TRACES: usize = 16;

/// A `traceline` that went through portals.
#[derive(Clone, Debug)]
pub(crate) struct Crossing {
    pub time: f64,
    pub pass: u32,
    pub start: Vec3,
    /// Where the trace would have ended with no portals in the way
    /// (the end point unfolded into the shooter's space).
    pub unfolded: Vec3,
    /// `trace_endpos`.
    pub end: Vec3,
    /// Portals entered, in order.
    pub entered: Vec<Portal>,
    /// A beam was sent for it.
    pub bolt: bool,
}

/// QC beam message being written.
#[derive(Clone, Copy, Debug, Default)]
struct Partial {
    dest: i32,
    step: usize,
    entity: i32,
    coords: [f32; 6],
}

/// One `Write*` value that can be part of a beam message.
#[derive(Clone, Copy)]
pub(crate) enum Put {
    Byte(i32),
    Entity(i32),
    Coord(f32),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Bolts {
    traces: Vec<Crossing>,
    partial: Partial,
}

impl Bolts {
    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record(&mut self, t: Crossing) {
        self.traces.retain(|o| o.time == t.time);
        if self.traces.len() == MAX_TRACES {
            self.traces.remove(0);
        }
        self.traces.push(t);
    }

    /// Follow a QC temp entity as it is written; the `//qsp_beam` line once a
    /// beam that crossed portals is complete.
    pub fn put(&mut self, time: f64, dest: i32, v: Put) -> Option<String> {
        use qnet::protocol::{SVC_TEMP_ENTITY, TE_BEAM, TE_LIGHTNING1, TE_LIGHTNING2, TE_LIGHTNING3};
        if self.traces.is_empty() {
            return None;
        }
        let p = &mut self.partial;
        if p.dest != dest {
            *p = Partial { dest, ..Default::default() };
        }
        p.step = match (p.step, v) {
            (1, Put::Byte(b)) if [TE_LIGHTNING1, TE_LIGHTNING2, TE_LIGHTNING3, TE_BEAM].iter().any(|&t| i32::from(t) == b) => 2,
            (_, Put::Byte(b)) => usize::from(b == i32::from(SVC_TEMP_ENTITY)),
            (2, Put::Entity(e)) => {
                p.entity = e;
                3
            }
            (s @ 3..=8, Put::Coord(c)) => {
                p.coords[s - 3] = c;
                s + 1
            }
            _ => 0,
        };
        if p.step < 9 {
            return None;
        }
        let Partial { entity, coords: [ax, ay, az, bx, by, bz], .. } = std::mem::take(p);
        self.beam(time, entity, Vec3::new(ax, ay, az), Vec3::new(bx, by, bz))
    }

    /// The line to send after a beam from `start` to `end`, if it crossed.
    pub fn beam(&mut self, time: f64, entity: i32, start: Vec3, end: Vec3) -> Option<String> {
        use std::fmt::Write;
        let t = self.traces.iter_mut().rev().find(|t| {
            t.time == time && (t.start - start).length() < START_SLACK && (t.end - end).length() < END_SLACK
        })?;
        t.bolt = true;
        let u = t.unfolded;
        let mut s = format!("//qsp_beam {entity} {} {} {}", u.x, u.y, u.z);
        for a in &t.entered {
            let o = a.origin;
            let _ = write!(s, " {} {} {}", o.x, o.y, o.z);
        }
        s.push('\n');
        Some(s)
    }

    /// A bolt's damage trace `v1 -> v2`, moved back onto the bolt's path.
    pub fn damage_line(&self, time: f64, pass: u32, v1: Vec3, v2: Vec3) -> Option<(Vec3, Vec3)> {
        let t = self.traces.iter().rev().find(|t| {
            t.bolt && t.time == time && t.pass == pass && (t.start - v1).length() < START_SLACK && (t.end - v2).length() < DAMAGE_SLACK
        })?;
        let off = v2 - t.end;
        Some((t.start + off, t.unfolded + off))
    }
}

/// Send `line` (from [`Bolts`]) after its beam, in the same buffer.
pub(crate) fn send(buf: &mut SizeBuf, line: &str) {
    if buf.len() + line.len() + 2 <= buf.maxsize {
        buf.byte(crate::msg::SVC_STUFFTEXT);
        buf.string(line.as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crossing() -> Crossing {
        Crossing {
            time: 2.0,
            pass: 1,
            start: Vec3::new(0.0, 0.0, 16.0),
            unfolded: Vec3::new(600.0, 0.0, 16.0),
            end: Vec3::new(0.0, 500.0, 16.0),
            entered: vec![Portal::new(Vec3::new(100.0, 0.0, 16.0), Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))],
            bolt: false,
        }
    }

    fn write(b: &mut Bolts, time: f64, end_y: f32) -> Option<String> {
        use Put::*;
        let mut out = None;
        for v in [Byte(23), Byte(6), Entity(1), Coord(0.0), Coord(0.0), Coord(16.0), Coord(0.0), Coord(end_y), Coord(16.0)] {
            assert_eq!(out, None);
            out = b.put(time, 0, v);
        }
        out
    }

    #[test]
    fn beam_of_a_crossing_trace_gets_a_line_and_redirects_damage() {
        let mut b = Bolts::default();
        assert_eq!(write(&mut b, 2.0, 500.0), None);
        b.record(crossing());
        let (a, e) = (Vec3::ZERO, Vec3::new(0.0, 504.0, 16.0));
        assert_eq!(b.damage_line(2.0, 1, a, e), None, "no bolt sent yet");
        assert_eq!(write(&mut b, 2.0, 300.0), None, "another beam");
        assert_eq!(write(&mut b, 2.5, 500.0), None, "another frame");
        assert_eq!(write(&mut b, 2.0, 500.0).as_deref(), Some("//qsp_beam 1 600 0 16 100 0 16\n"));
        let off = Vec3::new(0.0, 4.0, 0.0);
        assert_eq!(b.damage_line(2.0, 1, a, e), Some((crossing().start + off, crossing().unfolded + off)));
        assert_eq!(b.damage_line(2.0, 2, a, e), None, "another shooter");
    }

    #[test]
    fn other_messages_are_not_beams() {
        use Put::*;
        let mut b = Bolts::default();
        b.record(crossing());
        for v in [Byte(23), Byte(3), Coord(0.0), Coord(0.0), Coord(16.0), Coord(0.0), Coord(500.0), Coord(16.0)] {
            assert_eq!(b.put(2.0, 0, v), None);
        }
        // a message for someone else in between
        b.put(2.0, 0, Byte(23));
        b.put(2.0, 1, Byte(6));
        assert_eq!(write(&mut b, 2.0, 500.0).as_deref(), Some("//qsp_beam 1 600 0 16 100 0 16\n"));
    }
}
