use std::fmt;

/// A script failed to load: a Lua error, or a table of the wrong shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuaLoadError(pub String);

impl fmt::Display for LuaLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for LuaLoadError {}
