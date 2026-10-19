# Project Metaling — Custom ECS Engine Design (`crates/engine` / `ro-engine`)

## 1. Architectural Overview & Design Goals
`ro-engine` (`crates/engine`) is the deterministic, lock-free, data-oriented Map Server runtime for **Project Metaling**[cite: 2]. Each active map on the server owns an isolated `World` instance that replaces legacy eAthena/rAthena `struct block_list` pointer chains and `map_session_data` God Objects[cite: 4] with:
1. **64-bit Generational `Entity` IDs** (`u32` index + `u32` generation) to eliminate use-after-free bugs during Area-of-Interest (AoI) iteration[cite: 2, 4].
2. **Hybrid SoA + `SparseSet<T>` Storage** inside `World`, tailored to fixed RO entity categories and high-frequency transient states[cite: 2, 4].
3. **A 5-Stage Tick Pipeline** where simulation systems execute pure read-only queries (`&World`), emit buffered `EngineEvent` values, collapse net state deltas in a single-threaded barrier, and emit high-level `ServerEvent` messages[cite: 4].

---

## 2. Memory Layout in `World`: Hybrid Dense Columns & Sparse Sets

Because `World` has a known, fixed domain schema at compile time, it avoids the dynamic type erasure (`TypeId`, raw `BlobVec` byte buffers, and runtime borrow tracking) of general-purpose archetype ECS frameworks like `bevy_ecs`[cite: 4]. Instead, `World` organizes component data into **three explicit storage tiers** where the Rust compiler itself enforces disjoint borrows across systems[cite: 4]:

~~~rust
pub struct World {
    pub map_name: String,
    pub tick: ServerTick,                // Monotonic tick counter + now_ms
    pub entities: EntityAllocator,       // 64-bit Generational Entity allocator
    pub spatial_grid: SpatialGrid8x8,    // 8x8 cell blocks -> SmallVec<[Entity; 8]>

    // --- TIER 1: Universal Dense Arrays (Indexed directly by Entity::index()) ---
    pub positions: Vec<MapPosition>,     // Logical (x, y, dir) cell on the map
    pub kinds: Vec<EntityKind>,          // Player, Monster, Npc, Portal, SkillUnit, FloorItem

    // --- TIER 2: Combat Actor SparseSet<T> Columns ---
    pub stat_dirty: SparseSet<StatDirtyFlags>,         // DIRTY_BASE | DIRTY_EQUIP | DIRTY_STATUS
    pub base_attrs: SparseSet<BaseAttributes>,         // STR..LUK, Base/Job Lv, Base HP/SP/Speed
    pub equip_mods: SparseSet<EquipmentModifiers>,     // Flat + % mods from equipped items/cards
    pub status_mods: SparseSet<StatusModifiers>,       // Flat + % mods from ActiveStatuses
    pub present_attrs: SparseSet<PresentAttributes>,   // Authoritative ATK, DEF, ASPD, move_speed
    pub vitals: SparseSet<Vitals>,                     // Current HP, SP, ShieldHP
    pub active_statuses: SparseSet<ActiveStatuses>,    // FixedBitSet (600 bits) + SmallVec<[StatusEntry; 16]>

    // --- TIER 3: Transient State & Intent SparseSet<T> Columns ---
    pub primary_intents: SparseSet<PrimaryActionIntent>,
    pub instant_intents: SparseSet<SmallVec<[InstantIntent; 4]>>,
    pub walking_paths: SparseSet<WalkingPath>,         // A* waypoints + step timers
    pub active_casts: SparseSet<ActiveCast>,           // Skill cast bar timer & target
    pub combat_locks: SparseSet<CombatLocks>,          // amotion, dmotion (flinch), after-cast delay

    // --- TIER 4: External ID Index Maps & Scratch Buffers ---
    pub char_to_entity: FxHashMap<CharId, Entity>,     // O(1) network CharId -> ECS Entity resolution
    pub gid_to_entity: FxHashMap<u32, Entity>,         // O(1) wire GID (AccountId/Mob GID) -> Entity
}
~~~

---

## 3. `ro-engine` Library Stack & Custom Primitives

