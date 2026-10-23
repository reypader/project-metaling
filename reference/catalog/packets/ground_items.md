# Ground Items (floor items): packets, ownership, expiry

Target `PACKETVER = 20260219`, renewal. Citations are `$RATHENA_PATH/src/map/...` at commit `d4b8e7b8f`; conf defaults from `conf/battle/drops.conf`. Who drops what and where: [mechanics/mobs.md](../mechanics/mobs.md#4-item-drops). Inventory insertion (`pc_additem`, `0x0b41` ack layout) belongs to the inventory topic.

**Verification status:** source-derived only; no capture of `0x0add`, `0x009d`, `0x00a1` or `0x0362` exists (`tools/proxy/packet_captures/` is absent). See "Open issues" for a length discrepancy that a capture should settle.

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
2. Cell: see [mobs.md section 4](../mechanics/mobs.md#4-item-drops) (`dir` offset, `map_cell_free`, `map_search_freecell_dist`). Returns 0 (no item) if no id is free or `map_addblock` fails.
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

## Autoloot and other ground items

* Autoloot: `item_auto_get: no` (default) means `sd->state.autoloot` starts at 0 (`pc.cpp:2158`; yes = 10000); with autoloot the item is given in `mob_item_drop` and never becomes a floor item (see mobs.md).
* Player-dropped items (`pc_dropitem`, `pc.cpp:6171`) use `map_addflooritem(item, amount, m, x, y, 0, 0, 0, flags 2|4, mob_id 0, canShowEffect false, DIR_MAX (random neighbour cell), type BL_NUL or BL_ITEM depending on `item_stacking`)`: no owners (`first_get_charid == 0` -> anyone), 5x5 free-cell search. Not needed for mob death.
* Floor item ids are shared with skill units and chat objects (`map_get_new_object_id`), disjoint from NPC/mob ids (>= 110000000) and account ids (>= 2000000).

## Open issues / to verify with a capture

* **`0x0add` length:** `clif_packetdb.hpp:1921` says `packet(0x0ADD, 22)` (dated 2018-04-18, when `ITID` was u16) but `sizeof(packet_dropflooritem)` at 20260219 with `u32 ITID` is **24**. The sender uses the struct size (`clif_send(&p, sizeof(p), ...)`), so rAthena emits 24 bytes. This project's `crates/ro-packets/src/table.rs:956` has `Fixed(22)`; if the client really reads 24, a server->client decoder (proxy) would desync on this frame. Needs a real-client capture.
* `0x009d` (19) and `0x00a1` (6) lengths likewise rest on struct sizes only.
* The `0x0b41` failure frame content for pickup failures (the zeroed item fields, `result` at `6`) is owned by the inventory topic; confirm the client does not ignore `count = 0, Index = 2` ("client_index(0)").
