# Brief: warp map change (next feature)

This is a starting point for the next session, not the spec. The design below was planned and
approved on 2026-10-23. M0 (capture, catalog, spec, early review) comes before any code.

## Goal

When a player steps into a warp's trigger area, move them to the warp's destination. The
destination may be on the same map or another map hosted by this map-server. Only plain
`warp`/`warp2` NPCs are in scope. Script portals (`OnTouch: warp ...`) need scripting.

## What already exists (npc-spawning, commits `1111051..59665ff`)

- **Warp data:** `crates/ro-db/data/npc/<map>.yml` holds `Kind: Warp` entries with `Trigger: {X, Y}`
  half-extents and `Warp: {Map, X, Y, TriggerWhileHidden}`. The map name is bare (`geffen`).
- **Engine state** (`crates/ro-engine/src/world.rs`, `engine.rs`):
  - Each warp entity has a crate-internal `WarpTrigger { half_extents, dest: ro_db::WarpDest }`. Warps without a span get none.
  - `NpcInfo.state` keeps `Disabled`/`Cloaked`.
  - Gameplay intents are already ignored while `in_grid` is false (before `NotifyActorInit`).
- **Packets:** `ServerEvent::PlayerMapMove { dest_map, dest_x, dest_y }` already encodes
  `0x0091 ZC_NPCACK_MAPMOVE` (22 bytes, catalog `packets/map.md`).
- **Map-server** (`bins/map-server/src/service.rs`, `session.rs`):
  - `session.rs` binds `world_tx` once at `CZ_ENTER` and moves `outbound_tx` into the actor (dropping it ends the session).
  - `MapServiceHandle::get_or_spawn_world(map)` starts a world, loading its `.gat` and NPCs.
  - `SharedState.online` is the claim set; `persist_position` writes on leave.
  - `is_valid_map_name` requires a `.gat` suffix and rejects `@`.

## Decisions (settled with the user)

| Topic | Decision |
| :--- | :--- |
| Destination `.gat` missing | If `MAP_DATA_DIR` is set and `<dest>.gat` is not a file, the warp is inert (trigger cleared at map load, warn once per warp). Without `MAP_DATA_DIR` (dev/tests), destinations use the fallback grid. |
| `@`/instance destinations | Inert, same treatment. Out of scope. |
| Real-client capture | The user records it against rAthena **before coding**. The spec waits on it. |
| World cap | Warps don't check `MAX_HOSTED_WORLDS`: destinations come from static data. |
| RNG | Private SplitMix64 per map instance, seeded from the map name. No new dependency. |

## rAthena behavior (source-derived; catalog `mechanics/npcs.md` § "Warp trigger", `mechanics/map_entry_actions.md` § "Map change (same map-server)")

Not yet checked against a capture.
- **Same map and cross-map use one path** (`pc_setpos`):
  1. Observers get `0x0080` type 0.
  2. The walk is dropped with no packet.
  3. The player gets `0x0091` with the `.gat` name, even on the same map.
  4. The client reloads and sends `0x007d` on the same socket.
  5. `ZC_ACCEPT_ENTER` and `ZC_AID` are not re-sent.
- While the player is off the grid, every packet except `0x007d` is dropped.
- The check runs after **every completed step**, mid-path too, with no "already inside" memory. It also runs at the end of every `LoadEndAck` (first login included) for the arrival cell, so warps can chain.
- **Rewarp guard:** at the end of `pc_setpos`, `count_rewarp++` if the destination cell is in a warp area, otherwise it resets. If the count is above 10, the next trigger warns, resets and skips.
- **Skip the warp** if the NPC is disabled or cloaked, or the player is dead (`warp2` too). Also skip if the player is hiding, unless `warp2` (we have no hiding yet).
- **Destination:**
  - Out of bounds becomes `(0,0)`. `(0,0)` picks a random cell `x = rnd%(xs-2)+1`, `y = rnd%(ys-2)+1`. The cell must be walkable and not within 1 of a warp area. Give up after `xs*ys*3` tries, and then don't warp.
  - An in-bounds wall is used as-is.
- Nothing is persisted on a same-server map change.
- **Open question for the capture:** is `clif_map_property` (or anything else) needed for the client to leave the loading screen?

## Design

### `ro-core`: map names

`gat_map_name(bare) -> String` appends `.gat` unless it is already there. `bare_map_name(&str) -> &str`.
Use them in `load_npcs` and for warp destinations.

### `ro-engine`

- **`WarpTrigger`** is resolved at `spawn_npc`: `dest_map: Arc<str>` (`.gat`), `dest: (u16, u16)`, `trigger_while_hidden`, `same_map: bool`.
- **Trigger index** `World.warp_cells`: a bitmap with one bit per cell, set for the non-wall cells of each trigger area. On a hit, scan `world.warps` in spawn order for the first area that contains the cell, skipping `Disabled`/`Cloaked`.
- **Check points:** in `advance_walkers` right after `step(...)` (players only), and at the end of `actor_init`.
- **`warp_player(e, trigger)`:** skip if the player is dead, or if `rewarp_count > 10` (warn and reset).
  - **Same map:**
    1. Resolve the cell. If none is found, don't warp.
    2. `World::hide(e)` (new, the inverse of `show`).
    3. Vanish type 0 to the AoI, excluding self.
    4. Clear the walk and `pending_move`.
    5. Set the position and update `rewarp_count`.
    6. `PlayerMapMove { own map }` to self.
    7. The next `NotifyActorInit` re-runs `actor_init`.
  - **Cross map:**
    1. Vanish type 0 to the AoI.
    2. Despawn the entity and drop its queued intents (reuse the `remove_character` internals).
    3. Push `MapTransfer { spawn: CharacterSpawn, dest_map, dest, rewarp_count, from_pos }` into an outbox.
    4. A sends no `0x0091`; B sends it once the cell is resolved.
