# Movement, Pathfinding, and Area of Interest (AoI)

Target client version: `PACKETVER = 20260219`.
All `file:line` citations are `$RATHENA_PATH/src/...` (rAthena), verified by reading source. Entry handshake, sit/stand, direction, name, and time sync live in [map_entry_actions.md](./map_entry_actions.md).
Conventions: rAthena Y axis points up (north = y+1). `dirx={0,-1,-1,-1,0,1,1,1}`, `diry={1,1,0,-1,-1,-1,0,1}` (`map/unit.cpp:70-71`), dir 0=N,1=NW,2=W,3=SW,4=S,5=SE,6=E,7=NE (`map/path.hpp:17-25`).

## Overview (lifecycle summary)

* Provenance: Workspace policy plus the detailed sections below.
* Entry: `CZ_ENTER` -> `ZC_AID`/`ZC_ACCEPT_ENTER` -> client loads -> `CZ_NOTIFY_ACTORINIT`; see map_entry_actions.md.
* Move: `CZ_REQUEST_MOVE` -> `unit_walktoxy` -> A* (`path_search`) -> `ZC_NOTIFY_PLAYERMOVE` to self at step start; cell changes at step END via `unit_walktoxy_timer`.
* Warp portal stepping, map change and `ZC_NPCACK_MAPMOVE` are out of scope of this file.

## 1. Pathfinding (A*)

* Provenance: `map/path.hpp:11-14`, `map/path.cpp:21-55,132-193,269-432`, `map/unit.cpp:841-916`, `map/battle.cpp:8684`, `conf/battle/client.conf:42`, `config/core.hpp:26`. Applies to 20260219 (not packet-version dependent).
* `MAX_WALKPATH = 32` (`path.hpp:14`). `walkpath_data.path[32]` of `int8` enum, `path_len`/`path_pos` are `unsigned char` (`path.hpp:29-32`).
* Step costs: `MOVE_COST = 10`, `MOVE_DIAGONAL_COST = 14` (`path.hpp:11-12`).
* Heuristic: Manhattan distance times `MOVE_COST`: `10*(|dx|+|dy|)` (`path.cpp:55`). Comment says it is deliberately inadmissible (overestimates), mimicking the client. `f = g + h`; open set is a binary min-heap on `f_cost`; ties broken by heap order (not specified). Do not "fix" the heuristic if exact client-path parity matters.
* Neighbour expansion order is fixed (`path.cpp:392-407`): SE(x+1,y-1), E, NE(x+1,y+1), N(y+1), NW, W, SW, S. Order matters only for equal-f ties.
* Corner cutting rule (`path.cpp:103-110, 385-407`): allowed cardinals are computed from the current cell (each in-bounds neighbour not blocked). A diagonal is expanded only if BOTH adjacent cardinals are walkable AND the diagonal target cell itself is walkable. No corner cutting.
* Walkability test is `CELL_CHKNOPASS` (cell not `walkable`). Out-of-range tests: neighbour checks use `y < ys-1` style bounds with `xs = map.xs - 1`; and `map_getcellp` treats `x >= xs-1` or `y >= ys-1` (the LAST row and column) as non-passable (`map/map.cpp:3329-3331`, "intentionally overrides the last row and column"). So usable coordinates are `0..=xs-2` and `0..=ys-2`.
* Start cell is NOT checked for walkability ("would get you stuck", `path.cpp:284-286`). Start must be in-bounds.
* Destination must be in bounds and walkable, else `path_search` returns false (`path.cpp:289-290`) and nothing is sent to the client (see section 3). No "nearest cell" fallback for player clicks (flag `&8` in `unit_walktoxy` is only used for cell-stack bumping, `unit.cpp:850`; the client path uses flag 4, `clif.cpp:11425`).
* Click on the current cell: start node equals goal, loop exits immediately, `path_len = 0`, returns true.
* Length limit: after reconstruction, `if (len > sizeof(wpd->path)) return false` (`path.cpp:414-416`): sizeof is 32 bytes because the enum is `int8`, so paths of more than 32 steps are REJECTED (not truncated). Also the node table is `tp[32*32]` indexed by `(x + y*32) & 1023` (`path.cpp:51,330`); an index collision between two different nodes makes `add_path` return 1 and the whole search FAIL (`path.cpp:51-52`, FIXME at 327-329). This is a rAthena quirk bounding the effective search radius.
* `max_walk_path` battle config: default `17`, min 1, max `MAX_WALKPATH` (`battle.cpp:8684`; `conf/battle/client.conf:42`). Checked in `unit_walktoxy` AFTER `path_search` succeeds: `if (wpd.path_len > battle_config.max_walk_path) return 0;` (`unit.cpp:859-862`). It is a REJECT (whole command ignored, no packet), applies to non-NPC units only, and measures path steps (a diagonal counts as 1).
* `OFFICIAL_WALKPATH` is defined (`config/core.hpp:26`): if `path_len > 14` and there is no straight unobstructed line (`path_search_long`, Bresenham-like, `path.cpp:132-193`, cells tested with `CELL_CHKNOPASS`, endpoints excluded from the obstacle test) the command is rejected (`unit.cpp:864-869`, repeated in `unit_walktoxy_sub` at `unit.cpp:268-273`). Net effect: path of 15..17 steps is allowed only when the straight line start->dest is clear; anything with `path_len > 17` is rejected.
* There is NO explicit Chebyshev/distance check on `CZ_REQUEST_MOVE` before pathing (`clif.cpp:11379-11426`). The only gates are: dead (sends `CLR_DEAD` clearunit), progress bar, `pc_cant_act`, running status, west-adjacent occupied cell rule (only when `official_cell_stack_limit > 0`), then `unit_walktoxy(sd, x, y, 4)`. Distance is bounded implicitly by `path_len <= 17`.
* `unit_walktoxy` also silently returns 0 (no reply) if `!unit_can_move` (`unit.cpp:882`; `unit_can_move` at `unit.cpp:1799-1837` fails for: casting, `canmove_tick` in the future, **sitting** `pc_issit`, vending, buying store, `PCBLOCK_MOVE`, `sc->cant.move`). Flag 4: if `canmove_tick` is 0..2000 ms ahead the walk is deferred by a timer instead (`unit.cpp:872-880`).