`ro-engine` deliberately avoids heavy external ECS or pathfinding frameworks in favor of six focused crates and three small custom primitives[cite: 2, 4].

### 3.1 External Libraries Used Inside `ro-engine`

| Crate | Where It Fits in `ro-engine` | Why It Fits Our Model |
| :--- | :--- | :--- |
| **`smallvec`** *(features: `union`, `const_generics`)*[cite: 2] | • `SpatialGrid8x8` cell blocks (`SmallVec<[Entity; 8]>`)[cite: 2, 4]<br>• `ActiveStatuses.entries` (`SmallVec<[StatusEntry; 16]>`)[cite: 2, 4]<br>• `WalkingPath.waypoints` (`SmallVec<[(u16, u16); 32]>`)\br>• `instant_intents` (`SmallVec<[InstantIntent; 4]>`) | Keeps 99% of spatial block lists, status arrays, intent queues, and 32-step walk paths inline on the stack/struct without heap allocations during 50 Hz ticks, while spilling safely to the heap if 40 entities stack on one cell[cite: 4]. |
| **`fixedbitset`**[cite: 2] | • `ActiveStatuses.bitmask` (`FixedBitSet`) on every combat actor[cite: 2, 4] | Ragnarok Online has 600+ status effects (`SC_*`)[cite: 4]. `FixedBitSet` packs 640 status presence flags into ten `u64` words (80 bytes), giving Stage 1 and Stage 3 systems $O(1)$ `"is frozen/stunned?"` checks without archetype table moves[cite: 4]. |
| **`bitflags`** | • `StatDirtyFlags` (`DIRTY_BASE \| DIRTY_EQUIP \| DIRTY_STATUS`) in Stage 1<br>• `CombatLockFlags` & `EntityStateFlags` | Provides zero-cost, strongly-typed bitmask operations so Stage 1's `stat_aggregation_system` only recalculates entities whose specific attribute layers mutated. |
| **`rustc-hash`** (`FxHashMap` / `FxHashSet`)[cite: 2, 4] | • `char_to_entity` and `gid_to_entity` lookup tables in `World`<br>• Rare, low-cardinality state (<50 instances per map, such as open trade windows or chat rooms)[cite: 4] | Replaces Rust's default cryptographic SipHash with a fast single-multiply hash tailored for integer keys (`u32`, `u64`, `CharId`, `Entity`), resolving network IDs in nanoseconds[cite: 4]. |
| **`rayon`** | • Inter-map parallel `World::step_tick()` execution[cite: 4]<br>• Intra-map parallel iteration in Stage 1 (`par_iter_mut` over dirty stats), Stage 2b (Mob AI), Stage 3 (`par_iter().fold()` into thread-local `Vec<EngineEvent>` buffers), and Stage 5 (AoI gather) | Work-stealing data parallelism that pairs directly with contiguous `SparseSet<T>` dense slices (`&[T]` and `&mut [T]`) and read-only `&World` simulation passes[cite: 4]. |
| **`tracing`**[cite: 2] | • Structured tick instrumentation, unhandled intent diagnostics, and slow-tick warnings[cite: 2] | Zero-allocation structured logging compatible with async and multi-threaded `rayon` spans[cite: 2]. |

~~~toml
# crates/engine/Cargo.toml dependencies
[dependencies]
ro-contracts.workspace = true
smallvec.workspace = true
fixedbitset.workspace = true
rustc-hash.workspace = true
bitflags.workspace = true
rayon.workspace = true
tracing.workspace = true
~~~

### 3.2 Custom Primitives Written Inside `ro-engine` (Why We Don't Use `slotmap` or `pathfinding`)
1. **`Entity` & `EntityAllocator` (~40 lines of safe Rust):**
   * Packs a `u32` slot index and `u32` generation counter into a 64-bit `Entity` handle[cite: 4].
   * Exposes `Entity::index() -> usize` directly so Tier 1 universal columns (`positions`, `kinds`) can be indexed via a single bounds-checked array read (`world.positions[entity.index()]`) after verifying generation liveliness[cite: 1, 4].
