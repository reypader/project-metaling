//! `.gat` cell types and static-data load errors.

use serde::{Deserialize, Serialize};

/// Ragnarok Online `.gat` cell classification (rAthena raw values).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellType {
    /// Walkable ground (0).
    Walkable,
    /// Wall (1): not walkable, blocks line of sight.
    Wall,
    /// Walkable variant (2).
    Walkable2,
    /// Water (3): walkable.
    Water,
    /// Walkable variant (4).
    Walkable4,
    /// Cliff or gap (5): not walkable, does not block line of sight.
    Cliff,
    /// Walkable variant (6).
    Walkable6,
    /// Any other raw value. Not walkable.
    Unknown(u8),
}

impl CellType {
    #[inline]
    pub fn from_raw(val: u8) -> Self {
        match val {
            0 => Self::Walkable,
            1 => Self::Wall,
            2 => Self::Walkable2,
            3 => Self::Water,
            4 => Self::Walkable4,
            5 => Self::Cliff,
            6 => Self::Walkable6,
            other => Self::Unknown(other),
        }
    }

    /// Walkable: 0, 2, 3, 4, 6. Not walkable: 1, 5, and unknown values.
    #[inline]
    pub fn is_walkable(self) -> bool {
        matches!(
            self,
            Self::Walkable | Self::Walkable2 | Self::Water | Self::Walkable4 | Self::Walkable6
        )
    }
}

/// Errors encountered when loading map walkability and cell data.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum MapDataError {
    #[error("I/O error reading map '{path}': {message}")]
    Io { path: String, message: String },

    #[error("Bad magic header in map file '{path}'")]
    BadMagic { path: String },

    #[error("Truncated map file '{path}': expected {expected} bytes, got {actual}")]
    Truncated {
        path: String,
        expected: usize,
        actual: usize,
    },

    #[error("Invalid dimensions {width}x{height} in map file '{path}'")]
    InvalidDimensions {
        path: String,
        width: u32,
        height: u32,
    },

    #[error("Map not found: {0}")]
    MapNotFound(String),
}

/// Errors encountered when loading static Renewal YAML databases.
#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum DbLoadError {
    #[error("I/O error reading database '{path}': {message}")]
    Io { path: String, message: String },

    #[error("YAML parse error in '{path}' at line {line:?}: {message}")]
    Parse {
        path: String,
        line: Option<usize>,
        message: String,
    },

    #[error("Schema validation failed for '{path}': {message}")]
    Schema { path: String, message: String },

    #[error("Duplicate ID {id} found in database '{path}'")]
    DuplicateId { path: String, id: u32 },
}
