# Specification: Engine Foundation (movement first)

First working slice of `crates/ro-engine`: a single-threaded ECS map instance that hosts players,
moves them along rAthena-compatible paths, keeps an 8x8 spatial grid, and emits Area-of-Interest
(AoI) visibility changes. It is wired through `ro-contracts`, `ro-db`, `ro-packets`, and
`bins/map-server`.

Ground truth is rAthena at `PACKETVER = 20260219`, as recorded in the catalog:
- `reference/catalog/packets/map_units.md`: entity/AoI packet layouts and "which packet when"
- `reference/catalog/packets/map.md`: player wire identifier policy, CZ packets
- `reference/catalog/mechanics/movement_aoi.md`: pathfinding, step timing, redirect, AoI
- `reference/catalog/mechanics/map_entry_actions.md`: entry, sit/stand, direction, name, time
- `reference/catalog/db/map_gat.md`: `.gat` layout and cell types

When this spec and the catalog disagree, the catalog wins, and this spec gets fixed.

---

## 0. Ground rules

### 0.1 Player wire identifiers
- A player's **wire id** (`gid` below) is its **account_id**. This is the value in every unit-id field:
  `ZC_NOTIFY_VANISH`, `ZC_NOTIFY_ACT` src, `ZC_CHANGE_DIRECTION`, `ZC_ACK_REQNAMEALL`, the
  `CZ_REQNAME` target, and the `AID` field of `0x09fd/0x09fe/0x09ff`.
- The **char_id** appears only in the `GID` field of `0x09fd/0x09fe/0x09ff` (and is `0` for non-players).
- Tests use account `2000001..2000004` paired with char `150001..150004`.

### 0.2 Which packet when (players)
| Situation | Event | Packet | Recipients |
| :--- | :--- | :--- | :--- |
| Mover starts/redirects a walk | `PlayerMoved` | `0x0087` ZC_NOTIFY_PLAYERMOVE (12) | mover only |
| Visible unit starts/redirects a walk | `UnitWalking` | `0x09fd` ZC_NOTIFY_MOVEENTRY11 (114, dynamic) | AoI excluding mover |
| Unit walks into an observer's view | `UnitWalking` | `0x09fd` | that observer only |
| Standing unit comes into view | `UnitStanding` | `0x09ff` ZC_NOTIFY_STANDENTRY11 (108, dynamic) | that observer only |
| Unit spawns (map entry on ACTORINIT) | `UnitSpawned` | `0x09fe` ZC_NOTIFY_NEWENTRY11 (107, dynamic) | AoI excluding the spawner |
| Unit leaves view / logs out | `UnitVanished` | `0x0080` ZC_NOTIFY_VANISH (7) | type 0: both sides of the boundary; type 2: AoI excluding leaver |
| Walk stopped | `UnitStopped` | `0x0088` ZC_STOPMOVE (10, *verify in M3*) | AoI including unit |
| Direction change | `DirectionChanged` | `0x009c` ZC_CHANGE_DIRECTION (9) | AoI excluding unit |
| Sit / stand | `EntityAction` | `0x08c8` ZC_NOTIFY_ACT (34) | AoI including actor |
| Name request | `AckEntityName` | `0x0a30` ZC_ACK_REQNAMEALL (106) | requester only |
| Time sync | `TimeSynchronized` | `0x007f` ZC_NOTIFY_TIME (6) | requester only |

`0x0086` is never sent. `0x0af7` is ZC_ACK_REQNAME_BYGID (32 bytes) and is **not** the `CZ_REQNAME` reply.

### 0.3 Movement rules (rAthena)
- `MAX_WALKPATH = 32`. A path longer than 32 steps is **rejected**, not truncated.
- `MAX_WALK_PATH_CLICK = 17` (rAthena `max_walk_path`). After the path is found, a path longer
  than 17 steps is rejected. (The `OFFICIAL_WALKPATH` straight-line rule for 15–17 steps is deferred, see §6.)
- Step cost: 10 orthogonal, 14 diagonal. Heuristic: Manhattan distance × 10 (as rAthena does).
- A diagonal step is legal only if the target cell **and** both adjacent cardinal cells are walkable.
- The destination must be in bounds and walkable, otherwise the request is silently dropped.
- The last row and column of every map (`x >= width-1` or `y >= height-1`) are never walkable.
  The pathfinder enforces this; `MapGrid` stays raw.