## 2. Step timing

* Provenance: `common/mmo.hpp:93`, `map/status.cpp:3817`, `map/pc.cpp:1456`, `map/unit.cpp:180-247, 542-784`, `map/status.cpp:9104-9110`, `map/battle.cpp:8403,8969`.
* `DEFAULT_WALK_SPEED = 150` (ms per cardinal cell), the `status.speed` value for players (`mmo.hpp:93`, `pc.cpp:1456`, `status.cpp:3817`). Lower is faster; `max_walk_speed` battle config (default 300 -> min step 50 ms) clamps, `battle.cpp:8403,8969`.
* Per-step duration (`unit_walktoxy_nextcell`, `unit.cpp:226-231`): cardinal step = `speed`; diagonal step = `speed * MOVE_DIAGONAL_COST / MOVE_COST` = `speed * 14 / 10` using INTEGER arithmetic (150 -> 210 ms). The same factor is used for the stepaction half-delay (`unit.cpp:731-734`).
* The step timer is armed at step START with `add_timer(tick + speed, unit_walktoxy_timer, ...)` (`unit.cpp:238`). The unit's `x/y` stay on the OLD cell while the timer runs and are updated only when it fires (`map_moveblock(bl, x+dx, y+dy)` at `unit.cpp:623-625`), i.e. position changes at the END of the step. The direction is set at timer fire too (`unit_setdir`, `unit.cpp:576`). A player entering view mid-step is told the unit is walking from its current (old) cell (`clif_getareachar_unit` -> `clif_set_unit_walking`, `clif.cpp:5062-5063`).
* After moving: `ud->sx/sy` reset to 8 (cell centre, `unit.cpp:560-562`), NPC touch/warp checks (`unit.cpp:680-690`), then `path_pos++` and next cell timer (`unit.cpp:747-749`), or stop: `to_x/to_y = x/y` (`unit.cpp:761-763`). No packet is sent when the final step completes; the client knows the destination from `ZC_NOTIFY_PLAYERMOVE`.
* On a new walk, `sd->head_dir = 0` (`unit.cpp:299-302`).
* Start-of-walk packets (`unit_walktoxy_nextcell(bl, sendMove=true)`, `unit.cpp:240-245`): `clif_move` -> `ZC_NOTIFY_PLAYERMOVE`/unit-walking to `AREA_WOS` (others, excludes self; `clif.cpp:2066-2090`), then `clif_walkok` -> `ZC_NOTIFY_PLAYERMOVE` (`0x0087`) to SELF only with `moveStartTime = client_tick(gettick())` and `WBUFPOS2(src=(sd.x,sd.y), dst=(to_x,to_y), sx=8, sy=8)` (`clif.cpp:2053-2061`). Intermediate steps send nothing (sendMove=false) unless damaged recently (`MOVE_REFRESH_TIME`).

## 3. Mid-walk redirect (new CZ_REQUEST_MOVE while walking)

