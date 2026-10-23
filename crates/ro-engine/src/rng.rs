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
mod tests {
    use super::*;

    #[test]
    fn splitmix_is_deterministic_per_name() {
        let mut a = SplitMix64::from_name("prontera.gat");
        let mut b = SplitMix64::from_name("prontera.gat");
        let mut c = SplitMix64::from_name("geffen.gat");
        let seq_a: Vec<u64> = (0..4).map(|_| a.next_u64()).collect();
        let seq_b: Vec<u64> = (0..4).map(|_| b.next_u64()).collect();
        assert_eq!(seq_a, seq_b);
        assert_ne!(seq_a[0], c.next_u64());
    }

    #[test]
    fn range_inclusive_hits_both_ends_and_nothing_outside() {
        let mut rng = SplitMix64::from_name("t");
        let mut seen = [false; 5];
        for _ in 0..1000 {
            let v = rng.range_inclusive(-2, 2);
            assert!((-2..=2).contains(&v), "{v}");
            seen[(v + 2) as usize] = true;
        }
        assert!(seen.iter().all(|&s| s), "{seen:?}");
        assert_eq!(rng.range_inclusive(7, 7), 7);
        assert_eq!(rng.range_inclusive(7, 3), 7, "empty range gives a");
        let wide = rng.range_inclusive(i32::MIN, i32::MAX);
        assert!(
            (i32::MIN..=i32::MAX).contains(&wide),
            "full range does not overflow"
        );
        assert_eq!(rng.below(0), 0);
        assert!(rng.below(3) < 3);
    }
}
