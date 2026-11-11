/// Handle into a [`Pool`]; generation-checked.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct PoolHandle {
    slot: u32,
    generation: u32,
}

impl PoolHandle {
    #[inline]
    pub const fn slot(self) -> u32 {
        self.slot
    }
}

const EMPTY: u32 = u32::MAX;

/// Typed free-list pool with dense (contiguous) iteration.
/// Optional fixed capacity mirrors Quake's MAX_BEAMS / MAX_DLIGHTS / MAX_TEMP_ENTITIES.
#[derive(Debug, Clone)]
pub struct Pool<T> {
    data: Vec<T>,
    owners: Vec<u32>,      // dense -> slot
    dense_of: Vec<u32>,    // slot -> dense (EMPTY if free)
    generations: Vec<u32>, // slot -> generation
    free: Vec<u32>,
    max: usize,
}

impl<T> Default for Pool<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Pool<T> {
    /// Unbounded pool.
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            owners: Vec::new(),
            dense_of: Vec::new(),
            generations: Vec::new(),
            free: Vec::new(),
            max: usize::MAX,
        }
    }
    /// Fixed-capacity pool; preallocates so `insert` never allocates.
    pub fn with_max(max: usize) -> Self {
        Self {
            data: Vec::with_capacity(max),
            owners: Vec::with_capacity(max),
            dense_of: Vec::with_capacity(max),
            generations: Vec::with_capacity(max),
            free: Vec::with_capacity(max),
            max,
        }
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.data.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
    #[inline]
    pub fn is_full(&self) -> bool {
        self.data.len() >= self.max
    }
    pub fn max(&self) -> usize {
        self.max
    }

    /// Insert; `None` if at capacity.
    pub fn insert(&mut self, value: T) -> Option<PoolHandle> {
        if self.is_full() {
            return None;
        }
        let slot = match self.free.pop() {
            Some(s) => s,
            None => {
                self.dense_of.push(EMPTY);
                self.generations.push(0);
                (self.dense_of.len() - 1) as u32
            }
        };
        self.dense_of[slot as usize] = self.data.len() as u32;
        self.owners.push(slot);
        self.data.push(value);
        Some(PoolHandle { slot, generation: self.generations[slot as usize] })
    }

    #[inline]
    fn dense(&self, h: PoolHandle) -> Option<usize> {
        let s = h.slot as usize;
        let d = *self.dense_of.get(s)?;
        (d != EMPTY && self.generations[s] == h.generation).then_some(d as usize)
    }

    #[inline]
    pub fn contains(&self, h: PoolHandle) -> bool {
        self.dense(h).is_some()
    }
    #[inline]
    pub fn get(&self, h: PoolHandle) -> Option<&T> {
        self.dense(h).map(|d| &self.data[d])
    }
    #[inline]
    pub fn get_mut(&mut self, h: PoolHandle) -> Option<&mut T> {
        self.dense(h).map(|d| &mut self.data[d])
    }

    pub fn remove(&mut self, h: PoolHandle) -> Option<T> {
        let d = self.dense(h)?;
        Some(self.remove_dense(d))
    }
    fn remove_dense(&mut self, d: usize) -> T {
        let slot = self.owners.swap_remove(d);
        let v = self.data.swap_remove(d);
        self.dense_of[slot as usize] = EMPTY;
        self.generations[slot as usize] = self.generations[slot as usize].wrapping_add(1);
        self.free.push(slot);
        if let Some(&moved) = self.owners.get(d) {
            self.dense_of[moved as usize] = d as u32;
        }
        v
    }

    /// Dense slice (unordered).
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }
    pub fn iter(&self) -> impl Iterator<Item = (PoolHandle, &T)> {
        self.owners.iter().zip(&self.data).map(|(&s, v)| {
            (PoolHandle { slot: s, generation: self.generations[s as usize] }, v)
        })
    }

    /// Keep elements for which `f` returns true (swap-remove kill, order not preserved).
    pub fn retain(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        let mut d = 0;
        while d < self.data.len() {
            if f(&mut self.data[d]) {
                d += 1;
            } else {
                self.remove_dense(d);
            }
        }
    }

    /// Remove all; outstanding handles become stale.
    pub fn clear(&mut self) {
        while !self.data.is_empty() {
            self.remove_dense(self.data.len() - 1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_basics() {
        let mut p = Pool::with_max(3);
        let a = p.insert(1).unwrap();
        let b = p.insert(2).unwrap();
        let c = p.insert(3).unwrap();
        assert!(p.insert(4).is_none());
        assert_eq!(p.remove(a), Some(1));
        assert_eq!(p.get(a), None);
        assert_eq!(p.get(c), Some(&3));
        let d = p.insert(5).unwrap();
        assert_eq!(d.slot(), a.slot());
        assert_ne!(d, a);
        *p.get_mut(b).unwrap() = 20;
        let mut all: Vec<_> = p.as_slice().to_vec();
        all.sort();
        assert_eq!(all, vec![3, 5, 20]);
        p.retain(|v| *v != 5);
        assert!(!p.contains(d));
        assert_eq!(p.get(b), Some(&20));
        assert_eq!(p.iter().count(), 2);
        for (h, v) in p.iter() {
            assert_eq!(p.get(h), Some(v));
        }
        p.clear();
        assert!(p.is_empty() && p.get(b).is_none());
    }
}
