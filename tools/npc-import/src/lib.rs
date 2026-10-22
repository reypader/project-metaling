//! Offline converter from rAthena NPC source files to per-map `NPC_SPAWN_DB` YAML
//! (`crates/ro-db/data/npc/<map>.yml`). Dev-only; nothing in `crates/` or `bins/` uses it.
//!
//! Written from `reference/catalog/db/npc_files.md` (clean-room), not ported from rAthena.

mod convert;
mod header;
mod lex;
mod manifest;
mod sprite;

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::Context;
use ro_db::MapNpcs;

pub use convert::{Importer, Report};
pub use lex::{records, LexError, Record};
pub use manifest::{resolve as resolve_manifest, Manifest};
pub use sprite::SpriteTable;

/// Renewal manifest entry point, relative to the rAthena root.
pub const MANIFEST_ENTRY: &str = "npc/re/scripts_main.conf";

/// Loads the sprite tables, resolves the manifest and converts every listed file.
pub fn import(rathena: &Path) -> anyhow::Result<(BTreeMap<String, MapNpcs>, Report)> {
    let npc_hpp = read_text(&rathena.join("src/map/npc.hpp"))?;
    let mut mob_dbs = vec![read_text(&rathena.join("db/re/mob_db.yml"))?];
    let import_db = rathena.join("db/import/mob_db.yml");
    if import_db.is_file() {
        mob_dbs.push(read_text(&import_db)?);
    }
    let mob_refs: Vec<&str> = mob_dbs.iter().map(String::as_str).collect();
    let sprites = SpriteTable::from_sources(&npc_hpp, &mob_refs).context("sprite tables")?;

    let manifest = resolve_manifest(rathena, MANIFEST_ENTRY);
    let mut importer = Importer::new(sprites);
    for file in &manifest.files {
        let bytes = std::fs::read(rathena.join(file)).with_context(|| file.clone())?;
        importer.add_file(file, &bytes);
    }
    let (maps, mut report) = importer.finish();
    report.warnings.splice(0..0, manifest.warnings);
    Ok((maps, report))
}

/// One map's file: license/attribution comment, then the YAML.
pub fn render_map_file(npcs: &MapNpcs, rathena_commit: &str) -> anyhow::Result<String> {
    let yaml = npcs.to_yaml_string()?;
    Ok(format!(
        "# SPDX-License-Identifier: GPL-3.0-only\n\
         # Derived from rAthena (https://github.com/rathena/rathena) commit {rathena_commit}, files under npc/.\n\
         # Modified: converted from tab-delimited .txt to YAML by tools/npc-import. Do not edit by hand.\n\
         # See crates/ro-db/data/LICENSE and crates/ro-db/data/NOTICE.md.\n{yaml}"
    ))
}

/// Replaces every `*.yml` in `out_dir` with one file per map; other files are kept. Returns the
/// number of files written.
pub fn write_output(
    out_dir: &Path,
    maps: &BTreeMap<String, MapNpcs>,
    rathena_commit: &str,
) -> anyhow::Result<usize> {
    std::fs::create_dir_all(out_dir).with_context(|| out_dir.display().to_string())?;
    for entry in std::fs::read_dir(out_dir)? {
        let path = entry?.path();
        if path.extension().and_then(|e| e.to_str()) == Some("yml") {
            std::fs::remove_file(&path).with_context(|| path.display().to_string())?;
        }
    }
    for (map, npcs) in maps {
        let path = out_dir.join(format!("{map}.yml"));
        std::fs::write(&path, render_map_file(npcs, rathena_commit)?)
            .with_context(|| path.display().to_string())?;
    }
    Ok(maps.len())
}

fn read_text(path: &Path) -> anyhow::Result<String> {
    let bytes = std::fs::read(path).with_context(|| path.display().to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}