2. **`SparseSet<T>` (~60 lines of safe Rust) vs. `slotmap::SecondaryMap`:**
   * Consists of three contiguous vectors: `sparse: Vec<Option<u32>>`, `dense_entities: Vec<Entity>`, and `dense_data: Vec<T>`[cite: 4].
   * Unlike third-party arena maps, our `SparseSet<T>` exposes `(dense_entities: &[Entity], dense_data: &mut [T])` as raw contiguous slices, allowing `rayon`'s `.par_iter_mut()` to split component arrays across CPU cores in Stage 1 with zero wrapper overhead[cite: 4].
3. **`AStarScratchBuffer` (~70 lines of safe Rust) vs. `pathfinding` crate:**
   * Generic pathfinding crates allocate a new `HashMap` and `BinaryHeap` on the heap on every click.
   * Because RO walk paths are strictly bounded to `MAX_WALKPATH = 32` steps, `ro-engine` uses a pre-allocated, reusable `AStarScratchBuffer` (flat generational `visited` grid + fixed-capacity binary heap) that computes octile A* paths with **zero heap allocations** per click.

---

## 4. Multi-Channel Intent Model (Per Entity, Per Tick)

A player can send multiple packets within a single server tick window (e.g., clicking to move while drinking a potion and swapping gear). `World` partitions intents by **lock category**:

1. **`PrimaryActionIntent` (Single Slot — Last-Write-Wins):**
   * **Variants:** `MoveTo { dest_x, dest_y }`, `BasicAttack { target, continuous }`, `CastSkill { skill_id, level, target }`, `ToggleSit { sit: bool }`, `PickUpItem { target }`, `ContactNpc { target }`.
   * **Conflict Rule:** Only **one** `PrimaryActionIntent` exists per entity per tick. If multiple locomotion/combat packets arrive from the same client in one tick, the latest packet overwrites the slot.
2. **`InstantIntent` (`SmallVec<[InstantIntent; 4]>` — Queued):**
   * **Variants:** `UseConsumable { inv_index }`, `EquipItem { inv_index, equip_pos }`, `UnequipItem { inv_index }`.
   * **Conflict Rule:** Executes orthogonally to `PrimaryActionIntent` (players can drink potions or swap weapons mid-walk) unless blocked by status effects or item reuse delays.
3. **`ZeroLockIntent` (Immediate / Non-Gameplay):**
   * **Variants:** `ChangeDirection { head_dir, body_dir }`, `RequestEntityName { target }`, `RequestTime { client_tick }`[cite: 2].

---

## 5. The 5-Stage Deterministic Tick Schedule

`World::step_tick()` runs on a fixed heartbeat (e.g., **`TICK_DELTA_MS = 20ms` / 50 Hz**). Every tick advances a monotonic millisecond clock:

~~~rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServerTick {
    pub tick_count: u64,
    pub now_ms: u64, // Monotonic server time in milliseconds (sent to client as wrapping u32)
}
~~~

~~~text
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 1: PRESENT STATUS ESTABLISHMENT (Dirty-Only Parallel Pass)             │
│  [NextTickStatuses] ──► [BaseAttrs + EquipMods + StatusMods] ──► [Present]   │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 2: INTENT INGESTION & AI PLANNING                                      │
│  • Drain ClientIntents ──► PrimaryActionIntent & InstantIntents              │
│  • Mob AI Sensor (Read SpatialGrid + PresentAttr) ──► PrimaryActionIntent    │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 3: SIMULATION & READ-ONLY EVENT GENERATION (&World -> Buffered Events) │
│  • 3a. Pathfinding System: PrimaryActionIntent::MoveTo ──► A* WalkingPath    │
│  • 3b. Movement Step System: Advance cells by move_speed ──► EntityStepped   │
│  • 3c. Combat & Environment: Calc damage / portal hits ──► Damage / Warp     │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 4: EVENT TRAVERSAL, NET DELTA COLLAPSE & STATE COMMIT (Single-Thread)  │
│  • Fold EngineEvents into net EntityTickDelta (Clamped HP/SP, Final Pos)     │
│  • Commit Vitals, update SpatialGrid8x8, queue NextTickStatuses              │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 5: SERVER EVENT DISPATCH (AoI Diffing, Multicast & Singlecast)         │
│  • Emit ServerEvent { scope: BroadcastScope, payload: ServerEventPayload }   │
└──────────────────────────────────────────────────────────────────────────────┘
~~~

