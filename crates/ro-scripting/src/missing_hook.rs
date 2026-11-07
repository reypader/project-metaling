use std::fmt;

/// A definition names a hook its backend doesn't have.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingHook(pub &'static str);

impl fmt::Display for MissingHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the hook backend has no hook named {}", self.0)
    }
}

impl std::error::Error for MissingHook {}
