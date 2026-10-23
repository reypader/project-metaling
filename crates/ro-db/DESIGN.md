# Project Metaling — Static Database & Map Blueprint Design (`crates/ro-db`)

## 1. Architectural Overview & Crate Boundaries

`ro-db` is the static data ingestion, compilation, and map geometry cache layer for **Project Metaling**. It transforms raw file formats from `./env(RATHENA_PATH)/` and binary client map caches into immutable, cache-friendly Rust data structures that `ro-db` owns and exposes (`ro-engine` and `bins/map-server` depend on it).

~~~text
                        ./env(RATHENA_PATH)/ & Map Files
                 ┌──────────────┬───────────────┬──────────────┐
                 ▼              ▼               ▼              ▼
           (.gat / .rsw)   (db/re/*.yml)  (item_db Script) (npc/re/*.txt)
                 │              │               │              │
┌────────────────┴──────────────┴───────────────┴──────────────┴───────────────┐
│ crates/ro-db                                                                 │
│  1. MapCacheLoader      -> Flat 1D CellFlags grid (width * height)           │
│  2. YamlRegistryLoader  -> MobBlueprint, ItemBlueprint, SkillBlueprint       │
│  3. ItemBonusCompiler   -> AOT-compiled `EquipmentModifiers` structs         │
│  4. MapNpcs / MapMobSpawns (YAML) -> Per-map spawn lists (tools/npc-import)  │
└────────────────┬─────────────────────────────────────────────┬───────────────┘
                 │ (MapNpcs, MapMobSpawns, Registries)         │ (Uncompiled `{...}` blocks)
                 ▼                                             ▼
      crates/ro-engine                            crates/ro-scripting
  Spawns Portals, Mobs, & NPCs into               Compiles into Athena VM or Lua
  Tier 1 Dense Arrays & SpatialGrid8x8            and binds to opaque `ScriptId`
~~~

---

## 2. The Four Ingestion Pipelines

### 2.1 Pipeline 1: Map Walkability & Cell Geometry (`.gat` / `.rsw` / `.mcache`)

* **References:** `env(RATHENA_PATH)/src/map/map.cpp` (`map_gat2cell`).
* **Storage Layout:** Each map's terrain grid is stored as a contiguous 1D `Vec<CellFlags>` of length `width as usize * height as usize`, indexed via `y * width + x`.
* **Cell Bitflags (`CellFlags`):**
  * `WALKABLE` (`1 << 0`): Entities can pathfind and step onto this cell.
  * `SHOOTABLE` (`1 << 1`): Ranged attacks and magic Line-of-Sight (LoS) can pass across this cell (e.g., shootable walls/cliffs).
  * `WATER` (`1 << 2`): Water surface cell (required for *Water Ball* / *Aqua Benedicta*).
* **Contract Implementation:** Exposes the concrete `ro_db::MapGrid` (no trait, so the pathfinder's per-cell checks inline) so `ro-engine`'s Stage 3 Movement & A* Pathfinding system can query cell walkability in $O(1)$ time with zero lock contention. Each `MapInstance` holds an `Arc<MapGrid>`; runtime cell changes (`setcell`, Ice Wall) copy it on write.

### 2.2 Pipeline 2: YAML Static Registries (`db/re/` & `db/import/`)

* **References:** `env(RATHENA_PATH)/db/re/mob_db.yml`, `item_db.yml` (`item_db_usable.yml`, `item_db_equip.yml`, `item_db_etc.yml`), `skill_db.yml`, and `job_stats.yml`.
* **32-Bit ID Invariant:** Per our modern `TARGET_PACKETVER` (`20260219`), all item and monster identifiers deserialize into 32-bit newtypes (`ItemId(pub u32)`, `MobId(pub u32)`).
* **Layered Import Merging:** Following rAthena's YAML specification, `ro-db` loads `db/re/<file>.yml` first and overlays entries from `db/import/<file>.yml` by primary key (`Id`), supporting partial field overrides and `Clear: true` resets.
* **Lookup Performance:** Once loaded, registries are frozen into immutable `FxHashMap<ItemId, ItemBlueprint>` and `FxHashMap<MobId, MobBlueprint>` tables wrapped in `Arc<StaticDb>` for lock-free sharing across all `MapWorld` worker threads.

### 2.3 Pipeline 3: Ahead-of-Time (AOT) Item Bonus Compiler

In legacy rAthena, every equipped item's `Script` block (e.g., `bonus bStr,1; bonus bMaxHP,100;`) is compiled into Athena Script bytecode and executed inside the script VM every time `status_calc_pc()` recalculates player stats.

To keep `ro-engine`'s **Stage 1 (Present Status Establishment)** 100% pure and parallelizable (`par_iter` over dirty entities):

1. **Static Modifier Extraction:** When `ro-db` parses `item_db_equip.yml`, a lightweight combinator parser (`winnow` / `nom`) inspects the `Script:` string.
2. **Fast-Path Compilation (`EquipmentModifiers`):** Standard declarative stat statements (`bonus bStr, N;`, `bonus bAtkEle, Ele_Fire;`, `bonus2 bSubEle, Ele, Rate;`, `bonus2 bAddRace, Race, Rate;`) are compiled directly into the item's static `EquipmentModifiers` struct. When a player equips the item in `ro-engine`, Stage 1 folds `EquipmentModifiers` via pure SIMD-friendly addition without touching `ro-scripting`.
3. **Dynamic Fallback (`Option<ScriptId>`):** Only items containing imperative control flow (`if`, `callfunc`) or reactive combat procs (`autobonus`, `skill`) emit an uncompiled script snippet to `ro-scripting` and store a fallback `ScriptId` hook (`OnEquip`, `OnUnequip`, `OnAttack`, `OnWhenHit`).

### 2.4 Pipeline 4: NPC and Mob Spawn Lists (offline import -> per-map YAML)

rAthena defines warps, NPCs, shops, mob spawns and mapflags in tab-delimited `.txt` files reached from `npc/re/scripts_main.conf`. `ro-db` never parses that format at runtime:

1. **Offline import (`tools/npc-import`, dev-only, MIT, clean-room):** resolves the manifest, skips `{...}` script bodies, and parses top-level records in one pass:
   * positioned `warp`/`warp2`/`script`/`duplicate(...)`/shop-family headers, with sprite names and duplicates resolved, into one `NPC_SPAWN_DB` file per map. Grammar: `reference/catalog/db/npc_files.md`; spec: `specs/npc-spawning/spec.md`.
   * `monster`/`boss_monster` spawn lines, with `AegisName` refs and `--en--`/`--ja--` names resolved against mob_db, into one `MOB_SPAWN_DB` file per map. Line format: `reference/catalog/mechanics/mobs.md` §1.1, §1.6; spec: `specs/mob-spawning/spec.md`.
2. **Committed data (`crates/ro-db/data/{npc,mob_spawn}/<map>.yml`):** GPL-3.0, a modified copy of rAthena data. `crates/ro-db/data/LICENSE` and `NOTICE.md` cover every dataset under `data/` (one section each). Excluded from crate packaging.
3. **Runtime (`MapNpcs::load_yaml`, `MapMobSpawns::load_yaml`):** serde (`serde-saphyr`) into `NpcSpawn`s / `MobSpawn`s when a map world starts. A missing file means the map has none. Both datasets share one private header-check / file-reading helper (`yaml.rs`).

Script bodies are not imported; each entry records `Source: <file>:<line>` for the scripting phase. A mob spawn keeps the line's own values (center, raw span, amount, delays, level, boss keyword, event label); the rules that need map cells or battle config (spawn rectangle, map-wide forcing, respawn delay adjustments) run in `ro-engine`. Mapflags, floating NPCs and shop catalogs are future work (same importer, new YAML types).

---

## 3. Public Data Types (owned by `ro-db`)

`ro-db` exposes its parsed data to `ro-engine` and `bins/map-server` using these types, defined in `ro-db`:

~~~rust
// ItemId and ScriptId don't exist yet. Like MobId, an id lives here while only ro-db and its
// dependents use it; it moves to ro-core only when a crate that can't depend on ro-db needs it.
use ro_core::{ItemId, MapPosition};

/// mob_db id; valid ranges 1001..=3998 and 20021..=31998. Sent as the unit's `job`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MobId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptId(pub u32);

/// One map's NPCs (`MapNpcs::load_yaml`, `from_yaml_str`, `to_yaml_string`).
pub struct MapNpcs { pub npcs: Vec<NpcSpawn> }

pub struct NpcSpawn {
    pub name: String,                 // rAthena nd->name (keeps any `#suffix`)
    pub unique_name: String,          // exname
    pub x: u16, pub y: u16,
    pub dir: u8,                      // raw, not validated
    pub sprite: i16,                  // -1 / 32767 never sent
    pub sprite_is_mob: bool,          // object_type 0x0C
    pub state: NpcState,              // Normal | Hidden | Cloaked | Disabled
    pub trigger: Option<(u16, u16)>,  // half-extents
    pub duplicate_of: Option<String>,
    pub kind: NpcKind,                // Script | Shop | Warp(WarpDest { map, x, y, trigger_while_hidden })
    pub source: String,               // "<rAthena path>:<line>"
}

/// One map's permanent spawn lines, in rAthena load order
/// (`MapMobSpawns::load_yaml`, `from_yaml_str`, `to_yaml_string`).
pub struct MapMobSpawns { pub spawns: Vec<MobSpawn> }

pub struct MobSpawn {
    pub name: String,                 // <= 23 chars; --en--/--ja-- resolved
    pub mob_id: MobId,
    pub x: u16, pub y: u16,           // (0, 0) = map-wide
    pub span: (u16, u16),             // raw (xs, ys), before the runtime rectangle rules
    pub amount: u16,                  // 1..=1000
    pub delay1: u32,                  // ms
    pub delay2: u32,                  // ms
    pub level: Option<u16>,           // None = mob_db level
    pub boss: bool,                   // boss_monster keyword, not mob_db Class: Boss
    pub event: Option<String>,        // "npc::OnLabel" run on death
    pub source: String,               // "<rAthena path>:<line>"
}

/// Exists today as the concrete `ro_db::MapGrid`, one per map (shared via `Arc` across instances).
/// Methods marked `future` are added as inherent methods; don't introduce a walkability trait.
impl MapGrid {
    pub fn width(&self) -> u16;
    pub fn height(&self) -> u16;
    pub fn is_walkable(&self, x: u16, y: u16) -> bool;
    pub fn cell_type(&self, x: u16, y: u16) -> Option<CellType>;
    pub fn set(&mut self, x: u16, y: u16, cell: CellType) -> bool;
    pub fn is_shootable(&self, x: u16, y: u16) -> bool; // future
    pub fn random_walkable_cell(&self) -> Option<(u16, u16)>; // future
}
~~~