### 5.1 Stage-by-Stage Execution Rules
* **Stage 1 (Present Status Establishment):** Applies queued status changes from the end of the previous tick, expires statuses where `expires_at_ms <= tick.now_ms`, and runs a `rayon` parallel pass over entities with non-zero `StatDirtyFlags` (`BaseAttributes` $\rightarrow$ `EquipmentModifiers` $\rightarrow$ `StatusModifiers` $\rightarrow$ `PresentAttributes`)[cite: 4].
* **Stage 2 (Intent Ingestion & AI Planning):** Drains inbound `ClientIntent` items into `primary_intents` and `instant_intents`[cite: 2], then runs Monster AI sensors in parallel (`&World`) to populate monster `PrimaryActionIntent` slots.
* **Stage 3 (Simulation & Read-Only Event Generation):** Holds immutable borrows (`&World`)[cite: 4]. Worker threads evaluate consumables, pathfinding, cell steps, portal triggers, and combat formulas, pushing results into thread-local `Vec<EngineEvent>` buffers via `rayon::par_iter().fold()`[cite: 4].
* **Stage 4 (Event Traversal, Net Delta Collapse & State Commit):** Single-threaded reduction barrier[cite: 4]. Traverses all buffered `EngineEvent` items, folding net deltas to **HP, SP, Position, and MoveSpeed** per entity (clamping overkill damage at `0 HP` so dead entities ignore subsequent actions) while queueing status additions/removals for the next tick's Stage 1. Preserves discrete combat hits and movement starts so client visual animations are never lost.
* **Stage 5 (Server Event Dispatch):** Emits `ServerEvent` messages for singlecast gauge updates (`ZC_PAR_CHANGE`) and AoI multicast visual animations (`ZC_NOTIFY_PLAYERMOVE`, `ZC_NOTIFY_ACT`, `ZC_MSG_STATE_CHANGE2`)[cite: 2].

---

## 6. Deep Dive: Click-to-Move, A* Pathfinding & Speed-Based Position Updates

In *Ragnarok Online*, movement is **hybrid predictive**: the server does *not* broadcast a packet on every single tile step. When an entity begins moving (or changes direction), the server calculates the full path, validates walkability, and broadcasts the **start cell, destination cell, sub-cell offset, and start timestamp** (`WBUFPOS2`)[cite: 2]. Both the client and `World` then step the entity along the identical A* path at the entity's `move_speed`.

### 6.1 Move Speed, Step Duration & Sub-Cell Coordinates
1. **`move_speed` Definition (`PresentAttributes.move_speed: u16`):**
   * Expressed in **milliseconds required to cross 1 orthogonal cell**.
   * Default player walk speed is `150` (`150ms` per orthogonal tile, or `6.67` tiles/sec). Buffs like *Increase AGI* or Pecopeco mounts lower this value (e.g., `112ms`).
2. **Orthogonal vs. Diagonal Step Timing:**
   * **Orthogonal Step ($\Delta x + \Delta y = 1$):** Takes `step_duration_ms = move_speed as u64` (e.g., `150ms`).
   * **Diagonal Step ($\Delta x = 1 \land \Delta y = 1$):** Scaled by $\approx \sqrt{2}$ (`14 / 10` in Aegis/rAthena integer math):
     `step_duration_ms = (move_speed as u64 * 14) / 10` (e.g., `210ms` at default speed).
3. **Why Server Tick (`20ms`) is Finer Than `move_speed` (`150ms`):**
   * An entity does **not** change its `(x, y)` grid coordinate on every 20ms tick.
   * Instead, `WalkingPath` records `step_started_at_ms` and `next_step_at_ms`. On ticks where `now_ms < next_step_at_ms`, the entity is in transit between cells.
   * **Mid-Stride Redirection (`sx0, sy0` in `WBUFPOS2`):** If a player is 50% of the way from `(100, 100)` to `(101, 100)` and clicks a new destination `(101, 105)`, `ro-engine` computes the 4-bit sub-cell fraction (`0..15`, where `8` is the cell center) from the elapsed step time. Passing `(sx0, sy0)` in `ServerEventPayload::PlayerMovementStarted` tells the client to smoothly pivot from its exact sub-tile pixel position without rubber-banding!

