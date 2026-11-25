//! Demo timeline: a background pass replays the whole file through a bare
//! client parser and records, per map, where it starts and which server
//! times it covers. Playback turns server time into one running clock
//! across map changes (`base` + time since the map's first `svc_time`),
//! which the playback bar, seeking and clip export use. A map's clock
//! starts at its first frame in game (signon 4).

use qclient::{ClientEvent, ClientState};

use super::Demo;

/// One map (serverinfo) of a demo.
#[derive(Clone, Debug, PartialEq)]
pub struct MapSpan {
    pub name: String,
    /// Offset of the block holding the serverinfo.
    pub start_pos: usize,
    /// First and last server time seen on this map.
    pub t0: f64,
    pub t1: f64,
    /// Running demo time at `t0`.
    pub base: f64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DemoIndex {
    pub maps: Vec<MapSpan>,
    pub duration: f64,
}

impl DemoIndex {
    /// Running demo time for server time `t` on map `ord`.
    pub fn clock(&self, ord: usize, t: f64) -> f64 {
        match self.maps.get(ord) {
            Some(m) => m.base + (t - m.t0).clamp(0.0, (m.t1 - m.t0).max(0.0)),
            None => self.duration,
        }
    }

    /// The map and server time at running time `clock`.
    pub fn locate(&self, clock: f64) -> Option<(usize, f64)> {
        let clock = clock.max(0.0).min(self.duration);
        let ord = self.maps.iter().rposition(|m| m.base <= clock)?;
        let m = &self.maps[ord];
        Some((ord, m.t0 + (clock - m.base).min(m.t1 - m.t0)))
    }

    /// Map boundaries as running times (for ticks on the bar).
    pub fn boundaries(&self) -> impl Iterator<Item = f64> + '_ {
        self.maps.iter().skip(1).map(|m| m.base)
    }
}

/// A map being scanned: name, serverinfo block offset, first/last time.
type OpenMap = Option<(String, usize, Option<(f64, f64)>)>;

/// Replays `data` (a whole .dem) and builds its index. Damaged tails end
/// the scan where the player would stop too.
pub fn scan(data: &[u8]) -> DemoIndex {
    let mut d = Demo::new(data.to_vec());
    let mut cl = ClientState::new();
    cl.demoplayback = true;
    let mut idx = DemoIndex::default();
    // A map's clock starts at its first svc_time after the serverinfo.
    let mut open: OpenMap = None;
    let close = |idx: &mut DemoIndex, open: OpenMap| {
        let Some((name, start_pos, Some((t0, t1)))) = open else { return };
        let base = idx.duration;
        idx.duration += t1 - t0;
        idx.maps.push(MapSpan { name, start_pos, t0, t1, base });
    };
    loop {
        let pos = d.pos();
        let Some(msg) = d.next_message() else { break };
        if cl.parse_server_message(msg.data).is_err() {
            break;
        }
        let new_map = cl.events.iter().any(|e| matches!(e, ClientEvent::NewMap));
        let disconnect = cl.events.iter().any(|e| matches!(e, ClientEvent::Disconnect));
        drain(&mut cl);
        if new_map {
            close(&mut idx, open.take());
            open = Some((cl.mapname.clone(), pos, None));
        }
        // the clock starts with the first frame in game, where playback
        // takes its first snapshot
        if let Some((_, _, span)) = &mut open
            && cl.signon == 4
        {
            let t = cl.mtime[0];
            *span = Some(span.map_or((t, t), |(t0, t1)| (t0, t1.max(t))));
        }
        if disconnect {
            break;
        }
    }
    close(&mut idx, open);
    idx
}

/// The per-frame queues the host would drain.
fn drain(cl: &mut ClientState) {
    cl.events.clear();
    cl.weapon_impacts.clear();
    cl.shot_impacts.clear();
    cl.blood_hits.clear();
    cl.fte_queue.clear();
    cl.ack_frames.clear();
    cl.take_reliable_out();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(name: &str, t0: f64, t1: f64, base: f64) -> MapSpan {
        MapSpan { name: name.into(), start_pos: 0, t0, t1, base }
    }

    /// A damaged demo has any times.
    #[test]
    fn locate_takes_any_duration() {
        for d in [f64::NAN, f64::NEG_INFINITY, -1.0, f64::INFINITY] {
            let idx = DemoIndex { maps: vec![span("e1m1", 1.0, 11.0, 0.0)], duration: d };
            for t in [0.0, 5.0, f64::NAN] {
                let _ = idx.locate(t);
            }
        }
    }

    #[test]
    fn clock_runs_across_maps() {
        let idx = DemoIndex { maps: vec![span("e1m1", 1.0, 11.0, 0.0), span("e1m2", 0.5, 20.5, 10.0)], duration: 30.0 };
        assert_eq!(idx.clock(0, 6.0), 5.0);
        assert_eq!(idx.clock(1, 0.5), 10.0);
        assert_eq!(idx.clock(1, 99.0), 30.0);
        assert_eq!(idx.locate(5.0), Some((0, 6.0)));
        assert_eq!(idx.locate(12.0), Some((1, 2.5)));
        assert_eq!(idx.locate(-3.0), Some((0, 1.0)));
        assert_eq!(idx.locate(99.0), Some((1, 20.5)));
        assert_eq!(idx.boundaries().collect::<Vec<_>>(), vec![10.0]);
    }

    #[test]
    fn empty_index_locates_nothing() {
        assert_eq!(DemoIndex::default().locate(1.0), None);
    }
}
