use std::ops::Deref;

use smallvec::SmallVec;

use crate::Effect;

/// The caller-owned output of a resolve. Reused between casts, so the hot path doesn't
/// allocate for short lists.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EffectList(SmallVec<[Effect; 8]>);

impl EffectList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, effect: Effect) {
        self.0.push(effect);
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    /// Keeps the first `len` effects.
    pub fn truncate(&mut self, len: usize) {
        self.0.truncate(len);
    }
}

impl Deref for EffectList {
    type Target = [Effect];

    fn deref(&self) -> &[Effect] {
        &self.0
    }
}
