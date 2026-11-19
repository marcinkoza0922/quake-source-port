//! Snapshots of the client state taken during playback, so seeking back
//! restores the nearest earlier one and replays only the blocks after it.
//! Our own demos carry FTE replacement deltas (each frame only lists what
//! changed), so ironwail's way of rewinding (re-reading an older block over
//! the current state) would show wrong entities; a snapshot is exact.
//!
//! Spacing: dense ([`DENSE`]) within [`NEAR`] of the play head, so holding
//! rewind replays at most half a second per frame, and sparse ([`SPARSE`])
//! elsewhere. A memory budget thins the sparse ones further on long demos.

use qclient::ClientState;

/// Seconds between snapshots near the play head and elsewhere.
pub const DENSE: f64 = 0.5;
pub const SPARSE: f64 = 5.0;
/// Distance from the play head (s, same map) that keeps dense spacing.
pub const NEAR: f64 = 12.0;
/// Rough upper bound for all snapshots together.
pub const BUDGET: usize = 384 << 20;

pub struct Keyframe<T = Box<ClientState>> {
    /// Map ordinal (serverinfo count - 1) and server time (`mtime[0]`).
    pub ord: usize,
    pub time: f64,
    /// Offset of the next block to read after restoring.
    pub pos: usize,
    pub state: T,
    pub bytes: usize,
}

pub struct Keyframes<T = Box<ClientState>> {
    /// Sorted by (ord, time).
    list: Vec<Keyframe<T>>,
    bytes: usize,
}

impl<T> Default for Keyframes<T> {
    fn default() -> Self {
        Self { list: Vec::new(), bytes: 0 }
    }
}

impl<T> Keyframes<T> {
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.list.len()
    }

    /// True when a snapshot at (ord, time) would add nothing: one exists
    /// within [`DENSE`] of it.
    pub fn covered(&self, ord: usize, time: f64) -> bool {
        self.list.iter().any(|k| k.ord == ord && (k.time - time).abs() < DENSE)
    }

    /// Adds a snapshot, then thins those away from the play head (`ord`, `time`).
    pub fn insert(&mut self, k: Keyframe<T>) {
        let (ord, time) = (k.ord, k.time);
        let i = self.list.partition_point(|e| (e.ord, e.time) < (k.ord, k.time));
        self.bytes += k.bytes;
        self.list.insert(i, k);
        self.thin(ord, time, SPARSE);
        let mut spacing = SPARSE;
        while self.bytes > BUDGET && spacing < 3600.0 {
            spacing *= 2.0;
            self.thin(ord, time, spacing);
        }
    }

    /// Drops snapshots further than [`NEAR`] from the play head that are
    /// closer than `spacing` to the one kept before them. The first of each
    /// map always stays.
    fn thin(&mut self, ord: usize, time: f64, spacing: f64) {
        let mut last: Option<(usize, f64)> = None;
        let mut freed = 0;
        self.list.retain(|k| {
            let near = k.ord == ord && (k.time - time).abs() <= NEAR;
            let keep = near || last.is_none_or(|(o, t)| o != k.ord || k.time - t >= spacing);
            if keep {
                last = Some((k.ord, k.time));
            } else {
                freed += k.bytes;
            }
            keep
        });
        self.bytes -= freed;
    }

    /// The earliest snapshot on map `ord` (rewinding stops there).
    pub fn first(&self, ord: usize) -> Option<&Keyframe<T>> {
        self.list.iter().find(|k| k.ord == ord)
    }

    /// The latest snapshot on map `ord` at or before server time `time`.
    pub fn best(&self, ord: usize, time: f64) -> Option<&Keyframe<T>> {
        self.list.iter().rev().find(|k| k.ord == ord && k.time <= time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kf(ord: usize, time: f64) -> Keyframe<()> {
        Keyframe { ord, time, pos: (time * 10.0) as usize, state: (), bytes: 1 }
    }

    #[test]
    fn best_is_latest_before_target_on_the_same_map() {
        let mut k = Keyframes::default();
        k.insert(kf(1, 0.5));
        for t in [1.0, 2.0, 3.0] {
            k.insert(kf(0, t));
        }
        assert_eq!(k.best(0, 2.5).map(|k| k.time), Some(2.0));
        assert_eq!(k.best(0, 0.5).map(|k| k.time), None);
        assert_eq!(k.best(1, 9.0).map(|k| k.time), Some(0.5));
        assert!(k.covered(0, 2.2));
        assert!(!k.covered(1, 2.2));
        // moving on to another map thins this one to sparse spacing
        k.insert(kf(1, 9.0));
        assert_eq!(k.best(0, 9.0).map(|k| k.time), Some(1.0));
    }

    #[test]
    fn far_snapshots_thin_to_sparse_spacing() {
        let mut k = Keyframes::default();
        let mut t = 0.0;
        while t <= 60.0 {
            k.insert(kf(0, t));
            t += DENSE;
        }
        // play head at 60: dense back to 48, sparse before
        let times: Vec<f64> = k.list.iter().map(|k| k.time).collect();
        assert!(times.iter().filter(|&&t| t >= 60.0 - NEAR).count() >= (NEAR / DENSE) as usize);
        let far: Vec<f64> = times.iter().copied().filter(|&t| t < 60.0 - NEAR).collect();
        assert!(far.windows(2).all(|w| w[1] - w[0] >= SPARSE), "{far:?}");
        assert_eq!(far.first(), Some(&0.0));
        assert_eq!(k.bytes, k.len());
    }
}
