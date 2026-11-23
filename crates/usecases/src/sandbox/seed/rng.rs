//! A tiny seeded generator (SplitMix64): the same seed gives the same numbers
//! on every platform and every build, with no system randomness involved.

pub struct SeedRng(u64);

impl SeedRng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    /// A number in `0..bound` (`0` when `bound` is `0`).
    pub fn below(&mut self, bound: u64) -> u64 {
        if bound == 0 {
            return 0;
        }
        self.next_u64() % bound
    }

    /// An amount in cents between `low` and `high`, rounded to `step` cents.
    pub fn cents_between(&mut self, low: i64, high: i64, step: i64) -> i64 {
        let span = u64::try_from((high - low).max(0)).unwrap_or(0);
        let raw = low + i64::try_from(self.below(span + 1)).unwrap_or(0);
        ((raw + step / 2) / step * step).clamp(low, high.max(low))
    }
}
