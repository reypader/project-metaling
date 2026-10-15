# AGENTS.md — `crates/engine` Subagent (Map ECS Runtime)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/engine/**`, `./crates/contracts/src/**`, and `./reference/**`.
* **Write Scope:** `./crates/engine/**` ONLY.
* **Prohibition:** NEVER read or modify any other crate in `./crates/*` or `./bins/*`. Communicate with `packets`, `db`, `mechanics`, and `persistence` strictly through traits and events in `crates/contracts`.

## 2. ECS Architecture Rules
1. **Generational Entities & 8x8 Spatial Grid:** Every actor (Player, Monster, Portal, FloorItem) is a 64-bit generational `Entity`. Maintain an `8x8` cell `SpatialGrid` storing `SmallVec<[Entity; 8]>` per block. Clone target `Entity` IDs into a local stack buffer before iterating AoI.
2. **Storage Discipline:** Store buffs in a single dense `ActiveStatuses` component (`FixedBitSet` + `SmallVec`). Use `SparseSet<T>` storage for transient states (`WalkingPath`, `ActiveCast`, `MovementLock`).
3. **Deterministic Tick Loop:** Process each Map `World` tick in strict order:
   `Drain ClientIntents` -> `Timers & AI` -> `Movement & Portal Triggers` -> `Combat Read Phase` -> `Effect Application Phase` -> `Emit OutboundPacketEvents`.
4. **No Direct Socket I/O:** Never touch `tokio::net::TcpStream`. Ingest `ClientIntent` events and emit `OutboundPacketEvent { scope: BroadcastScope, packet }` via channels defined in `crates/contracts`.