### 6.2 RO-Compliant A* Pathfinding Rules
When `intent_to_pathfinding_system` queries `MapWalkabilityProvider`[cite: 1, 2]:
* **Max Path Length:** `MAX_WALKPATH = 32` waypoints.
* **Step Costs:** Orthogonal neighbor cost = `10`; Diagonal neighbor cost = `14`.
* **Heuristic:** Octile distance (`10 * (dx + dy) + (14 - 20) * min(dx, dy)`).
* **Strict Diagonal Corner-Cutting Rule:** A diagonal step from `(x, y)` to `(x + dx, y + dy)` is **only valid** if `(x + dx, y + dy)` is walkable **AND both adjacent cardinal cells `(x + dx, y)` and `(x, y + dy)` are also walkable**.

### 6.3 Component & System Sketches for Movement

~~~rust
pub const MAX_WALKPATH: usize = 32;

#[derive(Debug, Clone)]
pub struct WalkingPath {
    /// Complete A* waypoint list (excluding the starting cell).
    pub waypoints: SmallVec<[(u16, u16); MAX_WALKPATH]>,
    /// Index of the waypoint the entity is currently stepping toward.
    pub current_idx: usize,
    /// Server timestamp (ms) when the current single-cell step began.
    pub step_started_at_ms: u64,
    /// Server timestamp (ms) when the entity will arrive at `waypoints[current_idx]`.
    pub next_step_at_ms: u64,
    /// Final destination cell `(dest_x, dest_y)` requested by the actor.
    pub target_dest: (u16, u16),
}

impl WalkingPath {
    pub fn step_duration_ms(from: (u16, u16), to: (u16, u16), move_speed: u16) -> u64 {
        let is_diagonal = from.0 != to.0 && from.1 != to.1;
        if is_diagonal {
            (move_speed as u64 * 14) / 10
        } else {
            move_speed as u64
        }
    }

    /// Computes the 4-bit (0..15) sub-cell offset (sx0, sy0) for WBUFPOS2 bit-packing.
    pub fn current_subcell_offset(&self, cur_pos: MapPosition, now_ms: u64) -> (u8, u8) {
        let Some(&next_cell) = self.waypoints.get(self.current_idx) else {
            return (8, 8); // Center of current cell
        };
        let duration = self.next_step_at_ms.saturating_sub(self.step_started_at_ms).max(1);
        let elapsed = now_ms.saturating_sub(self.step_started_at_ms).min(duration);
        let progress = ((elapsed * 8) / duration) as i16; // 0..8 offset from center (8)

        let dx = (next_cell.0 as i16 - cur_pos.x as i16).signum();
        let dy = (next_cell.1 as i16 - cur_pos.y as i16).signum();
        ((8 + dx * progress).clamp(0, 15) as u8, (8 + dy * progress).clamp(0, 15) as u8)
    }
}
~~~

#### Sketch 1: Stage 3a — `intent_to_pathfinding_system`
Translates `PrimaryActionIntent::MoveTo` into an A* `WalkingPath` and emits `EngineEvent::MovementStarted`:

