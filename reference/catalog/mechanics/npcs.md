# NPC Presentation: ids, visibility, vanish, warps

Target `PACKETVER = 20260219`. Citations `$RATHENA_PATH/src/map/...` (commit d4b8e7b). Wire field values: [map_units.md NPC section](../packets/map_units.md#npc-units-in-entry-packets-static-scriptshopwarp-npcs). No packet capture contains NPC entries (`tools/proxy/packet_captures/` absent), so all of this is source-derived.

## Which packet / when

* Map load: `clif_spawn` is guarded by `map_getmapdata(m)->users` (`npc.cpp:3866,3934,4510`); at startup nothing is sent.
* Player enters map (`LoadEndAck`): `map_foreachinallarea(clif_getareachar, ..., BL_ALL)` -> `clif_getareachar_unit` -> `0x09ff` per NPC to that player (`clif.cpp:5029-5066`).
* Player walks: per step `map_foreachinmovearea(clif_outsight/clif_insight, bl, AREA_SIZE, dx, dy, sd ? BL_ALL : BL_PC, bl)` (`unit.cpp:621,631,1200,1205,1328,1338`). A PC mover scans `BL_ALL`, so NPCs enter/leave view per step exactly like other units; `clif_insight` -> `clif_getareachar_unit(sd, npc)` -> `0x09ff` (`clif.cpp:5640-5670`). NPCs never move, so they never send anything themselves.
* NPC (re)enabled at runtime (`npc_enable`): `clif_spawn` -> `0x09ff` `AREA_WOS`.

## Never sent

Checked in `clif_spawn` (L1680-1692), `clif_getareachar_unit` (L5029-5050); NPC view data defaults to `npc_viewdb[0]` = `JT_INVISIBLE` (`npc.cpp:3809`).

| Condition | Value | Effect |
| :--- | :--- | :--- |
| `vd->look[LOOK_BASE] == JT_INVISIBLE` | `32767` (`npc.hpp:1583`) | skipped in both spawn and getareachar |
| Script/shop/duplicate sprite id `-1` on a map | `-1` (`JT_FAKENPC`) | `status_set_viewdata(-1)` fails (logs error), vd stays at the `JT_INVISIBLE` default -> never sent. Real data uses it (e.g. `gef_d01_i,1,1,5 script ill_vampire_spawn_bomi -1,{`) |
| Unknown sprite constant | falls back to `32767` (`npc_parseview`, `npc.cpp:3779`) | never sent |
| `nd.is_invisible` ("maya purple" check, `clif_npc_mayapurple` L1642) | set by state `DISABLED` / `disablenpc` | never sent; also dropped in outsight |
| Floating NPC `-	script` (`m == -1`) | `JT_FAKENPC` | not on any map, no block, nothing sent |
| Dynamic NPC owned by another char (`npc_is_hidden_dynamicnpc`) | not used by us | only `getareachar_unit` |
| `hide_cloaked_units` battle conf | default `0` (`conf/battle/client.conf:177`) | if non-zero for BL_NPC, option HIDE/CLOAK/INVISIBLE NPCs are skipped |

**`status_set_viewdata(nd, class_)` for BL_NPC** (`status.cpp:9379-9390`, case at ~9477-9495). `vd` is chosen by the first match of: `mobdb_checkid` (or clone) -> mob vd; `npcdb_checkid` -> NPC vd; `homdb_checkid` -> homunculus vd; `mercenary_db.exists`; `elemental_db.exists`; else null. For BL_NPC: non-null `vd` is `memcpy`'d into `nd->vd`; else `pcdb_checkid` -> fresh vd with `look[LOOK_BASE]=look[LOOK_BODY2]=class_`; else `ShowError("Invalid view data")` and `nd->vd` stays `JT_INVISIBLE` (never sent). Result by class id:

| Class id | Branch | `look[LOOK_BASE]` | Sent? |
| :--- | :--- | :--- | :--- |
| 45-125, 139, 401-999, 10001-19999 (`npcdb_checkid`, `npc.hpp:1595`) | npc vd (`npc_get_viewdata`, `npc.cpp:899`) | id | yes |
| 32767 | npc vd[0] | 32767 | no |
| existing `mob_db` id (non-clone), e.g. 1001+ | mob vd | mob id (or its Sprite override) | yes; `clif_bl_type` 0xC (`mobdb_checkid`) |
| PC job ids (`pcdb_checkid`, `pc.hpp:1219`) not shadowed by a mob/NPC/hom/merc/elem id | PC fallback | id | yes; `clif_bl_type` 0x0 if not walking |
| 6001-6052 (`HM_CLASS_BASE..HM_CLASS_MAX`, `mmo.hpp:187-189`) | hom vd (`hom_viewdb[i].look[LOOK_BASE]=6001+i`, `homunculus.cpp:2056-2058`) | id | yes; `clif_bl_type` 0x6 |
| ids in `mercenary_db` (2114.. to 3570 in `db/re/mercenary_db.yml`) | merc vd (but mob_db checked first) | db Sprite | yes if not shadowed |
| ids in `elemental_db` | elemental vd | db Sprite | yes |
| `-1` (`JT_FAKENPC`), 0-44 non-PC, 126-138, 140-399, 1000 gaps, 20000+ unknown | none | stays 32767 | no (error logged) |

Example: `Jonathan#homun_s` (`npc/re/quests/homun_s.txt:1334`, sprite 6002) is sent: `homdb_checkid(6002)` true, `mob_db` has no 6002, so `nd->vd.look[LOOK_BASE]=6002`, other looks 0; `clif_bl_type` = `0x6` (not PC, `mobdb_checkid` false, `clif.cpp:364-374`); `0x09ff` with `job=6002` (`clif.cpp:1051-1063`, non-PC path). Client rendering of a homunculus sprite on an NPC is not captured. Our `Sprite: 6002` should be forwarded as-is.

**Sent even though the client draws nothing:** sprite `111` (`JT_HIDDEN_NPC`), `139` (`JT_HIDDEN_WARP_NPC`, `npc.hpp:333`) and warp class `45` (`JT_WARPNPC`, `npc.hpp:250`). The server sends a normal `0x09ff` with that `job`; invisibility is a client sprite property. `139` is a script-NPC sprite for invisible OnTouch triggers (e.g. `// ecl_in01,47,92,0 script #ecl_buff_N HIDDEN_WARP_NPC`), not a warp type.

**State keywords** (`warp(HIDDEN)`, `script(HIDDEN)` etc., `npc.cpp:3963-3978,4506-4520`; enum `npc.hpp:146-155`): `HIDDEN` -> `sc.option |= OPTION_HIDE (0x2)`, still in the block grid and still sent in `0x09ff` with `effect_state = 0x2` when a player later sees it (nothing filters OPTION_HIDE in `getareachar_unit` at default config); `CLOAKED` -> `OPTION_CLOAK (0x4)`, same, with a per-player xor; `DISABLED` -> `is_invisible = true` (never sent, trigger and click disabled). Applied after `map_addblock`, via `npc_enable_target` (`npc.cpp:1030-1110`).

## Hidden warps

Every `warp` line is class `45` (`JT_WARPNPC`) regardless of any sprite (`npc_parse_warp`, `npc.cpp:3926`; `warp_point_debug` swaps to the guild flag `JT_GUILD_FLAG`), so ordinary warps ARE sent as `0x09ff` (job 45, `object_type 0x06`, name = warp name) and the client draws the portal effect. Hidden/disabled warps: `HIDDEN` -> sent with `effect_state 0x2` (client behavior not captured, treat as not drawn); `DISABLED` -> not sent and does not trigger. `warp2` = `trigger_on_hidden` (works for hiding/cloaked players).

## Ids

* `START_NPC_NUM = 110000000` (`npc.hpp:245`). `npc_get_new_npc_id` (`npc.cpp:73-88`): static counter starts there, returns `npc_id++` if `!map_blid_exists`, else scans forward skipping used ids (wraps back to `START_NPC_NUM`), fatal at exhaustion.
* One global pool shared by NPCs, warps, mobs (`mob.cpp:469`), pets, homunculi, mercenaries, elementals and mob tombs (callers: `homunculus.cpp:1064`, `elemental.cpp:247`, `mob.cpp:196,469`, `mercenary.cpp:373`, `pet.cpp:1023`, `npc.cpp:3801`). Not per-map. Ids are assigned in file-load order and are not stable across restarts.
* Other ranges: floor items `[MIN_FLOORITEM=2, MAX_FLOORITEM=START_ACCOUNT_NUM=2000000)` (`map.hpp:76-77`), accounts `[2000000, 100000000)` (`mmo.hpp:176-177`), so NPC ids cannot collide with players. `MAX_NPC_PER_MAP = 512` (`map.hpp:65`).
* The dummy `FAKE_NPC` (`fake_nd`, `npc.cpp:6370`) takes the first id; it is off-map.

## Leaving view

`clif_outsight` (`clif.cpp:5608-5650`): for a player observer losing an NPC, `0x0080` `ZC_NOTIFY_VANISH` with **type `0` = `CLR_OUTSIGHT`**, `SELF` (`clif_clearunit_single`), skipped only if `is_invisible`. (Quirk: for BL_NPC the `JT_INVISIBLE` class is not checked in the "tsd lost sight of bl" branch, so a vanish may be sent for a never-announced NPC; harmless.) Runtime hide/disable of non-warp, non-flag classes: `clif_changeoption` (+ `clif_clearunit_area(CLR_OUTSIGHT)` if disabled); warp/flag classes (`45`, guild flag) are cleared/respawned with vanish/spawn because the client ignores option changes for them (`npc.cpp:1084-1100`).

## Warp trigger (for the warp-portal feature)

* Registration: `npc_parse_warp` (`npc.cpp:3900-3945`): `map_addnpc` (warps sorted first in `mapdata->npc[]`, `npc_num_warp < npc_num_area < npc_num`), `npc_setcells` (`npc.cpp:4972-4995`) marks every non-wall cell in the square `[x-xs, x+xs] x [y-ys, y+ys]` with `CELL_NPC`; `<spanx>,<spany>` in the file are the half-extents (inclusive square, `(2xs+1) x (2ys+1)`).
* Trigger: after each completed step `unit_walktoxy_timer` (`unit.cpp:676-690`): `map_getcell(CELL_CHKNPC)` -> `npc_touch_area_allnpc` -> `npc_touch_areanpc` (`npc.cpp:1896-1950`). It returns early (no warp) if `is_invisible`/cloaked; for warps it also skips hiding or dead players (unless `warp2`), checks `pc_job_can_entermap`, loop guard `count_rewarp > 10`, then `pc_setpos(sd, to_map, to_x, to_y, CLR_OUTSIGHT)`. Same check runs at `LoadEndAck` tail for the arrival cell and on teleport/warp landings (`unit.cpp:1210,1345`).
* Client-side (cross-map): `pc_setpos` (`pc.cpp:6930`, `:7116-7118`) runs `unit_remove_map_pc(sd, CLR_OUTSIGHT)` (others get `0x0080` type 0) then `clif_changemap` = `ZC_NPCACK_MAPMOVE` `0x0091`, see [packets/map.md#0x0091-zc_npcack_mapmove](../packets/map.md#0x0091-zc_npcack_mapmove); the client then loads the map and sends `CZ_NOTIFY_ACTORINIT`, which runs the map-entry sequence ([map_entry_actions.md](./map_entry_actions.md)) incl. the NPC scan above. Destination `(0,0)` or out of bounds = random walkable cell. Same-map warps: see next bullet.
* Capture check (2026-10-23, logs listed in [map_entry_actions.md#map-change-same-map-server](./map_entry_actions.md#map-change-same-map-server)):
  * **Confirmed on the wire:** six plain warps, `prontera` `prt001` (156,22) ±(3,2) -> `prt_fild08` (170,375); `prt_fild08` `prtf004` (170,378) ±(3,2) -> `prontera` (156,26); `prt_in` `prt20-1` (70,162) ±(2,1) -> (70,140) and `prt20` (70,143) ±(2,1) -> (70,165), the last two same-map.
    * Each fired on the first step that entered the area, mid-path. The client asked to walk 3 cells and the `0x0091` came about 300 ms (two 150 ms steps) after the `0x0087`.
    * The `0x0091` carried the warp's exact destination, including on the same map.
    * Observers in view got `0x0080` type 0, and observers out of view got nothing. Nobody got a `ZC_STOPMOVE`.
    * The destination cells were not inside a warp area, so there was no chain.
  * **Still source-only** (no capture exercises them): the rewarp guard, `(0,0)`/out-of-bounds random cells, the arrival-cell chain, and the dead/hiding/cloaked/disabled skips.
* Same-map warp (`pc.cpp:6930-7190`): there is no separate branch. `state.changemap = (sd->mapindex != mapindex)` is false (`pc.cpp:6951`), which skips the map-change block (status removal, channel quit, `cloaked_npc` clear); `state.warping = 1`. Otherwise identical: `unit_remove_map_pc` (others get `0x0080` type `CLR_OUTSIGHT` = 0 via `AREA_WOS`, `clif.cpp:970-977`; pet/homunculus/mercenary/elemental vanish the same way) then `clif_changemap` `0x0091` with the same map name and new x/y (`pc.cpp:7116-7118`). Only cloaking ends as a same-map side effect (`unit.cpp:3672-3675`). The client reloads and sends `0x007d` again; `LoadEndAck` runs the non-`connect_new` path with `changemap` false, so the changemap-only block is skipped but spawn, nearby-units, map property and the arrival-cell touch still run.
* Rewarp guard: `count_rewarp` (`pc.hpp:482`, init 0 `pc.cpp:2206`) is touched only at the tail of `pc_setpos` (`pc.cpp:7176-7181`): `npc_check_areanpc(1, m, x, y, 0)` (warps only, exact destination cell, skipping invisible and dynamic-owner NPCs; `npc.cpp:2076`) true -> `++`, else reset to 0. So it counts consecutive warps landing inside a warp area. In `npc_touch_areanpc`, `count_rewarp > 10` logs a warning, resets to 0 and `break`s: that warp is skipped and the player stays put (`npc.cpp:1934-1938`).
* Arrival-cell trigger (`clif.cpp:11135-11140`, tail of `LoadEndAck`, after spawn/nearby sends): if `map_getcell(CELL_CHKNPC)` at `(sd->x, sd->y)` then `npc_touch_area_allnpc`, else `sd->areanpc.clear()`. `npc_touch_area_allnpc` (`npc.cpp:1958-1995`) returns 0 if `PCBLOCK_NPCCLICK`, loops `npc[0..npc_num_area)` (warps first) and stops at the first touch returning 2 (warp fired). Landing in another warp's area chain-warps (`prev` is set again, normal remove + `0x0091`), bounded by the rewarp guard. A CHKNPC cell where no NPC returns nonzero logs "stray NPC cell".
* Walk onto a warp: the check runs after EVERY completed step, mid-path cells included (`unit.cpp:676-690`); if `bl->prev == nullptr` afterwards the timer function returns. The walk is not stopped beforehand and no packet is sent before `pc_setpos`; `unit_remove_map_` calls `unit_stop_walking(USW_RELEASE_TARGET)` (`unit.cpp:3590`), which sends no `ZC_STOPMOVE`. Warps have no "already touching" memory (`sd->areanpc` is for script OnTouch only): every step ending on a CHKNPC cell inside the area re-triggers, and arriving inside an area triggers via the `LoadEndAck` tail.
* Destination cell (`pc.cpp:7086-7106`): out of bounds (`x<0 || x>=xs || y<0 || y>=ys`) logs an error and becomes `(0,0)`. `(0,0)` picks a random cell (NOT `map_search_freecell`): `x = rnd()%(xs-2)+1`, `y = rnd()%(ys-2)+1`, repeat while `CELL_CHKNOPASS` or (`!teleport_on_portal` and a warp area lies within range 1 of the cell). Cap `xs*ys*3` tries; on exhaustion it logs an error and returns `SETPOS_OK` WITHOUT warping (player not removed). An in-bounds explicit destination is used as is, even a wall cell (no walkability check).
* `npc_touch_areanpc` skip conditions (`npc.cpp:1896-1950`): returns 1 (no warp) for `is_invisible`, `npc_is_cloaked(nd, sd)`, `npc_is_hidden_dynamicnpc`. The player must be inside `[x-xs, x+xs] x [y-ys, y+ys]` (else 0). For warps it breaks (no warp) if `(!nd->trigger_on_hidden && (pc_ishiding(sd) || SC_CAMOUFLAGE))` or `pc_isdead(sd)` (dead never warp, `warp2` included), or `!pc_job_can_entermap(class, dest mapid, group level)`, or the rewarp guard. `pc_ishiding` = `sc.option & (OPTION_HIDE | OPTION_CLOAK | OPTION_CHASEWALK)` (`pc.hpp:1172`). `warp2` sets `trigger_on_hidden = true` (`npc.cpp:3935-3938`), lifting only the hiding check. Then `pc_setpos(...)`, return 2.
* `0x0091` map name: `clif_changemap` (`clif.cpp:2155`) uses `mapindex_getmapname_ext` (`common/mapindex.cpp:45-68`), which strips any `#instance` prefix part and appends `.gat` unless present, so the wire name is e.g. `geffen.gat` in a 16-byte field. `npc_parse_warp` (`npc.cpp:3900-3945`) skips (logs "Unknown destination map") a warp whose destination is not in `map_index.txt` (`mapindex_name2id == 0`); an out-of-bounds source x/y only warns. It does not check that the destination is hosted by this map-server. At warp time `pc_setpos` takes the `m < 0` branch (`pc.cpp:7039-7085`) for indexed-but-unhosted maps: `map_mapname2ipport` (`map.cpp:3158`) failure returns `SETPOS_NO_MAPSERVER`; otherwise `unit_remove_map_pc`, `chrif_save(CSAVE_CHANGE_MAPSERV|...)`, `chrif_changemapserver`, `unit_free_pc`. Out of scope for a single map-server.
