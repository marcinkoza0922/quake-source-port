/// Capacity-capped dense list; removal swaps the last element into the hole,
/// so order is not preserved. The cap mirrors Quake's MAX_BEAMS / MAX_DLIGHTS.
#[derive(Debug, Clone)]
pub struct Pool<T> {
    data: Vec<T>,
    max: usize,
}

impl<T> Pool<T> {
    /// Preallocates so `insert` never allocates.
    pub fn with_max(max: usize) -> Self {
        Self { data: Vec::with_capacity(max), max }
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

    /// Append; returns false (and drops `value`) when at capacity.
    pub fn insert(&mut self, value: T) -> bool {
        if self.is_full() {
            return false;
        }
        self.data.push(value);
        true
    }

    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.data
    }
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        &mut self.data
    }
    pub fn iter(&self) -> std::slice::Iter<'_, T> {
        self.data.iter()
    }

    /// Keep elements for which `f` returns true (swap-remove, order not preserved).
    pub fn retain(&mut self, mut f: impl FnMut(&mut T) -> bool) {
        let mut d = 0;
        while d < self.data.len() {
            if f(&mut self.data[d]) {
                d += 1;
            } else {
                self.data.swap_remove(d);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pool_basics() {
        let mut p = Pool::with_max(3);
        assert!(p.insert(1) && p.insert(2) && p.insert(3));
        assert!(p.is_full() && !p.insert(4));
        p.as_mut_slice()[1] = 20;
        p.retain(|v| *v != 1);
        assert_eq!(p.as_slice(), &[3, 20]);
        assert_eq!(p.iter().count(), 2);
        p.retain(|_| false);
        assert!(p.is_empty());
    }
}
