# Equip / Unequip Packets (PACKETVER 20260219)

Scope: packets exchanged when a player wears or removes an item, plus the look-change broadcast. Rules and flow live in [../mechanics/equipment.md](../mechanics/equipment.md). Equipment rows in the inventory list packets are owned by `packets/inventory.md` (other agent); only the equip-specific fields are noted here.

**Provenance (all sections):** repo `rathena` (`$RATHENA_PATH`), files `src/map/{packets.hpp,packets_struct.hpp,clif.cpp,clif.hpp,clif_packetdb.hpp,clif_shuffle.hpp}`. **Applies to `20260219`: yes**, resolved with `PACKETVER_MAIN_NUM = 20260219` (`PACKETVER_RE` undefined, `src/config/packets.hpp:21-45`), so every `>= 20121205` / `>= 20120925` / `>= 20130000` / `>= 20181121` / `>= 20230906` branch is taken.

**Shuffle check:** the `clif_shuffle.hpp` branch that applies to 20260219 is `PACKETVER > 20180307` (L4723-4761). It does not list `0x0998`, `0x00ab` or `0x0bf5`, so these opcodes come straight from `packets.hpp` / `clif_packetdb.hpp`. See [PF-05](../../../PACKET_FINDINGS.md#pf-05).

**Captures:** the weapon swap (`0x0998`, `0x0999`, `0x099a`, `0x01d7`) is capture-verified; see [Captured weapon swap](#captured-weapon-swap). Take-off (`0x00ab`), unequip-all, failure acks, shields, headgear and ammo are not captured yet. If a capture disagrees, the capture wins.

## Captured weapon swap

Source: [`captures/packet_sequence-20261023_030246.txt`](../captures/packet_sequence-20261023_030246.txt) lines 841–865 (rAthena `d4b8e7b`). Session `#6` is account 2000001 / char 150006 (`test2`, a novice, the equipper). Session `#3` is account 2000000 (`test`, the observer). At login `#6` already wore a Knife at wire index 2 (the `0x0b39` entry at line 309: `02 00 | b1 04 00 00 | 05 | 02 00 00 00 | 02 00 00 00` = index 2, ITID 1201, type 5, location `EQP_HAND_R`, WearState `EQP_HAND_R`). A second Knife from `@item knife` (line 822) sits at wire index 6.

The proxy table had `0x01d7` at 11 bytes and no `0x0999`/`0x099a` rows when this was recorded, so the log splits frames wrongly. The bytes below are re-framed by hand from the logged payloads.

**Equip the Knife at wire 6 while the one at wire 2 is worn** (client `0x0998 {06 00 | 02 00 00 00}`, line 841):

| # | To | Packet | Bytes after the opcode | Meaning |
| :--- | :--- | :--- | :--- | :--- |
| 1 | self + observer | `0x01d7` | `81 84 1e 00 \| 02 \| 00 00 00 00 \| 00 00 00 00` | `LOOK_WEAPON` 0 / 0: the old knife is off |
| 2 | self | `0x099a` | `02 00 \| 02 00 00 00 \| 00` | take-off ack for wire 2, location `EQP_HAND_R`, flag 0 (ok) |
| 3 | self | `0x00b0` | `35 00 \| b8 01 00 00` | `SP_ASPD` 440 |
| 4 | self | `0x00b0` | `2a 00 \| 00 00 00 00` | `SP_ATK2` 0 |
| 5 | self | `0x0999` | `06 00 \| 02 00 00 00 \| 00 00 \| 00` | wear ack for wire 6, location `EQP_HAND_R`, view 0, result 0 (ok) |
| 6 | self + observer | `0x01d7` | `81 84 1e 00 \| 02 \| b1 04 00 00 \| 00 00 00 00` | `LOOK_WEAPON` 1201 / 0 |
| 7 | self | `0x00b0` | `35 00 \| 4e 02 00 00` | `SP_ASPD` 590 |
| 8 | self | `0x00b0` | `2a 00 \| 11 00 00 00` | `SP_ATK2` 17 (Knife `Attack`) |

Equipping the Knife at wire 2 again (line 854) gives the same eight packets with wire indexes 2 and 6 swapped.

What this settles:
* Lengths: `0x0998` 8, `0x0999` 11, `0x099a` 9, `0x01d7` 15.
* The displaced item is fully unequipped first: look packet, then its `0x099a`, then its own status recalculation push. Only after that come the new item's `0x0999`, its look and its recalculation push. This matches `pc_equipitem` step 11 calling `pc_unequipitem` with the recalc flag ([mechanics §3](../mechanics/equipment.md#3-pc_equipitem-flow)).
* On unequip the look packet comes before the ack. On equip the ack comes before the look.
* Each recalculation sends only the values that changed (here ASPD and ATK2), in the [recalculation push order](./status.md#recalculation-pushes). `SP_ATK2` is the weapon's `Attack` (refine 0).
* The observer gets only the two `0x01d7`.
* A weapon's `0x0999` view is 0 (`EQP_HAND_R` is not in `EQP_VISIBLE`).
* `SP_ASPD` 440 bare-handed and 590 with a Knife fit amotion for a novice with base stats (ASPD about 156 and 141). This supports [PF-16](../../../PACKET_FINDINGS.md#pf-16).
* No weight packet, attack-range packet or skill list follows (the Knife's range 1 equals bare hands).

**Index convention:** every `index` field on the wire is `server_index + 2` (`clif.cpp:122-128`, `client_index` / `server_index`). Server inventory slot 0 is wire index 2.

## Opcode summary

| Opcode | Name | Dir | Frame | Notes |
| :--- | :--- | :--- | :--- | :--- |
| `0x0998` | `CZ_REQ_WEAR_EQUIP` (V5) | C->Map | fixed 8 | index + u32 location mask |
| `0x00ab` | `CZ_REQ_TAKEOFF_EQUIP` | C->Map | fixed 4 | index only |
| `0x0bf5` | `CZ_REQ_TAKEOFF_EQUIP_ALL` | C->Map | fixed 6 | u32 location mask (>= 20230906) |
| `0x0999` | `ZC_REQ_WEAR_EQUIP_ACK` (V5) | Map->C | fixed 11 | SELF |
| `0x099a` | `ZC_REQ_TAKEOFF_EQUIP_ACK` (V5) | Map->C | fixed 9 | SELF |
| `0x0bae` | `ZC_ACK_TAKEOFF_EQUIP_ALL` | Map->C | fixed 3 | SELF |
| `0x01d7` | `ZC_SPRITE_CHANGE2` | Map->C | fixed 15 (see discrepancy) | AREA |
| `0x013c` | `ZC_EQUIP_ARROW` | Map->C | fixed 4 | SELF |
| `0x013b` | `ZC_ACTION_FAILURE` | Map->C | fixed 4 | SELF |

## `0x0998` CZ_REQ_WEAR_EQUIP

> Open findings: [PF-11](../../../PACKET_FINDINGS.md#pf-11).

Source: `packets.hpp:1503-1517` (branch `PACKETVER >= 20120925`), registered `clif_packetdb.hpp:58` (`parseable_packet(HEADER_CZ_REQ_WEAR_EQUIP, sizeof, clif_parse_EquipItem, 0)`), handler `clif.cpp:12113`.

| Offset | Size | Type | Field |
| :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | opcode `0x0998` |
| 2 | 2 | u16 | `index` (wire; server slot = value - 2) |
| 4 | 4 | u32 | `position`: requested `EQP_*` mask (see mechanics file) |

Handler behaviour (`clif.cpp:12113-12160`), in order:
1. Dead: `clif_clearunit_area(CLR_DEAD)`, return (no ack).
2. `index >= MAX_INVENTORY` after `-2`: silently return.
3. `sd->npc_id && !npc_item_flag` or `PCBLOCK_EQUIP`: red message `MSI_CAN_NOT_EQUIP_ITEM` (372), return. Else if storage open or `opt1` set: proceed. Else if `pc_cant_act2`: return.
4. Item not identified: `ZC_REQ_WEAR_EQUIP_ACK` with FAIL (2).
5. No item data in slot: return. Pet armor type: `pet_equipitem`, return.
6. Ammo items: client sends no useful position; server passes `EQP_AMMO` itself. Everything else passes `position` through.
7. Calls `pc_equipitem(sd, index, req_pos)`.

## `0x00ab` CZ_REQ_TAKEOFF_EQUIP

Source: `clif_packetdb.hpp:59` (`parseable_packet(0x00ab,4,clif_parse_UnequipItem,2)`), handler `clif.cpp:12166-12190`.

| Offset | Size | Type | Field |
| :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | opcode `0x00ab` |
| 2 | 2 | u16 | `index` (wire; slot = value - 2) |

Same dead / npc / block-action / `pc_cant_act2` gates as equip (those gates drop the request silently: no ack). Then `pc_unequipitem(sd, index, flag=1)`. No bounds check in the handler.

**Failure acks** (`pc.cpp:12398-12416`, rathena `d4b8e7b8f`), all a `0x099a` with `wearLocation = 0` and `flag = 1`:
* index out of `[0, MAX_INVENTORY)` (wire 0/1 give -2/-1, or too large): ack index field is `server_index 0` so wire `2`. Bytes `9a 09 02 00 00 00 00 00 01`.
* slot not worn (`equip == 0`, includes an empty slot): ack index = the requested slot + 2, location 0, flag 1. Example slot 5: `9a 09 07 00 00 00 00 00 01`.
* `sc.cant.unequip` (or `SC_PYROCLASTIC` and a weapon): same form as "not worn" (requested index, location 0, flag 1). `cant.unequip` comes from status DB flag `NoUnequipItem` (`status.cpp:5791`). The check is skipped when `flag & 2` (forced unequips: drop, delete, class change).
* Success: `0x099a {index+2, pos = item's equip mask, flag 0}`, ordering in [Take-off / drop order](#take-off--drop-order-pc_unequipitem).

## `0x0999` ZC_REQ_WEAR_EQUIP_ACK (V5)

> Open findings: [PF-11](../../../PACKET_FINDINGS.md#pf-11).

Source: `packets_struct.hpp:1268-1277`, `clif.cpp:4312-4330` (`clif_equipitemack`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | opcode `0x0999` | |
| 2 | 2 | u16 | `index` | wire index (slot + 2) |
| 4 | 4 | u32 | `wearLocation` | final `EQP_*` mask the item went into (0 on failure) |
| 8 | 2 | u16 | `wItemSpriteNumber` | item `View` (`item_data.look`) only on success and only if item location intersects `EQP_VISIBLE`, else 0 |
| 10 | 1 | u8 | `result` | `0` OK, `1` fail level, `2` fail other |

**Result codes at this packetver** (`clif.hpp:524-533`, branch `>= 20121205`): `ITEM_EQUIP_ACK_OK = 0`, `ITEM_EQUIP_ACK_FAILLEVEL = 1`, `ITEM_EQUIP_ACK_FAIL = 2`. (Older versions inverted: OK=1, FAIL=0.)

`EQP_VISIBLE` at this version = `EQP_HELM | EQP_GARMENT | EQP_COSTUME` (`pc.hpp:1141-1146`, branch `>= 20110111`): head low/mid/top, garment, the four costume slots. Note weapons/shields/shoes/armor/accessories send view 0 here. `wearLocation` on failure paths is 0 (`pos` default argument), except when OK it is the final slot mask.

## `0x099a` ZC_REQ_TAKEOFF_EQUIP_ACK (V5)

Source: `packets.hpp:1007-1013` (branch `PACKETVER >= 20130000`), `clif.cpp:4332-4351` (`clif_unequipitemack`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | opcode `0x099a` | |
| 2 | 2 | u16 | `index` | wire index (slot + 2) |
| 4 | 4 | u32 | `wearLocation` | the item's `equip` mask that was cleared (0 on failure) |
| 8 | 1 | u8 | `flag` | `0` = success, `1` = failure (rAthena inverts its bool for `>= 20110824`) |

Also sent as a side effect (success, flag 0) once for every item displaced by an equip, and for ammo auto-unequipped when its weapon is removed.

## `0x0bf5` / `0x0bae` unequip-all

Source: `packets_struct.hpp:5167-5185`, `clif_packetdb.hpp:2004` (inside `PACKETVER_MAIN_NUM >= 20220330` block), handler `clif.cpp:22820-22845`.
* Request `0x0bf5`: `{ u16 opcode; u32 location }` (6 bytes). `location` is an `EQP_*` mask selecting which slots to strip.
* Server (`clif_parse_unequipall`, `clif.cpp:22820-22845`; no dead check): if `pc_cant_act` (adds `npc_id` and `chatID` to the `pc_cant_act2` set; sitting is not included) -> `0x0bae` result 1, nothing removed; else for each `EQI_*` 0..`EQI_MAX-1` with an equipped item whose `equip_bitmask[i] & location` is non-zero, `pc_unequipitem(idx, flag=1)`; then ack OK. Each removed item sends its own `0x099a` (and look packets), and they all precede the single `0x0bae`. `pc_unequipitem` is called with `flag = 1` **per item**, so `status_calc_pc(SCO_FORCE)` runs once per removed item (inside each call, after that item's `0x099a`); each run pushes its own diff (see [status.md](./status.md#recalculation-pushes)), so the client sees up to N diff bursts interleaved with the `0x099a`s, not one at the end. Items occupying two `EQI` slots (2H weapon) are removed once because the first call sets all its `equip_index` entries to -1. If removing a weapon also removes worn matching ammo (see below), that ammo's `0x099a` is sent inside the weapon's call and the ammo is skipped later. Request `location` bit tests use `equip_bitmask[EQI]` of the slot, per item.
* Ack `0x0bae`: `{ u16 opcode; u8 result }` (3 bytes), `result` = 1 if failed (`p.result = failed`), 0 OK.

## `0x01d7` ZC_SPRITE_CHANGE2 (look change)

> Open findings: [PF-01](../../../PACKET_FINDINGS.md#pf-01).

Source: `packets_struct.hpp:2591-2603`, `clif.cpp:3935-3960` (`clif_sprite_change`), `clif.cpp:3965-4140` (`clif_changelook`); opcode chosen by `sendLookType` (`packets_struct.hpp:316-320`, `PACKETVER >= 4` -> `0x01d7`).

Layout at `PACKETVER_MAIN_NUM >= 20181121` (taken for 20260219):

| Offset | Size | Type | Field |
| :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | opcode `0x01d7` |
| 2 | 4 | u32 | `AID` (`bl.id`) |
| 6 | 1 | u8 | `type` (`LOOK_*`) |
| 7 | 4 | u32 | `val` |
| 11 | 4 | u32 | `val2` |

Total 15 bytes.

**Length:** `clif_packetdb.hpp:226` still lists `packet(0x01d7,11)` (the pre-20181121 u16/u16 layout), but the struct sent at this packetver is 15 bytes. Capture-confirmed and fixed in `table.rs` ([PF-01](../../../PACKET_FINDINGS.md#pf-01), resolved).

### `LOOK_*` values (`map.hpp:595-612`, enum starts at 0)

| Value | Name | Value | Name |
| :--- | :--- | :--- | :--- |
| 0 | `LOOK_BASE` (job/sprite class) | 7 | `LOOK_CLOTHES_COLOR` |
| 1 | `LOOK_HAIR` | 8 | `LOOK_SHIELD` |
| 2 | `LOOK_WEAPON` | 9 | `LOOK_SHOES` |
| 3 | `LOOK_HEAD_BOTTOM` | 10 | `LOOK_BODY` (never sent) |
| 4 | `LOOK_HEAD_TOP` | 11 | `LOOK_RESET_COSTUMES` |
| 5 | `LOOK_HEAD_MID` | 12 | `LOOK_ROBE` |
| 6 | `LOOK_HAIR_COLOR` | 13 | `LOOK_BODY2` |

### Which equip event sends which look

| Trigger | Packet sent | Source |
| :--- | :--- | :--- |
| Right-hand slot changes (equip/unequip) | one `0x01d7`, `type = LOOK_WEAPON(2)`, `val = vd.look[WEAPON]`, `val2 = vd.look[SHIELD]` | `clif.cpp:4119-4125` |
| Left-hand slot changes | same packet: `LOOK_SHIELD` is rewritten to `LOOK_WEAPON` with both values | same |
| Shoes slot changes | `type = LOOK_SHOES(9)`, `val` = shoes `view_id`, else shoes nameid, 0 if unequipped | `clif.cpp:4064-4077` |
| Head low/mid/top or garment (incl. costume) view changes | `type = LOOK_HEAD_BOTTOM(3)` / `LOOK_HEAD_MID(5)` / `LOOK_HEAD_TOP(4)` / `LOOK_ROBE(12)`, `val` = view id, `val2 = 0`; only sent for the slots whose resulting view differs from before | `pc.cpp:15490-15545` |

`LOOK_ROBE` is dropped for `PACKETVER < 20110111`; `LOOK_BODY2` rules do not matter for equipment.

**Value semantics (important):**
* Head/robe `val` = item `View` field (`item_data.look`), not the item id.
* Weapon/shield `val` = the equipped item's **alias id (`view_id`, from `AliasName`) if non-zero, else the item's own `nameid`**, via `map_session_data::update_look` (`pc.cpp:623-690`). So weapon sprite ids on the wire are item ids (e.g. 1201 for Knife), not weapon-type numbers. Shield likewise (Guard = 2101). This is the same `weapon`/`shield` u32 carried in the entry packets (cross-link [map_units.md](./map_units.md), `0x09fe`/`0x09ff`/`0x09fd` fields).
* Nothing equipped -> 0. Under `OPTION_COSTUME` both are 0.
* 2H weapon: `LOOK_SHIELD` value is forced to 0 because left and right slot indices are the same item.
* `AREA` target (everyone in view including the player). If the player has `OPTION_INVISIBLE` the target is `SELF`. Disguised players get `AREA_WOS` for the real id plus `SELF` for the disguise id.

## `0x013c` ZC_EQUIP_ARROW and `0x013b` ZC_ACTION_FAILURE

Source: `packets.hpp:1000-1004`, `1114-1118`; `clif.cpp:4204-4240`.

`0x013c`: `{ u16 opcode; u16 index }` 4 bytes, SELF. `index` = wire index of the ammo item (`slot + 2`). Before this packet, at `PACKETVER >= 20121128`, the server first sends a status-change packet for `EFST_CLIENT_ONLY_EQUIP_ARROW` (`clif_status_change(&sd, EFST_CLIENT_ONLY_EQUIP_ARROW, 1, INFINITE_TICK, 0,0,0)`); the status-change packet is `0x0983` (29 bytes, state 1, 9999 ms, vals 0), AREA target, EFST 695; full layout and the answer to what ends it (nothing) in [status_change.md](./status_change.md). Order on ammo equip: any displaced old ammo `0x099a`, then `0x0983`, `0x013c`, `0x013b type=3`. At login `clif_inventorylist` calls the same `clif_arrowequip` when an ammo slot is occupied (`clif.cpp:3125-3126`), so `0x0983` + `0x013c` are sent there too (between the normal list and the equip list).

**Ammo quantity:** `0x013c` is only sent from those two places (grep: `clif.cpp:3126`, `pc.cpp:12164`). Consuming arrows (`pc_delitem` with `0x07fa`) or adding more of the same item (merge `0x0a37`) sends no ammo packet; the worn flag is the item's `equip` field. When the whole worn stack is consumed, `pc_delitem` unequips first (`0x099a` before `0x07fa`). Nothing else is needed to equip ammo.

`0x013b`: `{ u16 opcode; u16 type }` 4 bytes, SELF. `type` (`clif.cpp:4216-4225`):
* `0` "Please equip the proper ammunition first."
* `1` weight limit exceeded (cannot attack or use skills)
* `2` weight limit exceeded (cannot use skills)
* `3` ammunition equipped message (`ARROWFAIL_SUCCESS`): throwing daggers for assassin family, bullets for gunslinger, generic "Ammunition has been equipped." otherwise (ninja gets none).

Successful ammo equip sends `0x013c` then `0x013b type=3`, and does **not** send `0x0999`. (`pc.cpp:12199-12204`: `if (pos == EQP_AMMO) { clif_arrowequip; clif_arrow_fail(ARROWFAIL_SUCCESS) } else clif_equipitemack(OK)`.)

Missing-ammo-on-attack uses the same `0x013b type=0` (combat side; see [combat.md](./../mechanics/combat.md)).

## Take-off / drop order (`pc_unequipitem`)

Source: `pc.cpp:12311-12460` (`pc_unequipitem_sub`, `pc_unequipitem`), `pc.cpp:12015-12260` (`pc_equipitem`), rathena `d4b8e7b8f`. Flag bits: `1` run `status_calc_pc(SCO_FORCE)` (inside `_sub`), `2` force (skip `cant.unequip`), `4` equip switch / skip ammo auto-unequip and `pc_checkallowskill`.

Sequence for `pc_unequipitem(n, flag)` on success, in packet order:
1. `equip_index[]` cleared for the item's slots (no packet).
2. Right hand: `0x01d7` look (weapon + shield values, AREA). Left hand: `0x01d7`. Shoes: `0x01d7` type 9 val 0. (Right and left each call `clif_changelook`, so a 2H weapon sends two identical `0x01d7`.)
3. `0x099a {index+2, equip mask, flag 0}` (SELF).
4. `pc_set_costume_view`: `0x01d7` for head/robe slots whose view changed.
5. Weapon (`EQP_ARMS`, type weapon) of a ranged class (bow, instrument, whip, gun types, grenade) and `ammo_unequip` battle option on (`battle.conf:173`, default yes) and `!(flag & 4)`: worn ammo is unequipped next: `0x099a {ammo index+2, ammo equip mask (EQP_AMMO), 0}`, no EFST end, no calc of its own.
6. `pc_unequipitem_sub`: if `flag & 1` (or a combo was removed): `status_calc_pc` then diff pushes (`SP_*` updates, `SP_ATTACKRANGE` etc.). Then unequip scripts.

Equipping a weapon that no longer matches worn ammo calls `pc_unequipitem(ammo, 2|4)` from `pc_equipitem`: only `0x099a` for the ammo, no calc of its own; `pc_equipitem` does one `status_calc_pc(SCO_NONE)` at the end. Equipping a new item over occupied slots unequips the old one with `1|2|4`: look + `0x099a` + its own `status_calc_pc` diff push (flag 1), and no ammo cascade (flag 4). The push is capture-confirmed ([Captured weapon swap](#captured-weapon-swap) rows 3–4), so a swap sends two recalculation pushes.

### Worn item deleted or dropped (`0x0363`)

`pc_dropitem` (`pc.cpp:6137-6177`): `map_addflooritem` first (the floor copy keeps the old `equip` bits; `pc_additem` clears `equip` on pickup, `pc.cpp:6049-6052`), then `pc_delitem(n, amount, type=1, reason 0)`, then `clif_dropitem`.

* **Full stack of a worn item**: `pc_delitem` reduces amount and `sd->weight`, then (amount reached 0 and `equip != 0`) `pc_unequipitem(n, 2|1)` with `type&4 == 0`. Flag 3 = forced (ignores `cant.unequip`) + status recalc. Packets to self in order: look `0x01d7` (AREA, if hand/shoes slot) -> `0x099a {idx+2, mask, 0}` -> costume-view `0x01d7` (if changed) -> ammo `0x099a` (if a weapon) -> status diff pushes (one `status_calc_pc`) -> then slot cleared; `type=1` suppresses `0x07fa` -> `SP_WEIGHT` (`0x00b0` weight update) -> `0x00af {idx+2, amount}`. Area gets only the `0x01d7` look packets. For a delete via other paths with `type` 0: `0x07fa` goes between the diff pushes and `SP_WEIGHT`.
* **Partial drop** (amount < stack, e.g. 5 of 100 worn arrows): no unequip at all; the ammo stays worn (`equip` unchanged, no `0x099a`, no `0x013c`/EFST resend). Packets: `SP_WEIGHT`, `0x00af {idx+2, 5}`.
* Drop gates (dead, `pc_cant_act2`, `npc_id`, `cant.drop`) fail with `0x00af count 0`; sitting does not block drop or equip.

Sitting/dead gates: `pc_cant_act2` (`pc.hpp:1156-1164`) = vending, buyingstore, `opt1` set (except `OPT1_BURNING`), trading, storage open, prevend, refine UI, stylist, inventory-expansion confirm, `npc_shopid`, barter (both), laphine synthesis/upgrade, roulette, enchantgrade, item reform, item enchant. Sitting (`pc_issit`) is **not** checked in `clif_parse_EquipItem`, `clif_parse_UnequipItem`, `clif_parse_DropItem`, `pc_equipitem` or `pc_unequipitem`: equip, take-off and drop all work while sitting. Dead: equip and take-off do `clif_clearunit_area(CLR_DEAD)` and return with no ack; drop sends `0x00af count 0`. Equip/take-off also pass when `opt1` is set or storage is open (the `else if` chain skips `pc_cant_act2`), but `PCBLOCK_EQUIP` / a running NPC script reject with a red `MSI_CAN_NOT_EQUIP_ITEM`.

## Messages sent as chat-style notices (`clif_msg`, `MSI_*`)

From `pc_isequip` (`pc.cpp:1901-1935`) for failed ammo/weapon pairing: `MSI_FAIL_NEED_EQUIPPED_BOW` for arrows without bow/instrument/whip, `MSI_WRONG_BULLET` for bullets/shells/grenades without a matching gun, `MSI_USESKILL_FAIL_MADOGEAR` and `MSI_USESKILL_FAIL_CART` for cannonballs. `pc_isequip` only uses plain `clif_msg`; `clif_msg_color` is used by pickup failures (`pc.cpp:6397-6412`), `clif_msg_value` by item reuse limit.

**`0x0291` ZC_MSG (4 bytes, fixed)**: `+0 packetType.W = 0x0291`, `+2 msgId.W` (zero-based `msgstringtable.txt` index, i.e. line number minus 1). Sent only to the owner (`SELF`). Struct `packets.hpp:1980-1984`; sender `clif.cpp:10382-10391` (`#if PACKETVER >= 20060424`). Applies to 20260219.

Siblings (not used by equip): `0x07e2` ZC_MSG_VALUE 8 bytes (`msgId.W`, `value.L`; `packets.hpp:1993-1998`); `0x07e6` ZC_MSG_SKILL 8 bytes (`skillId.W`, `msgId.L`; `packets.hpp:1986-1991`); `0x09cd` ZC_MSG_COLOR 8 bytes (`msgId.W`, `color.L`; `packets_struct.hpp:2229-2233`, sender `clif.cpp` `clif_msg_color`, `PACKETVER >= 20130807`).

**`MSI_*` values at 20260219** (`clif.hpp`, rAthena `d4b8e7b8f`): `MSI_CAN_NOT_EQUIP_ITEM` 372 (`:546`), `MSI_USESKILL_FAIL_MADOGEAR` 1435 (`:573`), `MSI_USESKILL_FAIL_CART` 1519 (`:576`), `MSI_FAIL_NEED_EQUIPPED_BOW` 1691 (`:597`), `MSI_WRONG_BULLET` **2493** (`:634`). The `#if` at `clif.hpp:599` is `PACKETVER` 20130807..20130814 and not `PACKETVER_ZERO`; 20260219 is outside it, so the `#else` values apply (2494 only in that 2013 window). Other values in that block shift by one the same way (`MSI_BUSY` 1923, `MSI_ITEM_REUSE_LIMIT_SECOND` 1862). Header comment says these values still "need client version validation": capture to confirm (PF-18).

## Side packets after a successful equip/unequip

No dedicated "stats changed" packet exists; `status_calc_pc` (see mechanics) diffs old/new status and pushes `clif_updatestatus` parameter updates (see `0x00b0` `ZC_PAR_CHANGE` in [map_units.md](./map_units.md); the `SP_*` ids and their packet choice are in the stats notes) for any changed value: STR..LUK, HIT, FLEE1/2, ASPD, SPEED, ATK1/ATK2, DEF1/DEF2, MDEF1/MDEF2, CRITICAL, MATK1/MATK2, **`SP_ATTACKRANGE`** (when right-hand weapon range changes), MAXHP/MAXSP, HP/SP (`status.cpp` around 6650-6760 in `status_calc_bl_main`). Weight is not recomputed by equipping. If the right-hand weapon range changed and `skillrange_from_weapon` applies to players, `clif_skillinfoblock` re-sends the skill list.

## Equipment in the inventory list at login

Handled by the inventory packet (`clif_inventorylist`, `clif.cpp:3062`); see [inventory.md](./inventory.md#0x0b39-zc_equipment_itemlist-non-stackable-items). Equip-relevant facts:
* Equippable (non-stackable) items go in the "equip" list packet; each entry built by `clif_item_equip` (`clif.cpp:2932-2988`) with `location` = `pc_equippoint_sub` (the **slot-capability** mask; for an assassin a one-hand weapon reports `EQP_ARMS`), and `WearState` = the item's `equip` (the **currently worn** mask; 0 if not worn). Both are u32 at this version (`packets_struct.hpp:457-470`, `>= 20120925`).
* `wItemSpriteNumber` = item `View` if `equip & EQP_VISIBLE` else 0.
* `RefiningLevel`, `grade` (enchant grade, `>= 20200916`), card slots (4 x u32 at `>= 20181121`), `HireExpireDate`, `bindOnEquipType` (2 if bound, 1 if bind-on-equip item, else 0), random options and flags are all in the same entry.
* After the lists, if an ammo slot is occupied, `0x013c` is sent.
* Client derives the on-body sprite from the entry packets, not from the list.

## Equip switch (out of scope)

`clif_equipswitch_*` (`clif.cpp:22236-22343`) handle the saved equip-switch feature (`equip_switch` column). Not documented here.

### `0x01d7` at `LoadEndAck` (map entry, to self)

Provenance: rathena `d4b8e7b8f`, `clif.cpp:10748-10776` (`clif_parse_LoadEndAck`), `clif.cpp:3965-4140` (`clif_changelook`), `pc.cpp:622-678` (`update_look`), `pc.cpp:15490-15560` (`pc_set_costume_view`), `clif.cpp:1656-1675` (`clif_refresh_clothcolor`). Applies to `20260219` (`PACKETVER >= 4` branch).

* Exactly one `0x01d7` is sent: `clif_changelook(sd, LOOK_WEAPON, 0)`. For a PC with `LOOK_WEAPON`/`LOOK_SHIELD` the passed `val` is discarded: both looks are recomputed by `update_look` and the packet is `type = LOOK_WEAPON (2)`, `val = vd.look[WEAPON]`, `val2 = vd.look[SHIELD]`, target `AREA` (includes self; observers in view also get it; `SELF` only if the char is `OPTION_INVISIBLE`).
* (a) nothing in either hand: `val = 0`, `val2 = 0`.
* (b) weapon in right hand: `val` = item `view_id` if non-zero else its item id (capture: Knife 1201 = `b1 04 00 00`); `val2` = left-hand item view id/item id if a different inventory index (shield), else 0. Two-handed weapons occupy both hands (same index) so `val2 = 0`. Costume option (`OPTION_COSTUME`) forces both 0.
* Other look packets at `LoadEndAck` to self for a plain novice: none. `pc_set_costume_view` only sends `LOOK_HEAD_BOTTOM/MID/TOP/ROBE` when the recomputed view differs from `status.head_*` loaded from the DB (no headgear or costumes -> nothing). `clif_refresh_clothcolor` (`LOOK_CLOTHES_COLOR` 7) is compiled only for `PACKETVER < 20091103`, so it is a no-op at 20260219 regardless of cloth color. No hair/hair-color packets either (those are in the spawn packet).
