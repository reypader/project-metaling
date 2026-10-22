# Brief: warp map change (next feature)

This is a starting point for the next session, not the spec. Write `spec.md` + `plan.md` here first
(CLAUDE.md), after the research below.

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
  - Warps are sent as `0x09ff` with job 45.
- **Packets:** `ServerEvent::PlayerMapMove { dest_map, dest_x, dest_y }` already encodes
  `0x0091 ZC_NPCACK_MAPMOVE` (22 bytes, catalog `packets/map.md`).
- **Map-server** (`bins/map-server/src/service.rs`, `session.rs`):
  - `MapServiceHandle::get_or_spawn_world(map)` starts a world, loading its `.gat` and NPCs.
  - `accept_transfer`/`pending_transfers` hold char-server handoff reservations.
  - `is_valid_map_name` requires a `.gat` suffix and rejects `@`.

## rAthena behavior (catalog `mechanics/npcs.md` § "Warp trigger")
- **Trigger area:** the inclusive square `[x-xs, x+xs] x [y-ys, y+ys]`. Non-wall cells are marked `CELL_NPC` at spawn (`npc_setcells`).
- **When it fires:** after each completed step (`unit_walktoxy_timer`) and at the end of `LoadEndAck` for the arrival cell.
- **When it doesn't fire:** disabled or cloaked warps. Hiding or dead players, unless `warp2`. A rewarp counter above 10.
- **Cross-map transfer:** `pc_setpos(..., CLR_OUTSIGHT)`.
  1. Others see `0x0080` type 0.
  2. The player gets `0x0091`.
  3. The client loads the map and sends `0x007d` on the **same connection**; no new `CZ_ENTER`.
  4. The normal entry sequence runs (`mechanics/map_entry_actions.md`).
- **Destination:** `(0,0)` or out of bounds means a random walkable cell.

## Decisions to make or research first
1. **Session re-homing.** `session.rs` binds `world_tx` once at `CZ_ENTER`.
   - Map change needs the session to leave world A and join world B on the same socket.
   - Positions must be persisted and the `online` claim kept.
   - This is the main design question, because it touches map-server shared state and concurrency. The spec should get an early `reviewer` pass.
2. **Trigger lookup.** Build a per-instance cell index of trigger areas in `spawn_npcs` (a bitmap or cell→entity map) instead of scanning warps on each step (`DEFERRED.md`).
3. **Map names.** Add one helper for bare ↔ `.gat` names. Decide what to do with `@` instance destinations: reject them, or support them later.
4. **Research** (metaling-explorer):
   - The same-map branch of `pc_setpos` (`pc.cpp:~7040-7080`): which packets, and whether `0x0091` is still sent.
   - What rAthena sends between `0x0091` and the next `0x007d`, and which entry packets are re-sent (`ZC_ACCEPT_ENTER`? par changes?).
   - The rewarp guard.
   - Whether the arrival-cell trigger check runs on map entry.
   - If you can, record a real-client capture of a warp through `tools/proxy` against rAthena.
5. **Destination map not hosted.** What happens if the map is unknown or has no `.gat`? Today it falls back to a 512x512 all-walkable grid. Decide whether warps should go to such maps.

## Out of scope
Script portals, warp skills (Teleport, Warp Portal), `@warp`/`@go` commands, multi-map-server
handoff (char-server reconnect), and instance maps (`1@...`).

## Pointers
- `DEFERRED.md` § npc-spawning, "Warp map change (next feature) prerequisites".
- `specs/npc-spawning/spec.md` (ENG-NPC-05), `reference/catalog/INDEX.md`.
