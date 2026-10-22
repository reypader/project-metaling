# Map Entry Handshake and Player Actions (sit, direction, name, time)

Target client version: `PACKETVER = 20260219`. Citations are `$RATHENA_PATH/src/map/...`. Movement, pathfinding and AoI are in [movement_aoi.md](./movement_aoi.md). Opcode values here come from rAthena's default tables; for the 20260219 client-to-map opcodes, `clif_shuffle.hpp` overrides apply (see `packets/map.md`), and `packet_db[...].pos[]` offsets decide field positions.

## Map entry: CZ_ENTER (`clif_parse_WantToConnection`)

* Provenance: `clif.cpp:10643-10743`. Recorded only (auth is out of scope).
* Reads `account_id, char_id, login_id1 (auth code), client_tick, sex` from the frame. Rejects (`0x006a` with error code, then EOF) when `clif_parse_WantToConnection_sub` fails (invalid account/char/sex); `authfail 1` if server not running; `authfail 8` on double login / existing auth node mismatch (`node->account_id == account_id && char_id == char_id && state == ST_LOGIN` required).
* On accept: creates the session, sends `ZC_AID` (`0x0283`, `<aid>.L`) immediately, then `chrif_authreq` to char-server; the rest (`ZC_ACCEPT_ENTER`) is sent after auth succeeds.

## Map entry: CZ_NOTIFY_ACTORINIT (`clif_parse_LoadEndAck`)