- **New API:**
  - `drain_transfers(&mut self, out: &mut Vec<MapTransfer>)`.
  - `enter_by_warp(spawn, dest, rewarp_count, now) -> Result<MapPosition, EngineError>`: resolve the cell (no random cell found falls back to the map centre with a warning), spawn hidden, update `rewarp_count`, queue `PlayerMapMove` to self.
- `PlayerInfo.rewarp_count: u8`.

### `map-server`: session handoff (risky; early `reviewer` pass on the spec)

- **`WorldCommand::JoinSession`** carries:
  - `spawn: CharacterSpawn`, which replaces `character` + `spawn_pos`;
  - `control_tx: mpsc::Sender<SessionControl>` (capacity 1);
  - `entry: Entry::Login | Entry::Warp { dest, rewarp_count }`. `Login` is today's behavior. `Warp` calls `enter_by_warp` and sends no `AcceptEnter`.
- **Actor A, after each tick's `flush`:**
  1. Drain the transfers.
  2. Remove the `Session` from `sessions`/`by_char` (no engine remove) and send what it has pending.
  3. `try_send(SessionControl::ChangeMap(MapHandoff { spawn, dest_map, dest, rewarp_count, from_map, from_pos, outbound_tx }))`.
  4. If the send fails: persist `from_pos`, release the claim, drop the sender.
  5. On success the claim stays held during transit.
- **Session task:**
  - Add `control_rx` to `select!`, and also apply queued control messages with `try_recv` before forwarding each parsed intent. `ChangeMap` is queued before the `0x0091` bytes reach the socket, so the client's `0x007d` always goes to B.
  - **Applying `ChangeMap`:**
    1. `world_tx = service.get_or_spawn_world(dest_map)`.
    2. Send `JoinSession { entry: Warp, .. }` to B.
    3. If that fails: persist `from_pos`, release the claim, and end the session.
  - **On exit:**
    1. `LeaveSession` to the current `world_tx`.
    2. Then `control_rx.close()` and drain it. A handoff still in the queue gets `from_pos` persisted and the claim released.
    3. Closing before draining makes a later `try_send` from A fail, so exactly one side releases the claim.
- **Ordering:** A's last batch, then B's `0x0091`, then B's entry burst all go through the same outbound mpsc. Stray intents land in A after the session has left, or in B before `actor_init`. Both are ignored.
- **Inert warps:** after `load_npcs`, `map_data_loader` clears `trigger` on warps with an invalid or `@` destination. It does the same when `MAP_DATA_DIR` is set and the `.gat` is missing.

## Milestones

- **M0: research, spec, early review**
  1. The user records captures against rAthena through `tools/proxy` (`./run_rathena_servers.sh`):
     - Cross-map: prontera `prt01` (107,215) to prt_in (240,139).
     - Same-map: prt_in `prt15` (217,163) to (236,163).
     - A second client as an observer, if possible.
  2. Diff the captures against the facts above (trust the capture). Settle the map-property question.
  3. Fix the catalog entries and drop the "not yet checked" notes.
  4. Write `spec.md`:
     - CORE-MAP-01.
     - ENG-WARP-01..08: index skips walls, same-map mid-path, cross-map transfer out, disabled/cloaked inert, arrival-cell chain, rewarp guard, random/OOB/wall/no-cell destination, `enter_by_warp`.
     - BIN-WARP-01: cross-map end to end. Covers `0x0091`, the observer's `0x0080` type 0, `0x007d` on the same socket, B's entry burst, no `0x02eb`/`0x0283`, position query on B, and walking on B.
     - BIN-WARP-02: same-map end to end.
     - BIN-WARP-03: a disconnect in transit releases the claim exactly once, and re-login works.
     - BIN-WARP-04: inert vs. fallback destination.
     - BIN-WARP-05: a move during loading is ignored.
     - BIN-WARP-06: restart to char select after a map change.
  5. Write `plan.md`.
  6. Run an early `reviewer` pass on the handoff section.
- **M1 ro-core:** name helpers.
- **M2 ro-engine trigger:**
  - `WarpTrigger` reshape, `warp_cells`, `hide`, `rewarp_count`, RNG, cell resolution.
  - Same-map warp and the step/`actor_init` hooks.
  - Tests in `crates/ro-engine/tests/warp_tests.rs`.
- **M3 ro-engine cross-map:** `MapTransfer`, `drain_transfers`, `enter_by_warp`. Update `DESIGN.md` §1a and §7.
- **M4 map-server:**
  - The handoff, the actor transfer processing, the session control loop and exit drain, and inert-warp filtering.
  - Tests in `bins/map-server/tests/warp_tests.rs` (`tests/common` `data_loader`, two maps; needs the db).
- **Finish:**
  - Run the `reviewer` over the full diff.
  - Update `DEFERRED.md`: remove the landed warp prerequisites. Add hiding/dead skips once those exist, `pc_job_can_entermap`, multi-map-server handoff, the blocking `.gat` load on the session task, and script portals.
  - Run `./verify.sh --smoke`.
  - Repeat the capture route against metaling and diff it.

## Out of scope

Script portals, warp skills (Teleport, Warp Portal), `@warp`/`@go` commands, multi-map-server
handoff (char-server reconnect), and instance maps (`1@...`).

## Pointers

- `DEFERRED.md` § npc-spawning, "Warp map change (next feature) prerequisites".
- `specs/npc-spawning/spec.md` (ENG-NPC-05), `reference/catalog/INDEX.md`.
