//! The result of a successful inventory add.

/// A successful add: where it went and how much.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Added {
    pub(crate) slot: usize,
    pub(crate) amount: u16,
}
