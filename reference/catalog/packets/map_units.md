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
| Walk interrupted (sit/stand/NPC click/pickup via `unit_stop_walking(USW_FIXPOS)`, failed mid-walk repath, knockback) | `clif_fixpos` L2205 | `0x0088` | `AREA` (includes the unit; see section) |

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

## `0x0088` `ZC_STOPMOVE`

* **Direction:** Map to Client. **Frame:** Fixed `10` = `sizeof(PACKET_ZC_STOPMOVE)` (`int16 + uint32 + uint16 + uint16`, packed). **Packet DB:** NOT in `clif_packetdb.hpp`/`clif_shuffle.hpp` (no `0x0088` entry; length is the struct size). Project `table.rs` has `Fixed(10)` (since engine-foundation M3). No capture shows it (`login_sequence-*.log` have no walk/sit). Applies to 20260219 (no `#if PACKETVER` branch).
* **Provenance:** `rathena` `src/map/packets.hpp:728-734` (struct + `DEFINE_PACKET_HEADER(ZC_STOPMOVE, 0x88)`), sender `clif.cpp:2204-2219` (`clif_fixpos(const block_list& bl)`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0088` |
| `2` | `4` | `u32` | `gid` | `bl.id` (account_id for players) |
| `6` | `2` | `u16` | `x` | `bl.x` at send time (cell the unit is pinned to) |
| `8` | `2` | `u16` | `y` | `bl.y` |

* **Sender:** `clif_fixpos(bl)`: `clif_send(&packet, 10, &bl, AREA)`. `AREA` = every player within `AREA_SIZE` (14) of the unit INCLUDING the unit itself (no `_WOS`; `clif.cpp:517-526`). If the unit is disguised, a second packet with the disguise id is sent `SELF`.
* **When (movement-relevant):**
  * `unit_stop_walking(bl, type)` with `USW_FIXPOS` (0x1) (`unit.cpp:1696-1755`, fixpos at `:1734-1738`). Sets `ud->sx = ud->sy = 8` first. It is a no-op returning false (NO packet) if `walktimer == INVALID_TIMER` and `USW_FORCE_STOP` (0x8) is not set (`:1711`). So a unit that is not walking gets no `0x0088` from a plain `USW_FIXPOS`.
  * Player non-attack `CZ_REQUEST_ACT` (sit, stand, pickup, ...): `unit_stop_walking(&sd, USW_FIXPOS)` (`clif.cpp:11704-11709`), and `pc_setsit` uses `USW_FIXPOS|USW_MOVE_FULL_CELL` (`pc.cpp:13099+`). With `USW_MOVE_FULL_CELL`, if at least half the current step has elapsed the unit is advanced to the next cell BEFORE the fixpos, so x/y in the packet is the destination cell of the in-flight step; otherwise the last completed cell. Sequence on the wire: `0x0088` then `0x08c8` (sit).
  * Mid-walk repath failure (`change_walk_target`), `unit.cpp:742`; monster blocked by icewall `:614`; skill/dash cancels (`unit.cpp:211,1034,2975,3286`), knockback/slide (`clif_blown` L2192-2197, `skill.cpp`, `status.cpp:15005`).
* The client treats it as "snap this unit to (x,y) and stop its walk animation". `ud->to_x/to_y` are reset to the current cell and the path is cleared after the send.

## `0x0086` `ZC_NOTIFY_MOVE` (unused)

* **Never sent by rAthena at 20260219** (struct commented out, `packets.hpp:667-679`). Layout if it were: `u16 id, u32 gid, [u8;6] move_data, u32 move_start_time` = 16 bytes. Not in rAthena's packet DB. Do not implement; observers get `0x09fd`.

## `0x009c` `ZC_CHANGE_DIRECTION`

* **Direction:** Map to Client. **Frame:** Fixed `9`. **Provenance:** `rathena` `packets.hpp:688-694`, `clif.cpp:11559-11600`. Not in rAthena packet DB; project `table.rs` has `Fixed(9)` (since M3).

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

* **Weight vars (`enum _sp`, `map.hpp:498-505`, positional):** `SP_WEIGHT = 24` (`0x18`), `SP_MAXWEIGHT = 25` (`0x19`). (Neighbours: `SP_NEXTJOBEXP = 23`, `SP_1a = 26`.) Sent via `clif_par_change` (`clif.cpp:3550-3558`) from `clif_updatestatus` (`:3637-3646`): `SP_WEIGHT` -> `sd.weight`, `SP_MAXWEIGHT` -> `sd.max_weight`, both plain `i32` in `0x00b0` (NOT `0x00b1 ZC_LONGPAR_CHANGE`, `packets_struct.hpp:361-366`, which rAthena does not use for weight). `SP_WEIGHT` also calls `pc_updateweightstatus` (SC_WEIGHT50/90 icons) first.
* **Units: 0.1 weight per unit** (item db weight 10 = 1.0). `max_weight = job_db max_weight_base + STR * 300 (+ bonuses)` (`status.cpp:3663`; base 20000 = 2000.0). Capture `login_sequence-rathena.log` (STR 1 novice): `SP_MAXWEIGHT` payload `19 00 4c 4f 00 00` = 20300 (20000 + 300) and `SP_WEIGHT` payload `18 00 f4 01 00 00` = 500 (50.0). Overweight thresholds use `weight * 100 / max_weight` (`pc.cpp:3016`).
* **Order on map entry:** `clif_parse_LoadEndAck` sends `SP_WEIGHT` then `SP_MAXWEIGHT` (`clif.cpp:10793-10794`); capture frames 72-73 confirm (after `CZ_NOTIFY_ACTORINIT`, before `0x099b` map property and the `0x09ff` nearby units). The same pair is also sent by `clif_refresh` (`:9851-9852`), and individually on change (`pc_additem` `pc.cpp:6070`, `status_calc_pc` `status.cpp:3693-3697`).

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
* Players always get `0x0a30`. Non-PC units (NPC/pet/hom/merc/elem) get `0x0adf` `ZC_ACK_REQNAMEALL_NPC` (see [below](#0x0adf-zc_ack_reqnameall_npc-reply-to-cz_reqname-for-an-npc)); mobs get `0x0195`-family layouts (guardian) or the NPC form.

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

* **Direction:** Map to Client. **Frame:** Fixed `32` (project `table.rs` fixed from Dynamic in M3). **Provenance:** `rathena` `packets.hpp:1736-1741` (branch `PACKETVER >= 20180221`), `clif.cpp:7057-7071` (`clif_solved_charname`, answer to `CZ_REQNAME_BYGID` via `map_reqnickdb`, `clif_parse_SolveCharName` L13553). Used for char_id -> name resolution (e.g. whisper/friend/party lookups), not for entities in view.

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

## NPC units in entry packets (static script/shop/warp NPCs)

Full lifecycle, hide/disable rules and ids: [mechanics/npcs.md](../mechanics/npcs.md). `PACKETVER = 20260219`, source `clif.cpp` unless noted.

* **Which packet:** an NPC (BL_NPC) never gets `0x09fe`/`0x09fd` in the static case. `clif_spawn` (L1680) sends `0x09ff` (`clif_set_unit_idle`, L1041) to `AREA_WOS` when `vd->dead_sit == 0` (always, for static NPCs; `0x09fe` only if `dead_sit != 0`). `clif_getareachar_unit` (L5029) sends `0x09ff` to the single observer (`SELF`) unless `ud->walktimer` is running (never for static NPCs). Map load spawns NOTHING to anyone (no players yet: `if(map_getmapdata(nd->m)->users) clif_spawn(nd)`, `npc.cpp:3866,3934`). Players get NPCs via `clif_getareachar` (map entry, `BL_ALL`) and `clif_insight` (walk).
* **`object_type`** = `clif_bl_type(bl, walking=false)` (L345-383, branch `PACKETVER >= 20170726`): `pcdb_checkid(job) && walking` -> `0` (irrelevant, static); `mobdb_checkid(job)` -> `0xC`; otherwise **`0x6`**. So every normal NPC and every warp is `0x06`; a script NPC that uses a mob sprite id (1001..) is `0x0C`; a script NPC that uses a PC job sprite id is still `0x06` when idle. No special case for warps / `NPCTYPE_WARP`.

| Field | Value for a static NPC | Source |
| :--- | :--- | :--- |
| `object_type` | `0x06` (`0x0C` if sprite id is a mob id) | L345-383 |
| `aid` | `nd->id` (>= 110000000, see npcs.md) | L1041-1140 |
| `gid` | `0` (`sd == nullptr`) | L1041-1140 |
| `speed` | `nd->speed` = `DEFAULT_NPC_WALK_SPEED` = **200** (`common/mmo.hpp:94`, set in every parse fn; `status_get_speed` returns `nd->speed` for BL_NPC, `status.cpp:9107`) | |
| `body_state`, `health_state` | `sc.opt1`, `sc.opt2` = 0 | |
| `effect_state` | `sc.option` = 0 normally; `0x2` (OPTION_HIDE) for state `HIDDEN`; `0x4` (OPTION_CLOAK) for `CLOAKED` (per-player xor for `cloaked_npc`, L1112-1125) | |
| `job` | sprite/class id (`vd->look[LOOK_BASE]`): numeric or constant from the script line (`npc_parseview`, `npc.cpp:3743`), warps = `45` (`JT_WARPNPC`) | `npc.cpp:3857,3926` |
| `head`, `weapon`, `shield`, `accessory`, `accessory2`, `accessory3`, `head_palette`, `body_palette`, `robe` | all `0` (`npc_viewdb` is zeroed, only `LOOK_BASE` set). Exception: sprite is a PC job id: `LOOK_BODY2 = class`, hair clamped to [MIN,MAX_HAIR_STYLE] (`status.cpp:9496-9500`). Exception: class `JT_GUILD_FLAG` (`warp_point_debug` only) swaps accessory/accessory2/accessory3 to emblem/guild id words. | |
| `head_dir` | `0` | |
| `guild_id`, `emblem_ver` | `0` unless a castle-flag script NPC (`u.scr.guild_id`, `status.cpp:9191,9240`) | |
| `honor`, `virtue`, `is_pk_mode_on` | `0` (`virtue` = `sc.opt3` = 0) | |
| `sex` | `0` (`vd->sex`, never set for NPCs) | |
| `pos_dir` | `WBUFPOS(x, y, unit_getdir)`; `dir` = facing from the script line (`nd->ud.dir`), warps `0` | |
| `x_size`, `y_size` | `0` (only PCs get 5) | |
| `state` | `0` (`vd->dead_sit`) | |
| `clevel` | `0` (`clif_setlevel` returns 0 for BL_NPC, default `client_limit_unit_lv: 0`, `clif.cpp:1025-1034`) | |
| `font` | `0` | |
| `max_hp`, `hp` | `-1`, `-1` (`0xFFFFFFFF`) | |
| `is_boss` | `0` (`BOSSTYPE_NONE`) | |
| `body` | `0` (`LOOK_BODY2`; = class only for PC-sprite NPCs) | |
| `name` | `nd->name` copied with `safestrncpy(.., NAME_LENGTH=24)` -> at most 23 chars + NUL, rest zero. NOT truncated at `#`; the `#...` suffix is sent and the client hides it. For `Display::unique` names, `nd->name` is the Display part only. Warps: `nd->name` = the warp name from the file (e.g. `prt001`). | `npc.cpp:3668-3706` |

* Total length stays `108` (`0x09ff`). Layout/offsets: see the `0x09ff` section above (identical).
* **`CLR_OUTSIGHT` vanish:** `0x0080` type `0` (see npcs.md).
* No capture available: `tools/proxy/packet_captures/` does not exist in the repo (captures were removed); everything here is source-derived, not capture-verified.

## `0x0adf` `ZC_ACK_REQNAMEALL_NPC` (reply to `CZ_REQNAME` for an NPC)

* **Direction:** Map to Client. **Frame:** Fixed `58`. **Provenance:** `rathena` `packets_struct.hpp:3586-3594` (branch `PACKETVER_MAIN_NUM >= 20180207`, true at 20260219; older form was `0x0095` 30 bytes), sender `clif.cpp:9987-10020` (`clif_name`, case BL_NPC; also pet/hom/merc/elem and mobs without `show_mob_info`). Request side: `0x0368` `CZ_REQNAME`, see [map.md](./map.md) / [map_entry_actions.md](../mechanics/map_entry_actions.md#name-request-cz_reqname). Not in rAthena packet DB (struct size).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0` | `2` | `u16` | `packet_id` | `0x0adf` |
| `2` | `4` | `i32` | `gid` | `nd->id` |
| `6` | `4` | `i32` | `group_id` | `ud->group_id` (0 unless set by script) |
| `10` | `24` | `[u8; 24]` | `name` | `nd->name`, `safestrncpy` 23 chars + NUL (includes any `#suffix`) |
| `34` | `24` | `[u8; 24]` | `title` | `ud->title` (empty unless `unittitle`-style script set it) |

* Range check: same map and `check_distance_bl(sd, bl, AREA_SIZE)` (Chebyshev 14) else silent drop; no visibility check (disabled/invisible NPCs still answer). Reply `SELF` only.

## `0x0091` `ZC_NPCACK_MAPMOVE` (length confirmation)

* Confirmed **22 bytes**: `packets.hpp:696-701` (`2 + 16 + 2 + 2`). Capture `login_sequence-rathena.log:35-36` proves it: the proxy table framed it as 18 bytes, and the leftover 4 bytes (`9b 00 b5 00` = x 155, y 181) were mis-parsed as bogus opcode `0x009b`. Project `table.rs` had `Fixed(18)`; fixed to `Fixed(22)` in engine-foundation M3. Section: [map.md#0x0091-zc_npcack_mapmove](./map.md#0x0091-zc_npcack_mapmove).

## Project length table status

All discrepancies found during engine-foundation research (`0x0091` 18 -> 22, `0x0af7` Dynamic -> Fixed(32), missing `0x0a30`/`0x009c`/`0x0088`) were fixed in `crates/ro-packets/src/table.rs` in M3 and are covered by its `pkt_01_engine_foundation_lengths` test. `0x0086` stays unmapped (never sent).
