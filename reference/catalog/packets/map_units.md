# Map Server Entity / AoI Packets

Target client version: `PACKETVER = 20260219`. All integers little-endian, structs are `__attribute__((packed))`. Player id policy: see [map.md#player-wire-identifiers-policy](./map.md#player-wire-identifiers-policy) (`bl.id` = account_id; char_id only in `GID` of the 0x09fx entry packets).

**Length source note:** rAthena's `clif_packetdb.hpp` registers only `0x09fd/0x09fe/0x09ff` (`:1731-1733`, all `-1` = Dynamic) among the opcodes below. The others are not in rAthena's packet DB; their lengths are `sizeof` of the `PACKET_*` structs in `src/map/packets.hpp` / `packets_struct.hpp` (computed below).

**Preprocessor resolution at 20260219** (`src/config/packets.hpp:21-45`: `PACKETVER_RE` undefined, so `PACKETVER_MAIN_NUM = 20260219`; `PACKETVER_ZERO` undefined): `shield` is `u32` (>= 20181121), `robe` present, `effectState`/`weapon`/`virtue` are 32-bit, `font`, `maxHP/HP/isBoss`, `body` (`u16`, `LOOK_BODY2`, >= 20231220), `name[24]` (`NAME_LENGTH = 24`, `common/mmo.hpp:154`) all present.

## Which packet is sent when (`PACKETVER = 20260219`)

| Situation | Function (`src/map/clif.cpp`) | Packet | Target |
| :--- | :--- | :--- | :--- |
| Observed unit starts/changes a walk | `clif_move` L2066 -> `clif_set_unit_walking` L1369 | `0x09fd` (NOT `0x0086`) | `AREA_WOS` (everyone in range except the mover) |
| The walking player itself | `clif_walkok` L2053 | `0x0087` | `SELF` |
| Unit enters an observer's view while walking (`ud.walktimer != INVALID_TIMER`) | `clif_getareachar_unit` L5029 (L5062-5065) -> `clif_set_unit_walking` | `0x09fd` | `SELF` of the observer |
| Unit enters an observer's view while standing | `clif_getareachar_unit` -> `clif_set_unit_idle` | `0x09ff` STANDENTRY | `SELF` of the observer |
| Unit (re)spawns: map enter (L10817), warp-in, mob spawn, respawn | `clif_spawn` L1680 -> `clif_spawn_unit` L1213 | `0x09fe` NEWENTRY | `AREA_WOS` |
| Same, but NPC with `!dead_sit` | `clif_spawn` -> `clif_set_unit_idle` | `0x09ff` | `AREA_WOS` |
| Unit leaves sight / dies / logs out / teleports | `clif_clearunit_area` L970 / `clif_clearunit_single` L952 | `0x0080` | `AREA_WOS` (`AREA` for type 1 died) / `SELF` |
| Turn | `clif_changed_dir` L11581 | `0x009c` | `AREA_WOS` (parser L11619) or `SELF` |
| Sit / stand | `clif_sitting` L5329 / `clif_standing` L5347 | `0x08c8` | `AREA` (includes the actor) |

`0x0086` (`ZC_NOTIFY_MOVE`, 16 bytes if it existed) is **commented out as "Unused packet (alpha?)"** (`packets.hpp:667-679`) and is never sent. `clif_walkok` sx/sy: `WBUFPOS2(moveData, 0, sd.x, sd.y, sd.ud.to_x, sd.ud.to_y, 8, 8)` (`clif.cpp:2059`) so the mover gets sx0 = sy0 = 8 always. `0x09fd` uses `ud.sx, ud.sy` (`clif.cpp:1414`), which are 8 at walk start (`unit.cpp:561-562,1685,1734`) and a sub-cell 0..15 only when a walk is shortened mid-cell (`unit.cpp:1654`).

## `0x0080` `ZC_NOTIFY_VANISH`

* **Direction:** Map to Client. **Frame:** Fixed `7`. **Packet DB:** not listed (struct size). 
* **Provenance:** `rathena` `src/map/packets.hpp:604-609`, `clif.cpp:944-990`. Applies to 20260219. Project `table.rs:44` agrees (`Fixed(7)`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0080` |
| `2` | `4` | `u32` | `gid` | `bl.id` (account_id for players) |
| `6` | `1` | `u8` | `type` | `0` out of sight, `1` died, `2` logged out, `3` teleport, `4` trickdead (`clr_type`, `clif.hpp:356`) |

## `0x0087` `ZC_NOTIFY_PLAYERMOVE`

* **Direction:** Map to Client (self only). **Frame:** Fixed `12`. **Provenance:** `rathena` `packets.hpp:681-686`, `clif.cpp:2053-2062`; `table.rs:48` agrees.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0087` |
| `2` | `4` | `u32` | `move_start_time` | `client_tick(gettick())` |
| `6` | `6` | `[u8; 6]` | `move_data` | `WBUFPOS2(x0,y0,x1,y1,sx0=8,sy0=8)`, see [bitpacking.md](./bitpacking.md) |

## `0x0086` `ZC_NOTIFY_MOVE` (unused)

* **Never sent by rAthena at 20260219** (struct commented out, `packets.hpp:667-679`). Layout if it were: `u16 id, u32 gid, [u8;6] move_data, u32 move_start_time` = 16 bytes. Not in rAthena's packet DB. Do not implement; observers get `0x09fd`.

## `0x009c` `ZC_CHANGE_DIRECTION`

* **Direction:** Map to Client. **Frame:** Fixed `9`. **Provenance:** `rathena` `packets.hpp:688-694`, `clif.cpp:11559-11600`. Not in rAthena packet DB, absent from project `table.rs`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x009c` |
| `2` | `4` | `u32` | `src_id` | `bl.id` (account_id for players) |
| `6` | `2` | `u16` | `head_dir` | `0` straight, `1` CW, `2` CCW (0 for non-PC) |
| `8` | `1` | `u8` | `dir` | `0` N, `1` NW, `2` W, `3` SW, `4` S, `5` SE, `6` E, `7` NE |

## `0x00b0` `ZC_PAR_CHANGE`

* **Direction:** Map to Client. **Frame:** Fixed `8`. **Provenance:** `rathena` `packets_struct.hpp:355-359`, `clif.cpp:3550`; capture lines 37-59 show 8-byte frames; `table.rs:64` agrees.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x00b0` |
| `2` | `2` | `u16` | `var_id` | `SP_*` status id (see `clif_updatestatus`, `clif.cpp:3600+`) |
| `4` | `4` | `i32` | `count` | New value |

## `0x08c8` `ZC_NOTIFY_ACT` (a.k.a. ZC_NOTIFY_ACT3)

* **Direction:** Map to Client. **Frame:** Fixed `34`. **Provenance:** `rathena` `packets.hpp:1405-1430` (branch `PACKETVER >= 20131223`), comment `clif.cpp:5220`; `table.rs:748` agrees.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x08c8` |
| `2` | `4` | `i32` | `src_id` | actor `bl.id` (account_id for players) |
| `6` | `4` | `i32` | `target_id` | target; **0 for sit/stand** |
| `10` | `4` | `i32` | `server_tick` | 0 for sit/stand |
| `14` | `4` | `i32` | `src_speed` | 0 for sit/stand (attack: amotion) |
| `18` | `4` | `i32` | `dmg_speed` | 0 for sit/stand |
| `22` | `4` | `i32` | `damage` | 0 for sit/stand |
| `26` | `1` | `i8` | `is_sp_damage` | 0 |
| `27` | `2` | `u16` | `div` | 0 for sit/stand |
| `29` | `1` | `u8` | `type` | `e_damage_type` (`clif.hpp:690`): `0` damage, `1` pick up, **`2` sit down, `3` stand up**, `4` endure, `8` multi-hit, `10` crit, `11` lucky dodge |
| `30` | `4` | `i32` | `damage2` | 0 for sit/stand |

`clif_sitting`/`clif_standing` zero-initialize the struct (`PACKET_ZC_NOTIFY_ACT p{}`) and set only `srcID` and `type` (`clif.cpp:5329-5360`), sent to `AREA` (actor included). `CZ_REQUEST_ACT` target_id is ignored by rAthena for sit/stand (`clif_parse_ActionRequest_sub`, L11697+; only attack uses it). What the 20260219 client actually puts there when sitting is **not verified** (no sit capture exists); do not validate it.

## `0x0a30` `ZC_ACK_REQNAMEALL` (reply to `CZ_REQNAME` for a player)

* **Direction:** Map to Client. **Frame:** Fixed `106`. **Provenance:** `rathena` `packets_struct.hpp:3562-3573` (branch `PACKETVER_MAIN_NUM >= 20150225`), `clif.cpp:9927-9985` (`clif_name`, BL_PC case; called from `clif_parse_GetCharNameRequest` L11471). Capture `login_sequence-rathena.log` line 111 shows `0x0a30` as reply to `CZ_REQNAME` (the proxy's table lacked it, so the frame was mis-split; ignore those garbage rows). Not in rAthena packet DB (struct size).
* Players always get `0x0a30`. Non-PC units (NPC/pet/hom/merc/elem) get a different `PACKET_ZC_ACK_REQNAMEALL_NPC` (`packets_struct.hpp:3586+`, not catalogued yet); mobs get `0x0195`-family layouts (guardian) or the NPC form.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0a30` |
| `2` | `4` | `i32` | `gid` | `bl.id` = account_id (negated for own disguise) |
| `6` | `24` | `[u8; 24]` | `name` | char name (or `fakename`), NUL-padded |
| `30` | `24` | `[u8; 24]` | `party_name` | only if in a guild or `display_party_name` |
| `54` | `24` | `[u8; 24]` | `guild_name` | |
| `78` | `24` | `[u8; 24]` | `position_name` | guild position, or clan name |
| `102` | `4` | `i32` | `title_id` | `status.title_id` |

## `0x0af7` `ZC_ACK_REQNAME_BYGID` (NOT the CZ_REQNAME reply)

* **Direction:** Map to Client. **Frame:** Fixed `32` (project `table.rs:960` marks it Dynamic; that disagrees with the struct). **Provenance:** `rathena` `packets.hpp:1736-1741` (branch `PACKETVER >= 20180221`), `clif.cpp:7057-7071` (`clif_solved_charname`, answer to `CZ_REQNAME_BYGID` via `map_reqnickdb`, `clif_parse_SolveCharName` L13553). Used for char_id -> name resolution (e.g. whisper/friend/party lookups), not for entities in view.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0af7` |
| `2` | `2` | `u16` | `flag` | `3` name found, `2` not found |
| `4` | `4` | `u32` | `char_id` | **char_id**, not account_id |
| `8` | `24` | `[u8; 24]` | `name` | |

## `0x09ff` `ZC_NOTIFY_STANDENTRY11` (idle unit comes into view)

* **Direction:** Map to Client. **Frame:** Dynamic (`-1`, `clif_packetdb.hpp:1733`); rAthena always writes `PacketLength = 108` (`sizeof(packet_idle_unit)`). Capture lines 85-91 show 108-byte frames. **Provenance:** `packets_struct.hpp:832-905`, `clif.cpp:1041-1210`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x09ff` |
| `2` | `2` | `u16` | `packet_length` | `108` |
| `4` | `1` | `u8` | `object_type` | `0` PC, `1` disguised PC, `2` item, `3` skill, `4` unknown/chat, `5` mob/NPC-mob, `6` NPC, `0xC` walking-capable NPC, `0xD` ABR, `0xE` bionic (`clif_bl_type` L345) |
| `5` | `4` | `u32` | `aid` | `bl.id` (account_id for players) |
| `9` | `4` | `u32` | `gid` | `char_id` for players, `0` otherwise |
| `13` | `2` | `i16` | `speed` | |
| `15` | `2` | `i16` | `body_state` | `sc.opt1` |
| `17` | `2` | `i16` | `health_state` | `sc.opt2` |
| `19` | `4` | `i32` | `effect_state` | `sc.option` |
| `23` | `2` | `i16` | `job` | `LOOK_BASE` |
| `25` | `2` | `u16` | `head` | hair style |
| `27` | `4` | `u32` | `weapon` | |
| `31` | `4` | `u32` | `shield` | |
| `35` | `2` | `u16` | `accessory` | head bottom |
| `37` | `2` | `u16` | `accessory2` | head top |
| `39` | `2` | `u16` | `accessory3` | head mid |
| `41` | `2` | `i16` | `head_palette` | |
| `43` | `2` | `i16` | `body_palette` | |
| `45` | `2` | `i16` | `head_dir` | |
| `47` | `2` | `u16` | `robe` | |
| `49` | `4` | `u32` | `guild_id` | `GUID` |
| `53` | `2` | `i16` | `emblem_ver` | |
| `55` | `2` | `i16` | `honor` | `status.manner` |
| `57` | `4` | `i32` | `virtue` | `sc.opt3` |
| `61` | `1` | `u8` | `is_pk_mode_on` | karma != 0 |
| `62` | `1` | `u8` | `sex` | |
| `63` | `3` | `[u8; 3]` | `pos_dir` | `WBUFPOS` x,y,dir |
| `66` | `1` | `u8` | `x_size` | `5` for PC else `0` |
| `67` | `1` | `u8` | `y_size` | |
| `68` | `1` | `u8` | `state` | `vd->dead_sit` (0 stand, 1 dead, 2 sit) |
| `69` | `2` | `i16` | `clevel` | |
| `71` | `2` | `i16` | `font` | |
| `73` | `4` | `i32` | `max_hp` | `-1` unless damaged mob |
| `77` | `4` | `i32` | `hp` | `-1` unless damaged mob |
| `81` | `1` | `u8` | `is_boss` | |
| `82` | `2` | `u16` | `body` | `LOOK_BODY2` |
| `84` | `24` | `[u8; 24]` | `name` | |

## `0x09fe` `ZC_NOTIFY_NEWENTRY11` (unit spawns in view)

* **Direction:** Map to Client. **Frame:** Dynamic (`-1`, `clif_packetdb.hpp:1732`); rAthena writes `PacketLength = 107`. **Provenance:** `packets_struct.hpp:687-754`, `clif.cpp:1213-1330`. Identical to `0x09ff` for offsets `0..67` (including `object_type`, `aid`, `gid`, ..., `pos_dir`, `x_size`, `y_size`) but **has no `state` byte**, so everything after `y_size` shifts down by 1.

| Offset | Size | Field (changes vs `0x09ff`) |
| :--- | :--- | :--- |
| `0..67` | `68` | same as `0x09ff` offsets `0..67` (`packet_length = 107`) |
| `68` | `2` | `clevel` |
| `70` | `2` | `font` |
| `72` | `4` | `max_hp` |
| `76` | `4` | `hp` |
| `80` | `1` | `is_boss` |
| `81` | `2` | `body` |
| `83` | `24` | `name` |

## `0x09fd` `ZC_NOTIFY_MOVEENTRY11` (walking unit, new or changed walk)

* **Direction:** Map to Client. **Frame:** Dynamic (`-1`, `clif_packetdb.hpp:1731`); rAthena writes `PacketLength = 114`. **Provenance:** `packets_struct.hpp:758-830`, `clif.cpp:1369-1440`. Same as `0x09ff` up to `accessory` (offset 35), then:

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0..36` | `37` | | as `0x09ff` offsets `0..36` | `packet_length = 114`; `object_type` via `clif_bl_type(bl, walking=true)` |
| `37` | `4` | `u32` | `move_start_time` | `client_tick(gettick())` |
| `41` | `2` | `u16` | `accessory2` | |
| `43` | `2` | `u16` | `accessory3` | |
| `45` | `2` | `i16` | `head_palette` | |
| `47` | `2` | `i16` | `body_palette` | |
| `49` | `2` | `i16` | `head_dir` | |
| `51` | `2` | `u16` | `robe` | |
| `53` | `4` | `u32` | `guild_id` | |
| `57` | `2` | `i16` | `emblem_ver` | |
| `59` | `2` | `i16` | `honor` | |
| `61` | `4` | `i32` | `virtue` | |
| `65` | `1` | `u8` | `is_pk_mode_on` | |
| `66` | `1` | `u8` | `sex` | |
| `67` | `6` | `[u8; 6]` | `move_data` | `WBUFPOS2(x, y, to_x, to_y, ud.sx, ud.sy)` |
| `73` | `1` | `u8` | `x_size` | |
| `74` | `1` | `u8` | `y_size` | (no `state` byte) |
| `75` | `2` | `i16` | `clevel` | |
| `77` | `2` | `i16` | `font` | |
| `79` | `4` | `i32` | `max_hp` | |
| `83` | `4` | `i32` | `hp` | |
| `87` | `1` | `u8` | `is_boss` | |
| `88` | `2` | `u16` | `body` | |
| `90` | `24` | `[u8; 24]` | `name` | |

## `0x0091` `ZC_NPCACK_MAPMOVE` (length confirmation)

* Confirmed **22 bytes**: `packets.hpp:696-701` (`2 + 16 + 2 + 2`). Capture `login_sequence-rathena.log:35-36` proves it: the proxy table framed it as 18 bytes, and the leftover 4 bytes (`9b 00 b5 00` = x 155, y 181) were mis-parsed as bogus opcode `0x009b`. Project `crates/ro-packets/src/table.rs:54` has `Fixed(18)`, which is wrong (see below). Section: [map.md#0x0091-zc_npcack_mapmove](./map.md#0x0091-zc_npcack_mapmove).

## Discrepancies found in the project length table (`crates/ro-packets/src/table.rs`, read-only check)

| Opcode | Table says | Correct at 20260219 |
| :--- | :--- | :--- |
| `0x0091` | `Fixed(18)` | `Fixed(22)` |
| `0x0af7` | `Dynamic` | `Fixed(32)` |
| `0x0a30` | missing | `Fixed(106)` |
| `0x009c` | missing | `Fixed(9)` |
| `0x0086` | missing | unused; `Fixed(16)` if ever needed |
| `0x09fd` comment | `ZC_NOTIFY_MOVE` | name is `ZC_NOTIFY_MOVEENTRY11` (distinct from `0x0086 ZC_NOTIFY_MOVE`) |