* Provenance: `clif.cpp:10748-11184`. Order of operations for the first login (`state.connect_new`) vs. map change:
  1. Guards: ignore if `sd->prev != nullptr` (already on the grid); if char not fully loaded, defer; `state.rewarp` -> re-send `clif_changemap` and return (`10752-10766`). `warping = 0`.
  2. Self-only look/inventory: `clif_changelook(LOOK_WEAPON,0)`, costume view, clothcolor, `clif_inventorylist`, equip switch list, cart list if any, `clif_updatestatus(SP_WEIGHT)` and `SP_MAXWEIGHT` (these are `ZC_PAR_CHANGE` `0x00b0 <var>.W <value>.L`; `clif.cpp:3550-3558, 3639-3645`) (`10770-10794`). Var ids `SP_WEIGHT = 24`, `SP_MAXWEIGHT = 25`; values in 0.1 weight units (e.g. 20300 = 2030.0 max, 500 = 50.0), always `ZC_PAR_CHANGE` (never `ZC_LONGPAR_CHANGE`), weight first then max weight (capture frames 72-73). Details: [map_units.md#0x00b0-zc_par_change](../packets/map_units.md#0x00b0-zc_par_change).
  3. Guild member info (before spawn so emblems show), `pc_setinvincibletimer`, map user counters (`10798-10813`).
  4. `map_addblock(sd)` (insert into 8x8 block grid; returns 1 on out-of-bounds -> abort) then `clif_spawn(sd)` (`10815-10817`). `clif_spawn` sends the spawn packet (`ZC_NOTIFY_NEWENTRY`-form via `clif_spawn_unit`, plus clothcolor and ball/status extras) to `AREA_WOS`, i.e. everyone within AREA_SIZE (14) EXCEPT the newcomer (`clif.cpp:1680-1740`).
  5. Party/BG hp, then `clif_map_property` (`MAPPROPERTY_NOTHING` for a plain map) to self (`10828-10846`).
  6. Nearby units to the newcomer: `map_foreachinallarea(clif_getareachar, m, x-AREA_SIZE, y-AREA_SIZE, x+AREA_SIZE, y+AREA_SIZE, BL_ALL, sd)` (`10850`). `clif_getareachar` skips the player itself, dispatches items/skill units to their own senders, others to `clif_getareachar_unit` (walking-entry form if the unit is mid-walk, else idle-entry `ZC_NOTIFY_STANDENTRY` form with `state = dead_sit`) (`clif.cpp:5574-5598, 5029-5066`).
  7. Pet, homunculus, mercenary, elemental: each `map_addblock` + `clif_spawn` + their info packets (`10853-10902`).
  8. First login only (`connect_new`, `10904-10988`): `clif_skillinfoblock`, hotkeys, `clif_updatestatus` for BASEEXP, NEXTBASEEXP, JOBEXP, NEXTJOBEXP, SKILLPOINT, then `clif_initialstatus` (`ZC_STATUS` plus `ZC_PAR_CHANGE` for STR/AGI/VIT/INT/DEX/LUK, ATTACKRANGE, ASPD and the renewal trait stats POW, STA, WIS, SPL, CON, CRT, PATK, SMATK, RES, MRES, HPLUS, CRATE, TRAITPOINT, AP, MAXAP, UPOW..UCRT; `clif.cpp:4123-4199`), riding/falcon status icons, friend list toggles, `NPCE_LOGIN` event, optional `clif_changed_dir` if `spawn_direction`.
  9. Map change (not first login): re-sends the six stats and renewal stats via `clif_updatestatus`, aborts scripts, partyinvitation state, config flags, reputation list, night effect, `status_calc_pc`, etc (`10989-11110`).
  10. Tail: guild notice, vending board, `NPCE_LOADMAP`, weather, NPC area touch check at spawn cell, death handling, `skill_unit_move`, `connect_new = 0`.
* `VAR_SPEED` (`SP_SPEED`, id 0) is NOT sent on a normal entry. It is sent only when `battle_status.speed` changes during `status_calc_pc` (`status.cpp:6594-6595`); the baseline is `DEFAULT_WALK_SPEED = 150` (`pc.cpp:1456`) so an unmodified character never triggers it. If the server wants the value, `clif_updatestatus(SP_SPEED)` = `ZC_PAR_CHANGE(var=0, value=speed)` (`clif.cpp:3646-3648`).
* Minimal subset an implementation needs for walking: `map_addblock`, spawn to others (AREA_WOS), nearby-units to self; everything else is feature content.

## Sit / stand: CZ_REQUEST_ACT (action 2 / 3)

* Provenance: `clif.cpp:11697-11830, 5329-5360`, `pc.hpp:1149,1151`, `pc.cpp:13099-13119`, `clif.hpp:691-695`.
* Packet field layout: `<target id>.L <action>.B`; action 0 attack, 1 pick up, 2 sit, 3 stand, 7 continuous attack (`clif.cpp:11807-11816`).
* Broadcast: `ZC_NOTIFY_ACT` (for PACKETVER >= 20131223 header `0x08c8`, 34 bytes: `packetType, srcID, targetID, serverTick, srcSpeed, dmgSpeed, damage(.L), isSPDamage(.B), div(.W), type(.B), damage2(.L)`; `packets.hpp:1412-1426`) with `srcID = unit id`, `type = DMG_SIT_DOWN (2)` / `DMG_STAND_UP (3)`, all other fields zero (`clif_sitting`/`clif_standing`, `clif.cpp:5329-5360`, enum `clif.hpp:691-695`). Target is `AREA`: all players within 14 cells INCLUDING the sitter. 
* Sit preconditions (`11740-11776`): basic-skill check (`basic_skill_check`), not already sitting, not casting, `opt1` clear (except stone-wait/burning), not dancing, not Gravitation self, not `PCBLOCK_SITSTAND`. Failures are silent `break` (or a skill-fail / blocked message).
* Sit effects: `pc_setsit` = `unit_stop_walking(USW_FIXPOS|USW_MOVE_FULL_CELL)`, `unit_stop_attack`, `state.dead_sit = vd.dead_sit = 2` (`pc.hpp:1149`). `pc_issit` is `vd.dead_sit == 2` (`pc.hpp:1151`).
* Sit while walking: before the switch, any action other than attack calls `unit_stop_walking(&sd, USW_FIXPOS)` (`11704-11709`): timer deleted, unit stays on its last COMPLETED cell, `ZC_STOPMOVE` (`0x0088`, `<aid>.L <x>.W <y>.W`) broadcast `AREA` incl. self (`unit.cpp:1695-1755`, `clif.cpp:2205-2213`), then sit proceeds. So the sequence is STOPMOVE then NOTIFY_ACT(sit). `clif_fixpos` is only reached if the unit was actually walking (`walktimer` set) since `USW_FORCE_STOP` is not passed (`unit.cpp:1711`); a standing player that sits gets no `0x0088`. Layout: [map_units.md#0x0088-zc_stopmove](../packets/map_units.md#0x0088-zc_stopmove).
* Repeated sit while sitting: "Bugged client? Just refresh them": re-broadcasts `clif_sitting` and returns (`11746-11750`). Repeated stand while standing: re-broadcasts `clif_standing` (`11778-11782`).
* Moving while sitting: `unit_can_move` is false for sitting players (`unit.cpp:1819`), so `CZ_REQUEST_MOVE` is silently ignored (no reply) after path search passes. 
* Stand: gated on `opt1` and `PCBLOCK_SITSTAND`; `pc_setstand(sd, false)` fails only with forced-sit statuses; it ends `SC_TENSIONRELAX`, calls `clif_standing` itself (`pc.cpp:13109`), clears `dead_sit`; then the parser calls `clif_standing` AGAIN (`clif.cpp:11800`), so rAthena emits the stand notify twice. A clean implementation can send it once.
* Entry packets: sitting state is carried in the idle-entry form (`ZC_NOTIFY_STANDENTRY`) field `state = vd->dead_sit` (`clif.cpp:1160`): 0 standing, 1 dead, 2 sitting. The new-entry (spawn) form and walking-entry form have no sit state field (`clif.cpp:1261-1330`), consistent with units being able to sit only when standing still.

## Direction change: CZ_CHANGE_DIRECTION

* Provenance: `clif.cpp:11565-11620`, `pc.hpp:499,1170`, `unit.hpp:49`.
* Frame: `<head dir>.W <dir>.B` (0x0361 = 5 bytes in packet table; head dir is read as a single byte at `pos[0]`, dir at `pos[1]`; `clif.cpp:11611-11617`).
* `pc_setdir(sd, dir, headdir)` is a bare macro `ud.dir = dir; head_dir = headdir` with NO validation (`pc.hpp:1170`). Documented ranges: body `dir` 0..7 (0 N, 1 NW, 2 W, 3 SW, 4 S, 5 SE, 6 E, 7 NE; `clif.cpp:11565-11580`), `head_dir` 0 straight, 1 turned CW, 2 turned CCW (`clif.cpp:11567-11570`; `pc.hpp:499` says right/left). A Rust server should validate (`dir < 8`, `head_dir <= 2`) and drop the session or ignore on violation; rAthena would store and broadcast garbage (and index `dirx[]` out of range later).
* Broadcast: `clif_changed_dir(*sd, AREA_WOS)`: recipients are everyone in the 29x29 AoI EXCEPT the sender. Packet `ZC_CHANGE_DIRECTION` `0x009c`: `<srcId>.L <headDir>.W <dir>.B`, with head dir 0 for non-players (`clif.cpp:11581-11594`, header `packets.hpp:694`).
* Walking resets `head_dir` to 0 at walk start (`unit.cpp:299-302`).

## Name request: CZ_REQNAME

* Provenance: `clif.cpp:11467-11505`, `packets_struct.hpp:3562-3583`, `clif.cpp:9928-9960`.
* Frame `<id>.L`. Lookup `map_id2bl(id)`; unknown id -> silent return. Negative id equal to `-sd->id` is the disguise self-lookup.
* Range check: `sd->m != bl->m || !check_distance_bl(sd, bl, AREA_SIZE)` -> silent drop (Chebyshev 14).
* Reply: `clif_name(sd, bl, SELF)` to the requester only. For a player target: `ZC_ACK_REQNAMEALL` (PACKETVER >= 20150225/RE 20141126 -> opcode `0x0a30`: `<gid>.L name[24] party_name[24] guild_name[24] position_name[24] title_id.L`; older `0x0195` without title; both with fixed 24-byte NUL-padded strings, empty strings when no party/guild). Party name is only shown together with a guild. NPC/mob targets use other reply forms (`ZC_ACK_REQNAMEALL_NPC` / `0x0095`).

## Time sync: CZ_REQUEST_TIME

* Provenance: `clif.cpp:11187-11209`, `packets.hpp:2011`.
* Frame `<client tick>.L`; stored into `sd->client_tick` (no validation). Reply `ZC_NOTIFY_TIME` `0x007f` `<time>.L` with `client_tick(gettick())`: the server's monotonic millisecond tick (the same time base as `moveStartTime` in `ZC_NOTIFY_PLAYERMOVE` and `ZC_ACCEPT_ENTER`). Self only. The client tick is NOT echoed back.
