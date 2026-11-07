//! Fixtures and helpers the test modules share.

use std::collections::BTreeMap;
use std::path::PathBuf;

use npc_import::{Imported, Importer, SpriteTable};
use ro_db::{MapNpcs, MobSpawn, NpcSpawn};

pub const NPC_HPP: &str = "enum e_job_types\n{\n\tNPC_RANGE1_START = 44,\n\tJT_WARPNPC,\n\tJT_1_ETC_01,\n\tJT_4_F_KAFRA1 = 117,\n\tJT_HIDDEN_WARP_NPC = 139,\n\tJT_INVISIBLE = 32767,\n\tJT_FAKENPC = -1\n};\n";
pub const MOB_DB: &str = concat!(
    "Header:\n  Type: MOB_DB\n  Version: 5\nBody:\n",
    "  - Id: 1002\n    AegisName: PORING\n    Name: Poring\n",
    "    RaceGroups:\n      A: true\n      A: true\n",
    "  - Id: 1063\n    AegisName: LUNATIC\n    Name: Lunatic\n    JapaneseName: Lunatic JP\n",
    "  - Id: 20255\n    AegisName: SPIRIT_TEST\n    Name: L Blue Earth Spirit\n",
    "  - Id: 5000\n    AegisName: OUT_OF_RANGE\n    Name: Custom\n",
);

pub fn importer() -> Importer {
    Importer::new(SpriteTable::from_sources(NPC_HPP, &[MOB_DB]).expect("tables"))
}

/// Converts one file and returns everything it produced.
pub fn import_one(text: &[u8]) -> Imported {
    let mut imp = importer();
    imp.add_file("npc/test.txt", text);
    imp.finish()
}

/// Converts one file and returns the NPC maps plus the report.
pub fn convert(text: &[u8]) -> (BTreeMap<String, MapNpcs>, npc_import::Report) {
    let out = import_one(text);
    (out.npcs, out.report)
}

pub fn only(maps: &BTreeMap<String, MapNpcs>, map: &str) -> Vec<NpcSpawn> {
    maps.get(map).map(|m| m.npcs.clone()).unwrap_or_default()
}

// ---------- TOOL-NPC-01: manifest ----------

pub struct TempDir(pub PathBuf);

impl TempDir {
    pub fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("npc-import-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        Self(dir)
    }

    pub fn write(&self, rel: &str, text: &str) {
        let path = self.0.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dirs");
        std::fs::write(path, text).expect("write");
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

// ---------- TOOL-MOB-01..05: mob spawn lines ----------

pub fn mobs_of(out: &Imported, map: &str) -> Vec<MobSpawn> {
    out.mobs
        .get(map)
        .map(|m| m.spawns.clone())
        .unwrap_or_default()
}
