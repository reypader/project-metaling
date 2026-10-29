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
│  2. ItemDb / MobDb (YAML) -> registries (tools/item-import, mob-import)      │
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

### 2.2 Pipeline 2: Item and Mob Registries (offline import -> YAML)

Like the spawn lists, the registries are imported offline into our own schema; `ro-db` never parses rAthena's item or mob YAML.

1. **Offline import (dev-only, MIT, clean-room):**
   * `tools/item-import` reads `db/re/item_db_{usable,equip,etc}.yml`, applies rAthena's load rules (`DelayConsume` -> `Usable`, equipment without `Locations` -> `Etc`, field-by-field merge of repeated ids, `AliasName` -> `ViewId`) and keeps only the fields Metaling uses, including the equipment fields with rAthena's type-clearing, level-clamping and forced-gender rules and the `Jobs` / `Classes` bitsets. It writes `ITEM_DATA` v2 to `data/item/item_{usable,equip,etc}.yml`, in id order.
   * `tools/mob-import` reads `db/re/mob_db.yml` and resolves `Drops` / `MvpDrops` AegisNames to item ids through `ItemDb` (loaded from `data/item/`, so run item-import first). It writes `MOB_DATA` v1 to `data/mob/mob_db.yml`.
   * Schemas: `specs/items/spec.md` §1; the v2 equipment keys in `specs/equipment/spec.md` §1.
2. **Runtime (`ItemDb::load_dir`, `MobDb::load_dir`):** the map-server loads both once at startup from `ITEM_DATA_DIR` / `MOB_DATA_DIR` and shares them through `Arc` with every map instance. A missing file in a configured directory is an error; an unset directory gives an empty registry.
3. **Lookups:** `ItemDb` by id, by case-insensitive AegisName (unique) and by case-insensitive Name (lowest id wins); `MobDb` by id. Both use `std` hash maps over an id-sorted `Vec`.
4. **Validation:** duplicate ids, duplicate AegisNames, unknown keys, bad headers; for mobs also the id ranges, at most 10 drops / 3 MVP drops, and rates in `1..=10000`.

`Level`, `Hp` and `WalkSpeed` are imported but not wired into spawned mobs yet (`DEFERRED.md`). Scripts, prices, mob combat stats and `db/import` overlays are not imported.

**Job and refine data** (`specs/stats/spec.md` §1.1–§1.2) follow the same pattern:

* **Job data.** `tools/job-import` merges `db/re/job_{stats,aspd,exp,basepoints}.yml` per job id (later keys override earlier ones). It fills missing HP/SP table slots with the fallback formula, so the file always has dense `BaseHp`/`BaseSp` lists of `MaxBaseLevel` entries. Fallback-filled Super Novice line slots get rAthena's extra HP (+2000 from base 99, another +2000 from 150), which capture 3 confirmed for Hyper Novice. `BaseAp` is filled the same way from rAthena's AP fallback (200 from base 200 for every job, so 3rd classes at B200 get 200 too; catalog `mechanics/trait_stats.md` §2) and omitted when all zero. It writes `JOB_DATA` v2 to `data/job/job_db.yml`, and `statpoint.yml` as `STATPOINT_DATA` v2 with the cumulative `TraitPoints` (omitted when 0).
* **`JobDb::load_dir`** reads both files from `JOB_DATA_DIR`. It accumulates the sparse `BonusStats` into one row per job level, and lays the ASPD bases out as an array indexed by `WeaponType` (unlisted = 200). Lookups are by id and by case-insensitive `Name`. Every per-level accessor clamps its level to the table. `Job::base_ap` reads 0 when the job has no AP table. `JobDb::trait_points` reads 0 below the first level that grants trait points, and the last row past the table (like `status_points`). Validation covers table lengths (an empty AP table is allowed), bonus levels within `MaxJobLevel`, unique ids and names, and status-point rows 1, 2, 3, ...
* **Refine data.** `tools/refine-import` keeps `Bonus`/`RandomBonus` per `(Armor|Weapon, item level)` as lists of 20 (cumulative per refine level, as in rAthena). It writes `REFINE_DATA` v1 to `data/refine/refine.yml`.
* **`RefineDb::bonus(group, level, refine)`** clamps refine to 20 and returns zero for refine 0 or a missing row. It is loaded from `REFINE_DATA_DIR`.

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
// An id lives here while only ro-db and its dependents use it, and moves to ro-core when a crate
// that can't depend on ro-db needs it. ItemId is in ro-core (persistence stores items).
use ro_core::ItemId;

/// mob_db id; valid ranges 1001..=3998 and 20021..=31998. Sent as the unit's `job`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MobId(pub u32);

/// Future: an opaque script handle from ro-scripting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptId(pub u32);

/// Item registry (`ItemDb::load_dir`, `parse_file`, `file_to_yaml`, `from_items`;
/// `get`, `by_aegis_name`, `by_name`).
pub struct ItemData {
    pub id: ItemId,
    pub aegis_name: String,
    pub name: String,
    pub item_type: ItemType,          // is_stackable(), wire_type(locations)
    pub weight: u32,                  // 0.1 units
    pub locations: u32,               // EQP_ mask
    pub view: u32,
    pub view_id: Option<ItemId>,      // AliasName target; client_id() picks it
    pub slots: u16,
    // Equipment (ITEM_DATA v2). Default = omitted in YAML; ItemData::default() for builders.
    pub weapon_type: WeaponType,      // W_* (id()); Fist unless a weapon
    pub ammo_type: Option<AmmoType>,  // AMMO_* (id()); ammo only
    pub attack: u32, pub magic_attack: u32, pub defense: u32,
    pub range: u8,                    // <= 14
    pub weapon_level: u8, pub armor_level: u8,
    pub equip_level_min: u16, pub equip_level_max: u16,   // 0 = no bound
    pub gender: Gender,               // Female | Male | Both (e_sex numbers)
    pub jobs: [u64; 3],               // class_base: first/2-1/2-2 branch bitsets; ALL_JOBS
    pub classes: u8,                  // ITEMJ_* mask; ALL_CLASSES = 0x7f
    pub refineable: bool,
    pub stack_amount: u16, pub stack_inventory: bool,
    pub unique_id: bool, pub bind_on_equip: bool,
    pub drop_effect: u8,              // e_item_drop_effect
    pub no_drop: bool, pub no_drop_override: u16,
}

/// Mob registry (`MobDb::load_dir`, `parse_file`, `file_to_yaml`, `from_mobs`; `get`).
pub struct MobData {
    pub id: MobId,
    pub aegis_name: String, pub name: String,
    pub level: u16, pub hp: u32, pub walk_speed: u16,   // imported, not wired yet
    pub class: MobClass,              // Normal | Boss | Guardian | Battlefield | Event
    pub mvp: bool,                    // boss_type(): 2 MVP, 1 Boss, else 0
    pub drops: Vec<MobDrop>,          // <= 10, db order; MobDrop { item: ItemId, rate: u16 }
    pub mvp_drops: Vec<MobDrop>,      // <= 3
}

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
