//! CPU lightmap requests shared by brush entities and destination views.
//! Lightmap ids are dense indices into `World::lm_surfs`; a bitset gives
//! the same ascending union as sort/dedup without sorting repeated ids.

#[derive(Default)]
pub(crate) struct LightmapRequests {
    words: Vec<u64>,
    ids: Vec<u32>,
    first: usize,
    end: usize,
}

impl LightmapRequests {
    pub(crate) fn begin(&mut self, lightmaps: usize) {
        self.words.resize(lightmaps.div_ceil(64), 0);
        self.words.fill(0);
        self.ids.clear();
        self.first = self.words.len();
        self.end = 0;
    }

    pub(crate) fn push(&mut self, id: u32) {
        let word = id as usize / 64;
        self.words[word] |= 1 << (id % 64);
        self.first = self.first.min(word);
        self.end = self.end.max(word + 1);
    }

    pub(crate) fn sorted(&mut self) -> &[u32] {
        self.ids.clear();
        // Restrict the scan for GPU-culled scenes that only request a few
        // brush lightmaps near the end of the world's lightmap table.
        for i in self.first..self.end {
            let mut bits = self.words[i];
            while bits != 0 {
                self.ids.push((i * 64) as u32 + bits.trailing_zeros());
                bits &= bits - 1;
            }
        }
        &self.ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_sorted_union_across_words_and_repeated_views() {
        let mut requests = LightmapRequests::default();
        requests.begin(194_369);
        let mut old = vec![0, 63, 64, 65, 127, 128, 194_368, 194_368];
        let mut seed = 23u32;
        for _ in 0..350_000 {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            old.push(seed % 194_369);
        }
        for &id in &old { requests.push(id); }
        old.sort_unstable();
        old.dedup();
        assert_eq!(requests.sorted(), old);
        assert_eq!(requests.sorted(), old, "extracting again must not duplicate ids");
    }

    #[test]
    fn new_frames_and_map_sizes_clear_requests_and_reuse_storage() {
        let mut requests = LightmapRequests::default();
        requests.begin(200);
        for id in [0, 64, 199] { requests.push(id); }
        assert_eq!(requests.sorted(), [0, 64, 199]);
        let words = requests.words.as_ptr();
        let ids = requests.ids.as_ptr();
        for size in [200, 65, 0, 200] {
            requests.begin(size);
            assert!(requests.sorted().is_empty(), "stale requests after begin({size})");
            assert_eq!(requests.words.as_ptr(), words);
            assert_eq!(requests.ids.as_ptr(), ids);
            if size > 0 {
                requests.push(size as u32 - 1);
                assert_eq!(requests.sorted(), [size as u32 - 1]);
            }
        }
    }

    #[test]
    fn destination_union_is_not_limited_by_the_gpu_cull_list() {
        let count = crate::MAX_VIS as usize + 65;
        let mut requests = LightmapRequests::default();
        requests.begin(count);
        for id in (0..count as u32).rev() { requests.push(id); }
        let ids = requests.sorted();
        assert_eq!(ids.len(), count);
        assert!(ids.iter().copied().eq(0..count as u32));
    }
}
