use crate::Entity;

const EMPTY: u32 = u32::MAX;

/// Sparse-set component storage keyed by entity index; generation-checked.
#[derive(Debug, Clone)]
pub struct SparseSet<T> {
    sparse: Vec<u32>,
    entities: Vec<Entity>,
    data: Vec<T>,
}

impl<T> Default for SparseSet<T> {
    fn default() -> Self {
        Self { sparse: Vec::new(), entities: Vec::new(), data: Vec::new() }
    }
}

impl<T> SparseSet<T> {
    pub fn new() -> Self {
        Self::default()
    }
    /// `max_index`: expected entity index bound; `cap`: expected component count.
    pub fn with_capacity(max_index: usize, cap: usize) -> Self {
        Self {
            sparse: vec![EMPTY; max_index],
            entities: Vec::with_capacity(cap),
            data: Vec::with_capacity(cap),
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
    fn slot(&self, e: Entity) -> Option<usize> {
        let d = *self.sparse.get(e.index() as usize)?;
        (d != EMPTY && self.entities[d as usize] == e).then_some(d as usize)
    }
    #[inline]
    fn raw_slot(&self, index: u32) -> Option<usize> {
        let d = *self.sparse.get(index as usize)?;
        (d != EMPTY).then_some(d as usize)
    }

    /// Insert or replace. A stale-generation component at the same index is replaced.
    /// Returns the previous value if `e` itself already had one.
    pub fn insert(&mut self, e: Entity, value: T) -> Option<T> {
        let i = e.index() as usize;
        if i >= self.sparse.len() {
            self.sparse.resize(i + 1, EMPTY);
        }
        let d = self.sparse[i];
        if d != EMPTY {
            let d = d as usize;
            let same = self.entities[d] == e;
            self.entities[d] = e;
            let old = std::mem::replace(&mut self.data[d], value);
            return same.then_some(old);
        }
        self.sparse[i] = self.data.len() as u32;
        self.entities.push(e);
        self.data.push(value);
        None
    }

    pub fn remove(&mut self, e: Entity) -> Option<T> {
        let d = self.slot(e)?;
        Some(self.remove_dense(d))
    }
    /// Remove whatever is stored at a raw index, regardless of generation.
    pub fn remove_raw(&mut self, index: u32) -> Option<T> {
        let d = self.raw_slot(index)?;
        Some(self.remove_dense(d))
    }
    fn remove_dense(&mut self, d: usize) -> T {
        let e = self.entities.swap_remove(d);
        let v = self.data.swap_remove(d);
        self.sparse[e.index() as usize] = EMPTY;
        if let Some(moved) = self.entities.get(d) {
            self.sparse[moved.index() as usize] = d as u32;
        }
        v
    }

    #[inline]
    pub fn contains(&self, e: Entity) -> bool {
        self.slot(e).is_some()
    }
    #[inline]
    pub fn get(&self, e: Entity) -> Option<&T> {
        self.slot(e).map(|d| &self.data[d])
    }
    #[inline]
    pub fn get_mut(&mut self, e: Entity) -> Option<&mut T> {
        self.slot(e).map(|d| &mut self.data[d])
    }
    /// Raw-index access (QC edict number), no generation check.
    #[inline]
    pub fn get_raw(&self, index: u32) -> Option<&T> {
        self.raw_slot(index).map(|d| &self.data[d])
    }
    #[inline]
    pub fn get_raw_mut(&mut self, index: u32) -> Option<&mut T> {
        self.raw_slot(index).map(|d| &mut self.data[d])
    }

    #[inline]
    pub fn entities(&self) -> &[Entity] {
        &self.entities
    }
    #[inline]
    pub fn values(&self) -> &[T] {
        &self.data
    }
    #[inline]
    pub fn values_mut(&mut self) -> &mut [T] {
        &mut self.data
    }
    pub fn iter(&self) -> impl Iterator<Item = (Entity, &T)> {
        self.entities.iter().copied().zip(&self.data)
    }
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (Entity, &mut T)> {
        self.entities.iter().copied().zip(&mut self.data)
    }

    /// Keep only components for which `f` returns true.
    pub fn retain(&mut self, mut f: impl FnMut(Entity, &mut T) -> bool) {
        let mut d = 0;
        while d < self.data.len() {
            if f(self.entities[d], &mut self.data[d]) {
                d += 1;
            } else {
                self.remove_dense(d);
            }
        }
    }

    pub fn clear(&mut self) {
        for e in &self.entities {
            self.sparse[e.index() as usize] = EMPTY;
        }
        self.entities.clear();
        self.data.clear();
    }
}

/// Iterate entities present in both storages, driving from the smaller one.
pub fn join2<'a, A, B>(
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
) -> impl Iterator<Item = (Entity, &'a A, &'a B)> {
    let drive = if a.len() <= b.len() { a.entities() } else { b.entities() };
    drive
        .iter()
        .filter_map(move |&e| Some((e, a.get(e)?, b.get(e)?)))
}

/// Iterate entities present in all three storages, driving from the smallest.
pub fn join3<'a, A, B, C>(
    a: &'a SparseSet<A>,
    b: &'a SparseSet<B>,
    c: &'a SparseSet<C>,
) -> impl Iterator<Item = (Entity, &'a A, &'a B, &'a C)> {
    let mut drive = a.entities();
    if b.len() < drive.len() {
        drive = b.entities();
    }
    if c.len() < drive.len() {
        drive = c.entities();
    }
    drive
        .iter()
        .filter_map(move |&e| Some((e, a.get(e)?, b.get(e)?, c.get(e)?)))
}

