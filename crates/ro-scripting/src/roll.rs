/// The engine's seeded random source.
pub trait Roll {
    /// rAthena's `rnd() % n`: a value in `0..n`, or 0 when `n` is 0.
    fn roll(&mut self, n: u32) -> u32;
}
