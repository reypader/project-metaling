# Project Metaling — Custom ECS Engine Design (`crates/engine`)

## 1. Architectural Overview & Design Goals
`crates/engine` is a deterministic, lock-free, data-oriented Map Server runtime for *Ragnarok Online*[cite: 1, 2]. It replaces legacy eAthena/rAthena `struct block_list` pointer chains and `map_session_data` God Objects with:
1. **64-bit Generational `Entity` IDs** (`u32` index + `u32` generation) to eliminate use-after-free bugs during Area-of-Interest (AoI) iteration[cite: 1, 2].
2. **Hybrid SoA + `SparseSet<T>` Storage** tailored to fixed RO entity categories and high-frequency transient states[cite: 1, 2].
3. **A 5-Stage Tick Pipeline** where simulation systems execute pure read-only queries (`&MapWorld`), emit buffered `EngineEvent` values, and collapse mutations into a single-threaded net delta reduction pass at the end of the tick[cite: 1, 2].

---

## 2. Memory Layout: Hybrid Dense Columns & Sparse Sets

Rather than using type-erased dynamic archetype tables (`BlobVec`) that thrash when components are rapidly toggled, each `MapWorld` uses **three storage tiers**[cite: 2]:

### Tier 1: Universal Dense Arrays (Indexed by `Entity::index()`)
Present on **every** active entity on the map (Players, Monsters, NPCs, Portals, SkillUnits, FloorItems)[cite: 1, 2]:
* `positions: Vec<MapPosition>` — Current `(x, y, dir)` on the map[cite: 1, 2].
* `kinds: Vec<EntityKind>` — Ontological tag (`Player`, `Monster`, `Npc`, `Portal`, `SkillUnit`, `FloorItem`)[cite: 1, 2].
* `spatial_grid: SpatialGrid8x8` — Divides the map into `8x8` cell blocks, each holding `SmallVec<[Entity; 8]>`[cite: 1, 2].

### Tier 2: Combat Actor `SparseSet<T>` Columns
Shared by all combat-capable entities (`Player`, `Monster`, `Homunculus`, `Mercenary`, `Barricade`)[cite: 1, 2]:
* `stat_dirty: SparseSet<StatDirtyFlags>` — Bitmask (`DIRTY_BASE | DIRTY_EQUIP | DIRTY_STATUS`) gating Stage 1 recalculations.
* `base_attrs: SparseSet<BaseAttributes>` — Base/Job level, STR, AGI, VIT, INT, DEX, LUK, base HP/SP[cite: 1].
* `equip_mods: SparseSet<EquipmentModifiers>` — Cached flat and percentage modifiers from equipped items/cards[cite: 1, 2].
* `status_mods: SparseSet<StatusModifiers>` — Cached flat and percentage modifiers aggregated from active statuses[cite: 1].
* `present_attrs: SparseSet<PresentAttributes>` — The authoritative, tick-current snapshot of derived stats (ATK, MATK, DEF, MDEF, HIT, FLEE, ASPD, MoveSpeed, MaxHP, MaxSP)[cite: 1].
* `vitals: SparseSet<Vitals>` — Current HP, SP, and absorb shields[cite: 2].
* `active_statuses: SparseSet<ActiveStatuses>` — Contains a `FixedBitSet` (for $O(1)$ status presence checks) and `SmallVec<[StatusInstance; 16]>` (for `val1..val4` and expiration tick)[cite: 1, 2].

### Tier 3: Transient State & Intent `SparseSet<T>` Columns
Attached and removed in $O(1)$ time without moving entity memory[cite: 1, 2]:
* `primary_intents: SparseSet<PrimaryActionIntent>` — Mutually exclusive locomotion/combat intent for the current tick.
* `instant_intents: SparseSet<SmallVec<[InstantIntent; 4]>>` — Non-blocking item/equipment intents for the current tick.
* `walking_paths: SparseSet<WalkingPath>` — Active A* waypoint buffer and sub-tile step timer[cite: 1, 2].
* `active_casts: SparseSet<ActiveCast>` — Active skill cast timer, target, and interruptible flag[cite: 1, 2].
* `combat_locks: SparseSet<CombatLocks>` — Expiration ticks for `amotion` (attack delay), `dmotion` (flinch/damage delay), and `acd` (after-cast delay).

---

## 3. Multi-Channel Intent Model (Per Entity, Per Tick)

An entity can receive multiple client packets within a single server tick (e.g., walking while drinking a potion and swapping gear). Intents are partitioned by **lock category**:

1. **`PrimaryActionIntent` (Single Slot — Last-Write-Wins):**
   * **Variants:** `MoveTo { dest }`, `BasicAttack { target, continuous }`, `CastSkill { skill_id, level, target }`, `ToggleSit { sit: bool }`, `PickUpItem { target }`.
   * **Conflict Rule:** Only **one** `PrimaryActionIntent` can exist per entity per tick. If multiple movement/combat packets arrive in the same tick window, the newest packet overwrites the slot.
2. **`InstantIntent` (`SmallVec<[InstantIntent; 4]>` — Queued):**
   * **Variants:** `UseConsumable { inv_index }`, `EquipItem { inv_index, equip_pos }`, `UnequipItem { inv_index }`.
   * **Conflict Rule:** Executes orthogonally to `PrimaryActionIntent` (players can drink potions or swap weapons mid-walk) unless blocked by specific status effects or reuse delays.
3. **`ZeroLockIntent` (Immediate / Event-Driven):**
   * **Variants:** `ChangeDirection { head, body }`, `RequestEntityName { target }`[cite: 1], `Chat`, `Emote`.