~~~rust
pub fn intent_to_pathfinding_system(
    world: &World,
    walkability: &dyn MapWalkabilityProvider,
    scratch: &mut AStarScratchBuffer,
    out_events: &mut Vec<EngineEvent>,
) {
    let now_ms = world.tick.now_ms;

    for (entity, intent) in world.primary_intents.iter() {
        let PrimaryActionIntent::MoveTo { dest_x, dest_y } = *intent else {
            continue;
        };

        // 1. Validate locks (cannot start moving while Frozen, Stunned, or in dmotion/CastLock)
        if let Some(locks) = world.combat_locks.get(entity) {
            if locks.is_movement_locked(now_ms) {
                continue;
            }
        }

        let cur_pos = world.positions[entity.index()];
        if (cur_pos.x, cur_pos.y) == (dest_x, dest_y) {
            continue;
        }

        // 2. Run zero-allocation RO-compliant A* pathfinding over .gat cells
        let Some(waypoints) = scratch.find_path(
            walkability,
            &world.map_name,
            (cur_pos.x, cur_pos.y),
            (dest_x, dest_y),
            MAX_WALKPATH,
        ) else {
            continue; // Unreachable or blocked cell
        };

        let move_speed = world
            .present_attrs
            .get(entity)
            .map(|a| a.move_speed)
            .unwrap_or(150);

        // 3. Calculate sub-cell offset if redirecting mid-stride
        let (sx0, sy0) = world
            .walking_paths
            .get(entity)
            .map(|p| p.current_subcell_offset(cur_pos, now_ms))
            .unwrap_or((8, 8));

        let first_step = waypoints[0];
        let step_dur = WalkingPath::step_duration_ms((cur_pos.x, cur_pos.y), first_step, move_speed);

        let new_path = WalkingPath {
            target_dest: *waypoints.last().unwrap(),
            waypoints,
            current_idx: 0,
            step_started_at_ms: now_ms,
            next_step_at_ms: now_ms + step_dur,
        };

        out_events.push(EngineEvent::MovementStarted {
            entity,
            src: (cur_pos.x, cur_pos.y),
            dst: new_path.target_dest,
            subcell: (sx0, sy0),
            start_tick_ms: now_ms as u32,
            path: new_path,
        });
    }
}
~~~

#### Sketch 2: Stage 3b — `movement_step_system`
Advances active `WalkingPath` timers against `world.tick.now_ms` and emits discrete cell transitions:

~~~rust
pub fn movement_step_system(
    world: &World,
    out_events: &mut Vec<EngineEvent>,
) {
    let now_ms = world.tick.now_ms;

    for (entity, path) in world.walking_paths.iter() {
        // Only step if the current cell transit timer has elapsed
        if now_ms < path.next_step_at_ms {
            continue;
        }

        let from_pos = world.positions[entity.index()];
        let to_coords = path.waypoints[path.current_idx];
        let new_dir = calc_ro_direction((from_pos.x, from_pos.y), to_coords);

        let move_speed = world
            .present_attrs
            .get(entity)
            .map(|a| a.move_speed)
            .unwrap_or(150);

        let next_idx = path.current_idx + 1;
        let next_step_at_ms = if let Some(&next_coords) = path.waypoints.get(next_idx) {
            let dur = WalkingPath::step_duration_ms(to_coords, next_coords, move_speed);
            // Carry over any sub-tick remainder so average velocity stays 100% exact
            Some((path.next_step_at_ms, path.next_step_at_ms + dur))
        } else {
            None // Path completed!
        };

        out_events.push(EngineEvent::EntitySteppedCell {
            entity,
            from: from_pos,
            to: MapPosition {
                x: to_coords.0,
                y: to_coords.1,
                dir: new_dir,
            },
            next_idx,
            next_timing: next_step_at_ms,
        });
    }
}
~~~

#### Sketch 3: Stage 4 Reducer — Committing Position Deltas & AoI Boundary Diffs
When Stage 4 folds `EngineEvent::MovementStarted` and `EngineEvent::EntitySteppedCell`:
1. **`MovementStarted`:** Installs the new `WalkingPath` into `world.walking_paths` and pushes `ServerEventPayload::PlayerMovementStarted` (to self) and `ServerEventPayload::ActorMovementStarted` (to AoI observers)[cite: 2].
2. **`EntitySteppedCell`:**
   * Updates `world.positions[entity.index()] = to`.
   * If `from.block_coords() != to.block_coords()` (where a block is `x / 8, y / 8`), moves `entity` in `world.spatial_grid` and computes the **Area-of-Interest (AoI) Set Difference**[cite: 2, 4]:
     * **Entering Blocks:** New observers entering visual range receive `ServerEventPayload::ActorMovementStarted` (`ZC_NOTIFY_MOVEENTRY`) if the entity is still walking, or `ServerEventPayload::ActorSpawnedOrEnteredAoI` (`ZC_NOTIFY_STANDENTRY`) if it finished its path on that cell[cite: 2].
     * **Leaving Blocks:** Old observers falling out of visual range receive `ServerEventPayload::ActorVanished` (`ZC_NOTIFY_VANISH`).
   * Queries the new cell in `world.spatial_grid` for overlapping `EntityKind::Portal` or `OnTouch` NPC trigger boxes, emitting `ServerEventPayload::PlayerMapMove` (`ZC_NPCACK_MAPMOVE`) or `ScriptTriggerEvent::TouchNpc` if triggered[cite: 1, 2].

