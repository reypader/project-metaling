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
