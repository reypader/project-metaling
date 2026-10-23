# Specification: Warp map change

When a player steps into a plain `warp`/`warp2` NPC's trigger area, move them to the warp's
destination. The destination can be on the same map or on another map hosted by this
map-server. Script portals (`OnTouch: warp ...`) wait for scripting.

The design comes from `brief.md` (approved 2026-10-23). The real-client captures recorded on
2026-10-23 confirm it, and nothing in them changed the design. The early review did change the map-server claim and handoff (§1). Where the brief and this spec differ, this spec wins. Ground truth is in the catalog:
- `reference/catalog/mechanics/map_entry_actions.md#map-change-same-map-server`: wire sequence, observers, the post-`0x007d` burst, and the map-property answer
- `reference/catalog/mechanics/npcs.md#warp-trigger-for-the-warp-portal-feature`: trigger, skips, rewarp guard, destination cell
- `reference/catalog/packets/map.md#0x0091-zc_npcack_mapmove`: `0x0091` layout

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. What the captures settled

| Question | Answer |
| :--- | :--- |
| Is anything needed between `0x0091` and the entry burst? | No. Neither side sends anything between `0x0091` and the client's `0x007d`. `0x099b` (map property) is inside the post-`0x007d` burst, in the same place as on first login. Our first-login burst already works without it, so a warp needs nothing new. |
| Same map vs cross map on the wire | Identical: `0x0091` to self, `0x0080` type 0 to observers in view, then `0x007d` on the same socket. |
| Walk cancel | Silent: no `0x0088`. The `0x0091` comes after the step that enters the area, mid-path too. |
| Entry after the warp | No `0x0283`, `0x0b18` or `0x02eb`. Observers at the destination get the spawn form (`0x09fe`). |
| Client after `0x0091` | Sends `0x007d` 20–630 ms later, then `0x0360` at once. May send `0x007d` twice. |

