//! Script records and the error that stops a file.

use std::fmt;

/// One header line split on tabs. `w4` holds the rest of the line, further tabs included.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// 1-based line of the header.
    pub line: usize,
    pub w1: String,
    pub w2: String,
    pub w3: String,
    pub w4: String,
}

/// A problem that makes rAthena abandon the rest of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexError {
    Bom,
    TooFewFields { line: usize },
    MissingBody { line: usize },
    UnterminatedBody { line: usize },
    NewlineInString { line: usize },
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bom => write!(f, "UTF-8 BOM at start of file"),
            Self::TooFewFields { line } => write!(f, "line {line}: fewer than 3 tab fields"),
            Self::MissingBody { line } => write!(f, "line {line}: script without ',{{'"),
            Self::UnterminatedBody { line } => write!(f, "line {line}: unterminated script body"),
            Self::NewlineInString { line } => write!(f, "line {line}: newline inside a string"),
        }
    }
}