---

## 7. `ServerEvent` Contract & Outbound `ZC_` Packet Translation

Systems inside `ro-engine` never construct raw byte buffers or touch TCP sockets[cite: 2]. At the end of Stage 4 / Stage 5, `World` emits high-level `ServerEvent` structs:

~~~rust
use crate::{AccountId, CharId, Entity, EntityKind, MapPosition, StatusId};

/// Envelope emitted by `ro-engine::World` at Stage 5 of every tick.
#[derive(Debug, Clone)]
pub struct ServerEvent {
    pub scope: BroadcastScope,
    pub payload: ServerEventPayload,
}

#[derive(Debug, Clone)]
pub enum BroadcastScope {
    /// Send only to a single player's TCP session.
    SelfOnly(CharId),
    /// Send to all players within visual AoI range of `center` (optionally excluding `exclude`).
    AreaOfInterest {
        map_name: String,
        center: MapPosition,
        exclude: Option<CharId>,
    },
    /// Send to all members of a party on the map.
    Party { party_id: u32 },
    /// Send to all connected players on `map_name`.
    MapWide { map_name: String },
}

#[derive(Debug, Clone)]
pub enum ServerEventPayload {
    // --- 1. Movement & Spatial AoI Events ---
    /// Sent to `SelfOnly(char_id)` when the player begins or redirects a walk path.
    /// Wire Packet: `ZC_NOTIFY_PLAYERMOVE` (`0x0087`) with 6-byte `WBUFPOS2`.
    PlayerMovementStarted {
        start_tick_ms: u32,
        src: (u16, u16),
        dst: (u16, u16),
        subcell: (u8, u8), // (sx0, sy0) 0..15
    },

    /// Sent to `AreaOfInterest` (excluding self) when any actor starts/redirects movement,
    /// OR sent to `SelfOnly` when a moving actor enters a player's AoI mid-walk.
    /// Wire Packet: `ZC_NOTIFY_MOVE` (`0x0086`) / `ZC_NOTIFY_MOVEENTRY` (`0x09FD`).
    ActorMovementStarted {
        entity: Entity,
        gid: u32,          // AccountId for players, runtime GID for mobs
        kind: EntityKind,
        start_tick_ms: u32,
        src: (u16, u16),
        dst: (u16, u16),
        subcell: (u8, u8),
        move_speed: u16,
        look: ActorLookSnapshot,
    },

    /// Sent when a stationary actor spawns or enters a player's AoI.
    /// Wire Packet: `ZC_NOTIFY_STANDENTRY` / `ZC_NOTIFY_NEWENTRY` (`0x09FE` / `0x09FF`).
    ActorSpawnedOrEnteredAoI {
        entity: Entity,
        gid: u32,
        kind: EntityKind,
        pos: MapPosition,
        move_speed: u16,
        body_state: u8,    // Sit / Stand / Dead
        look: ActorLookSnapshot,
    },

    /// Sent when an actor leaves AoI, dies, warps out, or disconnects.
    /// Wire Packet: `ZC_NOTIFY_VANISH` (`0x0080`, vanish_type: 0=OutOfSight, 1=Died, 2=LoggedOut, 3=Teleported).
    ActorVanished {
        gid: u32,
        vanish_type: u8,
    },

    /// Sent when an actor immediately snaps to a cell (knockback or stop-walk on hit/lock).
    /// Wire Packet: `ZC_STOPMOVE` (`0x0088`).
    ActorStoppedAt {
        gid: u32,
        x: u16,
        y: u16,
    },

