/// Generational entity handle. `index` doubles as the QC edict number.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    #[inline]
    pub const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
    #[inline]
    pub const fn index(self) -> u32 {
        self.index
    }
    #[inline]
    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    generation: u32,
    alive: bool,
    freed_at: f32,
}

/// Entity allocator with a free list and fixed-index reservation.
///
/// Reuse policy (e.g. Quake's `ED_Alloc` "freed > 0.5s ago" rule) is the
/// caller's concern: use [`EntityAllocator::alloc_with`] or [`EntityAllocator::free_time`].
#[derive(Default, Debug, Clone)]
pub struct EntityAllocator {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn with_capacity(n: usize) -> Self {
        Self { slots: Vec::with_capacity(n), free: Vec::with_capacity(n) }
    }

    /// Number of index slots ever created (high-water mark).
    #[inline]
    pub fn len(&self) -> usize {
        self.slots.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
    pub fn alive_count(&self) -> usize {
        self.slots.len() - self.free.len()
    }

    fn push_new(&mut self) -> Entity {
        let index = self.slots.len() as u32;
        self.slots.push(Slot { generation: 0, alive: true, freed_at: 0.0 });
        Entity::new(index, 0)
    }

    fn revive(&mut self, index: u32) -> Entity {
        let s = &mut self.slots[index as usize];
        s.alive = true;
        Entity::new(index, s.generation)
    }

    /// Allocate, reusing the most recently freed index if any.
    pub fn alloc(&mut self) -> Entity {
        match self.free.pop() {
            Some(i) => self.revive(i),
            None => self.push_new(),
        }
    }

    /// Allocate reusing the first free index for which `accept(index, freed_at)` holds,
    /// otherwise grow. Example: `alloc_with(|i, t| i >= min && now - t > 0.5)`.
    pub fn alloc_with(&mut self, mut accept: impl FnMut(u32, f32) -> bool) -> Entity {
        let pos = self
            .free
            .iter()
            .position(|&i| accept(i, self.slots[i as usize].freed_at));
        match pos {
            Some(p) => {
                let i = self.free.swap_remove(p);
                self.revive(i)
            }
            None => self.push_new(),
        }
    }

    /// Reserve a specific index (QC edict slots, worldspawn, clients).
    /// Grows as needed; returns `None` if the index is already alive.
    pub fn alloc_at(&mut self, index: u32) -> Option<Entity> {
        let i = index as usize;
        while self.slots.len() <= i {
            let n = self.slots.len() as u32;
            self.slots.push(Slot { generation: 0, alive: false, freed_at: f32::NEG_INFINITY });
            self.free.push(n);
        }
        if self.slots[i].alive {
            return None;
        }
        if let Some(p) = self.free.iter().position(|&f| f == index) {
            self.free.swap_remove(p);
        }
        Some(self.revive(index))
    }

    /// Free an entity at time `now`. Returns false if the handle is stale.
    pub fn free(&mut self, e: Entity, now: f32) -> bool {
        if !self.is_alive(e) {
            return false;
        }
        let s = &mut self.slots[e.index as usize];
        s.alive = false;
        s.generation = s.generation.wrapping_add(1);
        s.freed_at = now;
        self.free.push(e.index);
        true
    }

    #[inline]
    pub fn is_alive(&self, e: Entity) -> bool {
        self.slots
            .get(e.index as usize)
            .is_some_and(|s| s.alive && s.generation == e.generation)
    }

    /// Current live handle at a raw index (QC edict number).
    #[inline]
    pub fn get(&self, index: u32) -> Option<Entity> {
        self.slots
            .get(index as usize)
            .filter(|s| s.alive)
            .map(|s| Entity::new(index, s.generation))
    }

    /// Time the index was last freed; `None` if alive or never existed.
    #[inline]
    pub fn free_time(&self, index: u32) -> Option<f32> {
        self.slots.get(index as usize).filter(|s| !s.alive).map(|s| s.freed_at)
    }

    /// Iterate live entities in index order.
    pub fn iter(&self) -> impl Iterator<Item = Entity> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| s.alive)
            .map(|(i, s)| Entity::new(i as u32, s.generation))
    }

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_free_reuse_bumps_generation() {
        let mut a = EntityAllocator::new();
        let e0 = a.alloc();
        let e1 = a.alloc();
        assert_eq!((e0.index(), e1.index()), (0, 1));
        assert!(a.free(e0, 1.0));
        assert!(!a.is_alive(e0));
        assert!(!a.free(e0, 1.0));
        assert_eq!(a.free_time(0), Some(1.0));
        let e2 = a.alloc();
        assert_eq!(e2.index(), 0);
        assert_eq!(e2.generation(), 1);
        assert!(a.is_alive(e2) && !a.is_alive(e0));
        assert_eq!(a.get(0), Some(e2));
        assert_eq!(a.alive_count(), 2);
    }

    #[test]
    fn alloc_at_and_policy() {
        let mut a = EntityAllocator::new();
        let e5 = a.alloc_at(5).unwrap();
        assert_eq!(e5.index(), 5);
        assert!(a.alloc_at(5).is_none());
        assert_eq!(a.alive_count(), 1);
        assert_eq!(a.iter().collect::<Vec<_>>(), vec![e5]);
        // ED_Alloc-like: skip indices < 2 and recently freed.
        let e3 = a.get(3);
        assert!(e3.is_none());
        let e = a.alloc_at(3).unwrap();
        a.free(e, 10.0);
        let got = a.alloc_with(|i, t| i >= 2 && 10.2 - t > 0.5);
        assert_eq!(got.index(), 2);
        let got = a.alloc_with(|i, t| i >= 2 && 11.0 - t > 0.5);
        assert_eq!(got.index(), 3);
        assert_eq!(got.generation(), 1);
    }
}
