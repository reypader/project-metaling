# AGENTS.md: `crates/engine` Subagent (Map ECS Runtime)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/engine/**`, `./crates/contracts/**` (including `./crates/contracts/src/**`), and `./reference/**`.
* **Direct Contract Inspection (No `cargo doc`):** Read the source files in `./crates/contracts/src/` directly via file viewing tools. Avoid running `cargo doc` or background documentation builds to inspect contracts, because reading files directly eliminates compilation overhead.
* **Write Scope:** `./crates/engine/src/**` and `./crates/engine/Cargo.toml` ONLY.
* **Prohibition:** NEVER write to `./crates/contracts/**`, `./bins/*`, or another domain crate (`./crates/<other_domain>/**`). NEVER read implementation files in other domain crates (`./crates/<other_domain>/src/**`). Communicate with `packets`, `db`, `mechanics`, `persistence`, and `scripting` strictly through traits, DTOs, and events in `crates/contracts`. If you need a new shared type or trait in `crates/contracts`, STOP and request a Contract Amendment from the Main Agent.

---

## 2. Domain Architecture & ECS Simulation Mechanics

`crates/engine` (`ro-engine`) is the deterministic Entity-Component-System (ECS) simulation runtime for `map-server`. It manages map instances, spatial indexing, A* pathfinding, entity lifecycles, movement interpolation, and Area of Interest (AoI) event generation without owning network sockets or database connections.

### 2.1 ECS Data Layout & Spatial Indexing
1. **Generational Entities:** Every map actor (Player, Monster, Warp Portal, NPC, FloorItem) is identified within the ECS `World` by a 64-bit generational `Entity` handle (32-bit index + 32-bit generation) mapped bidirectional to its protocol actor ID (`AccountId` / `CharId` / monster GID).
2. **`8x8` Spatial Grid (`SpatialGrid`):**
   * Partitions each map's cell coordinates `(x, y)` into `8x8` cell blocks (`block_x = x / 8`, `block_y = y / 8`).
   * Each block stores `SmallVec<[Entity; 8]>` to avoid heap allocations in normal population densities.
   * **Borrow-Checker Discipline:** When broadcasting to or querying an Area of Interest (for example, a 14-cell radius / surrounding `8x8` blocks), copy matching `Entity` handles into a local stack `SmallVec<[Entity; 32]>` before mutating components in `World`.
3. **Component Storage Discipline:**
   * **Dense Storage:** Core components present on almost all actors (`MapPosition`, `ActorIdentity`, `Appearance`, `BaseStats`, `ActiveStatuses`). Store buffs inside a single dense `ActiveStatuses` component (`FixedBitSet` + `SmallVec`) rather than separate components per buff.
   * **Sparse Storage (`SparseSet<T>`):** Transient state components attached only while an action is active (`WalkingPath`, `ActiveCast`, `MovementLock`, `SittingState`).

### 2.2 Deterministic Map Tick Pipeline
Each Map `World` advances in discrete deterministic ticks (for example, 20ms steps tracked by a monotonic `server_tick: u32` in milliseconds). Every tick executes six phases in strict sequential order:
1. **Phase 1: Drain `ClientIntent` Queue:**
   * Process queued player intents in FIFO order per session:
     * `EnterMap`: Spawn player entity (or bind pending transfer), register in `SpatialGrid`, emit `ServerEvent::AcceptEnter { server_tick, pos }`.
     * `NotifyActorInit`: Mark client map-load complete; emit initial `ParameterChanged` (`SelfOnly`), `EntityAppeared` for existing AoI entities to the joining player, and `EntityAppeared` of the joining player to surrounding AoI neighbors.
     * `RequestTime`: Emit `ServerEvent::TimeSynchronized { server_tick }`.
     * `RequestMove`: Compute A* path from current `MapPosition` to `(dest_x, dest_y)` using `MapWalkability`; attach `WalkingPath` sparse component and broadcast `ServerEvent::EntityMoved` to AoI.
     * `ChangeDirection` & `RequestAction`: Update head/body direction or sitting/standing state (`action_type = 2` sit, `action_type = 3` stand) and broadcast `ServerEvent::EntityAction` to AoI.
     * `RequestEntityName`: Look up target entity name and emit `AckEntityName` (`SelfOnly`).
2. **Phase 2: Timers & Monster AI:**
   * Advance monster AI state machines, respawn timers, and status expirations in `ActiveStatuses`.
3. **Phase 3: Movement Step & Portal Collision Triggers:**
   * Advance entities with an active `WalkingPath` along their next cell step based on walk speed and elapsed `server_tick` (orthogonal step cost = `speed`, diagonal step cost = `speed * 14 / 10`).
   * Update `SpatialGrid` block membership when an entity crosses an `8x8` boundary, emitting `EntityAppeared` / `EntityVanished` for newly entering/leaving AoI observers.
   * Check whether a moving player stepped within the trigger radius of a Warp Portal entity; if triggered, emit a map-move / transfer event (`ZC_NPCACK_MAPMOVE`) and despawn from the current map.
4. **Phase 4: Combat Read Phase:**
   * Gather immutable attacker and defender stat blocks and invoke stateless combat calculators from `crates/mechanics` via `crates/contracts` traits.
5. **Phase 5: Effect Application Phase:**
   * Apply computed damage deltas, knockbacks, status changes, and death transitions to ECS components.
6. **Phase 6: Emit `ScopedServerEvent` Batch:**
   * Drain the tick's outbound `ScopedServerEvent` buffer (`BroadcastScope::SelfOnly`, `BroadcastScope::AreaOfInterest`, `BroadcastScope::MapWide`) and return it to the caller (`MapService`).

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Upstream Host (`bins/map-server`):** `MapService` owns the `ro-engine` instance(s), drives the periodic tick timer, feeds session-authenticated `ClientIntent` items into the engine, and routes returned `ScopedServerEvent` items to connected client sessions.
* **Downstream Domain Interfaces (via `crates/contracts` traits & DTOs):**
  * **To `crates/db`:** Queries `MapWalkability` (`crates/contracts/src/map.rs`) for cell walkability (`is_walkable`, `cell_type`, `width`, `height`) during A* pathfinding, and reads mob/warp spawn blueprints on map initialization.
  * **To `crates/mechanics`:** Passes stat/combat context DTOs to pure calculation traits during Phase 4 (`Combat Read Phase`).
  * **To `crates/scripting`:** Emits NPC interaction triggers and consumes script VM events (`mes`, `next`, `select`, `warp`).
  * **To `crates/persistence`:** Never executes SQL directly; exposes character position/state snapshots so `MapService` can flush updates to `CharacterRepository` on warp or disconnect.
