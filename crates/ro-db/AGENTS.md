# `crates/ro-db` Module Rules (Static Data & Map Cache)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-db.md` (`ro-db` custom subagent).
* **Write Scope:** `./crates/ro-db/**` exclusively.
* **Design Reference:** Read `./crates/ro-db/DESIGN.md` for an initial architectural design to follow. This document serves as a rough directional sketch of where the db domain is headed and allows flexibility as implementation details evolve.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever `.gat`/`.rsw` header layouts, Renewal YAML schemas, or spawn/warp script definitions from `./reference/**` are needed. Never read `./reference/**` directly.
* **Peer Consultation:** Coordinate with `ro-engine` and `ro-bins` via subagent messaging when integrating static registries and walkability providers.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-db` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit bounds checking and deserialization edge cases before completion.
* **Red-Green Testing & Spec Collaboration:** Collaborate with `spec-writer` to define database parsing, schema validation, and map cell walkability test scenarios supporting high-level bin-observable scenarios. Author or update database unit and integration tests upfront (red phase) before writing production code changes.

---

## 2. Static Data & Map Walkability Specifications

`crates/ro-db` loads, validates, and indexes immutable game world definitions at server startup: binary `.gat`/`.rsw` map cell walkability grids, Renewal YAML databases (`mob_db.yml`, `item_db.yml`, `skill_db.yml`), and NPC monster/warp spawn definitions.

### 2.1 Map Collision & `.gat` Walkability Grids
1. **Cell Classification (`CellType`):**
   * Implements the `MapWalkability` trait from `crates/ro-contracts/src/map.rs`.
   * Maps raw `.gat` cell type bytes (`0..=6`) to `CellType`:
     * `0` (`Walkable`), `2` (`Water`), `3` (`Water2`), `4` (`Walkable4`), `6` (`Walkable6`) evaluate to walkable (`is_walkable() == true`).
     * `1` (`Wall`, blocks movement and line of sight) and `5` (`Cliff`, blocks movement, allows line of sight) evaluate to non-walkable (`is_walkable() == false`).
2. **Contiguous Row-Major Storage:**
   * Store map cells in a flat `Vec<CellType>` of length `width as usize * height as usize` indexed by `(y as usize) * (width as usize) + (x as usize)`.
   * Bounds-check all `(x, y)` queries (`x < width && y < height`) so out-of-bounds coordinates return `false` for `is_walkable` and `None` for `cell_type` without panicking.

### 2.2 YAML Database Deserialization (`serde_yaml`)
1. **rAthena Renewal Schema:**
   * Renewal YAML files contain a `Header` (`Type`, `Version`) and a `Body` sequence of entries, often split across multiple files imported via `Footer.Imports`.
   * Load base `db/re/` entries first, then overlay `db/import/` entries by ID.
   * All `ItemId` values must deserialize into 32-bit `u32` (`ItemId(u32)` in `crates/ro-contracts`).
2. **Monster & Warp Portal Spawns:**
   * Parse warp definitions (`<src_map>,<src_x>,<src_y>,<facing>\twarp\t<name>\t<span_x>,<span_y>,<dst_map>,<dst_x>,<dst_y>`).
   * Parse monster spawn definitions (`<map>,<x>,<y>,<xs>,<ys>\tmonster\t<name>\t<mob_id>,<amount>,<delay1>,<delay2>`).

---

## 3. Contract Boundaries (`crates/ro-contracts`)
* **Upstream Initializer:** `bins/map-server` loads `ro-db` registries at process startup and passes `Arc<dyn MapWalkability>` and static blueprint registries into `ro-engine`.
* **Downstream Consumer:** `crates/ro-engine` queries `MapWalkability` during A* pathfinding and reads mob/warp spawn blueprints when initializing a Map `World`.
* **Isolation Invariant:** `crates/ro-db` is strictly read-only after startup. It has zero awareness of `ClientIntent`, `ServerEvent`, Tokio channels, TCP sockets, `sqlx` relational databases, or mutable ECS entity state.
