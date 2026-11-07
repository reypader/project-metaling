//! What an import produces: the converted mobs and a report.

use ro_db::MobData;

/// The converted mobs, sorted by id.
#[derive(Debug, Default)]
pub struct Converted {
    pub mobs: Vec<MobData>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Entries read.
    pub read: usize,
    /// Entries that merged into an earlier entry with the same id.
    pub merged: usize,
    pub warnings: Vec<String>,
}
