# Project Metaling — Custom ECS Engine Design (`crates/ro-engine`)

## 1. Architectural Overview & Design Goals
`ro-engine` is the deterministic, lock-free, data-oriented Map Server runtime for **Project Metaling**. Each active map on the server owns an isolated `World` instance that replaces legacy eAthena/rAthena `struct block_list` pointer chains and `map_session_data` God Objects with:
1. **64-bit Generational `Entity` IDs** (`u32` index + `u32` generation) to eliminate use-after-free bugs during Area-of-Interest (AoI) iteration.
2. **Hybrid SoA + `SparseSet<T>` Storage** inside `World`, tailored to fixed RO entity categories and high-frequency transient states.
3. **A 5-Stage Tick Pipeline** where simulation systems execute pure read-only queries (`&World`), emit buffered `EngineEvent` values, collapse net state deltas in a single-threaded barrier, and emit high-level `ServerEvent` messages.

---

## 1a. Foundation Subset (Implemented)

Spec: `specs/engine-foundation/spec.md`. Everything else in this document is a **future phase** unless listed here.

* **Single-threaded.** No `rayon` yet; one `MapInstance` per map, driven by the map-server actor's `step_tick(now_ms)`.
* **Entities:** `Entity` (u32 index + u32 generation) and `EntityAllocator` with a fixed capacity; a slot whose generation reaches `u32::MAX` is retired (`src/entity.rs`).
* **Storage (`src/world.rs`):** dense columns `positions`, `kinds`, `gids` (wire id of every entity), `dead_sit`, `head_dir`, `speed`, `in_grid`, `pending_move`; sparse `players: SparseSet<PlayerInfo>` (name, job, sex, hair, ids, `rewarp_count`), `npcs: SparseSet<NpcInfo>`, `mobs: SparseSet<MobInfo>` (gid, mob id, name, level, hp, spawn group), `inventories: SparseSet<PlayerInventory>`, `floor_items: SparseSet<FloorItem>`, `warps: SparseSet<WarpTrigger>` and `walking_paths: SparseSet<WalkingPath>`; `char_to_entity` / `gid_to_entity` (`FxHashMap`). `EntityKind::{Free, Player, Npc, Mob, FloorItem}`. `DEFAULT_ENTITY_CAPACITY` is 16384 (floor items count; the columns grow lazily).
* **Spatial grid (`src/spatial.rs`):** `SpatialGrid8x8` buckets entities per 8x8 block (`SmallVec<[Entity; 8]>`); `query_radius` filters by exact Chebyshev distance (`AREA_SIZE = 14`). Visibility on each step: one query of radius 15 around the old cell (a step moves at most one cell), then each candidate is classified by whether it was / is within 14 of the old / new cell.
* **Pathfinding (`src/path.rs`):** rAthena rules — cost 10/14, **Manhattan x 10** heuristic (not octile), no corner cutting, last row/column blocked, start cell unchecked, paths over `MAX_WALKPATH = 32` steps rejected; clicks additionally capped at `max_walk_path = 17` (a destination farther than the cap in either axis is rejected before searching). Closed nodes are reopened when a cheaper route appears, as rAthena's `add_path` does. The search runs in a fixed 65x65 window around the start with stamped scratch arrays, reused across searches.
* **Movement (`src/movement.rs`):** step duration `speed` / `speed * 14 / 10`; the cell changes at the END of a step; the next step starts when the previous one ended. Sub-cell is always (8, 8).
* **Tick order (`MapInstance::step_tick(now, out)`):** expire floor items -> advance due steps (AoI diff per step, pending-redirect repath) -> drain intents FIFO (last move per entity per tick wins, via a `pending_move` column) -> validate and plan paths -> the stat pass (Stages 3-4 of §5) -> append events to the caller's buffer. Steps run before intents so a click at `now` sees steps that ended before it.
* **Stats (`src/stats.rs`, `src/status.rs`, spec `specs/stats/spec.md`):** `PlayerInfo.derived` is the `ro_mechanics::Status` snapshot; `PlayerInfo.gear` caches the worn gear's `GearStatus` (per-hand `WeaponGear` with refine, shield, Σ DEF, Σ armor refine, range). Flows call `mark_dirty(e, gear)` and `queue_followup(e, FollowUp)`; the player is listed once in `MapInstance::stat_queue` (`LISTED` bit in `stat_flags`). At tick end `run_stat_pass` settles each listed player in marking order: rebuild the gear status if stale, `calc_status`, then `SP_MAXWEIGHT` if changed, the HP/SP clamp pushes, `status_diff` (STR..LUK `0x0141`, derived values in rAthena order, range, MAXHP/MAXSP/HP/SP), then the follow-ups (`ToSelf`, `ToArea`, `CoupleStat` (a send-time `0x0141` read from the new snapshot), `HealToMax`, `LevelGearCheck` (the `elvmax` unequip after a base level change, last as captured), `Save` → `drain_save_requests`). A follow-up that marks the player dirty again (an `elvmax` unequip) is settled next tick: the pass doesn't re-run a player in the tick that listed it. Stat-up is `src/stat_up.rs`; `@blvl`/`@jlvl`/`@job` are `src/level_command.rs`, with the gear re-checks in `wear.rs` (`unequip_over_level`, `unequip_unusable`) and `job::job_for_sex`. `progress_snapshot` / `play_snapshot` give the map-server the `CharacterProgress` to save. Spawn and cross-map arrival compute the snapshot synchronously without a diff (`init_status`); a cross-map warp settles the player before it leaves, a logout drops what was owed. The inputs come from `JobDb` (job bonus, HP/SP tables, ASPD bases, MaxWeight), `RefineDb`, `BattleConfig` and `job.rs` (`class_flags`, the Super Novice +10), all in `Registries`. A job missing from `JobDb` keeps its saved MaxHP/MaxSP with default MaxWeight and ASPD.
* **Warps (`src/warp.rs`, spec `specs/warp-map-change/spec.md`):** each plain warp NPC keeps a `WarpTrigger` (half-extents, `.gat` destination, raw cell, `same_map`). `warp_cells` is a bitmap of the walkable cells inside any trigger area; a hit scans `warps` for the first enabled (not `Disabled`/`Cloaked`) area in spawn order. The check runs after every completed player step and at the end of `actor_init` (arrival cell, so warps chain). `PlayerInfo.rewarp_count` stops loops after 11 chained warps. Random destinations use the instance's SplitMix64 (`src/rng.rs`, seeded from the map name, shared with mob placement).
  * **Same map:** vanish (type 0) to the AoI, `World::hide`, clear the walk silently, move, and send `PlayerMapMove` to self. The next `NotifyActorInit` shows the player again.
  * **Cross map:** vanish (type 0), despawn (shared with `remove_character`), and queue a `MapTransfer` that the actor takes with `drain_transfers`. The destination instance's `enter_by_warp` resolves the cell and spawns the player out of the grid without emitting anything; the caller sends the `0x0091`.