- Step duration: `speed` ms orthogonal, `speed * 14 / 10` ms diagonal (integer math). Default speed is 150.
- A step's timer starts when the step starts. The unit's cell changes when the step **ends**.
- **Click on own cell while idle:** send `PlayerMoved` with start == dest (client resync). Nothing goes to others.
- **Redirect while walking:** `path_search` from the current cell to validate. If that fails, ignore the
  click and keep walking. If it succeeds, store the new target and send **nothing yet**. When the in-flight
  step ends, repath from the reached cell, then emit `PlayerMoved` + `UnitWalking` for the new path. If that
  repath fails, emit `UnitStopped` and end the walk. `max_walk_path` is not re-checked at the repath.
- Sub-cell values in WBUFPOS2 are always `(8, 8)` in this feature.

### 0.4 AoI
- `AREA_SIZE = 14`, square (Chebyshev): visible iff `max(|dx|,|dy|) <= 14`. `BLOCK_SIZE = 8`.
- On each completed step, the engine computes the set of units visible before and after the step:
  - **Left view:** each side gets `UnitVanished{type 0}` about the other.
  - **Entered view:** the mover gets `UnitWalking`/`UnitStanding` about the other unit (walking or
    standing). The other unit gets `UnitWalking` about the mover (still walking) or `UnitStanding`
    (path just finished on this step).
