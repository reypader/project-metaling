//! Map collision, walkability abstractions, and cell types.

use serde::{Deserialize, Serialize};

/// Ragnarok Online .gat cell classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CellType {
    /// Walkable cell (0).
    Walkable,
    /// Solid wall (1), non-walkable, blocks line of sight.
    Wall,
    /// Walkable water (2).
    Water,
    /// Walkable water variant (3).
    Water2,
    /// Walkable cell variant (4).
    Walkable4,
    /// Non-walkable cliff or gap (5), passable for line of sight.
    Cliff,
    /// Walkable cell variant (6).
    Walkable6,
    /// Other unclassified cell types.
    Other(u8),
}

impl CellType {
    pub fn from_raw(val: u8) -> Self {
        match val {
            0 => Self::Walkable,
            1 => Self::Wall,
            2 => Self::Water,
            3 => Self::Water2,
            4 => Self::Walkable4,
            5 => Self::Cliff,
            6 => Self::Walkable6,
            other => Self::Other(other),
        }
    }

    pub fn is_walkable(self) -> bool {
        !matches!(self, Self::Wall | Self::Cliff)
    }
}

/// Abstract contract for reading map cell walkability without exposing internal grid storage.
pub trait MapWalkability: Send + Sync {
    /// Width of the map in cells.
    fn width(&self) -> u16;
    /// Height of the map in cells.
    fn height(&self) -> u16;
    /// Returns true if the coordinate is within boundaries and walkable.
    fn is_walkable(&self, x: u16, y: u16) -> bool;
    /// Returns the cell type at (x, y) if within bounds.
    fn cell_type(&self, x: u16, y: u16) -> Option<CellType>;
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