---

## 4. The 5-Stage Deterministic Tick Schedule

Each `MapWorld` executes its tick in five ordered stages. Stages 1, 2, 3, and 5 support intra-map data parallelism (`rayon`), while Stage 4 is a fast single-threaded reduction barrier[cite: 2].

### Stage 1: Present Status Establishment (Dirty-Only Parallel Pass)
Establishes the authoritative `PresentAttributes` of every entity before intents or simulation rules are evaluated[cite: 1]:
1. **Status Application & Expiration:** Apply queued `StatusChange` entries from the previous tick's Stage 4 and expire any `StatusInstance` where `expires_at <= current_tick`. Flip `DIRTY_STATUS` on affected entities.
2. **Layered Stat Aggregation (`BaseAttributes` $\rightarrow$ `EquipmentModifiers` $\rightarrow$ `ActiveStatuses` $\rightarrow$ `PresentAttributes`):**
   * Iterate **only** entities present in `stat_dirty`.
   * Recompute `EquipmentModifiers` if `DIRTY_EQUIP` is set[cite: 1].
   * Recompute `StatusModifiers` if `DIRTY_STATUS` is set[cite: 1].
   * Fold `BaseAttributes + EquipmentModifiers + StatusModifiers` into `PresentAttributes` and clear `stat_dirty`[cite: 1].

### Stage 2: Intent Ingestion & AI Planning
1. **Network Intent Drain:** Drain inbound `ClientIntent` messages from the network channel and populate `primary_intents` and `instant_intents`[cite: 1].
2. **Mob AI Planning (`par_iter`):** Read `SpatialGrid8x8` and `present_attrs` (`&MapWorld`) to evaluate monster aggro/assist ranges and write a `PrimaryActionIntent` for active monsters[cite: 1, 2].

### Stage 3: Simulation & Read-Only Event Generation (`&MapWorld` $\rightarrow$ Buffered `EngineEvent`s)
Systems in Stage 3 hold **immutable borrows** of `PresentAttributes`, `Vitals`, `ActiveStatuses`, and `SpatialGrid8x8`[cite: 1, 2]. All consequences are emitted into thread-local `Vec<EngineEvent>` buffers:
1. **Instant Intent System:** Validates consumables/equip swaps $\rightarrow$ emits `EngineEvent::HpHealed`, `EngineEvent::SpHealed`, `EngineEvent::EquipmentChanged`.
2. **Primary Intent System:** Validates `PrimaryActionIntent` against `CombatLocks` and `ActiveStatuses` $\rightarrow$ initiates/updates `WalkingPath` (A* over `.gat`), starts `ActiveCast`, or triggers immediate melee/skill calculation[cite: 1, 2].
3. **Movement & Environment System:** Steps `WalkingPath` components along `.gat` cells $\rightarrow$ emits `EngineEvent::EntityMoved { entity, from, to }` and checks `SpatialGrid8x8` for Warp Portal or Ground Trap (`SkillUnit`) triggers[cite: 1].
4. **Combat & Periodic Tick System:** Evaluates completed casts, melee swings, and periodic status ticks (e.g., Poison, Regen) via pure `crates/mechanics` functions $\rightarrow$ emits `EngineEvent::DamageDealt`, `EngineEvent::SpDrained`, `EngineEvent::StatusRequested`, `EngineEvent::Knockback`[cite: 1, 2].

### Stage 4: Event Traversal, Net Delta Reduction & State Commit (Single-Threaded Barrier)
Traverses all buffered `EngineEvent` items from Stage 3 and separates **Collapsible State Deltas** from **Discrete Visual Events**:
1. **Net Attribute Delta Folding (`EntityTickDelta`):**
   * Accumulates net deltas to **HP, SP, Position `(old_pos -> final_pos)`, and MoveSpeed** per entity.
   * **Overkill / Death Clamping:** If `vitals.hp + net_hp_delta <= 0`, clamps `hp = 0`, marks the entity dead for this tick, emits `VisualEvent::EntityDied`, and discards subsequent offensive events originating from that entity in the same tick buffer.
2. **Commit Deltas to `MapWorld`:**
   * Mutates `Vitals` (HP/SP) and updates `SpatialGrid8x8` cells once per moved entity using `final_pos`[cite: 1, 2].
   * Queues all `StatusRequested` events into the **Next-Tick Status Buffer** (to be applied in Stage 1 of `tick + 1` with `DIRTY_STATUS`).
3. **Preserve Discrete Visual Events:**
   * Individual combat hits (`DamageDealt`), skill casts, and movement starts are preserved as discrete `VisualBroadcastEvent` entries so client animations and floating damage numbers (`ZC_NOTIFY_ACT`) are never swallowed by net HP gauge folding.

### Stage 5: Outbound Multicast & Singlecast Dispatch
Translates Stage 4's committed deltas and visual events into `OutboundPacketEvent` messages[cite: 1]:
* **Singlecast (`BroadcastScope::SelfOnly`):** Net HP/SP/Speed parameter changes (`ZC_PAR_CHANGE`, `ZC_LONGPAR_CHANGE`)[cite: 1].
* **Multicast (`BroadcastScope::AreaOfInterest`):** Discrete movement paths (`ZC_NOTIFY_PLAYERMOVE`), combat animations/damage numbers (`ZC_NOTIFY_ACT`), status icon updates (`ZC_MSG_STATE_CHANGE`), and AoI spawn/despawn entries (`ZC_NOTIFY_NEWENTRY`, `ZC_NOTIFY_VANISH`)[cite: 1].