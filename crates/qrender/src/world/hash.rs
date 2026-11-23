/// Multiplicative hash for integer points (the default SipHash showed up in
/// profiles; these keys need no DoS resistance).
#[derive(Default, Clone, Copy)]
pub struct PointHash;

impl std::hash::BuildHasher for PointHash {
    type Hasher = PointHasher;
    fn build_hasher(&self) -> PointHasher {
        PointHasher(0)
    }
}

pub struct PointHasher(u64);

impl std::hash::Hasher for PointHasher {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u32(b as u32);
        }
    }
    fn write_u32(&mut self, v: u32) {
        self.0 = (self.0.rotate_left(5) ^ v as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
    fn write_i32(&mut self, v: i32) {
        self.write_u32(v as u32);
    }
    fn write_usize(&mut self, v: usize) {
        self.write_u32(v as u32);
    }
}

/// Result of a light trace (ironwail's lightcache_t.surfidx/ds/dt).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum LightHit {
    Miss,
    Black,
    Surf { surf: u32, ds: i32, dt: i32 },
}

/// Cached light trace plus the last evaluated colour/direction, surface style
/// slots (stored inline so hits avoid cold `surfaces[]` reads), and the style
/// generation it was last validated against.
#[derive(Clone, Copy, Debug)]
pub(super) struct LightCacheEntry {
    pub(super) hit: LightHit,
    pub(super) styles: [u8; 4],
    pub(super) validated_gen: u32,
    pub(super) style_vals: [i32; 4],
    pub(super) rgb: [f32; 3],
    pub(super) dir: Option<[f32; 3]>,
}