impl<T> SparseSet<T> {
    /// Mutable join: for each entity in `self` and `other`, call `f(e, &mut T, &B)`.
    /// Drives from the smaller storage.
    pub fn join_mut<B>(&mut self, other: &SparseSet<B>, mut f: impl FnMut(Entity, &mut T, &B)) {
        if self.len() <= other.len() {
            for (e, v) in self.iter_mut() {
                if let Some(o) = other.get(e) {
                    f(e, v, o);
                }
            }
        } else {
            for (&e, o) in other.entities.iter().zip(&other.data) {
                if let Some(v) = self.get_mut(e) {
                    f(e, v, o);
                }
            }
        }
    }

    /// Mutable 3-way join: `f(e, &mut T, &B, &C)`, driving from the smallest.
    pub fn join2_mut<B, C>(
        &mut self,
        b: &SparseSet<B>,
        c: &SparseSet<C>,
        mut f: impl FnMut(Entity, &mut T, &B, &C),
    ) {
        let min = self.len().min(b.len()).min(c.len());
        if self.len() == min {
            for (e, v) in self.iter_mut() {
                if let (Some(bv), Some(cv)) = (b.get(e), c.get(e)) {
                    f(e, v, bv, cv);
                }
            }
        } else {
            let drive = if b.len() == min { b.entities() } else { c.entities() };
            for &e in drive {
                if let (Some(bv), Some(cv)) = (b.get(e), c.get(e))
                    && let Some(v) = self.get_mut(e)
                {
                    f(e, v, bv, cv);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(i: u32) -> Entity {
        Entity::new(i, 0)
    }

    #[test]
    fn insert_get_remove() {
        let mut s = SparseSet::new();
        assert_eq!(s.insert(e(3), 30), None);
        assert_eq!(s.insert(e(1), 10), None);
        assert_eq!(s.insert(e(7), 70), None);
        assert_eq!(s.insert(e(1), 11), Some(10));
        assert_eq!(s.get(e(1)), Some(&11));
        assert_eq!(s.get(Entity::new(1, 1)), None);
        assert_eq!(s.get_raw(1), Some(&11));
        *s.get_mut(e(7)).unwrap() += 1;
        assert_eq!(s.remove(e(3)), Some(30));
        assert_eq!(s.remove(e(3)), None);
        assert_eq!(s.get(e(7)), Some(&71));
        assert_eq!(s.len(), 2);
        // stale generation replaced
        assert_eq!(s.insert(Entity::new(7, 1), 99), None);
        assert_eq!(s.get(e(7)), None);
        assert_eq!(s.len(), 2);
        s.retain(|_, v| *v != 11);
        assert_eq!(s.values(), &[99]);
        s.clear();
        assert!(s.is_empty() && s.get_raw(7).is_none());
    }

    #[test]
    fn joins() {
        let mut a = SparseSet::new();
        let mut b = SparseSet::new();
        let mut c = SparseSet::new();
        for i in 0..10 {
            a.insert(e(i), i);
        }
        for i in (0..10).step_by(2) {
            b.insert(e(i), i * 10);
        }
        for i in (0..10).step_by(3) {
            c.insert(e(i), ());
        }
        let j: Vec<_> = join2(&a, &b).map(|(e, _, v)| (e.index(), *v)).collect();
        assert_eq!(j, vec![(0, 0), (2, 20), (4, 40), (6, 60), (8, 80)]);
        let mut j3: Vec<_> = join3(&a, &b, &c).map(|(e, ..)| e.index()).collect();
        j3.sort();
        assert_eq!(j3, vec![0, 6]);
        a.join_mut(&b, |_, v, w| *v += *w);
        assert_eq!(a.get(e(2)), Some(&22));
        assert_eq!(a.get(e(3)), Some(&3));
        b.join2_mut(&a, &c, |_, v, _, _| *v = 0);
        assert_eq!(b.get(e(6)), Some(&0));
        assert_eq!(b.get(e(2)), Some(&20));
    }
}
