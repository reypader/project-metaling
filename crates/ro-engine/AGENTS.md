# `crates/ro-engine` Module Rules (Map ECS Runtime)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-engine.md` (`ro-engine` custom subagent).
* **Write Scope:** `./crates/ro-engine/src/**` and `./crates/ro-engine/Cargo.toml` exclusively.
* **Design Reference:** Read `./crates/ro-engine/DESIGN.md` for an initial architectural design to follow. This document serves as a rough directional sketch of where the engine domain is headed and allows flexibility as implementation details evolve.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever map state machine lifecycles, walking ticks, sitting rules, portal triggers, or AoI broadcast ranges from `./reference/**` are needed. Never read `./reference/**` directly.
* **Peer Consultation:** Coordinate with `ro-db`, `ro-mechanics`, `ro-scripting`, `ro-persistence`, and `ro-bins` via subagent messaging when integrating ECS systems with external traits or `map-server` actors.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-engine` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit ECS borrow safety, spatial indexing, and tick ordering before completion.

---

## 2. ECS Architecture & Simulation Mechanics

`crates/ro-engine` is the deterministic Entity-Component-System (ECS) simulation runtime for `map-server`. It manages map instances, spatial indexing, A* pathfinding, entity lifecycles, movement interpolation, and Area of Interest (AoI) event generation without owning network sockets or database connections.

### 2.1 ECS Data Layout & Spatial Indexing
1. **Generational Entities:** Every map actor (Player, Monster, Warp Portal, NPC, FloorItem) is identified within the ECS `World` by a 64-bit generational `Entity` handle (32-bit index + 32-bit generation) mapped bidirectionally to its protocol actor ID (`AccountId`, `CharId`, or monster GID).
2. **`8x8` Spatial Grid (`SpatialGrid`):**
   * Partitions each map's cell coordinates `(x, y)` into `8x8` cell blocks (`block_x = x / 8`, `block_y = y / 8`).
   * Each block stores `SmallVec<[Entity; 8]>` to avoid heap allocations in normal population densities.
   * **Borrow-Checker Discipline:** When broadcasting to or querying an Area of Interest (such as a 14-cell radius covering surrounding `8x8` blocks), copy matching `Entity` handles into a local stack `SmallVec<[Entity; 32]>` before mutating components in `World`.
3. **Component Storage Discipline:**
   * **Dense Storage:** Core components present on almost all actors (`MapPosition`, `ActorIdentity`, `Appearance`, `BaseStats`, `ActiveStatuses`). Store buffs inside a single dense `ActiveStatuses` component (`FixedBitSet` + `SmallVec`). Separate components per buff are prohibited.
   * **Sparse Storage (`SparseSet<T>`):** Transient state components attached only while an action is active (`WalkingPath`, `ActiveCast`, `MovementLock`, `SittingState`).

### 2.2 Deterministic Map Tick Pipeline
Each Map `World` advances in discrete deterministic ticks (such as 20ms steps tracked by a monotonic `server_tick: u32` in milliseconds). Every tick executes six phases in strict sequential order:
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
   * Update `SpatialGrid` block membership when an entity crosses an `8x8` boundary, emitting `EntityAppeared` and `EntityVanished` for newly entering and leaving AoI observers.
   * Check whether a moving player stepped within the trigger radius of a Warp Portal entity; when triggered, emit a map-move transfer event (`ZC_NPCACK_MAPMOVE`) and despawn the player from the current map.
4. **Phase 4: Combat Read Phase:**
   * Gather immutable attacker and defender stat blocks and invoke stateless combat calculators from `crates/ro-mechanics` via `crates/ro-contracts` traits.
5. **Phase 5: Effect Application Phase:**
   * Apply computed damage deltas, knockbacks, status changes, and death transitions to ECS components.
6. **Phase 6: Emit `ScopedServerEvent` Batch:**
   * Drain the tick's outbound `ScopedServerEvent` buffer (`BroadcastScope::SelfOnly`, `BroadcastScope::AreaOfInterest`, `BroadcastScope::MapWide`) and return it to the caller (`MapService`).

---

## 3. Contract Boundaries (`crates/ro-contracts`)
* **Upstream Host (`bins/map-server`):** `MapService` hosts `ro-engine` instances, drives the periodic tick timer, feeds session-authenticated `ClientIntent` items into the engine, and routes returned `ScopedServerEvent` items to connected client sessions.
* **Downstream Domain Interfaces:**
  * **To `crates/ro-db`:** Queries `MapWalkability` (`crates/ro-contracts/src/map.rs`) for cell walkability (`is_walkable`, `cell_type`, `width`, `height`) during A* pathfinding, and reads mob/warp spawn blueprints on map initialization.
  * **To `crates/ro-mechanics`:** Passes stat and combat context DTOs to pure calculation traits during Phase 4 (`Combat Read Phase`).
  * **To `crates/ro-scripting`:** Emits NPC interaction triggers and consumes script VM events (`mes`, `next`, `select`, `warp`).
  * **To `crates/ro-persistence`:** Never executes SQL directly; exposes character position and state snapshots so `MapService` can flush updates to `CharacterRepository` on warp or disconnect.
