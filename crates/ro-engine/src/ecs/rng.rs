//! The per-instance random generator (random warp cells, mob placement).

/// SplitMix64: a small deterministic generator, one per map instance.
#[derive(Debug)]
pub(crate) struct SplitMix64(u64);

impl SplitMix64 {
    /// Seeds from the FNV-1a hash of `name`, so every instance of a map rolls the same sequence.
    pub(crate) fn from_name(name: &str) -> Self {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in name.bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        Self(hash)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// A value in `0..n` (`rnd() % n`). `n == 0` gives 0.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        self.next_u64() % n
    }

    /// A value in `a..=b` (rAthena `rnd_value`). An empty range gives `a`.
    pub(crate) fn range_inclusive(&mut self, a: i32, b: i32) -> i32 {
        if b <= a {
            return a;
        }
        let span = u64::from(b.abs_diff(a)) + 1;
        // `below(span) < span <= 2^32`, so the sum stays within `a..=b`.
        a.wrapping_add(self.below(span) as i32)
    }
}

#[cfg(test)]
mod tests;
