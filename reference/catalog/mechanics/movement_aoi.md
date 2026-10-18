# Map Entry Handshake, Movement, and Area of Interest (AoI)

Target client version: `PACKETVER = 20260219`.

## Map Entry and Movement Lifecycle

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 5) & `rathena` (`src/map/clif.cpp`, `src/map/pc.cpp`, `src/map/unit.cpp`, `src/map/map.cpp`), `PACKETVER = 20260219`.
* **Map Entry Sequence:**
  1. Client sends `CZ_ENTER` (`0x0436`) with `(account_id, char_id, auth_code, client_tick, sex)`.
  2. Server responds with `ZC_AID` (`0x0283`), `ZC_INVENTORY_EXPANSION_INFO` (`0x0b18`), and `ZC_ACCEPT_ENTER` (`0x02eb`) containing the server tick and `WBUFPOS` packed spawn coordinates.
  3. Client finishes loading the map and sends `CZ_NOTIFY_ACTORINIT` (`0x007d`).
  4. Server registers the entity in the spatial grid, emits initial parameter packets (`ZC_PAR_CHANGE`) and nearby entity spawn notifications (`ZC_NOTIFY_STANDENTRY` / `ZC_NOTIFY_NEWENTRY`), and replies to `CZ_REQUEST_TIME` (`0x0360`) with `ZC_NOTIFY_TIME`.
* **Movement and AoI Broadcast Rules:**
  1. Client sends `CZ_REQUEST_MOVE` (`0x035f`) with 3-byte `WBUFPOS` destination coordinates.
  2. Server computes an A* path across walkable `.gat` cells, emits `ZC_NOTIFY_PLAYERMOVE` with 6-byte `WBUFPOS2` origin and destination coordinates, and advances cell coordinates on each map tick.
  3. Posture toggles (`CZ_REQUEST_ACT` `0x0437` with `action = 2` for sit or `action = 3` for stand) and orientation changes (`CZ_CHANGE_DIRECTION` `0x0361`) broadcast to entities within the Area of Interest (`SpatialGrid` `8x8` cells).
  4. Stepping within the trigger radius of a warp portal entity despawns the player from the origin map, transfers the entity to the destination map, emits `ZC_NPCACK_MAPMOVE` (`0x0091`), and awaits the next `CZ_NOTIFY_ACTORINIT` (`0x007d`) handshake.
