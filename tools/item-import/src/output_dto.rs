//! What an import produces: the converted items per source file, and a report.

use ro_db::ItemData;

/// The converted items, one list per entry of `SOURCES`, each sorted by id.
#[derive(Debug, Default)]
pub struct Converted {
    pub files: Vec<Vec<ItemData>>,
    pub report: Report,
}

#[derive(Debug, Default)]
pub struct Report {
    /// Entries read, per source.
    pub read: Vec<usize>,
    /// Entries that merged into an earlier entry with the same id.
    pub merged: usize,
    /// Consumables whose `Script` became a `Use` (a single `itemheal` or `percentheal`).
    pub lifted_item_heal: usize,
    pub lifted_percent_heal: usize,
    /// Consumables with a `Script` that was not lifted.
    pub not_lifted: usize,
    pub warnings: Vec<String>,
}
