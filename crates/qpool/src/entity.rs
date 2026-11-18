/// Generational entity handle. `index` doubles as the QC edict number.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord)]
pub struct Entity {
    index: u32,
    generation: u32,
}

impl Entity {
    #[inline]
    const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }
    #[inline]
    pub const fn index(self) -> u32 {
        self.index
    }
}

#[derive(Clone, Copy, Debug)]
struct Slot {
    generation: u32,
    alive: bool,
    freed_at: f32,
}

/// Entity allocator with a FIFO free list and fixed-index reservation.
///
/// Like ironwail's and vkQuake's `ED_Alloc`, only the oldest freed index (the
/// head of the list) is a reuse candidate; if the caller's policy (e.g. the
/// "freed > 0.5s ago" rule) rejects it, a new index is appended.
#[derive(Default, Debug, Clone)]
pub struct EntityAllocator {
    slots: Vec<Slot>,
    free: std::collections::VecDeque<u32>,
}

impl EntityAllocator {
    pub fn new() -> Self {
        Self::default()
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

    /// Allocate the oldest freed index if `accept(index, freed_at)` holds for
    /// it, otherwise grow. Example: `alloc_with(|i, t| t < 2.0 || now - t > 0.5)`.
    pub fn alloc_with(&mut self, mut accept: impl FnMut(u32, f32) -> bool) -> Entity {
        match self.free.front() {
            Some(&i) if accept(i, self.slots[i as usize].freed_at) => {
                self.free.pop_front();
                self.revive(i)
            }
            _ => self.push_new(),
        }
    }

    /// Whether `alloc_with(accept)` would reuse an index rather than grow.
    pub fn can_reuse(&self, mut accept: impl FnMut(u32, f32) -> bool) -> bool {
        self.free.front().is_some_and(|&i| accept(i, self.slots[i as usize].freed_at))
    }

    /// Reserve a specific index (QC edict slots, worldspawn, clients).
    /// Grows as needed; returns `None` if the index is already alive.
    pub fn alloc_at(&mut self, index: u32) -> Option<Entity> {
        let i = index as usize;
        while self.slots.len() <= i {
            let n = self.slots.len() as u32;
            self.slots.push(Slot { generation: 0, alive: false, freed_at: f32::NEG_INFINITY });
            self.free.push_back(n);
        }
        if self.slots[i].alive {
            return None;
        }
        if let Some(p) = self.free.iter().position(|&f| f == index) {
            self.free.remove(p);
        }
        Some(self.revive(index))
    }

    /// Free an entity at time `now` and queue its index for reuse. Returns
    /// false if the handle is stale.
    pub fn free(&mut self, e: Entity, now: f32) -> bool {
        self.free_inner(e, now, true)
    }

    /// Free without queueing for reuse (ED_AddToFreeList skips the world and
    /// client edicts).
    pub fn free_unlisted(&mut self, e: Entity, now: f32) -> bool {
        self.free_inner(e, now, false)
    }

    fn free_inner(&mut self, e: Entity, now: f32, listed: bool) -> bool {
        if !self.is_alive(e) {
            return false;
        }
        let s = &mut self.slots[e.index as usize];
        s.alive = false;
        s.generation = s.generation.wrapping_add(1);
        s.freed_at = now;
        if listed {
            self.free.push_back(e.index);
        }
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

    pub fn clear(&mut self) {
        self.slots.clear();
        self.free.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reuse_is_first_freed_first() {
        let mut a = EntityAllocator::new();
        let e: Vec<_> = (0..6).map(|_| a.alloc_with(|_, _| true)).collect();
        a.free(e[4], 1.0);
        a.free(e[2], 1.0);
        a.free_unlisted(e[1], 1.0);
        assert_eq!(a.alloc_with(|_, _| true).index(), 4);
        assert_eq!(a.alloc_with(|_, _| true).index(), 2);
        assert_eq!(a.alloc_with(|_, _| true).index(), 6, "unlisted indices are never reused");
    }

    #[test]
    fn free_bumps_generation_and_policy_reuses() {
        let mut a = EntityAllocator::new();
        let e5 = a.alloc_at(5).unwrap();
        assert_eq!(e5.index(), 5);
        assert!(a.alloc_at(5).is_none());
        assert_eq!(a.len(), 6);
        assert!(a.get(3).is_none());
        for i in [0, 1, 2, 4] {
            a.alloc_at(i).unwrap();
        }
        let e3 = a.alloc_at(3).unwrap();
        assert!(a.free(e3, 10.0));
        assert!(!a.is_alive(e3) && !a.free(e3, 10.0));
        assert_eq!(a.free_time(3), Some(10.0));
        // ED_Alloc-like: 3 was freed too recently, so grow
        let got = a.alloc_with(|_, t| 10.2 - t > 0.5);
        assert_eq!(got.index(), 6);
        let got = a.alloc_with(|_, t| 11.0 - t > 0.5);
        assert_eq!(got.index(), 3);
        assert_ne!(got, e3);
        assert_eq!(a.get(3), Some(got));
        a.clear();
        assert!(a.is_empty());
    }
}
