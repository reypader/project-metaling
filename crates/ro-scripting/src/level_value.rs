/// A per-level number: `base + per_level × level`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LevelValue {
    pub base: i32,
    pub per_level: i32,
}

impl LevelValue {
    /// The same value at every level.
    pub const fn fixed(value: i32) -> Self {
        Self {
            base: value,
            per_level: 0,
        }
    }

    pub const fn new(base: i32, per_level: i32) -> Self {
        Self { base, per_level }
    }

    /// The value at `level`, saturating.
    pub fn at(self, level: u8) -> i32 {
        self.base
            .saturating_add(self.per_level.saturating_mul(i32::from(level)))
    }
}
