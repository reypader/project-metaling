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
