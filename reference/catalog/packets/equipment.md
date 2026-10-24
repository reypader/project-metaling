# Equip / Unequip Packets (PACKETVER 20260219)

Scope: packets exchanged when a player wears or removes an item, plus the look-change broadcast. Rules and flow live in [../mechanics/equipment.md](../mechanics/equipment.md). Equipment rows in the inventory list packets are owned by `packets/inventory.md` (other agent); only the equip-specific fields are noted here.

**Provenance (all sections):** repo `rathena` (`$RATHENA_PATH`), files `src/map/{packets.hpp,packets_struct.hpp,clif.cpp,clif.hpp,clif_packetdb.hpp,clif_shuffle.hpp}`. **Applies to `20260219`: yes**, resolved with `PACKETVER_MAIN_NUM = 20260219` (`PACKETVER_RE` undefined, `src/config/packets.hpp:21-45`), so every `>= 20121205` / `>= 20120925` / `>= 20130000` / `>= 20181121` / `>= 20230906` branch is taken.

**Shuffle check:** the `clif_shuffle.hpp` branch that applies to 20260219 is `PACKETVER > 20180307` (L4723-4761). It does not list `0x0998`, `0x00ab` or `0x0bf5`, so these opcodes come straight from `packets.hpp` / `clif_packetdb.hpp`. See [PF-05](../../../PACKET_FINDINGS.md#pf-05).

**Captures:** `tools/proxy/packet_captures/` does not exist in this checkout, so none of this is capture-verified. If a capture disagrees, the capture wins.

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

Same dead / npc / block-action / `pc_cant_act2` gates as equip. Then `pc_unequipitem(sd, index, flag=1)`. Note: no bounds check in the handler; `pc_unequipitem` rejects out of range with a FAIL ack (index 0).

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
* Server: if `pc_cant_act` -> ack fail; else for each `EQI_*` 0..`EQI_MAX-1` with an equipped item whose `equip_bitmask[i] & location` is non-zero, `pc_unequipitem(idx, flag=1)`; then ack OK. Each removed item sends its own `0x099a`.
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

**DISCREPANCY (flag for the caller):** `clif_packetdb.hpp:226` still lists `packet(0x01d7,11)` (the pre-20181121 u16/u16 layout), and `crates/ro-packets/src/table.rs:235` has `0x01d7 => Fixed(11)`. The struct actually sent for this packetver is 15 bytes. Server-to-client lengths are not used to parse by rAthena, but a Rust length table used for framing or for proxy parsing should say 15. Unverified against a capture.

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

`0x013c`: `{ u16 opcode; u16 index }` 4 bytes, SELF. `index` = wire index of the ammo item (`slot + 2`). Before this packet, at `PACKETVER >= 20121128`, the server first sends a status-change packet for `EFST_CLIENT_ONLY_EQUIP_ARROW` (`clif_status_change(&sd, EFST_CLIENT_ONLY_EQUIP_ARROW, 1, INFINITE_TICK, 0,0,0)`); the status-change packet layout is outside this file's scope. `clif_inventorylist` also re-sends `0x013c` at login if an ammo slot is occupied (`clif.cpp:3125`).

`0x013b`: `{ u16 opcode; u16 type }` 4 bytes, SELF. `type` (`clif.cpp:4216-4225`):
* `0` "Please equip the proper ammunition first."
* `1` weight limit exceeded (cannot attack or use skills)
* `2` weight limit exceeded (cannot use skills)
* `3` ammunition equipped message (`ARROWFAIL_SUCCESS`): throwing daggers for assassin family, bullets for gunslinger, generic "Ammunition has been equipped." otherwise (ninja gets none).

Successful ammo equip sends `0x013c` then `0x013b type=3`, and does **not** send `0x0999`. (`pc.cpp:12199-12204`: `if (pos == EQP_AMMO) { clif_arrowequip; clif_arrow_fail(ARROWFAIL_SUCCESS) } else clif_equipitemack(OK)`.)

Missing-ammo-on-attack uses the same `0x013b type=0` (combat side; see [combat.md](./../mechanics/combat.md)).

## Messages sent as chat-style notices (`clif_msg`, `MSI_*`)

From `pc_isequip` for failed ammo/weapon pairing: `MSI_FAIL_NEED_EQUIPPED_BOW` (1691) for arrows without bow/instrument/whip, `MSI_WRONG_BULLET` for bullets/shells/grenades without a matching gun (value is 2494 or 2493 depending on packetver branch at `clif.hpp:616/634`; not resolved here), `MSI_USESKILL_FAIL_MADOGEAR` and `MSI_USESKILL_FAIL_CART` for cannonballs. The `clif_msg` packet layout is not covered here.

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