Out of scope, logged in `PACKET_FINDINGS.md`: rAthena also sends `0x0091` on every fresh login ([PF-20](../../PACKET_FINDINGS.md#pf-20)), and sends `0x01d7` to observers at entry ([PF-01](../../PACKET_FINDINGS.md#pf-01)). Neither is part of this feature.

## 1. Design

### `ro-core`
- `gat_map_name(bare: &str) -> String` appends `.gat` unless the name already ends with it.
- `bare_map_name(name: &str) -> &str` strips one trailing `.gat`.
- Both are case-sensitive and do nothing else.
- `load_npcs` and warp destinations use them.

### `ro-engine`
- **`WarpTrigger`** is resolved at `spawn_npc`:
  - `half_extents`;
  - `dest_map: Arc<str>`, the `.gat` name;
  - `dest: (u16, u16)`;
  - `trigger_while_hidden`;
  - `same_map: bool`, compared against the instance's `.gat` name.
- **`World.warp_cells`:** a bitmap with one bit per cell. Each trigger area sets its non-wall cells, clipped to the map.
  - On a hit, scan `world.warps` in spawn order (rAthena load order) and take the first area that contains the cell.
  - The scan skips `Disabled` and `Cloaked` warps.
- **Check points:**
  - in `advance_walkers`, right after each `step(...)`, for players only;
  - at the end of `actor_init`, for the arrival cell, after the spawn and entry events.
- **`PlayerInfo.rewarp_count: u8`.**
- **RNG:** a private SplitMix64 per instance, seeded from a hash of the map name. No new dependency.
- **`warp_player(e, trigger)`:**
  - Skip if the player is dead.
  - Skip if `rewarp_count > 10`: warn, reset the count to 0, and don't warp.
  - **Same map:**
    1. Resolve the cell (§ENG-WARP-07). If none is found, don't warp and emit nothing.
    2. Vanish (type 0) to the AoI, excluding self.
    3. `World::hide(e)`, the inverse of `show`.
    4. Clear the walk and `pending_move` without emitting anything.
    5. Set the position and update `rewarp_count`.
    6. Emit `PlayerMapMove { own .gat, dest }` to self.
    7. The next `NotifyActorInit` runs `actor_init` again.
  - **Cross map:**
    1. Vanish (type 0) to the AoI, excluding self.
    2. Despawn the entity and drop its queued intents. This shares code with `remove_character`, but the vanish type is 0, not 2.
    3. Push a `MapTransfer` into the outbox. It carries:
       - `spawn: CharacterSpawn`, rebuilt from the entity;
       - `dest_map`;
       - `dest`, unresolved;
       - `rewarp_count`;
       - `from_pos`, the trigger cell.
    4. This instance sends no `0x0091`.
- **New public API:**
  - `drain_transfers(&mut self, out: &mut Vec<MapTransfer>)`.
  - `enter_by_warp(spawn, dest, rewarp_count, now) -> Result<MapPosition, EngineError>`:
    1. Resolve the cell. If no random cell is found, use the map centre and warn.
    2. Spawn the player hidden.
    3. Update `rewarp_count`.
    4. Return the resolved cell.
    - It emits no events. The caller sends the `0x0091`, as it does `AcceptEnter` on login, so nothing is queued in the engine between ticks.

### `map-server`: claim and session handoff

**Invariant:** each character's entry in `SharedState.online` is released exactly once, and the last known position is persisted before that release. Both happen in one place: dropping the character's `Claim`. Nothing else calls `release`.

#### `Claim` (RAII)
- **Contents:** `Claim { shared: Arc<SharedState>, char_id, last: Option<(String, MapPosition)> }`. `set_last(map, pos)` records the newest known position.
- **Drop runs in this order:**
  1. If `last` is set, insert it into `recent_positions` with a new sequence number.
  2. Queue the database write on the persistence writer (below).
  3. Remove `char_id` from `online`.
  - Recording the position before releasing (step 1 before step 3) means a re-login that claims right after the release sees the new position.
- **No double release:** `SharedState::release` returns whether the character was claimed. An unheld release logs `error!` and fails a `debug_assert!`, so any double release fails the tests.
- **Who owns it:**
  - `resolve_entry` creates it.
  - It travels in `JoinSession` to the actor, and the actor's `Session` keeps it.
  - On a transfer it moves into `MapHandoff`, then into B's `JoinSession`, then into B's `Session`.
  - Any path that drops one of those values releases the claim: a failed `send`, a `try_send` error, a rejected join, eviction, leave, a closed channel, or the session task ending.
- **`MapHandoff` and `JoinSession` must not carry a `control_tx` clone in a way that keeps a channel alive after its session ends.** `MapHandoff` carries none. The session task owns its own `control_tx` and passes a clone in each `JoinSession`. An actor's `Session` keeps that clone; it is dropped on removal and never stored in `detached`.

#### Persistence writer
- **What it replaces:** `persist_position`'s per-write `tokio::spawn`.
- **Shape:** one writer task per process, fed by an unbounded mpsc in `SharedState`. It runs writes in the order they were queued, which is release order for a given character, so an older write can't land after a newer one.
- **After a write:** it removes the `recent_positions` entry only if the sequence number still matches.
- **At shutdown:** a send that fails because the writer is gone logs an error.
- **Inside a session:** an actor still calls `set_last` and lets the drop persist. There are no direct writes.

#### `resolve_entry`
- **Claim before reading** `pending_transfers`/`recent_positions`. A release racing this login has then already recorded its position (see Drop order).
- **Returns** `(world_tx, CharacterSpawn, Claim)`. Any rejection after the claim drops the `Claim`, which releases it. `release_character` goes away.

#### `WorldCommand::JoinSession`
- **Fields:**
  - `session_id`, `account_id`;
  - `spawn: CharacterSpawn`, which replaces `character` + `spawn_pos`;
  - `claim: Claim`, `outbound_tx`, `control_tx`;
  - `entry`, either `Entry::Login` or `Entry::Warp { dest, rewarp_count }`.
- **`Entry::Login`:** today's behaviour, `AcceptEnter` included.
- **`Entry::Warp`:**
  1. Call `enter_by_warp`, which emits nothing.
  2. Encode `PlayerMapMove { gat_map_name(this map), resolved x, y }` straight into the session's pending batch (no `AcceptEnter`).
  3. `flush`.
- **On failure** (duplicate session, `enter_by_warp` error): the command is dropped. Its `Claim` still holds `from_map`/`from_pos` from A, so the drop persists them and releases. Dropping `outbound_tx` ends the session.

#### Actor A: transfers, every tick
1. `step_tick`. Whether it returns `Ok` or `Err`, drain the transfers into a reused buffer.
2. For each transfer, call `set_last(map A, from_pos)` on the session's `Claim`. The entity is already gone, so this is the only record of the position.
3. `flush`.
   - This sends A's last batch for the warping session.
   - If that session's queue failed, eviction drops it and the `Claim` releases with `from_pos`. Eviction no longer depends on the engine knowing the character.
4. For each transfer whose session is still present:
   1. Remove it from `sessions`/`by_char`.
   2. `try_send(SessionControl::ChangeMap(MapHandoff { claim, spawn, dest_map, dest, rewarp_count, outbound_tx }))`.
   3. If the send fails, the returned message is dropped, which releases the claim and drops `outbound_tx`.
5. If a transfer's session is gone, there is nothing to do: its claim was already handled.

#### Session task
- **Main loop:** `control_rx` is a third branch of `select!`. There's no `try_recv` before each intent: the client's `0x007d` can only follow B's `0x0091`, which B sends after the session has switched `world_tx`.
- **Applying `ChangeMap`:**
  1. `world_tx = service.get_or_spawn_world(dest_map)`.
  2. `send(JoinSession { entry: Warp, claim, outbound_tx, control_tx: clone, .. })` to B.
  3. If that fails, the returned command is dropped, which releases the claim, and the session ends.
- **On exit:**
  1. `LeaveSession` to the current `world_tx`.
  2. Drop the session's `control_tx`, then `control_rx`, then any `outbound_tx` it still holds.
  3. Await the writer task.
  - Dropping the receiver drops every queued handoff, which releases its claim and drops its `outbound_tx`.
  - A handoff pushed after the receiver is dropped is dropped with the channel when A's sender goes away, right after its `try_send`.
  - Either way the writer task's sender count reaches zero, so no exit hits the 1 s timeout.
- **Ordering:** A's last batch (step 3) is queued before `ChangeMap` (step 4), and B's `0x0091` and entry burst come after the session hands `outbound_tx` to B. All of it goes through one mpsc, in that order.
  - Stray intents reach A after the session has left, where they are dropped as from an unjoined session.
  - Or they reach B before `actor_init`, where the engine ignores them.
  - The exception is `RequestTime`, which is answered in either state.

#### Socket lifecycle
The server closes the socket first only in these cases:
- the cases `bins/CLAUDE.md` already allows;
- B rejects a warp join (entity capacity exhausted, or a duplicate);
- the `JoinSession` send to B fails (B's actor is gone).

Both are unrecoverable for that session. With capacity exhausted, a re-login on the warp cell loops warp → fail → close until capacity frees up. That is acceptable at 4096 entities per map.

#### Inert warps
- `map_data_loader` clears `Trigger` on a warp after `load_npcs` and before `spawn_npcs`, in two cases:
  - the destination is invalid or an `@` map;
  - `MAP_DATA_DIR` is set and `<dest>.gat` is not a file.
- It warns once per warp.
- Without `MAP_DATA_DIR`, destinations use the fallback grid.
- Warps don't check `MAX_HOSTED_WORLDS`, because destinations come from static data.

**Deviation from rAthena:** on a disconnect in transit, we persist `from_pos` (the trigger cell on A). rAthena would save the destination, because `pc_setpos` sets map/x/y before the client loads. We persist `from_pos` because it is a known cell on a hosted map, while a cross-map destination may still be unresolved. On re-login the arrival-cell check fires the same warp. The player ends where rAthena would put them, with the cell rolled again for a random destination. The reviewer judged this sound. The caveats are that `rewarp_count` resets, and that a warp disabled later leaves the player standing on it.

**Duplicate `0x007d` and chains:** if a login lands on a cross-map warp cell, the first `0x007d` fires the warp. A second, duplicate `0x007d` can reach B and complete B's entry before the client has loaded map 2. rAthena would do the same: `prev == nullptr`, so `LoadEndAck` runs. We accept this as parity.

### Early review (2026-10-23, applied)
The `reviewer` read the first draft of this section before any code. All findings are applied above:
- H1 (a handoff lost in the exit drain) and H2 (eviction during the transfer tick): the `Claim`.
- M1: claim before reading the saved position, and record the position before releasing.
- M2: the duplicate-`0x007d` note above, and BIN-WARP-05.
- L1: `enter_by_warp` emits nothing, and B encodes the `0x0091`.
- L2: no separate pending-batch send.
- L3: `control_tx` ownership.
- L4: a rejected warp join drops the `Claim`.
- L5: socket-lifecycle exceptions, and dropping senders before awaiting the writer.
- L6: an unheld release is an error.
- L7: a single persistence writer.

## 2. Scenarios

### CORE-MAP-01: Map name helpers
- `gat_map_name("prontera") == "prontera.gat"` and `gat_map_name("prontera.gat") == "prontera.gat"`.
- `bare_map_name("prontera.gat") == "prontera"` and `bare_map_name("prontera") == "prontera"`.
- `bare_map_name(&gat_map_name(x)) == x` for a bare `x`.

### ENG-WARP-01: Trigger index
- A warp at `(x, y)` with half-extents `(xs, ys)` marks the walkable cells of `[x-xs, x+xs] × [y-ys, y+ys]`, clipped to the map. Wall cells inside the area are not marked.
- A wall cell inside an area never triggers, even if a player is placed there.
- A warp without a trigger, or an inert one (cleared by the loader), marks nothing.
- Where two areas overlap, the earlier warp in spawn order wins.

### ENG-WARP-02: Same-map warp, mid-path
A player walking a path that passes through a same-map warp's area. The warp fires on the first completed step that ends in the area:
- Observers in view at the source get `UnitVanished { type 0 }`. The player doesn't.
- No `UnitStopped` is emitted, and the walk and `pending_move` are cleared.
- Self gets exactly one `PlayerMapMove { dest_map: "<this map>.gat", dest }`.
- `character_position` returns the destination, and the player is out of the grid. Observers in view of the destination get nothing yet.
- A `RequestMove` before `NotifyActorInit` is ignored.
- After `NotifyActorInit`:
  - observers at the destination get `UnitSpawned`;
  - self gets an entry event for each unit in view;
  - a `RequestMove` works again.
- A second `NotifyActorInit` is a no-op.

### ENG-WARP-03: Cross-map transfer out
A player stepping into a warp to another map:
- Observers get `UnitVanished { type 0 }`.
- Self gets no `PlayerMapMove` from this instance.
- The entity is gone: `character_position` is `None`, and its queued intents are dropped.
- `drain_transfers` yields exactly one `MapTransfer`:
  - `spawn` keeps identity, look, level and speed;
  - `dest_map` is the `.gat` name;
  - `dest` is the warp's raw destination;
  - `rewarp_count` is the current count;
  - `from_pos` is the trigger cell.
- A second drain yields nothing.

### ENG-WARP-04: Inert warps
None of these produce a warp or any event:
- a `Disabled` warp;
- a `Cloaked` warp;
- a warp without a trigger;
- a dead player stepping in (`dead_sit == 1`).

A `Hidden` warp still fires.

### ENG-WARP-05: Arrival-cell chain
`NotifyActorInit` at a cell inside a warp area fires that warp after the spawn and entry events. This applies on login, after `enter_by_warp`, and after a same-map warp. Observers therefore see `UnitSpawned` and then `UnitVanished { type 0 }`. An arrival cell outside every area fires nothing.

### ENG-WARP-06: Rewarp guard
- After each warp, `rewarp_count` is incremented if the destination cell is inside an enabled warp area, and reset to 0 otherwise.
- A trigger with `rewarp_count > 10` warns, resets the count to 0, and doesn't warp.
- Test: two warps that land in each other's areas, with `NotifyActorInit` sent after each `PlayerMapMove`. Exactly 11 warps happen. The 12th trigger is skipped, and the player stays put at the 11th destination.
- The count travels in `MapTransfer` and `enter_by_warp`.

### ENG-WARP-07: Destination cell
- An in-bounds destination is used as-is, even a wall.
- Out of bounds becomes `(0, 0)`.
- `(0, 0)` picks `x = rnd % (xs-2) + 1` and `y = rnd % (ys-2) + 1`. It retries while the cell is not walkable or a marked warp cell is within Chebyshev distance 1, up to `xs * ys * 3` tries.
- When the tries run out: a same-map warp doesn't happen (no events), and `enter_by_warp` uses the map centre and warns.
- The RNG is deterministic per map name: two instances of the same map pick the same sequence.

### ENG-WARP-08: `enter_by_warp`
- The player is spawned out of the grid, and no events are emitted at all, to self or to others.
- The returned position is the resolved cell, and `rewarp_count` is updated for it.
- `NotifyActorInit` then behaves as on login (ENG-WARP-02's "after" list, and ENG-WARP-05).
- A character already present gives `EngineError` and changes nothing.

### BIN-WARP-01: Cross-map end to end
Two maps in one map-server (`tests/common` `data_loader`). Client A and observer O are on map 1, and observer P is on map 2 near the destination. A walks into a warp to map 2:
- A gets `0x0091` with `<map2>.gat` and the destination.
- O gets `0x0080` with A's account id and type 0.
- A sends `0x007d` on the same socket and gets:
  - `0x00b0` WEIGHT and MAXWEIGHT;
  - an entry packet for P;
  - no `0x0283`, `0x0b18` or `0x02eb`.
- P gets `0x09fe` for A.
- A's `0x0360` gets a `0x007f`.
- `character_position(map2, A)` is the destination, and `character_position(map1, A)` is `None`.
- A walks on map 2 and gets `0x0087`.

### BIN-WARP-02: Same-map end to end
A and observer O are on one map, and A walks into a same-map warp:
- A gets `0x0091` with the same map's `.gat` and the destination.
- O gets `0x0080` type 0 if O was in view of the trigger cell.
- After A's `0x007d`, A gets the entry burst, and observers at the destination get `0x09fe`.
- A can walk from the destination.

### BIN-WARP-03: Disconnect in transit
The client closes right after stepping onto a cross-map warp, in a loop that covers both races (before and after the session applies `ChangeMap`). After each close:
- An immediate re-login succeeds, so the claim was released.
- A second concurrent login of the same character is rejected.
- No unheld-release error is logged, and no `debug_assert!` fires, so the claim wasn't released twice.
- The re-login enters at `from_map`/`from_pos` (and the arrival check sends it on to map 2), or at a cell on map 2.

### BIN-WARP-04: Inert vs fallback destination
- With `MAP_DATA_DIR` set and the destination's `.gat` missing: stepping on the warp sends no `0x0091`, and one warning is logged per warp.
- Same for a warp to an `@` map.
- Without `MAP_DATA_DIR`: the warp works, and the player arrives on the fallback grid.

### BIN-WARP-05: Moves while loading are ignored
After `0x0091` (same-map and cross-map), a `0x035f` sent before `0x007d` gets no `0x0087`, and the position doesn't change. After `0x007d`, moves work. A duplicate `0x007d` sends nothing more, when the arrival cell is not a warp. A duplicate after an arrival-cell chain is accepted parity; see §1.

### BIN-WARP-06: Restart after a map change
After a cross-map warp and `0x007d`, `0x00b2` type 1 gets `0x00b3`. The stored map and position are on map 2. Re-entering from char select lands on map 2 at that position.

### BIN-WARP-07: `Claim` lifecycle (unit tests in `bins/map-server`)
- Dropping a `Claim` with `last` set records `recent_positions`, queues the write, and only then releases. Without `last`, it only releases.
- `SharedState::release` of a character that isn't claimed returns `false` and logs an error, and the `debug_assert!` fires in tests.
- A `MapHandoff` inside a channel whose receiver is dropped, with the message pushed before or after the drop, releases its claim once the last sender drops.
- `resolve_entry` claims before it reads `recent_positions`. A position recorded by a racing release is used.
- Writes for one character land in queue order. The writer clears a `recent_positions` entry only when the sequence number matches.

## 3. Deferred
- Login-time `0x0091` ([PF-20](../../PACKET_FINDINGS.md#pf-20)), and `0x01d7` and the inventory re-send at entry. They are entry content, not warp behaviour.
- Hiding and the `warp2` hidden exception: no hiding state yet.
- `pc_job_can_entermap`.
- Multi-map-server handoff (char-server reconnect), and instance (`1@...`) destinations.
- The blocking `.gat` load on the session task in `get_or_spawn_world`.
- Script portals, warp skills, `@warp`/`@go`.