* **Mobs (`src/mob.rs`, spec `specs/mob-spawning/spec.md`):** `spawn_mobs(&MapMobSpawns)` runs once at world start, after NPCs. Each line becomes a `SpawnGroup` (the loaded line plus the rectangle rules: zero span -> 1, map-wide when `(0, 0)` or a 1x1 on an unwalkable cell) in `MapInstance::spawn_groups`; each mob keeps its group index for respawn. Placement follows rAthena's `mob_spawn` at default config: a per-mob randomized center, the center shortcut (boss 1x1, or `rnd() % (xs*ys) == 0` on a walkable center), 8 free-cell tries, then the center, then 50 map-wide tries; a mob with no cell is not created. Gids come from the NPC id counter (one pool from `START_NPC_ID`). Stats are placeholders (1 HP, level 1 or the line's level, speed 150). Mobs are visible like NPCs: `0x09ff` on entry/walk-in with `object_type` `Mob`, `0x0adf` on name request. No movement, AI, death or respawn yet.
* **Items (`src/inventory.rs`, `src/floor_item.rs`, spec `specs/items/spec.md`):** the global `ItemDb` / `MobDb` are shared `Arc`s set with `set_registries`.
  * **Inventory:** `PlayerInventory` (100 slots with holes, weight, max weight from the status: job `MaxWeight` + allocated STR × 300, `unique_id` counter, a `revision` bumped on every change) implements rAthena's `pc_additem` / `pc_delitem` order and result codes. Spawning moves `CharacterSpawn.inventory` into the component; a cross-map warp moves it back out into the `MapTransfer` (no clone). The map-server reads `play_snapshot` (a clone, only on save, with the char-select `CharacterLooks` of the worn gear and the `CharacterProgress`) and `inventory_revision`; a `MapTransfer` carries the looks beside the moved inventory. Every `NotifyActorInit`, repeats included, sends the entry lists (`0x0b08`, `0x0b09`, `0x0b39`, `0x0b0b`, `0x0a9b`, SP_WEIGHT, SP_MAXWEIGHT) before the "already visible" check.
  * **Floor items** are entities of kind `FloorItem` in the spatial grid, so they reuse `step_visibility` and the AoI emitters: `entry_event` gives `0x009d`, a floor item leaving a mover's view gives `0x00a1` (never `0x0080`), and they never see anything themselves. Ids cycle through `[2, 2_000_000)` skipping ids in use. Expiry is a `BinaryHeap<Reverse<(expire_ms, id)>>` popped at the start of each tick; an entry whose item is gone (or was replaced) is skipped, so no per-tick scan. Pickup and drop follow rAthena's gate order; every refusal sends the failure ack the client needs (`0x0b41` result 6 / `0x00af` count 0). The drop cell uses rAthena's 8-neighbour search with random mirrors (`search_free_cell`), with item stacking on.
* **Equipment (`src/equipment.rs`, `src/job.rs`, `src/wear.rs`, `src/status.rs`, spec `specs/equipment/spec.md`):** `equipment.rs` holds the pure rules (equip point, the `pc_isequip` subset, hand and accessory choice, ammo pairing, looks and gear status of a worn set); `job.rs` the job id → mapid table; `wear.rs` the flows. `PlayerInventory` owns a worn-slot table (`WornSlots`, slot per position in `EQI` order) beside the items' `equip` masks; looks and gear status read the table, so an equip takes its positions one at a time and each displaced item's looks and push already count the new item (the captured interleave). Every equip change bumps the inventory revision. `PlayerInfo.looks` caches `UnitLooks`; entry packets read it, never the inventory, and every equip change refreshes it. Head and robe looks go out only when the resolved view changes (costumes override); hand and shoe looks always. Every equip change marks the player dirty with a stale gear status; the tick's stat pass sends one net diff after all the acks (§5.1). Ammo: the icon `0x0983` is started on equip (area) and at every actor init (self, between the inventory lists), never ended; a ranged weapon's take-off and a weapon the ammo can't pair with take the ammo off. A dropped worn stack comes off first. Unequip-all while dead is refused on purpose (rAthena allows it; spec §2.5).
* **Entry settings (`src/settings.rs`, spec `specs/map-entry-settings/spec.md`):** `PlayerInfo` carries rAthena's `connect_new` (set by `spawn_character`) and `changemap` (set by every spawn, so login and cross-map arrival, not by a same-map warp). The `NotifyActorInit` that shows the player sends, after the unit entries, the skill list and both hotkey banks if `connect_new`, then `0x02c9`, `0x02da`, `0x02d9` ×4 and `0x0b8d` if `changemap`, and clears both. All defaults; nothing is saved. `BlockingPlayCancel` (`0x0447`) answers `LoadConfirmed` (`0x0b1b`), which unblocks the client's input. `spawn_character` returns the login burst instead of queuing it, so the map-server can send it right after `ZC_ACCEPT_ENTER` and the friends list: the MOTD, `0x0091` to the player's own cell, the status push, `0x09e7`, `0x0a24` and `0x0ade`. The client reloads the map on that `0x0091` and sends `0x007d` twice: the first runs the actor init and the settings, the second only re-sends the inventory. The client sends `0x0447` only after a load that a `0x0091` started. Every actor init that shows the player starts with `0x01d7` (weapon look, self only) and sends `0x099b` after the spawn to others.
* **Event routing:** AoI events are resolved to recipients **at emission** (`BroadcastScope::Characters`, sorted), like rAthena's `clif_send(AREA)`; per-observer visibility events are `SelfOnly`. Resolving after the tick would double-send entries to units that come into view later in the same tick.
* **Not implemented yet:** item use, item scripts, looter mobs, modifier sources of §9.3 beyond the Super Novice class extra (item bonuses, passives), status effects (§10), mob AI, mob movement, death and respawn, combat (§11), script portals, warp skills, `rayon` parallelism, sub-cell redirect offsets, `OFFICIAL_WALKPATH`.

---

## 2. Memory Layout in `World`: Hybrid Dense Columns & Sparse Sets

Because `World` has a known, fixed domain schema at compile time, it avoids the dynamic type erasure (`TypeId`, raw `BlobVec` byte buffers, and runtime borrow tracking) of general-purpose archetype ECS frameworks like `bevy_ecs`. Instead, `World` organizes component data into **three explicit storage tiers** where the Rust compiler itself enforces disjoint borrows across systems:

~~~rust
pub struct World {
    pub map_name: String,
    pub tick: ServerTick,                // Monotonic tick counter + now_ms
    pub entities: EntityAllocator,       // 64-bit Generational Entity allocator
    pub spatial_grid: SpatialGrid8x8,    // 8x8 cell blocks -> SmallVec<[Entity; 8]>

    // --- TIER 1: Universal Dense Arrays (Indexed directly by Entity::index()) ---
    pub positions: Vec<MapPosition>,     // Logical (x, y, dir) cell on the map
    pub kinds: Vec<EntityKind>,          // Player, Monster, Npc, Portal, SkillUnit, FloorItem

