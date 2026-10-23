# Inventory Packets

Target client version: `PACKETVER = 20260219`.

* **Provenance (all sections):** `rathena` @ `d4b8e7b8f`: `src/map/packets_struct.hpp`, `src/map/packets.hpp`, `src/map/clif.cpp`, `src/map/clif_packetdb.hpp`. Opcode selection done by hand against the `#if PACKETVER...` ladders with `PACKETVER_MAIN_NUM = 20260219` (`PACKETVER_RE_NUM`/`ZERO` unset). Sizes computed by summing the packed fields. No proxy capture of these packets was available in the workspace: **the server->client layouts are source-derived; the client->server opcodes have a caveat (see "Client to server opcodes").**
* Semantics (checks, ordering, weight): [mechanics/inventory.md](../mechanics/inventory.md). Item fields: [db/item_db.md](../db/item_db.md). Weight updates (`0x00b0`): [map_units.md#0x00b0-zc_par_change](./map_units.md#0x00b0-zc_par_change). Expansion info (`0x0b18`): [map.md#0x0b18-zc_inventory_expansion_info](./map.md#0x0b18-zc_inventory_expansion_info).
* **Conventions:** little-endian; all `index` fields are the **wire index = server slot + 2** (`clif_index`, `clif.cpp:122`). All item-id fields are `u32` and carry the client view id: `AliasName` target if set (`view_id`), else the real id (`client_nameid`, `clif.cpp:144-151`, `PACKETVER_MAIN_NUM >= 20181121`). Card slots are `u32`. Bitfield bytes are LSB-first (`bit0` first, C bitfield order on little-endian).
* **Item `type` byte** (`itemtype()`, `clif.cpp:109-118`): item DB type, except PetEgg -> 4 (Armor); ShadowGear with a shadow-weapon location -> 5 (Weapon), any other ShadowGear -> 4 (Armor). Values: 0 Healing, 2 Usable, 3 Etc, 4 Armor, 5 Weapon, 6 Card, 8 PetArmor, 10 Ammo, 18 Cash. (DelayConsume 11 never appears: it is rewritten to Usable at DB load.) See [db/item_db.md](../db/item_db.md#type-enum).

## Sequence summary

> Open findings: [PF-08](../../../PACKET_FINDINGS.md#pf-08).

| When | Server -> Client (in order) |
| :--- | :--- |
| Auth OK (`pc_authok`) | `0x0b18` expansion info, then `0x02eb` |
| Every `LoadEndAck` (login + map change) | `0x0b08` start, `0x0b09` normal items (if any stackables), `0x013c` + status icon (if ammo equipped), `0x0b39` equipment items (if any), `0x0b0b` end, then equip-switch list, cart, `0x00b0` weight, `0x00b0` max weight |
| Item gained (pickup, reward, merge) | `0x0b41` (success) then `0x00b0` `SP_WEIGHT` |
| Item removed (consume/sell/etc.) | `0x07fa` (unless suppressed) then `0x00b0` `SP_WEIGHT` |
| Item dropped by player | `0x00af`, plus `0x00b0` weight (`pc_delitem` type 1 keeps the weight update) |
| Item used | `0x01c8` (to AREA), then `0x00b0` weight (not `0x07fa`) |

## `0x0b08` `ZC_INVENTORY_START`

* **Direction:** Map to Client. **Length:** variable (`packetLength`), always 6 for the plain inventory (name is the empty string). `clif_inventoryStart`, `clif.cpp:3020-3045`; struct `packets_struct.hpp:1218-1233`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x0b08` |
| 2 | 2 | i16 | `packet_length` | `5 + name_len_with_nul` (6 for empty name; storage uses the storage name, max 24 incl. NUL) |
| 4 | 1 | u8 | `inv_type` | `0` inventory, `1` cart, `2` storage, `3` guild storage (`clif.cpp:98-103`) |
| 5 | n | char[] | `name` | NUL-terminated; empty (1 byte `00`) for inventory |

## `0x0b09` `ZC_NORMAL_ITEMLIST` (stackable items)

* **Direction:** Map to Client. **Length:** variable: `5 + 34 * count`. One packet per up to 963 items (`MAX_INVENTORY_ITEM_PACKET_NORMAL = (32767-5)/34`); with 200 slots always a single packet. Omitted entirely when there are no stackable items. Opcode from `inventorylistnormalType` (`packets_struct.hpp:137-146`, branch `PACKETVER_MAIN_NUM >= 20181002`). Entries are in slot order; empty slots and slots with a null `inventory_data` are skipped.
* **Header:** `0 u16 0x0b09`, `2 i16 packet_length`, `4 u8 inv_type`, then entries from offset 5.

`NORMALITEM_INFO` (34 bytes, `packets_struct.hpp:418-447`, filled by `clif_item_normal`, `clif.cpp:2987-3017`):

| Entry off | Size | Type | Field | Source / notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | i16 | `index` | slot + 2 |
| 2 | 4 | u32 | `ITID` | view id (alias) or `nameid` |
| 6 | 1 | u8 | `type` | `itemtype()` above |
| 7 | 2 | i16 | `count` | `item.amount` |
| 9 | 4 | u32 | `WearState` | the item DB `Locations` mask (`id->equip`), NOT the currently-worn mask (ammo shows `EQP_AMMO = 0x8000`) |
| 13 | 16 | u32[4] | `slot.card[4]` | `clif_addcards` (pet eggs: 0,0,intimacy>>1,renamed; forged/created: verbatim; else card ids as view ids) |
| 29 | 4 | i32 | `HireExpireDate` | `expire_time` (0 = none) |
| 33 | 1 | u8 flags | `Flag` | bit0 `IsIdentified`, bit1 `PlaceETCTab` (favorite), bits 2-7 = 0 |

## `0x0b39` `ZC_EQUIPMENT_ITEMLIST` (non-stackable items)

* **Direction:** Map to Client. **Length:** variable: `5 + 68 * count`, max 481 per packet (`(32767-5)/68`); single packet in practice. Omitted when no equipment-type items. Opcode `inventorylistequipType = 0xb39` for `PACKETVER_MAIN_NUM >= 20200916` (`packets_struct.hpp:148-162`; the older `0x0b0a` variant is NOT used). Sent after the normal list (and after `0x013c`). Contains every non-stackable item, equipped or not: weapons, armor, shadow gear, pet eggs, pet armor.
* **Header:** same 5-byte header as `0x0b09` (`0x0b39`, length, inv_type).

`EQUIPITEM_INFO` (68 bytes, `packets_struct.hpp:457-502`, filled by `clif_item_equip`, `clif.cpp:2932-2980`):

| Entry off | Size | Type | Field | Source / notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | i16 | `index` | slot + 2 |
| 2 | 4 | u32 | `ITID` | view id or `nameid` |
| 6 | 1 | u8 | `type` | `itemtype()` |
| 7 | 4 | u32 | `location` | `pc_equippoint`: item DB `Locations` mask; one-hand dagger/sword/axe becomes `EQP_ARMS (0x22)` for assassin-class or `AS_LEFT` users (`pc.cpp:1464-1488`) |
| 11 | 4 | u32 | `WearState` | `item.equip`: currently worn positions (0 = not worn) |
| 15 | 16 | u32[4] | `slot.card[4]` | as above |
| 31 | 4 | i32 | `HireExpireDate` | `expire_time` |
| 35 | 2 | u16 | `bindOnEquipType` | `bound ? 2 : (BindOnEquip flag ? 1 : 0)` |
| 37 | 2 | u16 | `wItemSpriteNumber` | item DB `View` if its `Locations` intersect `EQP_VISIBLE` (heads, garment, costumes), else 0 |
| 39 | 1 | u8 | `option_count` | always 0 (client ignores / recomputes) |
| 40 | 25 | 5 x {i16 `index`, i16 `value`, u8 `param`} | `option_data[5]` | random options; `id == 0` entries zeroed |
| 65 | 1 | u8 | `RefiningLevel` | `item.refine` |
| 66 | 1 | u8 | `grade` | `item.enchantgrade` |
| 67 | 1 | u8 flags | `Flag` | bit0 `IsIdentified`, bit1 `IsDamaged` (`attribute != 0`), bit2 `PlaceETCTab` (favorite), bits 3-7 = 0 |

Field order note: for this version `RefiningLevel` moved to AFTER the option array (`>= 20200916`); `IsIdentified`/`IsDamaged` moved into `Flag` (`>= 20120925`).

## `0x013c` `ZC_EQUIP_ARROW`

4 bytes: `u16 0x013c`, `u16 index` (wire index of the equipped ammo). Sent only between the two lists when `equip_index[EQI_AMMO] >= 0`, preceded by a status-change icon `EFST_CLIENT_ONLY_EQUIP_ARROW` (`clif.cpp:4204-4217`). Equip flow is out of scope here.

## `0x0b0b` `ZC_INVENTORY_END`

* **Length:** fixed 4 bytes (`packets_struct.hpp:1236-1245`, `clif_inventoryEnd`, `clif.cpp:3047-3060`).

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x0b0b` |
| 2 | 1 | u8 | `inv_type` | 0 inventory |
| 3 | 1 | i8 | `flag` | always 0 |

## `0x0b41` `ZC_ITEM_PICKUP_ACK` (add item / pickup result)

> Open findings: [PF-07](../../../PACKET_FINDINGS.md#pf-07).

* **Direction:** Map to Client. **Length:** fixed **70 bytes**. Opcode: `DEFINE_PACKET_HEADER(ZC_ITEM_PICKUP_ACK, 0x0b41)` for `PACKETVER_MAIN_NUM >= 20200916` (`packets_struct.hpp:540-595`). Built by `clif_additem` (`clif.cpp:2838-2891`).
* The same packet reports success and failure. On failure only `index` (= 2), `count` (= 0), `result` are non-zero; all other bytes are zero.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x0b41` |
| 2 | 2 | u16 | `index` | slot + 2 (failure: `2`, i.e. server index 0) |
| 4 | 2 | u16 | `count` | amount ADDED by this event (not the new total); 0 on failure |
| 6 | 4 | u32 | `nameid` | view id or `nameid` |
| 10 | 1 | u8 | `is_identified` | `identify != 0` |
| 11 | 1 | u8 | `is_damaged` | `attribute != 0` |
| 12 | 16 | u32[4] | `slot.card[4]` | `clif_addcards` |
| 28 | 4 | u32 | `location` | `pc_equippoint` mask (0 for non-equip) |
| 32 | 1 | u8 | `type` | `itemtype()` |
| 33 | 1 | u8 | `result` | 0 ok; failure codes below |
| 34 | 4 | i32 | `hire_expire_date` | `expire_time` |
| 38 | 2 | u16 | `bind_on_equip_type` | here: `bound && !stackable ? 2 : (BindOnEquip ? 1 : 0)` (stackables never show 2 in this packet, note differs from list structs) |
| 40 | 25 | 5 x {i16,i16,u8} | `option_data[5]` | random options |
| 65 | 1 | u8 | `favorite` | `item.favorite != 0` |
| 66 | 2 | u16 | `look` | item DB `View` sprite |
| 68 | 1 | u8 | `refining_level` | |
| 69 | 1 | u8 | `grade` | `enchantgrade` |

`result` values (the `e_additem_result` numbers go straight to the wire, `pc.hpp:113-122`; comment in `pc.cpp:5976-5989`): 0 success; 1 invalid item / bad amount; 2 overweight; 3 unused; 4 no free inventory slot; 5 would exceed max stack (30000 or `Stack.Amount`); 6 generic "pickup failed" (sent by `clif_parse_TakeItem` when pickup is refused for any pre-check reason; the client needs this to unblock pickups, `clif.cpp:12034`); 7 stack limit (single add larger than `Stack.Amount`). The client message text per code is client-side (not in rAthena); do not assume wording.

## `0x00af` `ZC_ITEM_THROW_ACK` (item left inventory by drop)

* **Length:** fixed 6 bytes. `packets.hpp:818-823`, `clif_dropitem` `clif.cpp:2895-2905`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x00af` |
| 2 | 2 | u16 | `index` | slot + 2 |
| 4 | 2 | u16 | `count` | amount removed; `0` = request refused (client un-sticks, no change) |

## `0x07fa` `ZC_DELETE_ITEM_FROM_BODY`

* **Length:** fixed 8 bytes. `packets.hpp:825-831`, `clif_delitem` `clif.cpp:2917-2930`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x07fa` |
| 2 | 2 | i16 | `delete_type` | 0 normal, 1 used for a skill, 2 refine failed, 3 material changed, 4 moved to storage, 5 moved to cart, 6 sold, 7 consumed by Four Spirit Analysis |
| 4 | 2 | u16 | `index` | slot + 2 |
| 6 | 2 | i16 | `count` | amount removed (not the remaining amount) |

## `0x01c8` `ZC_USE_ITEM_ACK` (ack of using a consumable)

* **Length:** fixed 15 bytes. Opcode `useItemAckType = 0x1c8` (`PACKETVER >= 3`; `packets_struct.hpp:311-315`); struct `:2577-2588` (`>= 20181121` layout). `clif_useitemack`, `clif.cpp:4468-4493`.

| Offset | Size | Type | Field | Notes |
| :--- | :--- | :--- | :--- | :--- |
| 0 | 2 | u16 | `packet_id` | `0x01c8` |
| 2 | 2 | i16 | `index` | slot + 2 |
| 4 | 4 | u32 | `item_id` | view id or `nameid` of the used item (read from the slot BEFORE deletion) |
| 8 | 4 | u32 | `aid` | user's account id (`sd->id`) |
| 12 | 2 | i16 | `amount` | **remaining** amount after use (`stack - 1`), or full `amount` for delay-consume items; 0 on failure |
| 14 | 1 | u8 | `result` | 1 ok, 0 failed |

Routing: `result == 1` is sent to `AREA` (including self, so observers see the use effect); `result == 0` to `SELF` only. Not sent if the slot is empty at send time. The ack carries the remaining count; the paired deletion uses no `0x07fa`.

## Client to server opcodes

> Open findings: [PF-05](../../../PACKET_FINDINGS.md#pf-05).

| Opcode | Name | Len | Fields (byte offsets) | Handler |
| :--- | :--- | :--- | :--- | :--- |
| `0x0362` | `CZ_ITEM_PICKUP` (`..._PICKUP2`) | 6 | `object_id` u32 @2 (floor-item id) | `clif_parse_TakeItem` (`clif.cpp:12002`) |
| `0x0363` | `CZ_ITEM_THROW` (`..._THROW2`) | 6 | `index` u16 @2 (wire index, subtract 2), `amount` u16 @4 | `clif_parse_DropItem` (`clif.cpp:12042`) |
| `0x0439` | `CZ_USE_ITEM` (`..._USE_ITEM2`) | 8 | `index` u16 @2 (subtract 2), `aid` u32 @4 (ignored by server) | `clif_parse_UseItem` (`clif.cpp:12080`) |

* Source: `0x0362` and `0x0363` come from the `PACKETVER > 20180307` branch of `clif_shuffle.hpp` (L4723-4761), which is registered after `clif_packetdb.hpp` and overrides it (`clif.cpp:25843-25844`). That branch is what applies to 20260219, so the later `clif_packetdb.hpp` blocks that re-register `0x0362`/`0x0363` as other handlers don't matter. `0x0439` is not in the shuffle branch; its last `clif_packetdb.hpp` registration (`:1482`) is UseItem, 8 bytes. Settled from source in [PF-05](../../../PACKET_FINDINGS.md#pf-05); a capture should still confirm.
* Not in this file: `CZ_REQ_WEAR_EQUIP` / `CZ_REQ_TAKEOFF_EQUIP` (equip agent), favorite toggle, storage/cart moves, merge-item.

## `0x0b18` `ZC_INVENTORY_EXPANSION_INFO` (cross-reference)

Fixed 4 bytes `{u16 0x0b18, i16 expansion_size}`; `expansion_size = inventory_slots - 100` (0 by default, up to 100). Sent from `pc_authok` right before `0x02eb` (`pc.cpp:2238`) and after a successful expansion purchase (`clif.cpp:23187`). Layout row already in [map.md#0x0b18-zc_inventory_expansion_info](./map.md#0x0b18-zc_inventory_expansion_info) (there named `expansion_size` u16; source declares `int16`, value never negative).
