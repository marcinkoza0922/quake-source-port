use super::*;

/// Sub-allocation from a [`Block`].
#[derive(Debug)]
pub struct Mem {
    pub memory: vk::DeviceMemory,
    pub offset: u64,
    pub size: u64,
    /// Host pointer at `offset` (null if not host visible).
    pub ptr: *mut u8,
    pub(super) key: (u32, bool),
    pub(super) block: usize,
}

pub(super) struct Block {
    pub(super) memory: vk::DeviceMemory,
    pub(super) size: u64,
    pub(super) ptr: *mut u8,
    /// Sorted free ranges (offset, size).
    pub(super) free: Vec<(u64, u64)>,
}

/// Big-block allocator: one pool per (memory type, linear) so buffer/image
/// granularity never matters; first-fit with neighbour merging on free.
#[derive(Default)]
pub struct Allocator {
    pub(super) pools: HashMap<(u32, bool), Vec<Block>>,
}

impl Allocator {
    pub(super) fn alloc(
        &mut self,
        dev: &ash::Device,
        props: &vk::PhysicalDeviceMemoryProperties,
        req: vk::MemoryRequirements,
        flags: vk::MemoryPropertyFlags,
        linear: bool,
    ) -> Result<Mem> {
        let ty = (0..props.memory_type_count)
            .find(|&i| {
                req.memory_type_bits & (1 << i) != 0
                    && props.memory_types[i as usize]
                        .property_flags
                        .contains(flags)
            })
            .ok_or_else(|| Error::new("no suitable memory type"))?;
        let host = flags.contains(vk::MemoryPropertyFlags::HOST_VISIBLE);
        let key = (ty, linear);
        let pool = self.pools.entry(key).or_default();
        let align = req.alignment.max(16);
        for (bi, b) in pool.iter_mut().enumerate() {
            if let Some(off) = take(&mut b.free, req.size, align) {
                return Ok(Mem {
                    memory: b.memory,
                    offset: off,
                    size: req.size,
                    ptr: if b.ptr.is_null() {
                        b.ptr
                    } else {
                        // SAFETY: the block is mapped in full; off + size <= block size.
                        unsafe { b.ptr.add(off as usize) }
                    },
                    key,
                    block: bi,
                });
            }
        }
        let size = req.size.max(BLOCK_SIZE).next_multiple_of(BLOCK_SIZE);
        let info = vk::MemoryAllocateInfo::default()
            .allocation_size(size)
            .memory_type_index(ty);
        // SAFETY: valid device and allocate info.
        let memory = unsafe { dev.allocate_memory(&info, None)? };
        let ptr = if host {
            // SAFETY: memory is host visible and not mapped yet.
            let p =
                unsafe { dev.map_memory(memory, 0, vk::WHOLE_SIZE, vk::MemoryMapFlags::empty())? };
            p as *mut u8
        } else {
            std::ptr::null_mut()
        };
        let mut b = Block {
            memory,
            size,
            ptr,
            free: vec![(0, size)],
        };
        let off = take(&mut b.free, req.size, align).ok_or_else(|| Error::new("alloc"))?;
        pool.push(b);
        Ok(Mem {
            memory,
            offset: off,
            size: req.size,
            ptr: if ptr.is_null() {
                ptr
            } else {
                // SAFETY: inside the freshly mapped block.
                unsafe { ptr.add(off as usize) }
            },
            key,
            block: pool.len() - 1,
        })
    }

    pub(super) fn free(&mut self, m: &Mem) {
        let Some(b) = self.pools.get_mut(&m.key).and_then(|p| p.get_mut(m.block)) else {
            return;
        };
        let i = b.free.partition_point(|r| r.0 < m.offset);
        b.free.insert(i, (m.offset, m.size));
        // merge with next, then previous
        if i + 1 < b.free.len() && b.free[i].0 + b.free[i].1 == b.free[i + 1].0 {
            b.free[i].1 += b.free[i + 1].1;
            b.free.remove(i + 1);
        }
        if i > 0 && b.free[i - 1].0 + b.free[i - 1].1 == b.free[i].0 {
            b.free[i - 1].1 += b.free[i].1;
            b.free.remove(i);
        }
    }

    pub(super) fn destroy(&mut self, dev: &ash::Device) {
        for (_, p) in self.pools.drain() {
            for b in p {
                let _ = b.size;
                // SAFETY: device idle at teardown; memory owned by the block.
                unsafe { dev.free_memory(b.memory, None) };
            }
        }
    }
}

pub(super) fn take(free: &mut Vec<(u64, u64)>, size: u64, align: u64) -> Option<u64> {
    for i in 0..free.len() {
        let (o, s) = free[i];
        let a = o.next_multiple_of(align);
        if a + size <= o + s {
            let tail = (o + s) - (a + size);
            let head = a - o;
            match (head > 0, tail > 0) {
                (false, false) => {
                    free.remove(i);
                }
                (true, false) => free[i] = (o, head),
                (false, true) => free[i] = (a + size, tail),
                (true, true) => {
                    free[i] = (o, head);
                    free.insert(i + 1, (a + size, tail));
                }
            }
            return Some(a);
        }
    }
    None
}
