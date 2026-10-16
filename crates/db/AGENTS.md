# AGENTS.md: `crates/db` Subagent (Static Data & Map Cache)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/db/**`, `./crates/contracts/**` (including `./crates/contracts/src/**`), and `./reference/**`.
* **Direct Contract Inspection (No `cargo doc`):** Read the source files in `./crates/contracts/src/` directly via file viewing tools. Avoid running `cargo doc` or background documentation builds to inspect contracts, because reading files directly eliminates compilation overhead.
* **Write Scope:** `./crates/db/src/**` and `./crates/db/Cargo.toml` ONLY.
* **Prohibition:** NEVER write to `./crates/contracts/**`, `./bins/*`, or another domain crate (`./crates/<other_domain>/**`). NEVER read implementation files in other domain crates (`./crates/<other_domain>/src/**`). If you need a new shared type or trait in `crates/contracts`, STOP and request a Contract Amendment from the Main Agent.

---

## 2. Domain Architecture & Static Data Mechanics

`crates/db` (`ro-db`) loads, validates, and indexes immutable game world definitions at server startup: binary `.gat`/`.rsw` map cell walkability grids, Renewal YAML databases (`mob_db.yml`, `item_db.yml`, `skill_db.yml`), and NPC monster/warp spawn definitions.

### 2.1 Map Collision & `.gat` Walkability Grids
1. **Cell Classification (`CellType`):**
   * Implements the `MapWalkability` trait from `crates/contracts/src/map.rs`.
   * Maps raw `.gat` cell type bytes (`0..=6`) to `CellType`:
     * `0` (`Walkable`), `2` (`Water`), `3` (`Water2`), `4` (`Walkable4`), `6` (`Walkable6`) -> walkable (`is_walkable() == true`).
     * `1` (`Wall`, blocks movement and line of sight) and `5` (`Cliff`, blocks movement, allows line of sight) -> non-walkable (`is_walkable() == false`).
2. **Contiguous Row-Major Storage:**
   * Store map cells in a flat `Vec<CellType>` of length `width as usize * height as usize` indexed by `(y as usize) * (width as usize) + (x as usize)`.
   * Bounds-check all `(x, y)` queries (`x < width && y < height`) so out-of-bounds coordinates return `false` for `is_walkable` and `None` for `cell_type` without panicking.
   * Reference `reference/rathena/src/map/map.cpp` and `reference/rust-ro/tools/map-cache/` for binary header layout and water-height overrides.

### 2.2 YAML Database Deserialization (`serde_yaml`)
1. **rAthena Renewal Schema (`reference/rathena/db/re/`):**
   * rAthena YAML files contain a `Header` (`Type`, `Version`) and a `Body` sequence of entries, often split across multiple files imported via `Footer.Imports`.
   * Apply `reference/rathena/db/re/` first, then overlay `reference/rathena/db/import/` entries by ID.
   * All `ItemId` values must deserialize into 32-bit `u32` (`ItemId(u32)` in `crates/contracts`).
2. **Monster & Warp Portal Spawns (`reference/rathena/npc/re/`):**
   * Parse warp definitions (`<src_map>,<src_x>,<src_y>,<facing>\twarp\t<name>\t<span_x>,<span_y>,<dst_map>,<dst_x>,<dst_y>`) from `reference/rathena/npc/re/warps/`.
   * Parse monster spawn definitions (`<map>,<x>,<y>,<xs>,<ys>\tmonster\t<name>\t<mob_id>,<amount>,<delay1>,<delay2>`) from `reference/rathena/npc/re/mobs/`.

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Upstream Initializer:** `bins/map-server` loads `ro-db` registries at process startup and passes `Arc<dyn MapWalkability>` (and static blueprint registries) into `ro-engine`.
* **Downstream Consumer:** `crates/engine` queries `MapWalkability` during A* pathfinding and reads mob/warp spawn blueprints when initializing a Map `World`.
* **Isolation Boundary:** `crates/db` is strictly read-only after startup. It has zero awareness of `ClientIntent`, `ServerEvent`, Tokio channels, TCP sockets, `sqlx` relational databases, or mutable ECS entity state.