    // --- 2. Collapsed Net Attribute Deltas (End-of-Tick Gauges) ---
    /// Sent to `SelfOnly(char_id)` when net HP, SP, MoveSpeed, or ASPD changes in Stage 4.
    /// Wire Packet: `ZC_PAR_CHANGE` (`0x00B0`) / `ZC_LONGPAR_CHANGE` (`0x00B1`).
    PlayerParamsUpdated {
        hp: Option<(u32, u32)>,   // (current_hp, max_hp)
        sp: Option<(u32, u32)>,   // (current_sp, max_sp)
        move_speed: Option<u16>,  // VAR_SPEED
        aspd: Option<u16>,
    },

    // --- 3. Discrete Visual Action & Combat Events ---
    /// Sent to `AreaOfInterest` for melee attacks, sitting (action=2), standing (action=3), or pickup.
    /// Wire Packet: `ZC_NOTIFY_ACT` (`0x008A` / `0x08C8`).
    ActorActionPerformed {
        source_gid: u32,
        target_gid: u32,
        server_tick_ms: u32,
        src_speed_ms: i32, // amotion
        dst_speed_ms: i32, // dmotion
        damage: i32,
        div: i16,          // Hit count
        action_type: u8,   // 0=Damage, 2=Sit, 3=Stand, 8=MultiHit, 10=Critical
        dual_wield_damage: i32,
    },

    /// Sent to `AreaOfInterest` when a status effect starts or expires in Stage 1.
    /// Wire Packet: `ZC_MSG_STATE_CHANGE` (`0x0196`) / `ZC_MSG_STATE_CHANGE2` (`0x043F` / `0x0983`).
    StatusStateChanged {
        gid: u32,
        status_icon_id: u16, // SI_* client icon constant
        active: bool,
        remaining_ms: u32,
        val1: i32,
        val2: i32,
        val3: i32,
    },

    // --- 4. Map Transfers & Portal Collision ---
    /// Sent to `SelfOnly(char_id)` when stepping onto an `EntityKind::Portal` in Stage 3/4.
    /// Wire Packet: `ZC_NPCACK_MAPMOVE` (`0x0091`).
    PlayerMapMove {
        dest_map: String, // e.g., "prt_fild08.gat"
        dest_x: u16,
        dest_y: u16,
    },
}
~~~

---

## 8. Summary Mapping: `ServerEventPayload` $\rightarrow$ Wire Packets

| `ServerEventPayload` Variant | `BroadcastScope` | Target `ZC_` Wire Packet | Trigger Stage in `World` |
| :--- | :--- | :--- | :--- |
| `PlayerMovementStarted` | `SelfOnly(char_id)` | `ZC_NOTIFY_PLAYERMOVE` (`0x0087`)[cite: 2] | Stage 4 (from Stage 3a A* Path) |
| `ActorMovementStarted` | `AreaOfInterest` *(excl. self)* | `ZC_NOTIFY_MOVE` / `ZC_NOTIFY_MOVEENTRY` | Stage 4 (Path start or AoI enter) |
| `ActorSpawnedOrEnteredAoI` | `AreaOfInterest` / `SelfOnly` | `ZC_NOTIFY_STANDENTRY` / `ZC_NOTIFY_NEWENTRY`[cite: 2] | Stage 4 (AoI block diff or Spawn) |
| `ActorVanished` | `AreaOfInterest` | `ZC_NOTIFY_VANISH` (`0x0080`) | Stage 4 (AoI leave, Death, Warp) |
| `ActorStoppedAt` | `AreaOfInterest` | `ZC_STOPMOVE` (`0x0088`) | Stage 4 (Knockback / Immobilize) |
| `PlayerParamsUpdated` | `SelfOnly(char_id)` | `ZC_PAR_CHANGE` (`0x00B0`)[cite: 2] | Stage 5 (Collapsed Net Deltas) |
| `ActorActionPerformed` | `AreaOfInterest` | `ZC_NOTIFY_ACT` (`0x008A` / `0x08C8`)[cite: 2] | Stage 5 (Discrete Combat/Sit/Stand) |
| `StatusStateChanged` | `AreaOfInterest` | `ZC_MSG_STATE_CHANGE2` (`0x0983`) | Stage 1 (Status Applied / Expired) |
| `PlayerMapMove` | `SelfOnly(char_id)` | `ZC_NPCACK_MAPMOVE` (`0x0091`)[cite: 2] | Stage 4 (Portal collision) |