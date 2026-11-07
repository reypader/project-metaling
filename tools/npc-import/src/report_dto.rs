//! What an import produced: the converted maps, and a report of what was converted and skipped.

use std::collections::BTreeMap;

use ro_db::{MapMobSpawns, MapNpcs};

/// Counts of what was converted and skipped, plus messages worth a look.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Report {
    pub files: usize,
    /// Converted positioned NPCs by header keyword (`warp`, `warp2`, `script`, `duplicate`, `shop`, ...).
    pub converted: BTreeMap<String, usize>,
    /// Records not converted, by reason.
    pub skipped: BTreeMap<String, usize>,
    /// Problems rAthena would warn about (bad header, unknown sprite, renamed duplicate name, ...).
    pub warnings: Vec<String>,
    /// Files rAthena would stop reading early, with the reason.
    pub file_errors: Vec<String>,
    /// Spawn lines (`monster` / `boss_monster`).
    pub mobs: MobReport,
}

/// What happened to the spawn lines.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MobReport {
    /// Converted lines by keyword (`monster`, `boss_monster`).
    pub converted: BTreeMap<String, usize>,
    /// Lines not converted, by reason.
    pub skipped: BTreeMap<String, usize>,
    /// Sum of `Amount` over converted lines.
    pub total_amount: u64,
    /// The map with the most mobs, and its total.
    pub largest_map: Option<(String, u64)>,
    /// Lines converted with a change (cut name, dropped size/ai, ...) or skipped.
    pub warnings: Vec<String>,
}

/// Everything one import produced, keyed by map name.
#[derive(Debug, Default)]
pub struct Imported {
    pub npcs: BTreeMap<String, MapNpcs>,
    pub mobs: BTreeMap<String, MapMobSpawns>,
    pub report: Report,
}

impl Report {
    pub(crate) fn count(map: &mut BTreeMap<String, usize>, key: &str) {
        *map.entry(key.to_string()).or_default() += 1;
    }
}
