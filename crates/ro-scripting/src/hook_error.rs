use std::fmt;

use crate::{CastSource, HookId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookError {
    /// The backend has no hook with this id.
    UnknownHook(HookId),
    /// The call's source isn't implemented yet.
    Unsupported(CastSource),
    /// The hook rejected its input.
    Failed(String),
}

impl fmt::Display for HookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownHook(id) => write!(f, "no hook with id {}", id.0),
            Self::Unsupported(source) => write!(f, "cast source {source:?} is not supported"),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HookError {}
