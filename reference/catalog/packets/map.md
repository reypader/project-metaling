# Map Server Packets (`CZ_*` / `ZC_*`)

Target client version: `PACKETVER = 20260219`. Packet entries are sorted by ascending hexadecimal opcode.

## Packet Shuffling Precedence and Macro Offset Rules

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 4) & `rathena` (`src/map/clif_shuffle.hpp`, `src/map/clif_packetdb.hpp`, `src/map/packets_struct.hpp`, `PACKETVER = 20260219`).
* **Shuffle Precedence:** For `PACKETVER = 20260219`, incoming client-to-map (`CZ_*`) opcodes defined in `env(RATHENA_PATH)/src/map/clif_shuffle.hpp` override baseline definitions in `clif_packetdb.hpp`. The branch that applies is the last one, `#elif PACKETVER > 20180307` (L4723-4761); every earlier branch is `== <date>`, so searching for `20260219` finds nothing. Opcodes that branch does not list keep their last `clif_packetdb.hpp` registration (cumulative `#if PACKETVER >= date` blocks). Full evidence: [PF-05](../../../PACKET_FINDINGS.md#pf-05).
* **Macro Offset Interpretation:** In `parseable_packet(opcode, length, handler, pos0, pos1, ...)`, the trailing integer arguments (`pos0`, `pos1`, and subsequent indices) specify exact byte offsets from byte `0` of the raw packet frame.
* **32-bit Item Identifiers:** `MAX_ITEMID` exceeds `UINT16_MAX` in `PACKETVER = 20260219`. Every item identifier field across map packets uses `u32`.
* **Unhandled Opcode Policy:** When the map server receives an unimplemented opcode, it logs `tracing::debug!("Unhandled packet opcode={:#06x} len={}", opcode, bytes.len())`, advances the read buffer using the `PacketLengthTable`, and continues processing the session.

## Packet Specifications

### `0x007d` `CZ_NOTIFY_ACTORINIT`

* **Opcode:** `0x007d`
* **Canonical Name:** `CZ_NOTIFY_ACTORINIT` (`clif_parse_LoadEndAck`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `2` bytes
* **Provenance:** `rathena` (`src/map/clif_packetdb.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x007d`; signals client finished loading map assets |

### `0x0091` `ZC_NPCACK_MAPMOVE`

* **Opcode:** `0x0091`
* **Canonical Name:** `ZC_NPCACK_MAPMOVE` (`PACKET_ZC_NPCACK_MAPMOVE`)
* **Direction:** Server to Client (Map Server)
* **Frame Length:** Fixed `22` bytes
* **Provenance:** `rathena` (`src/map/packets_struct.hpp`, `src/map/clif.cpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0091` |
| `2` | `16` | `[u8; 16]` | `map_name` | Null-terminated destination `.gat` filename |
| `18` | `2` | `u16` | `x` | Destination X coordinate |
| `20` | `2` | `u16` | `y` | Destination Y coordinate |

* **Capture-confirmed** (rAthena, 2026-10-23): `70 72 74 5f 66 69 6c 64 30 38 2e 67 61 74 00 00 aa 00 77 01` is `prt_fild08.gat` (NUL-padded to 16) at (170,375). Same-map warps send the same map's name.
* **When rAthena sends it:** on every warp (same map or not; see [mechanics/map_entry_actions.md#map-change-same-map-server](../mechanics/map_entry_actions.md#map-change-same-map-server)), and once on every fresh login with the character's own map and cell, after `0x02eb` and before the client's first `0x007d` (see [mechanics/map_entry_actions.md#map-entry-cz_enter-clif_parse_wanttoconnection](../mechanics/map_entry_actions.md#map-entry-cz_enter-clif_parse_wanttoconnection) and [PF-20](../../../PACKET_FINDINGS.md#pf-20)). `@goto`/`@warp` use it too.

### `0x0283` `ZC_AID`

* **Opcode:** `0x0283`
* **Canonical Name:** `ZC_AID`
* **Direction:** Server to Client (Map Server)
* **Frame Length:** Fixed `6` bytes
* **Provenance:** `rathena` (`src/map/clif.cpp`, `clif_authok`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0283` |
| `2` | `4` | `u32` | `account_id` | Confirmed player Account ID (`AID`) |

### `0x02eb` `ZC_ACCEPT_ENTER`

* **Opcode:** `0x02eb`
* **Canonical Name:** `ZC_ACCEPT_ENTER` (`PACKET_ZC_ACCEPT_ENTER`)
* **Direction:** Server to Client (Map Server)
* **Frame Length:** Fixed `13` bytes
* **Provenance:** `rathena` (`src/map/packets_struct.hpp`, `src/map/clif.cpp`), `PACKETVER = 20260219` (`PACKETVER >= 20141022`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x02eb` |
| `2` | `4` | `u32` | `start_time` | Server tick in milliseconds |
| `6` | `3` | `[u8; 3]` | `pos_dir` | `WBUFPOS` packed `(x, y, dir)` spawn coordinates |
| `9` | `1` | `u8` | `x_size` | Always `5` |
| `10` | `1` | `u8` | `y_size` | Always `5` |
| `11` | `2` | `u16` | `font` | Character font ID (`0` default) |

### `0x035f` `CZ_REQUEST_MOVE`

* **Opcode:** `0x035f`
* **Canonical Name:** `CZ_REQUEST_MOVE` (`clif_parse_WalkToXY`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `5` bytes
* **Provenance:** `rathena` (`src/map/clif_shuffle.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x035f` |
| `2` | `3` | `[u8; 3]` | `dest` | `WBUFPOS` packed target `(x, y)` coordinates |

### `0x0360` `CZ_REQUEST_TIME`

* **Opcode:** `0x0360`
* **Canonical Name:** `CZ_REQUEST_TIME` (`clif_parse_TickSend`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `6` bytes
* **Provenance:** `rathena` (`src/map/clif_shuffle.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0360` |
| `2` | `4` | `u32` | `client_tick` | Client millisecond tick counter |

### `0x018a` `CZ_REQ_DISCONNECT`

* **Opcode:** `0x018a` ("Exit to Windows")
* **Canonical Name:** `CZ_REQ_DISCONNECT` (`clif_parse_QuitGame`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `4` bytes
* **Provenance:** `rathena` `src/map/clif_packetdb.hpp:177` (`parseable_packet(0x018a,4,clif_parse_QuitGame,2)`, not overridden in `clif_shuffle.hpp`), `src/map/clif.cpp:11446-11465`. Captures: [`packet_sequence-20261023_031902.txt`](../captures/packet_sequence-20261023_031902.txt) L883-887, [`packet_sequence-20261023_030246.txt`](../captures/packet_sequence-20261023_030246.txt) L894-895, [`login_sequence-rathena.txt`](../captures/login_sequence-rathena.txt) L215-216. Applies to `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x018a` |
| `2` | `2` | `u16` | `type` | `0` = quit. rAthena never reads it (the parse offset `2` is declared but unused); real client sends `00 00` |

* **Refusal conditions (rAthena):** refuse (ack result `1`) if the char has any of `SC_CLOAKING`, `SC_HIDING`, `SC_CHASEWALK`, `SC_CLOAKINGEXCEED`, `SC_SUHIDE`, `SC_NEWMOON`, OR `prevent_logout != 0` and `canlog_tick != 0` and `now - canlog_tick <= prevent_logout`. Dead state is NOT checked.
* **`prevent_logout`** (`conf/battle/player.conf:143`, `battle.cpp:8522`): default `10000` ms (range 0..60000, 0 disables). `canlog_tick` is set to now by: login (`PLT_LOGIN`=1, `pc.cpp:1453`), attacking (`PLT_ATTACK`=2, `unit.cpp:3341`), skill cast start/end (`PLT_SKILL`=4, `unit.cpp:2571,2771`), taking damage (`PLT_DAMAGE`=8, `pc.cpp:9706`). `prevent_logout_trigger` default `14` (`player.conf:152`, = attack|skill|damage; login NOT included). On map entry `canlog_tick = now - prevent_logout` (`pc.cpp:10144`), i.e. logout allowed immediately.
* **Success path:** server sends `0x018b` result `0`, flushes the fifo, and closes the socket only if `drop_connection_on_quit` is set (`conf/battle/client.conf:148`, default `no`, "official: no"). Default: the server does NOT close; it waits for the client to close the TCP connection.
* **On socket close** (`clif.cpp:25695` `clif_quitsave`): if the logout is still prevented (`prevent_logout` window) the session is detached and `map_quit` runs 10 s later (`clif_delayquit`, `clif.cpp:2147-2149`); otherwise `map_quit` runs immediately. `map_quit` (`map.cpp:2221`): `chrif_save(sd, CSAVE_QUIT|CSAVE_INVENTORY|CSAVE_CART)` (char-server `0x2b01` with quitting flag byte at offset 12 = 1; `chrif.cpp:319-323`), then `unit_free_pc` (pet/homun/merc/elemental freed, `unit_free(sd, CLR_TELEPORT)`). `chrif_char_offline` (`0x2b17`, `chrif.cpp:1431`) is used for failed auth/map-change paths, not the normal quit path.
* **Observed (capture 031902):** `0x018a` at T, `0x018b` +24 ms, observer receives `0x0080 ZC_NOTIFY_VANISH` type `2` (wire value 2 = logged out) +13 ms later; rAthena emits it via `unit_free(sd, CLR_TELEPORT)`.

### `0x018b` `ZC_ACK_REQ_DISCONNECT`

* **Opcode:** `0x018b`
* **Canonical Name:** `ZC_ACK_REQ_DISCONNECT` (`clif_disconnect_ack`)
* **Direction:** Server (Map Server) to Client
* **Frame Length:** Fixed `4` bytes
* **Provenance:** `rathena` `src/map/clif_packetdb.hpp:178`, `src/map/clif.cpp:11430-11443`; captures as for `0x018a`. Applies to `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x018b` |
| `2` | `2` | `u16` | `result` | `0` = disconnect (client quits), `1` = cannot disconnect (client shows "wait 10 seconds" message, stays in game); other values ignored by client (per rAthena comment) |

* Also sent with result `1` when `0x00b2` `CZ_RESTART` type 1 (char select) is refused by the same conditions (`clif.cpp:11848-11852`).

### `0x0361` `CZ_CHANGE_DIRECTION`

* **Opcode:** `0x0361`
* **Canonical Name:** `CZ_CHANGE_DIRECTION` (`clif_parse_ChangeDir`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `5` bytes
* **Provenance:** `rathena` (`src/map/clif_shuffle.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0361` |
| `2` | `2` | `u16` | `head_dir` | Head direction (`0` = Straight, `1` = Right, `2` = Left) |
| `4` | `1` | `u8` | `body_dir` | Body orientation (`0`..`7` counterclockwise starting North) |

### `0x0368` `CZ_REQNAME`

* **Opcode:** `0x0368`
* **Canonical Name:** `CZ_REQNAME` (`clif_parse_GetCharNameRequest`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `6` bytes
* **Provenance:** `rathena` (`src/map/clif_shuffle.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0368` |
| `2` | `4` | `u32` | `entity_id` | Target entity block ID (`AccountId` or `EntityId`) |

### `0x0436` `CZ_ENTER`

* **Opcode:** `0x0436`
* **Canonical Name:** `CZ_ENTER` (`clif_parse_WantToConnection`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `23` bytes
* **Provenance:** `rathena` (`src/map/clif_shuffle.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0436` |
| `2` | `4` | `u32` | `account_id` | Player Account ID (`AID`) |
| `6` | `4` | `u32` | `char_id` | Selected Character ID (`CID`) |
| `10` | `4` | `u32` | `auth_code` | Session `login_id1` authentication code |
| `14` | `4` | `u32` | `client_tick` | Client connection tick |
| `18` | `4` | `[u8; 4]` | `padding` | Unused bytes between `pos3=14` and `pos4=22` |
| `22` | `1` | `u8` | `sex` | Account sex (`0` = Female, `1` = Male) |

### `0x0437` `CZ_REQUEST_ACT`

* **Opcode:** `0x0437`
* **Canonical Name:** `CZ_REQUEST_ACT` (`clif_parse_ActionRequest`)
* **Direction:** Client to Server (Map Server)
* **Frame Length:** Fixed `7` bytes
* **Provenance:** `rathena` (`src/map/clif_shuffle.hpp`), `PACKETVER = 20260219`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0437` |
| `2` | `4` | `u32` | `target_id` | Target entity ID (`0` for self actions like sit/stand) |
| `6` | `1` | `u8` | `action` | Action type (`0` = Attack, `2` = Sit, `3` = Stand, `7` = Continuous Attack) |

### `0x0b18` `ZC_INVENTORY_EXPANSION_INFO`

* **Opcode:** `0x0b18`
* **Canonical Name:** `ZC_INVENTORY_EXPANSION_INFO` (`PACKET_ZC_INVENTORY_EXPANSION_INFO`)
* **Direction:** Server to Client (Map Server)
* **Frame Length:** Fixed `4` bytes
* **Provenance:** `rathena` (`src/map/packets_struct.hpp`, `src/map/clif.cpp`), `PACKETVER = 20260219` (`PACKETVER >= 20181031`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | Always `0x0b18` |
| `2` | `2` | `u16` | `expansion_size` | Additional inventory capacity (`0` default) |

## Player Wire Identifiers (Policy)

* **Provenance:** `rathena` `src/map/pc.cpp:1444` (`sd->id = account_id`), `src/map/clif.cpp` (`clif_set_unit_idle` ~L1129, `clif_spawn_unit`, `clif_set_unit_walking` L1369-1380, `clif_clearunit_area` L970, `clif_name` L9927, `clif_sitting` L5329, `clif_parse_GetCharNameRequest` L11471), capture [`login_sequence-rathena.txt`](../captures/login_sequence-rathena.txt) lines 28, 110. Applies to `PACKETVER = 20260219`.
* **Rule:** For a player (`BL_PC`), the in-map object id `bl.id` is the **account_id**. Every map-wire "unit id" field for a player carries `account_id`: `ZC_NOTIFY_VANISH.gid`, `ZC_NOTIFY_ACT.srcID`/`targetID`, `ZC_CHANGE_DIRECTION.srcId`, `ZC_ACK_REQNAMEALL.gid`, `CZ_REQNAME` target, `CZ_REQUEST_ACT` target, and the `AID` field of `0x09fd`/`0x09fe`/`0x09ff`.
* **`char_id` appears only in** the `GID` field of `0x09fd`/`0x09fe`/`0x09ff` (players only; `0` for mobs/NPCs/pets) and in `ZC_ACK_REQNAME_BYGID` (`0x0af7`, answer to a char-id lookup, not to `CZ_REQNAME`).
* **Capture proof:** `CZ_ENTER` carried `account_id = 2000000`, `char_id = 150000`; the later `CZ_REQNAME` for the player's own entity carried `entity_id = 2000000` (account_id).
* **Disguise quirk:** rAthena sends `-bl.id` (negated) in `0x0a30.gid` for a self-name-request while disguised and accepts negative ids in `CZ_REQNAME`/`CZ_REQUEST_ACT`; irrelevant unless disguises are implemented.
* Layouts of the entity/AoI packets: [map_units.md](./map_units.md).