    // --- TIER 2: Combat Actor SparseSet<T> Columns (model and rules: §9) ---
    pub base_stats: SparseSet<BaseStats>,              // Players: allocated STR..LUK, levels, job, points (persisted). Mobs: mob_db row
    pub gear: SparseSet<GearStatus>,                   // Per-hand weapon records (ATK, refine, MATK, wlv, range, type), hard DEF/MDEF
    pub base_bonus: SparseSet<BonusTotals>,            // Summed modifiers of the base layer: gear bonuses, cards, passive skills
    pub battle_bonus: SparseSet<BonusTotals>,          // Summed modifiers of active statuses (the battle layer)
    pub combat_bonus: SparseSet<CombatBonus>,          // Hit-time tables: race/size/element %, ignore-DEF, crit ATK (§11)
    pub status: SparseSet<Status>,                     // Computed battle status: the snapshot every system reads
    pub vitals: SparseSet<Vitals>,                     // Current HP, SP (AP later)
    pub active_statuses: SparseSet<ActiveStatuses>,    // FixedBitSet (presence) + SmallVec<[StatusEntry; 16]> (§10)
    pub stat_dirty: DirtySet,                          // Actors whose status must be rebuilt this tick, + which sums are stale

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

`ro-engine` deliberately avoids heavy external ECS or pathfinding frameworks in favor of six focused crates and three small custom primitives.

### 3.1 External Libraries Used Inside `ro-engine`

| Crate | Where It Fits in `ro-engine` | Why It Fits Our Model |
| :--- | :--- | :--- |
| **`smallvec`** *(features: `union`, `const_generics`)* | • `SpatialGrid8x8` cell blocks (`SmallVec<[Entity; 8]>`)<br>• `ActiveStatuses.entries` (`SmallVec<[StatusEntry; 16]>`)<br>• `WalkingPath.waypoints` (`SmallVec<[(u16, u16); 32]>`)\br>• `instant_intents` (`SmallVec<[InstantIntent; 4]>`) | Keeps 99% of spatial block lists, status arrays, intent queues, and 32-step walk paths inline on the stack/struct without heap allocations during 50 Hz ticks, while spilling safely to the heap if 40 entities stack on one cell. |
| **`fixedbitset`** | • `ActiveStatuses.bitmask` (`FixedBitSet`) on every combat actor | Ragnarok Online has 600+ status effects (`SC_*`). `FixedBitSet` packs 640 status presence flags into ten `u64` words (80 bytes), giving Stage 1 and Stage 3 systems $O(1)$ `"is frozen/stunned?"` checks without archetype table moves. |
| **`bitflags`** | • `StaleSums` (`GEAR \| PASSIVES \| STATUSES`) in the dirty set (§9.5)<br>• `StateFlags` (cant move / attack / cast ...) in `Status`, `CombatLockFlags` | Strongly-typed bitmasks: the recalc pass re-sums only the modifier sources that changed, and gates read one bitfield. |
| **`rustc-hash`** (`FxHashMap` / `FxHashSet`) | • `char_to_entity` and `gid_to_entity` lookup tables in `World`<br>• Rare, low-cardinality state (<50 instances per map, such as open trade windows or chat rooms) | Replaces Rust's default cryptographic SipHash with a fast single-multiply hash tailored for integer keys (`u32`, `u64`, `CharId`, `Entity`), resolving network IDs in nanoseconds. |
| **`rayon`** *(future phase)* | • Inter-map parallel `World::step_tick()` execution<br>• Intra-map parallel iteration in Stage 1 (`par_iter_mut` over dirty stats), Stage 2b (Mob AI), Stage 3 (`par_iter().fold()` into thread-local `Vec<EngineEvent>` buffers), and Stage 5 (AoI gather) | Work-stealing data parallelism that pairs directly with contiguous `SparseSet<T>` dense slices (`&[T]` and `&mut [T]`) and read-only `&World` simulation passes. |
| **`tracing`** | • Structured tick instrumentation, unhandled intent diagnostics, and slow-tick warnings | Zero-allocation structured logging compatible with async and multi-threaded `rayon` spans. |

~~~toml
# crates/ro-engine/Cargo.toml dependencies
[dependencies]
ro-core.workspace = true
ro-db.workspace = true
smallvec.workspace = true
fixedbitset.workspace = true
rustc-hash.workspace = true
bitflags.workspace = true
rayon.workspace = true
tracing.workspace = true
~~~

### 3.2 Custom Primitives Written Inside `ro-engine` (Why We Don't Use `slotmap` or `pathfinding`)
1. **`Entity` & `EntityAllocator` (~40 lines of safe Rust):**
   * Packs a `u32` slot index and `u32` generation counter into a 64-bit `Entity` handle.
   * Exposes `Entity::index() -> usize` directly so Tier 1 universal columns (`positions`, `kinds`) can be indexed via a single bounds-checked array read (`world.positions[entity.index()]`) after verifying generation liveliness.
2. **`SparseSet<T>` (~60 lines of safe Rust) vs. `slotmap::SecondaryMap`:**
   * Consists of three contiguous vectors: `sparse: Vec<Option<u32>>`, `dense_entities: Vec<Entity>`, and `dense_data: Vec<T>`.
   * Unlike third-party arena maps, our `SparseSet<T>` exposes `(dense_entities: &[Entity], dense_data: &mut [T])` as raw contiguous slices, allowing `rayon`'s `.par_iter_mut()` to split component arrays across CPU cores in Stage 1 with zero wrapper overhead.
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
   * **Variants:** `ChangeDirection { head_dir, body_dir }`, `RequestEntityName { target }`, `RequestTime { client_tick }`.

---

## 5. The 5-Stage Deterministic Tick Schedule

> **Future phase.** The foundation runs a reduced, single-threaded version of this schedule (see §1a). Mob AI, combat, status effects and all `rayon` usage are not implemented. The stats feature (`specs/stats/`) adds the Stage 3 recalc pass and the Stage 4 follow-ups.
>
> **Revised 2026-10-25** (stats planning, settled with the user). The earlier draft recalculated status at the *start* of a tick, so a change made during tick N showed up in tick N+1 and HP/SP clamps used the old maximum. The recalc now runs after all of the tick's mutations and before HP/SP are committed, so a change, its stat push and its follow-ups (heal to the new max after a level-up, clamp when the max drops) land in the same tick. All simulation still reads one frozen snapshot: the `status` committed at the end of the previous tick.

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
│ STAGE 1: TIMERS, INTENTS & HITS          (reads last tick's `status` only)   │
│  • Timers: floor-item expiry, status expiry/periodic ticks, walk steps       │
│  • Drain ClientIntents (move, equip, stat-up, GM commands, attack, cast)     │
│  • Mob AI sensors ──► mob intents                                            │
│  • Pathfinding, cell steps, portal hits                                      │
│  • Swings/skill hits resolve against the snapshot ──► HitResolved events     │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 2: EFFECTS ──► MODIFIERS                                               │
│  • Scripts, procs, skill effects start/end statuses, change gear/levels      │
│  • Every change that can move a stat marks the actor dirty (+ stale sums)    │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 3: RECALC (dirty actors only; pure, parallel-friendly)                 │
│  • Re-sum stale modifier sources ──► base_bonus / battle_bonus               │
│  • ro_mechanics::calc_status: primary (base ──► battle) ──► secondary        │
│  • Diff against the stored `status` ──► stat pushes; store the new snapshot  │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 4: COMMIT VITALS & FOLLOW-UPS (single-threaded)                        │
│  • Fold HP/SP deltas in event order, clamp to the NEW max, deaths            │
│  • Follow-ups registered by Stage 1–2 changes (heal to max after level-up)   │
└──────────────────────────────────────┬───────────────────────────────────────┘
                                       ▼
┌──────────────────────────────────────────────────────────────────────────────┐
│ STAGE 5: SERVER EVENT DISPATCH (AoI routing, multicast & singlecast)         │
│  • Emit ServerEvent { scope, payload }                                       │
└──────────────────────────────────────────────────────────────────────────────┘
~~~

### 5.1 Stage-by-Stage Execution Rules
* **Stage 1 (Timers, Intents & Hits):** Pops due timers, drains inbound `ClientIntent` items in FIFO order (equip, stat-up and GM commands run their flows here and only *mark* the actor dirty), runs mob AI sensors, plans paths, steps walkers and checks portals. Attacks and skill hits compute their `DamageResult` from the `status` snapshot committed last tick (§11) and emit `HitResolved` events; they never mutate HP directly. With `rayon`, the read-only parts run as `&World` passes folding into thread-local event buffers.
* **Stage 2 (Effects → Modifiers):** Applies what Stage 1 decided: status starts and ends (§10), proc rolls, script world commands, gear and level changes. Anything that can move a stat marks the actor in `stat_dirty` together with which modifier sums are stale (`GEAR`, `PASSIVES`, `STATUSES`).
* **Stage 3 (Recalc):** For each dirty actor: re-sum only the stale sources, call the pure `ro_mechanics::calc_status` (§9.4), diff the result against the stored snapshot and push what changed in rAthena's order (§9.6), then store it. **Recalc never runs inline inside a flow.** The only exception is spawn/login, where the first snapshot is computed synchronously with nothing to diff against. A pure function over `&[Entity]` makes this the natural `par_iter` pass later.
* **Follow-up queue (implemented, `specs/stats/spec.md` §2.1):** a flow sends at once what rAthena sends before its recalc, and queues per player what it sends after (`FollowUp`: a ready packet to self or area, heal to max, save). Per player the pass emits `SP_MAXWEIGHT`, clamps, the diff, then that player's follow-ups in queue order, so the captured wire orders (stat-up, `@blvl`, `@job`) come out unchanged.
* **Stage 4 (Commit Vitals & Follow-ups):** Single-threaded reduction. Folds HP/SP deltas from `HitResolved`, heals and drains **in event order** (deterministic), clamps each to the max computed in Stage 3, records deaths, and consumes stateful defences in order (barriers, hit-count walls, §11.4). Then it runs the follow-ups that Stage 1–2 changes registered (level-up: full heal to the new MaxHP/MaxSP; max dropped: clamp and push `SP_HP`/`SP_SP`). Swings already resolved against a target that dies in this tick still show their animation, and their damage counts as overkill (rAthena's damage log counts overkill too).
* **Stage 5 (Server Event Dispatch):** Emits `ServerEvent` messages: singlecast stat pushes (`0x00b0`, `0x0141`, `0x013a`) and AoI multicast visuals (`ZC_NOTIFY_PLAYERMOVE`, `ZC_NOTIFY_ACT`, `0x0983`).

**Deliberate packet-order deviation.** rAthena recalculates inline, so a captured equip sequence interleaves each displaced item's stat diff between the acks (catalog `packets/equipment.md` "Captured equip sequences"). Here one net diff follows all of the tick's acks. Unequip-all of N items sends one diff instead of N. The client is expected to accept this because every push is an independent absolute value. A manual client check confirms it (`specs/stats/spec.md`).

---

## 6. Deep Dive: Click-to-Move, A* Pathfinding & Speed-Based Position Updates

In *Ragnarok Online*, movement is **hybrid predictive**: the server does *not* broadcast a packet on every single tile step. When an entity begins moving (or changes direction), the server calculates the full path, validates walkability, and broadcasts the **start cell, destination cell, sub-cell offset, and start timestamp** (`WBUFPOS2`). Both the client and `World` then step the entity along the identical A* path at the entity's `move_speed`.

### 6.1 Move Speed, Step Duration & Sub-Cell Coordinates
1. **`move_speed` Definition (`Status.speed: u16`, §9):**
   * Expressed in **milliseconds required to cross 1 orthogonal cell**.
   * Default player walk speed is `150` (`150ms` per orthogonal tile, or `6.67` tiles/sec). Buffs like *Increase AGI* or Pecopeco mounts lower this value (e.g., `112ms`).
2. **Orthogonal vs. Diagonal Step Timing:**
   * **Orthogonal Step ($\Delta x + \Delta y = 1$):** Takes `step_duration_ms = move_speed as u64` (e.g., `150ms`).
   * **Diagonal Step ($\Delta x = 1 \land \Delta y = 1$):** Scaled by $\approx \sqrt{2}$ (`14 / 10` in Aegis/rAthena integer math):
     `step_duration_ms = (move_speed as u64 * 14) / 10` (e.g., `210ms` at default speed).
3. **Why Server Tick (`20ms`) is Finer Than `move_speed` (`150ms`):**
   * An entity does **not** change its `(x, y)` grid coordinate on every 20ms tick.
   * Instead, `WalkingPath` records `step_started_at_ms` and `next_step_at_ms`. On ticks where `now_ms < next_step_at_ms`, the entity is in transit between cells.
   * **Mid-Stride Redirection (`sx0, sy0` in `WBUFPOS2`):** If a player is 50% of the way from `(100, 100)` to `(101, 100)` and clicks a new destination `(101, 105)`, (future phase; the foundation always sends (8, 8)) `ro-engine` computes the 4-bit sub-cell fraction (`0..15`, where `8` is the cell center) from the elapsed step time. Passing `(sx0, sy0)` in `ServerEventPayload::PlayerMovementStarted` tells the client to smoothly pivot from its exact sub-tile pixel position without rubber-banding!

### 6.2 RO-Compliant A* Pathfinding Rules
When `intent_to_pathfinding_system` queries the map's `MapGrid`:
* **Max Path Length:** `MAX_WALKPATH = 32` waypoints.
* **Step Costs:** Orthogonal neighbor cost = `10`; Diagonal neighbor cost = `14`.
* **Heuristic:** Manhattan distance x 10 (`10 * (dx + dy)`), as rAthena does (deliberately inadmissible; see `reference/catalog/mechanics/movement_aoi.md`).
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
    walkability: &MapGrid,
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
            .status
            .get(entity)
            .map(|s| s.speed)
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
            .status
            .get(entity)
            .map(|s| s.speed)
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
1. **`MovementStarted`:** Installs the new `WalkingPath` into `world.walking_paths` and pushes `ServerEventPayload::PlayerMovementStarted` (to self) and `ServerEventPayload::ActorMovementStarted` (to AoI observers).
2. **`EntitySteppedCell`:**
   * Updates `world.positions[entity.index()] = to`.
   * If `from.block_coords() != to.block_coords()` (where a block is `x / 8, y / 8`), moves `entity` in `world.spatial_grid` and computes the **Area-of-Interest (AoI) Set Difference**:
     * **Entering Blocks:** New observers entering visual range receive `ServerEventPayload::ActorMovementStarted` (`ZC_NOTIFY_MOVEENTRY`) if the entity is still walking, or `ServerEventPayload::ActorSpawnedOrEnteredAoI` (`ZC_NOTIFY_STANDENTRY`) if it finished its path on that cell.
     * **Leaving Blocks:** Old observers falling out of visual range receive `ServerEventPayload::ActorVanished` (`ZC_NOTIFY_VANISH`).
   * Queries the new cell in `world.spatial_grid` for overlapping `EntityKind::Portal` or `OnTouch` NPC trigger boxes, emitting `ServerEventPayload::PlayerMapMove` (`ZC_NPCACK_MAPMOVE`) or `ScriptTriggerEvent::TouchNpc` if triggered.

---

## 7. `ServerEvent` Contract & Outbound `ZC_` Packet Translation

Systems inside `ro-engine` never construct raw byte buffers or touch TCP sockets. At the end of Stage 4 / Stage 5, `World` emits high-level `ServerEvent` structs:

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
    /// Wire Packet: `ZC_NOTIFY_MOVEENTRY11` (`0x09FD`). `0x0086` is never sent at 20260219.
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

    /// Sent to `AreaOfInterest` when a status effect starts or expires (Stage 2, §10).
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
    /// Sent to `SelfOnly(char_id)` by a same-map warp. A cross-map warp emits nothing to self:
    /// it queues a `MapTransfer`, and the map-server sends this packet from the destination
    /// map after `enter_by_warp` resolves the cell.
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
| `PlayerMovementStarted` | `SelfOnly(char_id)` | `ZC_NOTIFY_PLAYERMOVE` (`0x0087`) | Stage 4 (from Stage 3a A* Path) |
| `ActorMovementStarted` | `AreaOfInterest` *(excl. self)* | `ZC_NOTIFY_MOVEENTRY11` (`0x09fd`; never `0x0086`) | Stage 4 (Path start or AoI enter) |
| `ActorSpawnedOrEnteredAoI` | `AreaOfInterest` / `SelfOnly` | `ZC_NOTIFY_STANDENTRY` / `ZC_NOTIFY_NEWENTRY` | Stage 4 (AoI block diff or Spawn) |
| `ActorVanished` | `AreaOfInterest` | `ZC_NOTIFY_VANISH` (`0x0080`) | Stage 4 (AoI leave, Death, Warp) |
| `ActorStoppedAt` | `AreaOfInterest` | `ZC_STOPMOVE` (`0x0088`) | Stage 4 (Knockback / Immobilize) |
| `PlayerParamsUpdated` | `SelfOnly(char_id)` | `ZC_PAR_CHANGE` (`0x00B0`) | Stage 3 (recalc diff) / Stage 4 (HP/SP commit) |
| `ActorActionPerformed` | `AreaOfInterest` | `ZC_NOTIFY_ACT` (`0x008A` / `0x08C8`) | Stage 5 (Discrete Combat/Sit/Stand) |
| `StatusStateChanged` | `AreaOfInterest` | `ZC_MSG_STATE_CHANGE2` (`0x0983`) | Stage 2 (Status Applied / Expired) |
| `PlayerMapMove` | `SelfOnly(char_id)` | `ZC_NPCACK_MAPMOVE` (`0x0091`) | Stage 4 (Portal collision) |

---

## 9. Stats: Sources, Modifiers & Recalculation

> **Status:** design settled with the user on 2026-10-25. The core player part is planned in `specs/stats/` (job data, every derived stat, refine, stat-up, `@blvl`/`@jlvl`/`@job`), and trait stats, AP, the stat-point grants and `pc_resetstate` in `specs/traits/`; item bonuses, passive skills, status effects (§10) and damage (§11) are later plans that plug into the same model. rAthena is the behavioural reference (catalog `mechanics/stats.md`, `packets/status.md`, `db/job_stats.md`, `db/refine.md`, `mechanics/battle_config.md`); the structure is ours.

### 9.1 Why not rAthena's shape, and why not the old Tier 2 sketch
rAthena's `status_calc_pc` rebuilds a player's whole status from zero on every change, then `status_calc_bl_main` layers status effects on top per `SCB_*` flag, and pushes a diff. Its layers are not clean "flat + percent" tiers:
* Max weight reads **allocated** STR only (no job bonus, no gear, no buffs).
* MaxHP scales the base table by **battle** VIT (buffs included) and then adds **equipment-only** VIT flat on top.
* MaxHP has several separate percent pools applied in a fixed order (equip %, item %, `hp_rate`), so one generic `pct` per stat cannot reproduce it.
* Item scripts run in the middle of the rebuild and can read the stats accumulated so far.

So the earlier `equip_mods` / `status_mods` "flat + %" tiers are dropped. What we keep from rAthena is the **full rebuild** (no incremental math on stored values, so nothing drifts) and the **two layers** (base, battle). What we add is ECS-shaped: every stat effect is plain data with a source, the summed data is cached per source kind, and the rebuild is a pure function.

### 9.2 Inputs (all read-only at runtime)
| Input | Where it comes from | Holds |
| :--- | :--- | :--- |
| `BaseStats` (component) | players: `characters` row via `CharacterSpawn` (persisted); mobs: `MobDb` row at spawn | allocated STR..LUK and POW..CRT, base/job level, job id, status/trait/skill points, current AP (persisted; MaxAP is derived) |
| Job row | `ro_db::JobDb` ← `tools/job-import` ← rAthena `job_stats`/`job_basepoints`/`job_aspd`/`job_exp`/`statpoint` (GPL data under `crates/ro-db/data/job/`) | cumulative job-level bonus row (stats and traits), base HP/SP/AP tables, ASPD base per weapon + shield, MaxWeight, max levels, status- and trait-point tables |
| Refine table | `ro_db::RefineDb` ← `tools/refine-import` (GPL data under `crates/ro-db/data/refine/`) | cumulative bonus (1/100) and overrefine per group, item level and refine |
| `GearStatus` (component) | worn items (`ItemDb` fields + instance refine) | per-hand weapon records (`atk`, `atk2` refine, `matk`, `wlv`, `overrefine`, `range`, weapon type), hard DEF and hard MDEF, ammo |
| `base_bonus: BonusTotals` | Σ modifiers of gear bonuses, cards, combos, passive skills | named slots (§9.3) |
| `battle_bonus: BonusTotals` | Σ modifiers of active statuses (§10) | named slots |
| `BattleConfig` | our own YAML (MIT, rAthena default values), `BATTLE_CONFIG` path, unset = defaults | caps, rates, enable flags (catalog `mechanics/battle_config.md`) |

`ro-mechanics` never sees `ro-db` types: `ro-engine` hands it plain borrowed views (`&JobRow`-shaped structs) so the formulas stay pure and the crate stays dependency-free.

### 9.3 Modifiers: named slots with a source
A **modifier** is `{ slot, value }` and always belongs to a **source**: a worn item (its compiled bonuses, §9.8), a card or combo, a learned passive skill, or an active status effect (§10). Slots are named after rAthena's `b*` bonus types, because each one enters its formula at a specific place:

* **Primary:** `Str`, `Agi`, `Vit`, `Int`, `Dex`, `Luk` (and `AllStats` expanded at compile time).
* **Secondary flat:** `BaseAtk`, `Atk` (equip ATK, `eatk`), `Matk`, `Def`, `Def2`, `Mdef`, `Mdef2`, `Hit`, `Flee`, `Flee2`, `Critical`, `MaxHp`, `MaxSp`, `AspdFixed`, `AspdAdd` (amotion), `MaxWeight`, `Range`.
* **Secondary percent pools, each kept separate:** `AtkRate`, `MatkRate`, `DefRate`, `MdefRate`, `HitRate`, `FleeRate`, `CriticalRate`, `MaxHpRate` (equip), `MaxHpRateItem`, `MaxSpRate`, `MaxSpRateItem`, `AspdRate`, `SpeedRate`, `HpRecovRate`, `SpRecovRate`.
* **Hit-time tables** (`AddRace`, `AddSize`, `AddEle`, `SubRace`, `SubEle`, `IgnoreDefRace`, `CritAtkRate`, ...) are not stats: they live in `CombatBonus` and are read only when damage is computed (§11).

Each slot has an **aggregation rule**: most `Sum`, some `Max`/`Min` (rAthena takes the strongest movement-speed bonus and the strongest penalty rather than summing them). `BonusTotals` is a fixed array indexed by slot (`[i32; SLOT_COUNT]`), cache-friendly and allocation-free.

### 9.4 Layers and the recalc function
`ro_mechanics::calc_status(&StatusInputs, &BattleConfig) -> Status` is pure: no allocation, no RNG, no world access. In dependency order:

1. **Base primary** = job bonus row[job level] + allocated + `base_bonus` primary slots (+ fixed class extras such as Super Novice's +10).
2. **Battle primary** = base primary + `battle_bonus` primary slots, clamped. `0x0141` sends `base = allocated`, `bonus = battle − allocated`.
3. **Secondary**, each formula reading the layer rAthena reads:

| Secondary | Reads |
| :--- | :--- |
| Max weight | job MaxWeight + **allocated** STR × 300 + `MaxWeight` slots |
| Status ATK (`batk`) | battle STR/DEX/LUK (swapped for ranged weapons) + base level + `BaseAtk` |
| Weapon ATK (`SP_ATK2`) | `GearStatus` hands (`atk` + refine `atk2`) + `Atk` slots |
| Status MATK / weapon MATK | battle INT/DEX/LUK + level; weapon MATK + refine (no refine MATK on bows) + `Matk` |
| Soft DEF / soft MDEF | battle VIT/AGI (INT/DEX/VIT) + level + `Def2`/`Mdef2` (`float` math) |
| Hard DEF / hard MDEF | `GearStatus` hard DEF/MDEF + `(Σ refine + 50)/100` + `Def`/`Mdef`, then rates |
| HIT, FLEE, CRIT, perfect dodge | battle DEX/AGI/LUK + level + flat slots, then the rate slots |
| ASPD (amotion) | job ASPD base for the right weapon (+ shield or 1/4 left weapon) + battle AGI/DEX + `AspdFixed` + `AspdRate` + `AspdAdd`, caps from config |
| MaxHP / MaxSP | job base table[level] × (1 + battle VIT/INT %) × class factor + **equipment-only** VIT/INT flat + flat slots, then each % pool in order, then config rate and caps |
| Regen amounts, speed | battle VIT/INT + MaxHP/SP; base speed + `SpeedRate` (Max/Min aggregation) |
| `StateFlags` | OR of the behaviour flags of active statuses (§10) |
| Traits (POW..CRT) | job trait bonus row + allocated (base), + battle slots later; cap `max_trait_parameter` for a trait job, else 0. Battle POW adds 5 × POW to `batk`, battle CON adds 2 × CON to HIT and FLEE |
| PATK, SMATK, RES, MRES, HPLUS, CRATE | battle traits only, integer division (`specs/traits/spec.md` §2.2). Nothing consumes them until damage (§11) |
| MaxAP | job `base_ap`[level] × `ap_rate`, capped at `max_ap`. No trait-job guard: 3rd classes get the fallback table from B200, as in rAthena |

**Precision:** where rAthena computes in `float` or `double` and truncates once, `ro-mechanics` uses the same C type, because the truncation edge differs (soft DEF/MDEF, the ASPD square root, the MaxHP percentages). Tests pin the catalog's worked values (bare level-1 Novice: ASPD 156, amotion 440).

### 9.5 Dirty tracking and caching
`DirtySet` holds the actors to rebuild this tick, plus `StaleSums` bits per actor:
* `GEAR`: equip, unequip, refine or card change → rebuild `GearStatus` and the gear part of `base_bonus`.
* `PASSIVES`: skill learned or reset, or a weapon change for weapon-conditional passives → rebuild the passive part of `base_bonus`.
* `STATUSES`: a status started, ended or changed its vals → rebuild `battle_bonus` and `StateFlags`.
* No bit, only dirty: allocated stats, levels or job changed → the cached sums are still valid, only `calc_status` runs.
* `SKILLS` (implemented, `stats::SKILLS`; `specs/skills` §2.6): base or job level changed, or a skill reset → the pass recomputes the visible skill tree. If it changed, it sends `0x0441` for each id that left the list (ascending) and one `0x010f`, after `SP_MAXWEIGHT` and before the clamps and the diff, as rAthena's tree diff inside `status_calc_pc`. It can be set without `DIRTY` (then only the tree re-check runs). Skill-up and `@allskill` compute the tree inline instead, so the pass finds no change. `@job` sends its inline first pass at step 8 and marks `SKILLS`, so the pass sends rAthena's second pass (deletes and list) when it differs.

The expensive part, summing the modifiers of 10+ items and their cards, runs only when its source changes. A recalc is otherwise pure arithmetic over a few cached arrays. Each actor is marked at most once per tick, so unequip-all of N items costs one rebuild.

### 9.6 Pushes
After Stage 3 computes the new `Status`, the diff against the stored snapshot emits one packet per changed field in rAthena's fixed order (catalog `packets/status.md` "Recalculation pushes"): `0x0141` STR..LUK, HIT, FLEE1, ASPD, SPEED, ATK1, DEF1/DEF2 (hard), ATK2, DEF2/DEF1 (soft), FLEE2, CRITICAL, MATK2/MATK1, MDEF1/MDEF2, MDEF2/MDEF1, ATTACKRANGE (`0x013a`), MAXHP, MAXSP, HP, SP, then the trait block: `0x0141` POW..CRT, PATK, SMATK, RES, MRES, HPLUS, CRATE, MAXAP, AP (`specs/traits/spec.md` §2.3). A `0x0141` goes out when the battle value **or** the allocated value changed. `SP_WEIGHT`/`SP_MAXWEIGHT` are their own pushes when they change. **Captured order per player** (`specs/stats/spec.md` §2.3): `SP_MAXWEIGHT` if changed, then the HP/SP/AP clamp pushes (`SP_HP`/`SP_SP`/`SP_AP` when a lower max cut them), then the diff (which sends the clamped field again after MAXHP/MAXSP), then the player's follow-ups. Mobs have no push: their HP bar (`0x0977`) belongs to combat.

### 9.7 Where each subsystem enters
| Change | Stage it happens in | Marks | Follow-up (Stage 4) |
| :--- | :--- | :--- | :--- |
| Equip / unequip / refine | 1 (intent) | dirty + `GEAR` | clamp HP/SP if the max dropped |
| Stat-up (`0x00bb`) | 1 (intent; failure `0x00bc` at once) | dirty | queued: `0x00be`, `SP_STATUSPOINT`, `0x00bc` (after the diff, as captured) |
| Base level up (exp, `@blvl`) | 1/2 | dirty | full heal to the **new** MaxHP/MaxSP (positive base change only), effect, the level tail; then `LevelGearCheck` unequips `elvmax` items (their own diff next tick) |
| Trait-up (`0x0b24`) | 1 (intent; failure `0x00bc` at once, negative amount dropped silently) | dirty | queued: `0x00be` `SP_U<trait>`, `SP_TRAITPOINT`, `0x00bc` (value = allocated) |
| Job level / job change | 1/2 | dirty + `SKILLS`; `@job` also sends its first-pass `0x010f` at step 8, and force-unequips in the flow (each unequip marks `GEAR`) | clamp; `@job` queues its looks (area), message and `Save` |
| Stat reset (`pc_resetstate`: `@streset`, `@blvl −n`, `@job`) | 1 | dirty | none of its own: the `0x00be` ×12, `SP_STATUSPOINT`, `SP_TRAITPOINT` go out at once, the derived values come from the diff; `@streset` queues its message |
| Passive skill learned | 1 | dirty + `PASSIVES` (once passives give bonuses; today the skill-up only re-checks the tree inline) | — |
| Status start / end / expiry | 2 (expiry is a Stage 1 timer feeding Stage 2) | dirty + `STATUSES` | clamp |
| Damage, heal, drain | 1 (`HitResolved`) | not dirty (vitals only) | fold, clamp, death |

**`pc_resetstate`** (`reset_stats` in `src/level_command.rs`) reads the class and level **in effect when it runs**: status points = table + 52 (`transcendent_status_points`) for an upper or primary 4th class, trait points = table + 7 (`trait_points_job_change`) for a trait job, stats to 1, traits to 0. `@blvl −n` runs it when either pool can't pay for the levels removed. **`@job` grant steps** (`specs/traits/spec.md` §2.7), in order: the level-cap reset and base tail; ±52 when entering or leaving "upper or primary 4th" (leaving with < 52 points resets instead); `0x00be` ×6; then +7 trait points into a trait job, or, leaving one, zero the pool (allocated traits stay and count with a cap of 0) or reset when < 7. Because each reset already sees the new class, B195 Dragon Knight → Knight resets twice (captured); each reset sends its own immediate pushes, and the derived diff comes once at tick end.

### 9.8 Item bonuses (future plan)
* The item importer **compiles** the common unconditional script lines (`bonus bStr,1;`, `bonus2 bAddRace,RC_DemiHuman,5;`, `bonus bMaxHPrate,5;`, ...) into a declarative `Bonuses:` list on the item. The engine only sums data; no VM runs inside the recalc.
* Scripts the compiler can't express (conditionals on refine, job or other stats; `autobonus`; `skill`-granting) are flagged on the item and left for the scripting VM. A VM-evaluated item becomes one more modifier **source**, re-evaluated on its own triggers and never inside `calc_status`.
* **Hard MDEF ruling:** hard MDEF is a base equipment stat like DEF. The importer lifts unconditional `bonus bMdef,N;` into a first-class `MagicDefense` item field next to `Defense`. Conditional MDEF stays in compiled bonuses. Both feed the same hard-MDEF slot. Until that plan, gear hard MDEF is 0.
* Cards merge their compiled bonuses into the carrier item's source. Combos are matched against the worn set when the `GEAR` sum is rebuilt.
* `EquipScript` / `UnEquipScript` are one-shot events to the scripting VM (Stage 2), not modifiers.

### 9.9 Skills
* **Learned set (implemented, `specs/skills`; `src/skills.rs`):** each player has a `SkillSet`, a small id-sorted `Vec` of `{id, level, flag, visible}`. `compute_visible` is rAthena's `pc_calc_skilltree`: the tree job (the `player_skillup_limit` downgrade, on the mapid), then a fixed point over that job's resolved tree from `SkillTreeDb`. Hidden slots with a level (from a previous class) stay in memory, count for nothing and aren't saved. Skill effects are placeholders: no casting, no passive bonuses, no `NV_BASIC` gates. The learned set is what the passive sums and casting below will read.
* **Ordering note for passives:** rAthena recalculates on a passive skill-up and sends that stat diff (and a changed tree's deletes and list) *before* `0x010e`. Today the skill-up sends the tree change inline. Once passives give bonuses, the skill-up must mark `PASSIVES` and the diff must still precede `0x010e`, which the end-of-tick pass doesn't do. That is a known deviation to settle then (`DEFERRED.md`).
* **Passive skills** (Owl's Eye DEX+lv, Dragonology INT, Cavalier Mastery's ASPD restore) are modifier sources in the base layer, from a passive-skill table keyed by `(skill, level)` plus an optional weapon-type condition.
* **Weapon masteries** that add ATK only on hit (Sword Mastery) go to `CombatBonus`, not the status.
* **Active buffs** are status effects (§10). **Offensive skills** are damage contexts (§11) with a skill ratio.

---

## 10. Status Effects (future plan)

> Design only. The catalog entry (`mechanics/status.md`) is still a placeholder and must be filled before the status-effect plan.

### 10.1 Data-driven registry
rAthena hard-codes each status change's stat effect in `status_calc_*` switch cases. We describe it as data instead (decided with the user): a **status registry** in our own format, one entry per SC:

~~~yaml
- Status: Blessing
  Icon: EFST_BLESSING            # client icon (0x0983 / 0x0196)
  Modifiers:                     # slot <- a*valN + b, summed into battle_bonus
    - { Slot: Str, From: Val1 }
    - { Slot: Int, From: Val1 }
    - { Slot: Dex, From: Val1 }
    - { Slot: Hit, From: Val2 }
  Flags: []                      # NoMove, NoAttack, NoCast, NoPickup, NoRegen, NoEquip, ... -> StateFlags
  EndOn: [Death]                 # Death, Hit, Logout, MapChange, Dispel, ...
  Cancels: [Curse, Stone]        # statuses removed on start
  Hook: none                     # optional named Rust hook for behaviour data can't express
~~~

* The registry's **format and the modifier definitions are ours**. They are written from catalogued facts (which stat, what value), which the licensing rules allow. Anything copied wholesale from rAthena's `status.yml` (icons, flags, durations) comes in only through a `tools/status-import` into `crates/ro-db/data/status/`, like every other GPL dataset. The status-effect plan settles the split.
* Odd cases get a small named Rust hook (Kyrie's barrier, Hiding's visibility, Poison's periodic damage) rather than a per-SC `match` over hundreds of statuses.
* Forking a mechanic means editing data, not code.

### 10.2 Runtime
* `ActiveStatuses` per actor: a `FixedBitSet` for O(1) "has SC?" checks, plus `SmallVec<[StatusEntry; 16]>` holding `{ id, val1..val4, started_ms, expires_ms, next_tick_ms, source }`.
* **Timers:** one per-map `BinaryHeap<Reverse<(due_ms, entity, sc, generation)>>`, popped in Stage 1 like floor-item expiry. Entries that no longer match (status refreshed or removed) are skipped, so there's no per-tick scan.
* **Start, refresh and end** happen in Stage 2. Each one marks the actor dirty + `STATUSES` and emits the icon events (`0x0983` start, `0x0196` end) in the order rAthena sends them.
* **Periodic effects** (poison, bleeding, regen-style buffs) emit HP/SP deltas at their tick. Stage 4 folds them with everything else.
* **Resistance and immunity** are checked at start (a pure function of the target's `Status` + `StateFlags` + level) in `ro-mechanics`.
* `StateFlags` lives in `Status`, so every gate (move, attack, cast, pick up, equip, regen) reads one bitfield of the committed snapshot.
* **Mobs** use the same components. A mob's `Status` is computed once at spawn from its `MobDb` row and rebuilt only when its statuses change.

---

## 11. Damage Pipeline: Outgoing & Incoming (future plan)

> Design only. Formulas: catalog `mechanics/combat.md` (normal attack, renewal) and `mechanics/auto_attack.md` (timing). Magic, skills and monster-vs-player are not yet catalogued.

### 11.1 Shape
Damage is a **pure function** in `ro-mechanics`:

~~~text
calc_damage(attacker: AttackerView, defender: DefenderView, ctx: AttackContext, rng: &mut Rng) -> DamageResult

AttackerView  = &Status (battle) + GearStatus hands + CombatBonus (offensive tables) + level, size/race/element
DefenderView  = &Status (battle) + CombatBonus (defensive tables) + size, race, element & level, StateFlags
AttackContext = kind (Weapon | Magic | Misc), skill id/level/ratio, element override, hit count (div),
                range class (short/long), flags (no-crit, ignore-flee, ignore-DEF ...)
DamageResult  = { damage, damage2 (left hand), div, type (normal/crit/miss/lucky/multi), flags }
~~~

Both views are borrowed from the snapshot committed last tick, so every hit in a tick sees the same world. Damage then doesn't depend on processing order, and a `rayon` fold can compute it.

### 11.2 Outgoing side (attacker), normal weapon attack, renewal order
1. Lucky dodge (defender `flee2`), multi-hit roll (Double Attack), crit roll (attacker `cri` − 2×defender LUK; a crit always hits), hit roll (`clamp(hit − flee, min_hitrate, max_hitrate)`).
2. Parts:
   * status ATK ×2 (neutral element)
   * weapon ATK: variance from the item ATK and weapon level, plus the refine `atk2`, plus overrefine `rnd()%over + 1`; size fix, weapon element
   * equip ATK
   * percent ATK (`AtkRate` slots)
   * the attacker's **offensive** `CombatBonus` (race, size and element %) applied to the weapon and equip parts before they are summed
3. P.ATK multiplier, mastery ATK (passives in `CombatBonus`), crit ATK rate, skill ratio.

### 11.3 Incoming side (defender)
1. Res.
2. Hard DEF `(4000 + def)/(4000 + 10·def)`, then soft DEF subtraction.
3. Minimum 1 on a landed hit.
4. Crit ×(1.4 + C.RATE).
5. The defender's **defensive** `CombatBonus` (`SubRace`/`SubEle`/`SubSize` %), then status-based final reductions (damage-reduction statuses), then `div`.
6. Magic: MATK variance, then the renewal MDEF formula on hard and soft MDEF.

The order is rAthena's (`battle_calc_weapon_attack`); the structure is a fixed sequence of small pure steps, each unit-testable against catalog values.

### 11.4 What is not pure, and where it goes
* **Stateful defences** (Kyrie's barrier HP, Safety Wall's hit count, Energy Coat's SP drain) consume state. `calc_damage` returns the raw result. In Stage 4 the fold walks hits in event order and lets each defence consume its charge before HP is reduced, deterministically.
* **Reactions** (reflect, autospell, status-on-hit procs, auto-guard) are rolled when the hit resolves (Stage 1, from the snapshot) and emitted as events. Reflect becomes a reverse HP delta in the same Stage 4 fold. Procs start statuses in Stage 2 of the **next** tick, or cast through the normal skill path, so a proc can never re-enter the tick that produced it.
* **HP application:** `HitResolved` events carry the result. Stage 4 folds them in order, clamps at 0, records the kill and the damage log (overkill counted), and emits `0x08c8` timing as in `mechanics/auto_attack.md`. A target already dead in the fold ignores later deltas but keeps their animation.

### 11.5 Mobs and players are one path
Mob attackers and defenders use the same `Status`. They differ in data (mob_db `Attack` as weapon ATK with ±20 % variance, `batk = STR + level`, no crit by default via `enable_critical`, `flee2` 0 via `enable_perfect_flee`), so `BattleConfig` and the mob's `Status` decide what differs, not separate code paths.
