//! Generational entity handles.

/// A slot index plus the generation it was allocated with. A handle is stale once its slot
/// is freed, even if the slot is later reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct Entity {
    pub(super) index: u32,
    pub(super) generation: u32,
}

impl Entity {
    pub(crate) const fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    /// Index into the dense columns.
    pub(crate) const fn index(self) -> usize {
        self.index as usize
    }

    #[cfg(test)]
    pub(crate) const fn generation(self) -> u32 {
        self.generation
    }
}