- The engine resolves AoI-wide events **when it emits them** (like rAthena's `clif_send(AREA)`) into
  `BroadcastScope::Characters(Vec<CharId>)`: every visible character within 14 of the unit, sorted,
  minus the excluded one. Resolving after the tick instead would double-send entry packets to units
  that come into view later in the same tick. Per-observer visibility events use
  `BroadcastScope::SelfOnly(char_id)`. `query_aoi_characters` remains for queries.

### 0.5 Clock
The map-server owns one process-wide monotonic clock (`now_ms` since the service started, from `Instant`),
shared by every map actor, which passes it to `step_tick(now_ms)`, `spawn_character`, and so on. Every
wire tick field is `now_ms as u32`.

---

## 1. Contract touchpoints (`ro-contracts`)

### 1.1 `engine.rs`
```rust
pub trait MapEngine: Send {
    fn map_name(&self) -> &str;
    /// Creates the entity (not yet visible). Duplicate -> Err(CharacterAlreadyEntered).
    fn spawn_character(&mut self, spawn: CharacterSpawn, now_ms: u64)
        -> Result<Vec<ScopedServerEvent>, EngineError>;
    /// Queues an intent for the next step_tick. EnterMap is not accepted here.
    fn push_intent(&mut self, intent: SessionIntent);
    /// Bad per-intent input is logged and dropped. Err only for engine-wide faults.
    /// Appends this tick's events to `out` (the caller reuses the buffer).
    fn step_tick(&mut self, now_ms: u64, out: &mut Vec<ScopedServerEvent>) -> Result<(), EngineError>;
    /// Clears and fills `out` (reused by the caller; no per-event allocation).
    fn query_aoi_characters(&self, center: MapPosition, out: &mut Vec<CharId>);
    fn character_position(&self, char_id: CharId) -> Option<MapPosition>;
    /// Removes the entity; emits UnitVanished{type 2} to AoI excluding the leaver.
    fn remove_character(&mut self, char_id: CharId) -> Result<Vec<ScopedServerEvent>, EngineError>;
}
```
`EngineError` adds `MapNotFound(String)`, `CharacterAlreadyEntered(CharId)`, and `WorldCapacityExceeded`.

### 1.2 `character.rs`: `CharacterSpawn`
A snapshot the actor builds from `CharacterSummary` plus the session: `account_id`, `char_id`, `name`,
`sex`, `job_class`, `base_level`, `hair_style`, `hair_color`, `pos`, `speed` (default 150).
Built with `CharacterSpawn::from_summary(&summary)`; the actor first checks `summary.account_id`
against the session's account and closes the session on mismatch.

### 1.3 `map.rs`: `CellType` (fixed to rAthena)
`Walkable(0)`, `Wall(1)`, `Walkable2(2)`, `Water(3)`, `Walkable4(4)`, `Cliff(5)`, `Walkable6(6)`,
`Unknown(u8)`. Walkable: 0, 2, 3, 4, 6. **Not** walkable: 1, 5, and any unknown value.
`MapDataError` keeps `path` fields. Byte parsers take a `path` label argument.

### 1.4 `packet.rs`
- `BroadcastScope::Characters(Vec<CharId>)`: recipients the engine resolved at emission (§0.4).
- `BroadcastScope::AreaOfInterest { map_name, center, exclude: Option<CharId> }`. Update
  `OutboundPacket::broadcast_aoi` and its callers.
- `UnitAppearance` (shared by entry packets): `gid` (account_id), `char_id: Option<CharId>`, `name`,
  `job`, `sex`, `hair_style`, `hair_color`, `head_dir`, `pos` (incl. body dir), `speed`, `dead_sit`
  (0 stand / 1 dead / 2 sit; only the `0x09ff` `state` byte, not the wire `body_state`/opt1 field),
  `clevel`.
- `ServerEvent` replaces `EntityMoved`/`EntityAppeared`/`EntityVanished` with:
  `PlayerMoved { start: (u16,u16), dest: (u16,u16), move_start_tick: u32 }`,
  `UnitWalking { unit: UnitAppearance, start, dest, move_start_tick }` (cells from `start`/`dest`; `unit.pos` ignored),
  `UnitStanding { unit: UnitAppearance }`, `UnitSpawned { unit: UnitAppearance }`,
  `UnitVanished { gid: u32, vanish_type: u8 }`, `UnitStopped { gid: u32, x: u16, y: u16 }`,
  `DirectionChanged { gid: u32, head_dir: u16, body_dir: u8 }`,
  `AckEntityName { gid: u32, name, party_name, guild_name, position_name, title_id: i32 }`,
  `PlayerMapMove { dest_map, dest_x, dest_y }`. `EntityAction`, `ParameterChanged`,
  `TimeSynchronized`, and `AcceptEnter` stay.
- Opcode names in `packet/opcode.rs` are fixed: `0x09fd` = `ZC_NOTIFY_MOVEENTRY11`, `0x09fe` =
  `ZC_NOTIFY_NEWENTRY11`, `0x09ff` = `ZC_NOTIFY_STANDENTRY11`, `0x0af7` = `ZC_ACK_REQNAME_BYGID`.
  Add `ZC_ACK_REQNAMEALL` (`0x0a30`), `ZC_CHANGE_DIRECTION` (`0x009c`), `ZC_STOPMOVE` (`0x0088`).

---

## 2. Bin-observable scenarios (`bins/map-server`)

All BIN tests use the in-memory repo seeded with characters `150001..150004` (accounts
`2000001..2000004`) on `prontera.gat`, and a `MapGrid` injected through the service constructor.

### BIN-01: Map entry and scene initialization
- **Given** A at (100,100), and B already in game at (103,100).
- **When** A sends `CZ_ENTER`, then `CZ_NOTIFY_ACTORINIT`.
- **Then**
  - A's first burst is 23 bytes: `ZC_AID` + `ZC_INVENTORY_EXPANSION_INFO` + `ZC_ACCEPT_ENTER`, and starts with `0x0283`.
  - After ACTORINIT, A gets at least one `ZC_PAR_CHANGE` (weight / max weight) and a `0x09ff` for B (AID 2000002, GID 150002).
  - B gets a `0x09fe` for A with AID 2000001 and GID 150001.
- **Edge cases**
  - A truncated `CZ_ENTER` followed by EOF closes the connection, and no bytes are sent. (A pre-auth
    handshake timeout is deferred.)
  - A stored position off the map enters at the map centre instead of failing.
  - A character already online (on any map) can't enter again; the new session closes with no bytes.
  - An unknown char_id closes the connection without `ZC_AID`.
  - A second session entering an already-present char is rejected (closes the new session).

### BIN-02: Click-to-move
- **Given** A idle at (100,100) with (101..104,100) walkable, and speed 150.
- **When** A sends `CZ_REQUEST_MOVE` to (104,100).
- **Then** A gets one 12-byte `0x0087`, unpacking to start (100,100), dest (104,100), and sub-cell (8,8).
  Within one tick after 600 ms, `character_position` is (104,100).
- **Edge cases**
  - A click on (100,100) while idle gives a `0x0087` with start == dest, and the position is unchanged.
  - A click farther than 17 steps gives no packet and no movement.

### BIN-03: AoI movement multicast
- **Given** A (100,100), B (103,100), and C (150,150).
- **When** A moves to (100,106).
- **Then** A gets `0x0087`. B gets `0x09fd` with AID 2000001. A gets no `0x09fd` about itself. C gets nothing.

### BIN-04: Visibility while walking
- **Given** A (100,100), observer O (100,100) idle, and D (120,100) idle, on a clear row.
- **When** A moves to (117,100) (17 steps).
- **Then**
  - When A reaches (106,100), D gets `0x09fd` for A and A gets `0x09ff` for D.
  - When A reaches (115,100), O gets `0x0080` (gid 2000001, type 0) and A gets `0x0080` for O.
  - No duplicate entry packets arrive within the same 8x8 block.

### BIN-05: Wall and boundary rejection
- **Given** A at (100,100), with (105,100) a wall, on a 200x200 map.
- **When** A clicks (105,100), then (250,100).
- **Then** A gets no `0x0087`, the position stays (100,100), and the server keeps running.

### BIN-06: Direction and name
- **Given** A (100,100) and B (102,100).
- **When** A sends `CZ_CHANGE_DIRECTION` (head 2, body 4), then B sends `CZ_REQNAME` for gid 2000001.
- **Then**
  - B gets a 9-byte `0x009c` (src 2000001, head 2, dir 4), and A does not.
  - B gets a 106-byte `0x0a30` with gid 2000001 and A's name.
- **Edge cases**
  - A name request for an unknown gid, or for a unit beyond 14 cells, is silently ignored.
  - An out-of-range direction (body > 7 or head > 2) closes the sender's session.

### BIN-07: Disconnect cleanup
- **Given** A and B in AoI.
- **When** A closes its socket.
- **Then** B gets a 7-byte `0x0080` with gid 2000001 and type 2. `query_aoi_characters` no longer lists
  150001, and A's last position is persisted.

### BIN-08: Clock sync
- **When** a client in game sends `CZ_REQUEST_TIME(50000)`.
- **Then** it gets a 6-byte `0x007f` carrying the server clock (not 50000), which is > 0.
- **Edge case:** `CZ_REQUEST_TIME` before `CZ_ENTER` closes the connection.

### BIN-09: Sit and stand
- **Given** A (100,100) and B (102,100).
- **When** A sends `CZ_REQUEST_ACT` type 2, then type 3.
- **Then** A **and** B each get `0x08c8` with src 2000001 and type 2, then type 3.
- **Edge cases**
  - A move while sitting is ignored.
  - A sit while walking first stops the walk (`0x0088` to the AoI), then sits.

### BIN-SRV-01: Tick loop
The actor calls `step_tick(now_ms)` every 20 ms on a monotonic clock. On `Err`, it logs and continues.

### BIN-SRV-02: Intent bridge
Gameplay packets become `SessionIntent`s and are pushed into the engine before the next `step_tick`.
A gameplay packet before `CZ_ENTER` closes the session. If the actor channel is closed, the session ends.

### BIN-SRV-03: Event routing
- The actor encodes each `ScopedServerEvent` once.
- `SelfOnly(char)` goes to that session.
- `Characters(list)` goes to each listed character's session.
- `AreaOfInterest{center, exclude}` (not emitted by the engine today) goes to `query_aoi_characters(center)` minus `exclude`.
- `MapWide` goes to all sessions.
- Sending uses `try_send`. A full or closed channel removes the session, calls `remove_character`, and persists the position.

### BIN-SRV-04: Leave
`LeaveSession` → `remove_character` → vanish dispatched → persist `character_position` asynchronously.
`UnknownCharacter` is logged, and cleanup still proceeds. A re-entry before the write lands uses the
just-left position. `CZ_RESTART` type 1 leaves the map the same way but keeps the socket open until the
client closes it. Each session gets at most one queued write per flush (its frames are batched).

### BIN-SRV-06: Transfer reservation (existing behaviour, kept)
`accept_transfer` validates the name, reserves a slot, and spawns the world. The next `CZ_ENTER` claims the
reservation and enters at the reserved position. Invalid names return false.

### BIN-CFG-01: Map data loading
`MAP_DATA_DIR` set and `<dir>/<map>.gat` present: the world uses the parsed grid. File missing: a warning is
logged and an all-walkable 512x512 `MapGrid` is used (one shared instance).

---

## 3. Packet scenarios (`ro-packets`)

### PKT-01: Length table
These opcodes resolve to these lengths:

| Opcode | Length |
| :--- | :--- |
| `0x035f` | Fixed(5) |
| `0x0361` | Fixed(5) |
| `0x0368` | Fixed(6) |
| `0x0437` | Fixed(7) |
| `0x0360` | Fixed(6) |
| `0x0283` | Fixed(6) |
| `0x0b18` | Fixed(4) |
| `0x02eb` | Fixed(13) |
| `0x007f` | Fixed(6) |
| `0x0087` | Fixed(12) |
| `0x0088` | Fixed(10)\* |
| `0x009c` | Fixed(9) |
| `0x09fd` | Dynamic |
| `0x09fe` | Dynamic |
| `0x09ff` | Dynamic |
| `0x0080` | Fixed(7) |
| `0x0a30` | Fixed(106) |
| `0x0af7` | Fixed(32) |
| `0x08c8` | Fixed(34) |
| `0x00b0` | Fixed(8) |
| `0x0091` | Fixed(22) |

`0xffff` and `0x0086` → `None`. (\*verify `0x0088` in M3.)

### PKT-02: Streaming framing
Partial frames return `Ok(None)` and keep their bytes. Concatenated frames decode in order. A dynamic
length below 4 returns `MalformedHeader`. (A dynamic length can't exceed `MAX_PACKET_SIZE`: the field is
a u16 and the cap is 65535, so `PacketTooLarge` is unreachable for dynamic frames.)

### PKT-03: Move / direction decode
- `0x035f` to (104,100) decodes to `RequestMove{104,100}`, and 1023 boundaries round-trip.
- `0x0361` with head 2 / body 4 decodes to `ChangeDirection`.
- Head > 2 or body > 7 → `InvalidField`.
- A truncated payload → `TruncatedPayload`.

### PKT-04: Act / name decode
- `0x0437` decodes to `RequestAction{target_id, action_type}`, and unknown action types pass through.
- `0x0368` decodes to `RequestEntityName{entity_id}`.
- Truncated payloads → `TruncatedPayload`.

### PKT-05/06: Bit packing
WBUFPOS (3 bytes) and WBUFPOS2 (6 bytes) round-trip. `(100,140,6)` packs to `[0x19,0x08,0xc6]`.

### PKT-07: `PlayerMoved` → `0x0087`
- 12 bytes, with tick at bytes 2..6.
- WBUFPOS2 at 6..12 unpacks to start/dest and sub-cell (8,8).

### PKT-08: `UnitWalking` → `0x09fd`
- 114 bytes, with length 114 at 2..4.
- AID (account_id) at 5..9, GID (char_id) at 9..13.
- `moveStartTime` at 37, MoveData at 67..73, name at the tail.
- `char_id: None` writes GID 0.

### PKT-09: `UnitSpawned` → `0x09fe` (107), `UnitStanding` → `0x09ff` (108)
- `state` (`dead_sit`) is at offset 68 in `0x09ff`; the wire `body_state` (opt1) at 15 stays 0.
- The fields carry the real speed, job, sex, hair, head dir, and name.
- Offsets per `map_units.md`.

### PKT-10: `UnitVanished` → `0x0080`
7 bytes, and vanish types 0–3 round-trip at byte 6.

### PKT-11: `AckEntityName` → `0x0a30`; `PlayerMapMove` → `0x0091`
- `0x0a30`: 106 bytes, with gid at 2..6, four 24-byte NUL-padded strings, then `title_id`.
- `0x0091`: 22 bytes, with a 16-byte map name and x/y.
- Overlong strings truncate safely.

### PKT-12: Entry burst, par change, act, time
- `AcceptEnter` encodes to a 23-byte burst.
- `ParameterChanged` encodes to an 8-byte `0x00b0`.
- `EntityAction` encodes to a 34-byte `0x08c8` with only src and type set.
- `TimeSynchronized` encodes to a 6-byte `0x007f`.

### PKT-13: `DirectionChanged` → `0x009c`; `UnitStopped` → `0x0088`
- `0x009c`: 9 bytes (src, head u16, dir u8).
- `0x0088`: layout per catalog after M3 verification.

---

## 4. Engine scenarios (`ro-engine`)

### ENG-01: Generational entity allocator
- **Allocation and reuse**
  - The first allocation gets index 0, generation 1.
  - Freeing it and allocating again reuses index 0 with generation 2.
  - The stale handle is not alive.
- **Error cases**
  - Double-free returns an error and leaves the free list intact.
  - Allocating past capacity → `WorldCapacityExceeded`.
  - Generation wrap never revives a stale handle.

### ENG-02: World storage
- **Dense columns:** `positions` and `kinds` are indexed by `Entity::index()`.
- **`SparseSet<T>` operations**
  - `insert` / `get` / `contains` / `remove` (swap-remove keeps the sparse index valid).
  - Dense slices can be iterated.
  - Removing a missing component is a no-op.
  - Using a stale handle returns `None`.

### ENG-03: Spatial grid query
- **Setup:** a 200x200 grid with A (100,100), B (103,100), C (114,114), D (115,100), and E (150,150).
- **Expectation:** `query_radius((100,100),14)` = {A,B,C}.
- **Edge case:** a query at (0,0) clamps safely.

### ENG-04: Block move and visibility diff
- **Setup:** A at (103,100), D at (120,100), and O at (100,100).
- **Block move:** stepping A to (104,100) moves it from block (12,12) to (13,12). A step within a block
  does not touch the block lists.
- **Visibility events**
  - A reaching (106,100) produces the entered-view events for A↔D (§0.4).
  - A reaching (115,100) produces the type-0 vanish both ways for A↔O.

### ENG-05: Pathfinding
- **Setup:** a 30x30 grid with walls at (15,10) and (14,10).
- **Expectations**
  - A path to (15,10) → `None`, because the destination is a wall.
  - (14,9)→(15,10) diagonal is rejected because the corner (14,10) is blocked. No path uses that diagonal.
  - (10,10)→(13,10) on clear ground gives 3 orthogonal waypoints.
  - A path needing more than 32 steps → `None`.
  - A path to the current cell → an empty path.
  - An enclosed start → `None`.
  - Any path into the last row/column → `None`.
- **Performance:** no heap allocation per search after warm-up (scratch buffers are reused).

### ENG-06: Step timing
- Speed 150 gives 150 ms orthogonal and 210 ms diagonal.
- Speed 112 gives 112 and 156.

### ENG-07: Intent handling
- **Setup:** 150001 is visible at (100,100), and B is in AoI.
- **When** one tick carries `RequestTime`, then `ChangeDirection{2,4}`, then `RequestEntityName{2000002}`.
- **Then**
  - Events come in FIFO order.
  - `TimeSynchronized{now_ms}` goes SelfOnly.
  - `DirectionChanged{2000001,2,4}` goes to AoI excluding self, and the stored dir is 4.
  - `AckEntityName` with B's name goes SelfOnly.
- **Edge case:** an intent for an unknown char is dropped, and the tick still returns `Ok`.

### ENG-08: Tick stepping
- **Given** 150001 at (100,100), speed 150, and a path to (102,100) started at t=1000.
- **Then**
  - At t=1050 and t=1149 it is still (100,100).
  - At t=1150 it is (101,100).
  - At t=1300 it is (102,100), and the path is removed.
- **Edge case:** a single `step_tick(2000)` after the start runs every pending step in order, with an AoI diff per step.

### ENG-09: Lifecycle
- **Steps**
  1. `map_name()` = "prontera.gat".
  2. `spawn_character(150001 @ 100,100)` → `Ok`. The character is not yet visible: `query_aoi_characters` excludes it.
  3. `NotifyActorInit`, then `step_tick` emits SelfOnly `ParameterChanged` (weight, max weight), a
     `UnitSpawned` to AoI excluding self, and a SelfOnly `UnitStanding`/`UnitWalking` for each unit already in view.
  4. `query_aoi_characters` includes 150001.
  5. `remove_character` emits `UnitVanished{2000001, 2}` to AoI excluding self. The character is then
     excluded from queries.
- **Error cases**
  - A duplicate spawn → `CharacterAlreadyEntered`.
  - Removing an unknown character → `UnknownCharacter`.

### ENG-10: Rejected moves
- **Given** a 200x200 map, a wall at (105,100), and 150001 at (100,100).
- **When** it requests moves to (250,100), (105,100), and (130,100) (more than 17 steps).
- **Then** there are no movement events, the position stays (100,100), there is no `WalkingPath`, and the tick returns `Ok`.

### ENG-11: Sit / stand
- **Sit and stand**
  - `RequestAction` type 2 → dead_sit 2, and `EntityAction{src 2000001, type 2}` to AoI including self.
  - Type 3 → dead_sit 0, and `EntityAction` type 3.
- **Edge cases**
  - A repeated sit re-broadcasts (as rAthena does), and the state stays sitting.
  - A move while sitting is ignored.
  - A sit while walking emits `UnitStopped` at the current cell, removes the path, then sits.

### ENG-12: Mid-walk redirect
- **Given** 150001 walking (100,100)→(104,100), with the first step started at t=1000.
- **When** at t=1075 it requests (101,105).
- **Then**
  - At t=1075, no events are emitted.
  - At t=1150 it reaches (101,100), then repaths from there, and emits `PlayerMoved` (start (101,100), dest (101,105)) and `UnitWalking`.
- **Edge cases**
  - An invalid redirect target keeps the old walk.
  - If the repath at step end fails, it emits `UnitStopped` and ends the walk.

---

## 5. Data scenarios (`ro-db`)

### DB-01: `.gat` parsing
- **Valid input:** `MapGrid::from_gat_bytes(label, &buf)` with the `"GRAT\x01\x02"` header, width/height
  `u32` LE at offsets 6/10, and `w*h` 20-byte records (type `u32` at record offset 16) gives a row-major grid with width 3, height 2.
- **Errors**
  - First 4 bytes not `GRAT` → `BadMagic`. The 2 version bytes are not checked (rAthena checks neither).
  - Fewer than 14 bytes, or fewer than `14 + w*h*20` → `Truncated`.
  - Zero width/height, or a dimension > `u16::MAX` → `InvalidDimensions`.
- **File loading:** `MapGrid::load_gat(path)` wraps I/O errors in `Io`.

### DB-02: Cell types
- Raw 0–6 map to the §1.3 variants, and other values map to `Unknown`.
- Walkable: 0, 2, 3, 4, 6. Not walkable: 1, 5, and unknown values.

### DB-03: Bounds
- `(0,0)` and `(w-1,h-1)` return the stored value.
- `x >= w`, `y >= h`, and `u16::MAX` return `false` / `None` without panicking.

### DB-04: In-memory `MapGrid`
- A public builder: `all_walkable(w,h)` and `set(x,y,CellType)`.
- `MapGrid` is a concrete `Send + Sync` type (no walkability trait), shared as `Arc<MapGrid>`.
- `MapGrid::blocked()` is an infallible 1x1 grid with no walkable cell.
- Zero dimensions → `InvalidDimensions`.
- This is also the runtime fallback (BIN-CFG-01).

---

## 6. Deferred (not in this feature)
- Slow-client eviction scenario and tick-budget assertion (old BIN-SRV-05). The existing `try_send`
  eviction stays.
- Map-side auth check of `login_id1` against a char-server ticket (needs a handoff that doesn't exist yet).
- `OFFICIAL_WALKPATH` straight-line rule for 15–17 step paths.
- rAthena's 32x32 A* node-table collision quirk and equal-cost tie-breaking parity.
- Sub-cell offsets other than (8,8) in `0x09fd` (rAthena only uses them when a walk is shortened mid-cell).
- Non-player name replies (`ZC_ACK_REQNAMEALL_NPC`), portals/warps triggering `ZC_NPCACK_MAPMOVE`,
  monsters, status, combat, and rayon parallelism (see `crates/ro-engine/DESIGN.md`).
- `.rsw` water-level promotion of type-0 cells.
- `object_type` in `UnitAppearance` (every unit encodes as PC `0` until NPCs/mobs exist).
