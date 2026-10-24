# Ground Items (floor items): packets, ownership, expiry

Target `PACKETVER = 20260219`, renewal. Citations are `$RATHENA_PATH/src/map/...` at commit `d4b8e7b8f`; conf defaults from `conf/battle/drops.conf`. Who drops what and where: [mechanics/mobs.md](../mechanics/mobs.md#4-item-drops). Inventory insertion (`pc_additem`, `0x0b41` ack layout) belongs to the inventory topic.

**Verification status:** [`captures/item_drop_pickup_sequence.txt`](../captures/item_drop_pickup_sequence.txt) (2026-10-23) confirms:
* `0x0add` is 24 bytes, with the layout below ([PF-02](../../../PACKET_FINDINGS.md#pf-02)). The first mob drop lands on the mob's own cell.
* `0x00a1` is 6 bytes, sent to `AREA` ([PF-04](../../../PACKET_FINDINGS.md#pf-04)).
* `0x0362` is 6 bytes with the floor id at offset 2.
* The successful pickup order is `0x0b41` → `0x00b0` SP_WEIGHT → `0x08c8` type 1 → `0x00a1`. Observers get only `0x08c8` and `0x00a1`.

Still source-only:
* `0x009d` ([PF-03](../../../PACKET_FINDINGS.md#pf-03)).
* The pickup failure form.
* Everything about player drops: the capture's drop reply was lost to a proxy log stall ([PF-21](../../../PACKET_FINDINGS.md#pf-21)).

## Packet summary (PACKETVER 20260219)

| Opcode | Name | Dir | Length | Sender / parser |
| :--- | :--- | :--- | :--- | :--- |
| `0x0add` | `ZC_ITEM_FALL_ENTRY` (a.k.a. ZC_ITEM_FALL_ENTRY5): item dropped in view | S->C | **24** by struct (see issues) | `clif_dropflooritem`, `clif.cpp:865` |
| `0x009d` | `ZC_ITEM_ENTRY`: item already on the floor, shown to one player | S->C | **19** by struct | `clif_getareachar_item`, `clif.cpp:5391` |
| `0x00a1` | `ZC_ITEM_DISAPPEAR` | S->C | 6 | `clif_clearflooritem`, `clif.cpp:930` |
| `0x08c8` | `ZC_NOTIFY_ACT` type 1 = pick up (`DMG_PICKUP_ITEM`) | S->C | 34 | `clif_takeitem`, `clif.cpp:5315` |
| `0x0362` | `CZ_ITEM_PICKUP` (client->map, shuffled opcode) | C->S | 6 | `clif_parse_TakeItem`, `clif.cpp:12006` |
| `0x0b41` | `ZC_ITEM_PICKUP_ACK` (success *and* failure reply) | S->C | see inventory topic | `clif_additem`, `clif.cpp:2838` |

Opcode choice: `0x0add` because `PACKETVER >= 20180418` (`packets_struct.hpp:130-135`); `0x0362` from the `PACKETVER > 20180307` block of `clif_shuffle.hpp:4723-4733` (`parseable_packet(0x0362,6,clif_parse_TakeItem,2)`); `0x0b41` because `PACKETVER_MAIN_NUM >= 20200916` (`packets_struct.hpp:582`). `0x009d`/`0x00a1` have no `clif_packetdb.hpp` entry (rAthena sends them with `sizeof(struct)`); note `0x009d` is in this project's `ro-packets` "legacy opcodes rejected" test list (`crates/ro-packets/src/table.rs:1206`) and `0x00a1` has no table row, so both need adding for a server->client table.

## `0x0add` ZC_ITEM_FALL_ENTRY

> Open findings: [PF-02](../../../PACKET_FINDINGS.md#pf-02).

`struct packet_dropflooritem` (`packets_struct.hpp:597-618`, branch `PACKETVER >= 20181121` for `u32 ITID`, `>= 20130000` for `type`, `>= 20180418` for the effect fields). Packed, little endian.

| Offset | Size | Type | Field | Value |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x0add` |
| 2 | 4 | u32 | `ITAID` | floor item object id `fitem->id` (range `[MIN_FLOORITEM=2, MAX_FLOORITEM=2000000)`; allocated by `map_get_new_object_id`, `map.cpp:1649`, cycling through free ids; shared with skill units and chat rooms) |
| 6 | 4 | u32 | `ITID` | `client_nameid(nameid)` = item `View` id if non-zero, else the item id |
| 10 | 2 | u16 | `type` | `itemtype(nameid)` (`clif.cpp:109`): the `item_types` value (`common/mmo.hpp:224`: HEALING 0, USABLE 2, ETC 3, ARMOR 4, WEAPON 5, CARD 6, PETEGG 7, PETARMOR 8, AMMO 10, DELAYCONSUME 11, SHADOWGEAR 12, CASH 18), except PETEGG is sent as ARMOR (4) and SHADOWGEAR as WEAPON (5, if it goes on the shadow weapon slot) or ARMOR (4) |
| 12 | 1 | u8 | `IsIdentified` | `item.identify ? 1 : 0` (`itemdb_isidentified`: 1 for items that cannot be unidentified, e.g. ETC/USABLE) |
| 13 | 2 | u16 | `xPos` | cell x |
| 15 | 2 | u16 | `yPos` | cell y |
| 17 | 1 | u8 | `subX` | `rnd_value(1,4) * 3` -> one of 3,6,9,12 (sub-cell position, fixed at creation) |
| 18 | 1 | u8 | `subY` | same |
| 19 | 2 | i16 | `count` | stack amount |
| 21 | 1 | i8 | `showdropeffect` | 1 if a drop effect applies else 0 |
| 22 | 2 | i16 | `dropeffectmode` | `DropEffect - 1` from `item_db` when `showdropeffect = 1`; else 0 |

* Drop effect (`clif.cpp:880-912`): only when the caller passes `canShowEffect` (true for regular mob drops, false for looted-by-mob items and `@item`-style drops). `itemdb_dropeffect(nameid) > 0` -> `showdropeffect = 1`, `dropeffectmode = DropEffect - 1`. Else, if the floor item has random options and `rndopt_drop_pillar: on` (default), blue/yellow/purple pillar for 1/2/3+ options (`DROPEFFECT_*_PILLAR - 1`). Else both 0. `e_item_drop_effect` at 20260219 (>= 20200304): NONE 0, CLIENT 1, BLUE 2, YELLOW 3, PURPLE 4, GREEN 5, RED 6 (`itemdb.hpp:3262-3291`). For plain drops (no db `DropEffect`, no options) both bytes are 0.
* Send target: `AREA` centered on the item cell (all players within 14 cells, including any attacker). Players who arrive later get `0x009d` instead.
* No owner/priority information is in the packet; the client does not know about loot rights (the server answers a too-early pickup with a failure ack, below).

## `0x009d` ZC_ITEM_ENTRY (already on the ground)

> Open findings: [PF-03](../../../PACKET_FINDINGS.md#pf-03).

`PACKET_ZC_ITEM_ENTRY` (`packets_struct.hpp:2350-2364`; opcode via `DEFINE_PACKET_HEADER(ZC_ITEM_ENTRY, 0x9d)`, `packets.hpp:2211`). Sent `SELF` for each floor item in the area at map entry (`clif_getareachar`, `clif.cpp:5587`) and when one comes into view during a walk (`clif_insight`, `clif.cpp:5676`).

| Offset | Size | Type | Field |
| :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `0x009d` |
| 2 | 4 | u32 | `AID` = floor item id |
| 6 | 4 | u32 | `itemId` = `client_nameid` (u32 for `>= 20181121`) |
| 10 | 1 | u8 | `identify` |
| 11 | 2 | u16 | `x` |
| 13 | 2 | u16 | `y` |
| 15 | 2 | u16 | `amount` |
| 17 | 1 | u8 | `subX` |
| 18 | 1 | u8 | `subY` |

Total 19. Note the field order differs from `0x0add` (position before amount; no `type`, no effect fields).

## `0x00a1` ZC_ITEM_DISAPPEAR

> Open findings: [PF-04](../../../PACKET_FINDINGS.md#pf-04).

`u16 0x00a1, u32 itemAid` = 6 bytes (`packets.hpp:611-615`). Targets:
* item picked up or expired: `AREA` around the item (`clif_clearflooritem(fitem, nullptr)`; `map_clearflooritem` `map.cpp:1703`, expiry timer `map.cpp:1681`);
* item leaves one player's view while walking: `SELF` to that player (`clif_outsight`, `clif.cpp:5632`).

## Floor item object and creation

Source: `flooritem_data` `map.hpp:489`, `map_addflooritem` `map.cpp:2007-2075`.

Fields: block (`type = BL_ITEM`, m, x, y, id), `subx/suby`, `cleartimer`, `first/second/third_get_charid`, `first/second/third_get_tick` (absolute ticks), the `struct item` copy (amount set to the dropped amount), `mob_id` (0 if not from a mob).

Creation steps:
1. `flags&4` bypasses the "untradable items can't be dropped" check (`item_onfloor`); mob drops pass `flags = 4`.
2. Cell: see [Player drop](#player-drop-pc_dropitem---map_addflooritem) (`dir` offset, `map_cell_free`, `map_search_freecell_dist`; `map.cpp:2017-2027`). Returns 0 (no item) if no id is free, `map_addblock` fails, or (only when `type&BL_ITEM`) no free cell is found. 
3. Loot-priority ticks (all relative to `gettick()` at creation):
   * normal: `first = now + item_first_get_time (3000)`, `second = first + item_second_get_time (2000)`, `third = second + item_third_get_time (2000)`;
   * extended (MVP item, `flags&1`, or the dropping mob has `bosstype != NONE`, i.e. `Class: Boss` or `Modes: Mvp`): `first = now + mvp_item_first_get_time (10000)`, `second = first + mvp_item_second_get_time (10000)`, `third = second + mvp_item_third_get_time (2000)`.
4. `cleartimer = add_timer(now + flooritem_lifetime (60000), map_clearflooritem_timer, id)`. Expiry: pet egg -> `intif_delete_petdata`; then `clif_clearflooritem` (`0x00a1` `AREA`), id removed, block removed, object freed. There is no warning or fade packet.
5. `map_addiddb`, `map_addblock` (so AoI works), then `clif_dropflooritem` (`0x0add` `AREA`).

The three charids are the top-3 damage dealers of the mob (`first_sd`, `second_sd`, `third_sd` from `mob_dead`; 0 if absent), NOT the killer.

## Pickup `0x0362` CZ_ITEM_PICKUP

> Open findings: [PF-05](../../../PACKET_FINDINGS.md#pf-05), [PF-07](../../../PACKET_FINDINGS.md#pf-07).

Layout: `u16 0x0362, u32 object_id` (6 bytes; id at offset 2, `clif_shuffle.hpp:4727`). Handler `clif_parse_TakeItem` (`clif.cpp:12006-12035`), then `pc_takeitem` (`pc.cpp:6189-6268`). Checks in order:

1. Player dead -> `clif_clearunit_area(sd, CLR_DEAD)` and the generic failure below.
2. Target must exist as a `BL_ITEM` on the player's map (`map_id2bl`), `pc_cant_act` false (`pc.hpp:1156-1168`: open NPC dialog (`npc_id`), chat room, vending, buying store, trading, storage open, NPC shop, status `opt1` other than burning (stone/freeze/stun/sleep...), and several UI windows).
3. Range: `check_distance_bl(fitem, sd, 2)` = Chebyshev distance `max(|dx|,|dy|) <= 2` (unless the player is mid-`BS_GREED`). Too far -> failure.
4. `sd->sc.cant.pickup` false.
5. Loot-right check with `now = gettick()`: `item_get_tick = third_get_tick` by default;
   * `first_get_charid == 0` (nobody owns it) or == own char_id -> `0` (no wait);
   * own char_id == second charid (and not party item-share on) -> wait until `first_get_tick`;
   * own char_id == third charid (not share) -> wait until `second_get_tick`;
   * party with item share (`party.item & 1`): a member of the first/second/third owner's party (owner online) gets that owner's tier;
   * everyone else waits until `third_get_tick`.
   If `now < item_get_tick` -> failure. So an unrelated player may pick up after `3000 + 2000 + 2000 = 7000` ms (normal) or `10000 + 10000 + 2000 = 22000` ms (boss/MVP), and the top damage dealer immediately.
6. `party_share_loot(party, sd, &fitem->item, first_charid)` (`party.cpp:1298`) gives the item (`pc_additem`, with party-distribution `item & 2` rules: random or round-robin among eligible members). If it returns nonzero (e.g. overweight 2, inventory full 4): `clif_additem(sd, 0, 0, flag)` (failure ack with that result) and **the item stays on the floor** (the function still returns true).
7. Success: `unit_stop_attack`; `clif_takeitem` = `0x08c8` with `src = picker`, `target = floor item id`, `type = 1` (pick up), `AREA`; item broadcast for flagged items; `map_clearflooritem(fitem)` -> `0x00a1` `AREA`. The inventory add ack (`0x0b41`, result 0) is sent by `pc_additem` itself during step 6, so on the wire: ack to picker, `0x08c8` pick-up act, `0x00a1`.

Generic failure (steps 1-5): `clif_additem(sd, 0, 0, 6)`, i.e. `ZC_ITEM_PICKUP_ACK` with `result = 6` (`ADDITEM_REFUSED_TIME` in `e_additem_result`, `pc.hpp:113`, reused as "cannot pick up"). The code comment says the client REQUIRES a failure packet or it can no longer pick up items; it is sent for every failure including "item no longer exists". Nothing else (no chat message) is sent.

### Pickup gate details

Source: `rathena` @ `d4b8e7b8f`, `src/map/pc.hpp:1150-1168`, `clif.cpp:12006-12035`, `pc.cpp:6189-6268`, `clif.cpp:970-981`, `clif.cpp:5315-5324`. Applies to 20260219.

* **`pc_cant_act2`** (`pc.hpp:1150`) is true if any of: `state.vending`, `buyingstore`, `sc.opt1` set and not `OPT1_BURNING` (stone/freeze/stun/sleep/...), `state.trading`, `storage_flag`, `prevend`, `refineui_open`, `stylist_open`, `inventory_expansion_confirmation`, `npc_shopid`, `barter_open`, `barter_extended_open`, `laphine_synthesis`, `laphine_upgrade`, `roulette_open`, `enchantgrade_open`, `item_reform`, `item_enchant_index`. **`pc_cant_act`** (`pc.hpp:1166`) = `npc_id` (open NPC dialog) || `chatID` (chat room) || `pc_cant_act2`. Only `pc_cant_act` (not `act2`) gates pickup; drop uses `act2` + `npc_id` (no chat-room check).
* **Dead / sit / walk are NOT in either list.** Metaling states dead, sitting, walking: of these only dead matters, and dead is handled by the separate `pc_isdead` branch (below). With none of the other systems implemented, `pc_cant_act` is always false for us (no `opt1` status effects either).
* **Sitting is not refused.** `clif_parse_TakeItem` and `pc_takeitem` never test `pc_issit`. A sitting player within range 2 picks up normally and stays sitting (only the client UI prevents it). Metaling may add a sit refusal but it would deviate from rAthena.
* **Walking/attack on success:** `pc_takeitem` calls only `unit_stop_attack(sd)` (`pc.cpp:6254`). It does NOT call `unit_stop_walking`; a walk in progress continues. Nothing at all stops movement on a failed pickup either. `pc_takeitem` is purely range-checked against the player's CURRENT server cell (`check_distance_bl(fitem, sd, 2)`). There is no server-side "walk to item" and no pending-pickup state: the client first sends the move request (`CZ_REQUEST_MOVE`) and, when it arrives, `0x0362`. If `0x0362` arrives while still out of range it is simply refused (`0x0b41` result 6).
* **Pickup animation:** `clif_takeitem(src = picker, dst = floor item)` = `ZC_NOTIFY_ACT` (`0x08c8`) with `srcID = picker`, `targetID = floor item id`, `type = DMG_PICKUP_ITEM`, sent to `AREA` around the picker (includes the picker), `clif.cpp:5315`.
* **Dead picker:** `clif_clearunit_area(sd, CLR_DEAD)` = `ZC_NOTIFY_VANISH` `0x0080 <u32 gid = picker id><u8 type = CLR_DEAD>` to `AREA` (incl. the picker; `CLR_DEAD` is the "died" value 1, `clif.cpp:962-981`), then falls to the generic failure `0x0b41` (index 2, count 0, result 6) to the picker only. Both are sent; the dead check runs before the item-exists check. (Note `AREA_WOS` is used for other clear types; only `CLR_DEAD` uses `AREA`.)
* **Failure order in code:** dead -> item missing / not `BL_ITEM` / other map -> `pc_cant_act` -> `pc_takeitem` (range, `sc.cant.pickup`, loot tick, share) -> failure = `0x0b41` result 6. `party_share_loot` failure returns true with the specific result code (not 6) and the item stays.

## Autoloot and other ground items

* Autoloot: `item_auto_get: no` (default) means `sd->state.autoloot` starts at 0 (`pc.cpp:2158`; yes = 10000); with autoloot the item is given in `mob_item_drop` and never becomes a floor item (see mobs.md).
* Player-dropped items: see [Player drop](#player-drop-pc_dropitem---map_addflooritem).
* Floor item ids are shared with skill units and chat objects (`map_get_new_object_id`), disjoint from NPC/mob ids (>= 110000000) and account ids (>= 2000000).

## Player drop (pc_dropitem -> map_addflooritem)

Source `$RATHENA_PATH/src/map/` at `d4b8e7b8f`; applies to 20260219. Wire opcode: `0x0363` `CZ_ITEM_THROW` `<u16 op><u16 index+2 @2><u16 amount @4>` = 6 bytes (`clif_shuffle.hpp:4733`, same `PACKETVER > 20180307` block as pickup `0x0362`; handler `clif_parse_DropItem`, `clif.cpp:12042`, `item_index = RFIFOW(2) - 2`).

**Handler gates, in order** (`clif.cpp:12048-12062`; any failure `break`s to the failure ack):
1. `pc_isdead`; 2. `pc_cant_act2(sd) || sd->npc_id` (`pc.hpp:1150`: vending, buyingstore, opt1 other than burning, trading, storage, prevend, many UI windows, npc shop; note chat room is NOT checked here, only npc dialog); 3. `sd->sc.cant.drop`; 4. `pc_dropitem` false.
Failure ack: `clif_dropitem(sd, item_index, 0)` = `0x00af` with `index = item_index+2` (`client_index`), `count = 0` ("client does not like being ignored", `clif.cpp:12072`); nothing else is sent. Success also updates idletime (config-dependent) and returns.

**`pc_dropitem` checks** (`pc.cpp:6137-6174`), in order, all return false (so failure ack): `n` outside `[0, MAX_INVENTORY)`; `amount <= 0`; slot empty / `amount` > stack / `sd->state.trading` / `vending` / no item data; `equipSwitch` set (item in the equip-switch window); mapflag `nodrop` (also `msg_txt 271` "You can't drop items on this map." via `clif_displaymessage`); `pc_candrop` false (`pc.cpp:11356`: `sc.cant.drop`, hatched pet egg, no PC_PERM_TRADE_UNCONDITIONAL and (`!pc_can_give_items` or item `expire_time` or bound item without permission), or `itemdb_isdropable` i.e. item `trade_restriction.drop` unless GM level >= `gm_lv_trade_override`) -> also `msg_txt 263` "This item cannot be dropped.". Then `map_addflooritem(&inv[n], amount, sd->m, sd->x, sd->y, 0,0,0, flags=2|4, mob_id=0, canShowEffect=false, DIR_MAX, type = item_stacking ? BL_NUL : BL_ITEM)`; if it returns 0 -> false (item stays, failure ack, no message). Else `pc_delitem(sd, n, amount, type=1, 0, LOG_TYPE_PICKDROP_PLAYER)` then `clif_dropitem(sd, n, amount)`.

**Equipped items:** there is NO `equip != 0` check. Dropping the whole stack of an equipped item is allowed: `pc_delitem` (`pc.cpp:6108-6112`) calls `pc_unequipitem(sd, n, 2|1)` (flag 1 = recalc status, 2 = no clif ack for the slot) when amount reaches 0. Equipment amount is 1 so the whole item drops; the client normally prevents it, but the server does not. (Metaling may choose to reject; rAthena allows.) Item is copied into the floor item before deletion, so the floor item keeps `equip` field value as in the `struct item` copy (stale equip bits; rAthena does not clear it).

**Cell selection** (`map.cpp:2017-2027`): `dir == DIR_MAX` forces the search branch regardless of whether own cell is free (the `dir > DIR_CENTER && dir < DIR_MAX` offset step is skipped). Search = `map_search_freecell_dist(m,&x,&y, distmin=1, distmax=(flags&2)?2:1 = 2, type)`. So the item NEVER stays on the dropper's own cell when the search succeeds. On search failure: if `type&BL_ITEM` (item_stacking off) -> return 0 (drop fails, item stays in inventory); otherwise (item_stacking on, `type=BL_NUL`) it falls through and drops on the ORIGINAL cell (the dropper's own cell, even if not "free").

**`map_search_freecell_dist`** (`map.cpp:1757-1779`): pick `mirrorx`, `mirrory` each randomly -1 or +1 (`rnd()%2`). Then loop `i` in {-1,0,1} outer, `j` in {-1,0,1} inner, skipping `(0,0)`: 8 candidates, in that order. Each candidate = `(x + mirrorx*i*rnd_value(1,2), y + mirrory*j*rnd_value(1,2))` (a fresh random 1..2 for each axis of each candidate). First candidate for which `map_cell_free` is true wins. So candidates lie on 8 rays at distance 1 or 2 (Chebyshev), not uniformly over the 5x5; at most 8 tries, no retries. Axis-aligned candidates (i=0 or j=0) have only the nonzero axis randomised.

**`map_cell_free(m,x,y,type)`** (`map.cpp:1724-1745`): false if map data missing; false if `x < 5 || x > xs-5 || y < 5 || y > ys-5` where `edge = min(battle_config.map_edge_size (default 15, misc.conf:192), 5) = 5` (so valid x in [5, xs-5], y in [5, ys-5]; comment says officially NE 4 / SW 5, not symmetric); false if `CELL_CHKNOPASS` (non-walkable, i.e. wall/water-not-passable etc.; NOT a walk-path check); false if `map_count_oncell(m,x,y,type,0) > 0`.
`map_count_oncell` (`map.cpp:641-686`) counts only blocks whose type is in the `type` mask: with `type = BL_NUL (0)` nothing is ever counted, so players, mobs, NPCs and other floor items do NOT occupy a cell (items stack freely). With `type = BL_ITEM`, only other floor items count (players, mobs, NPCs do not). (Hidden NPCs are skipped when BL_NPC is in the mask, irrelevant here.) Mob drops use their own `type`; see mobs.md.

**`item_stacking`** (`conf/battle/misc.conf:199`, `battle.cpp:8894`): default **yes (1)**, range 0..1. Comment: officially there is no limit on items per cell; if "no", an item drops only on a cell with no item, searched in 8 directions (see above), and the drop fails if none is found.

**`flags` bits** (`map.cpp:2000-2005` doc comment, uses at 2012, 2022, 2043): `&1` MVP item -> extended loot-protection ticks (mvp_item_*_get_time); `&2` free-cell search distance max 2 (5x5) instead of 1 (3x3); `&4` skip the "untradable cannot be on floor" check (`item_onfloor` && `itemdb_traderight().trade`; conf key `item_flooritem_check`, default 1 (`battle.cpp:8690`)). Player drop uses `2|4`: no owners, so anyone can pick up immediately (`first_get_charid == 0`); standard 60 s lifetime.

**Packet order on a successful player drop** (all within one `pc_dropitem` call, in this order):
1. `map_addflooritem` -> `clif_dropflooritem` -> `0x0add` to `AREA` of the item cell (`clif.cpp:923`; AREA includes the dropper and all observers in range; no exclusion). `showdropeffect = 0`, `dropeffectmode = 0` since `canShowEffect = false` (the effect fields are zeroed, `clif.cpp:908-911`). `subX = rnd_value(1,4)*3`, `subY` same, independent (`map.cpp:2048-2049`; values 3,6,9,12).
2. `pc_delitem(..., type=1)`: bit 1 suppresses `clif_delitem` (0x07fa), bit 2 not set so `clif_updatestatus(SP_WEIGHT)` -> `0x00b0` `SP_WEIGHT` to the dropper (SELF), `pc.cpp:6119-6120`. No `0x0b41`/`0x07fa`; no `SP_MAXWEIGHT`. If the equipped-item branch ran, `pc_unequipitem` may send its own equip/look/status packets before the weight update (flag 2 suppresses the slot ack but status recalc runs).
3. `clif_dropitem` -> `0x00af` `<u16 index+2><u16 amount>` SELF (`clif.cpp:2895-2903`), 6 bytes.
Dropper order: `0x0add`, `0x00b0`, `0x00af`. Observers: only `0x0add` (those within AREA). Later arrivals get `0x009d`.

## Open issues / to verify with a capture

* **`0x0add` length (settled by capture: 24, see PF-02; kept for history):** `clif_packetdb.hpp:1921` says `packet(0x0ADD, 22)` (dated 2018-04-18, when `ITID` was u16) but `sizeof(packet_dropflooritem)` at 20260219 with `u32 ITID` is **24**. The sender uses the struct size (`clif_send(&p, sizeof(p), ...)`), so rAthena emits 24 bytes. This project's `crates/ro-packets/src/table.rs:956` has `Fixed(22)`; if the client really reads 24, a server->client decoder (proxy) would desync on this frame. Needs a real-client capture.
* `0x009d` (19) and `0x00a1` (6) lengths likewise rest on struct sizes only.
* The `0x0b41` failure frame content for pickup failures (the zeroed item fields, `result` at `6`) is owned by the inventory topic; confirm the client does not ignore `count = 0, Index = 2` ("client_index(0)").
