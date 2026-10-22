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
│  4. NpcHeaderParser     -> Per-map `MapSpawnBlueprint` + raw script bodies   │
└────────────────┬─────────────────────────────────────────────┬───────────────┘
                 │ (MapSpawnBlueprint, Registries)             │ (Uncompiled `{...}` blocks)
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
* **Contract Implementation:** Defines and implements the walkability trait (today `ro_db::MapWalkability`) so `ro-engine`'s Stage 3 Movement & A* Pathfinding system can query cell walkability in $O(1)$ time with zero lock contention.

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

### 2.4 Pipeline 4: Tab-Delimited Map Entity & Spawn Parser (`env(RATHENA_PATH)/npc/re/`)

In rAthena, warp portals, monster spawns, shops, mapflags, and interactive NPCs are all defined inside tab-delimited `.txt` files organized under `env(RATHENA_PATH)/npc/re/scripts_main.conf`.

`ro-db` decouples **where entities exist on a map** from **how scripts execute**:

1. **Recursive Manifest Resolver:** Starts at `npc/re/scripts_main.conf` and follows `import:` directives (`scripts_warps.conf`, `scripts_monsters.conf`, `scripts_athena.conf`, `scripts_jobs.conf`).
2. **Strict 4-Column Tab Splitter:** Splits top-level definition lines strictly by literal ASCII Tab (`\t`) characters into 4 header columns and categorizes them into a per-map `MapSpawnBlueprint`:

| Column 2 Keyword | Header Format (`\t`-delimited) | Resulting `ro-db` Blueprint Variant |
| :--- | :--- | :--- |
| **`warp`** | `<map>,<x>,<y>,0\twarp\t<name>\t<xs>,<ys>,<dest_map>,<dx>,<dy>` | `WarpBlueprint`: Pure spatial trigger box `(xs, ys)` and destination `(dest_map, dx, dy)`. Zero scripting required. |
| **`monster`** / **`boss_monster`** | `<map>,<x>,<y>\tmonster\t<name>\t<mob_id>,<amount>,<delay1>,<delay2>,"<event>"` | `MobSpawnBlueprint`: Spawns `amount` entities of `MobId(u32)` at `(x, y)` (or random walkable cell if `0,0`) with respawn timers. |
| **`shop`** / **`cashshop`** | `<map>,<x>,<y>,<dir>\tshop\t<name>\t<sprite>,<item_id>:<price>,...` | `ShopBlueprint`: Spawns an NPC entity with a static buy/sell catalog (`Vec<(ItemId, Option<u32>)>`). Zero scripting required. |
| **`mapflag`** | `<map>\tmapflag\t<flag_name>\t<opt_param>` | Mutates `MapFlags` bitmask on the map's `MapSpawnBlueprint` (`NO_TELEPORT`, `NO_MEMO`, `PVP`, `INDOORS`). |
| **`script`** | `<map>,<x>,<y>,<dir>\tscript\t<name>\t<sprite>,{<xs>,<ys>,}{ ... }` | `NpcBlueprint` + `RawScriptRegistration`: Extracts coordinates, sprite ID, and optional `OnTouch` `(xs, ys)` area for `MapWorld`, and forwards the `{ ... }` source slice to `ro-scripting`. |
| **`duplicate(<src>)`** | `<map>,<x>,<y>,<dir>\tduplicate(<src>)\t<name>\t<sprite>` | `NpcBlueprint`: Clones the `ScriptId` handle of `<src>` at the new map coordinates and direction. |

---

## 3. Public Data Types (owned by `ro-db`)

`ro-db` exposes its parsed data to `ro-engine` and `bins/map-server` using these types, defined in `ro-db`:

~~~rust
// ItemId, MobId and ScriptId don't exist yet; add them to ro-core when first used.
use ro_core::{ItemId, MapPosition};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MobId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScriptId(pub u32);

/// All static entities and rules to initialize when spinning up a `MapWorld`.
#[derive(Debug, Clone, Default)]
pub struct MapSpawnBlueprint {
    pub map_name: String,
    pub flags: MapFlags,
    pub warps: Vec<WarpBlueprint>,
    pub mobs: Vec<MobSpawnBlueprint>,
    pub shops: Vec<ShopBlueprint>,
    pub npcs: Vec<NpcBlueprint>,
}

#[derive(Debug, Clone)]
pub struct WarpBlueprint {
    pub name: String,
    pub pos: MapPosition,
    pub trigger_half_extents: (u8, u8), // (xs, ys) -> (2*xs + 1) x (2*ys + 1) area
    pub dest_map: String,
    pub dest_pos: (u16, u16),
}

#[derive(Debug, Clone)]
pub struct MobSpawnBlueprint {
    pub display_name: String,
    pub mob_id: MobId,
    pub spawn_pos: Option<(u16, u16)>,  // None if (0, 0) -> random walkable cell
    pub spawn_rect: Option<(u16, u16)>, // Optional bounding box (xs, ys)
    pub count: u16,
    pub respawn_delay_ms: u32,
    pub variance_delay_ms: u32,
    pub is_boss: bool,
    pub on_death_event: Option<String>, // e.g., "NpcName::OnMyMobDead"
}

#[derive(Debug, Clone)]
pub struct ShopBlueprint {
    pub unique_name: String,
    pub pos: MapPosition,
    pub sprite_id: i16,
    pub catalog: Vec<(ItemId, Option<u32>)>, // None = use default ItemBlueprint BuyPrice
}

#[derive(Debug, Clone)]
pub struct NpcBlueprint {
    pub unique_name: String,
    pub display_name: String,
    pub pos: MapPosition,
    pub sprite_id: i16,                 // -1 for invisible/floating controller
    pub touch_half_extents: Option<(u8, u8)>,
    pub script_id: ScriptId,
}

/// Exists today as `ro_db::MapWalkability`, one instance per map. Methods marked `future` extend
/// it; don't add a second walkability trait.
pub trait MapWalkability: Send + Sync {
    fn width(&self) -> u16;
    fn height(&self) -> u16;
    fn is_walkable(&self, x: u16, y: u16) -> bool;
    fn cell_type(&self, x: u16, y: u16) -> Option<CellType>;
    fn is_shootable(&self, x: u16, y: u16) -> bool; // future
    fn random_walkable_cell(&self) -> Option<(u16, u16)>; // future
}
~~~