* Provenance: `map/unit.cpp:841-916, 254-320, 738-745`, `map/clif.cpp:11379-11426`.
* `unit_walktoxy` runs a fresh `path_search` from the unit's CURRENT server cell (the cell it stands on, i.e. last completed cell) to the new target, applying all checks of section 1 (including `max_walk_path` measured from that cell). If any fails, returns 0, no packet, and the existing walk CONTINUES unchanged.
* On success with an active walk (`ud->walktimer != INVALID_TIMER`): sets `ud->to_x/to_y = target`, `ud->state.change_walk_target = 1`, returns 1 and sends NOTHING now (`unit.cpp:885-899`). The current step is not interrupted.
* When the current step timer fires (cell change completes, `unit.cpp:625`), `if (ud->state.change_walk_target) unit_walktoxy_sub(bl)` (`unit.cpp:738-745`): re-runs `path_search` from the NEW cell (the one just reached) to `to_x/to_y` (the OFFICIAL_WALKPATH >14 check applies, but not the `max_walk_path` check), replaces `ud->walkpath`, clears the flag, arms the next step timer and sends `clif_move` (others) + `clif_walkok` (self, from the new cell) at that moment (`unit.cpp:254-320`). If the repath fails: `clif_fixpos` (`ZC_STOPMOVE` 0x0088 to `AREA`, incl. self, at current cell) and the walk ends.
* So: repath cell = the cell reached at the end of the in-flight step; new `ZC_NOTIFY_PLAYERMOVE` is sent when that step completes (up to one step duration, 150/210 ms, after the click), with start = that cell.
* Click on the current cell while idle: `path_len = 0`; `unit_walktoxy_sub` -> `unit_walktoxy_nextcell` hits "reached end of walkpath" and sends `clif_walkok` with src == dst, no timer (`unit.cpp:186-193`, comment: "send the reply even if already at the target cell, this allows the client to synchronize the position"). Click on the current cell while walking: normal redirect; the target is the cell the unit is standing on at click time, so after finishing the in-flight step the unit paths back one cell.
* Other stop sources: sit/stand/attack-action requests call `unit_stop_walking(USW_FIXPOS)` (see map_entry_actions.md) which deletes the timer, keeps the unit on its last completed cell, and broadcasts `ZC_STOPMOVE` (`unit.cpp:1695-1755`, `clif.cpp:2205-2219`).

## 4. Area of Interest

* Provenance: `map/map.hpp:66`, `map/battle.cpp:8553`, `conf/battle/client.conf:39`, `map/map.cpp:120,407-459,518-600,837-1190`, `map/clif.cpp:405-535,5604-5690`, `map/unit.cpp:620-632`, `config/core.hpp:39`.
* `AREA_SIZE = battle_config.area_size`, default `14` (`battle.cpp:8553`). View is a square of side 29 (`x +/- 14`, `y +/- 14`), Chebyshev distance 14. `CIRCULAR_AREA` is NOT defined (`core.hpp:39` commented), so `check_distance`/`distance` are square/Chebyshev (`path.cpp:436-475`). `check_distance_client` (circular, `-0.1` bias) is only used for client-issued attack ranges.
* `BLOCK_SIZE = 8` (`map.cpp:120`); blocks hold lists of units, mobs in a separate `block_mob` grid (`map.cpp:440-452`). `map_moveblock` re-links the unit only when its 8x8 block index changes (`map.cpp:524, 556-565`). Area queries iterate blocks overlapping the rectangle then filter by exact cell bounds, so the effective test is the exact square.
* Step handling (`unit_walktoxy_timer`, `unit.cpp:620-632`), with `(dx,dy)` = step vector, `range = AREA_SIZE`, type `BL_ALL` for players (`BL_PC` for non-players):
  1. BEFORE moving: `map_foreachinmovearea(clif_outsight, bl, AREA_SIZE, dx, dy, ...)` with the old position: selects the strip of the old view square on the TRAILING side (cells that will no longer be within 14 after the move): for a cardinal step a 1-wide row/column; for a diagonal step the L-shaped union (`map.cpp:1107-1183`).
  2. `map_moveblock` to the new cell.
  3. AFTER moving: `map_foreachinmovearea(clif_insight, bl, AREA_SIZE, -dx, -dy, ...)` with the new position: selects the LEADING strip (cells newly inside the square). `walktimer` is temporarily set to `CLIF_WALK_TIMER` so spawn packets for the mover use the "walking" form.
* `clif_outsight(bl, tbl)` (`clif.cpp:5604-5657`; `bl` = unit in the strip, `tbl` = mover): if the mover is a player it gets `clif_clearunit_single(bl->id, CLR_OUTSIGHT)` for each non-invisible unit in the strip (items/skill units have their own clear packets); if `bl` is a player it gets `clif_clearunit_single(tbl->id, CLR_OUTSIGHT)`. Packet is `ZC_NOTIFY_VANISH` `0x0080` `<gid>.L <type>.B`, `CLR_OUTSIGHT = 0` (`clif.cpp:952-960`, `clif.hpp:356-363`: 0 outsight, 1 dead, 2 respawn/logout, 3 teleport, 4 trickdead).
* `clif_insight` (`clif.cpp:5662-5690`): mover (if a player) gets `clif_getareachar_unit` for each unit in the leading strip; each player in the strip gets `clif_getareachar_unit(mover)`. `clif_getareachar_unit` (`clif.cpp:5029-5130`) skips invisible units and sends walking-entry form if `walktimer != INVALID_TIMER` else idle-entry (`ZC_NOTIFY_STANDENTRY`) form, then extras (clothcolor, status icons, etc).
* Broadcast helper `clif_send(..., AREA)` sends to all players in the 29x29 square around the source incl. self; `AREA_WOS` excludes the source (`clif.cpp:405-445, 517-526`).
* Name requests and other "view range" gates use `check_distance_bl(..., AREA_SIZE)` (`clif.cpp:11485`).
