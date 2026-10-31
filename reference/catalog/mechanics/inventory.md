# Player Inventory Mechanics

Target client version: `PACKETVER = 20260219` (main-server numbering: `PACKETVER_MAIN_NUM`; `PACKETVER_RE` is NOT defined because 20260219 is outside both `PACKETVER_RE` ranges, `src/config/packets.hpp:20-30`). Renewal build.

* **Provenance (all sections):** `rathena` @ `d4b8e7b8f` (2026-10-06), paths relative to `$RATHENA_PATH`. Line numbers are for that commit. Written from source. [`captures/item_drop_pickup_sequence.txt`](../captures/item_drop_pickup_sequence.txt) (2026-10-23) since confirms these points:
* The pickup ack precedes the weight update (§3 step 8, §9).
* The inventory list is re-sent on every `LoadEndAck` (§10).
* `SP_MAXWEIGHT` = 20000 + 300×STR for a novice (20300 at STR 1, 23300 at STR 11; §8).
* A fresh add takes the lowest empty slot (the Jellopy landed at wire index 5 with holes below).

Everything else is still source-only.
* Wire layouts: [packets/inventory.md](../packets/inventory.md). Item DB fields: [db/item_db.md](../db/item_db.md). SQL: [db/sql_schemas.md#inventory-table-schema](../db/sql_schemas.md#inventory-table-schema).

## 1. Data model: `struct item`

`src/common/mmo.hpp:313-328` (packed). One element per inventory slot; an empty slot is `nameid == 0` (all fields zero).

| Field | Type | Meaning |
| :--- | :--- | :--- |
| `id` | `i32` | SQL row id of the `inventory` table (`inventory.id`). Only meaningful for the DB diff on save; not sent to the client. |
| `nameid` | `u32` (`t_itemid`, `mmo.hpp:76`) | Item id (`item_db.yml` `Id`). `0` = empty slot. |
| `amount` | `i16` | Stack size. `MAX_AMOUNT = 30000` (`mmo.hpp:81`). Non-stackable items always 1. |
| `equip` | `u32` | Bitmask of `enum equip_pos` the item is currently worn at (`mmo.hpp:336-362`); 0 = not worn. Cleared on `pc_additem` copy-in. |
| `identify` | `i8` | 0 = unidentified, nonzero = identified. Sent to client as a 1-bit flag. Start items are inserted with `identify=1` (`char.cpp:1519`). |
| `refine` | `i8` | Refine level 0..20 (`RefiningLevel` on the wire). |
| `attribute` | `i8` | Non-zero = broken (`IsDamaged` flag on the wire is `attribute != 0`). |
| `card[4]` | `u32` x `MAX_SLOTS=4` | Card/enchant item ids. Special `card[0]` markers (`itemdb.hpp:33-35`): `0x00FF` forged (`CARD0_FORGE`; card1..3 carry forge data, sent to the client verbatim), `0x00FE` created (`CARD0_CREATE`, same verbatim send), `0x0100` pet egg (`CARD0_PET`; on the wire cards 0/1 are zeroed, card2 = `card[3]>>1` (intimacy), card3 = `card[3]&1` (renamed flag), `clif_addcards`, `clif.cpp:2757-2799`). |
| `option[5]` | `{i16 id, i16 value, i8 param}` x `MAX_ITEM_RDM_OPT=5` | Random options (`mmo.hpp:113, 307-311`). `id == 0` = unused. |
| `expire_time` | `u32` | Unix time a rental item expires; 0 = permanent. Stackable rental items never merge (see 3). |
| `favorite` | `i8` | Favorite (client "move to ETC tab") flag. |
| `bound` | `i8` | `enum bound_type` (`mmo.hpp:1123-1133`): 0 none, 1 account, 2 guild, 3 party, 4 character. Wire `bindOnEquipType`: `2` if bound and non-stackable (shows yellow), else `1` if the item DB has `BindOnEquip`, else `0`. |
| `unique_id` | `u64` | Per-instance id (see 5). |
| `equipSwitch` | `u32` | Equip-bitmask for the equip-switch (alt gear) window; like `equip`, cleared on add. |
| `enchantgrade` | `u8` | Item grade (0 = none). Wire `grade` (since 20200916). |

* **Capacity constants:** `INVENTORY_BASE_SIZE = 100`; `INVENTORY_EXPANSION_SIZE = 100` for `PACKETVER >= 20181031` (true for 20260219); `MAX_INVENTORY = 200` (`mmo.hpp:44-62`). Per-character effective capacity is `char.inventory_slots` (default 100, max 200): every add/blank-count loops only `i < inventory_slots` (`pc.cpp:5756, 5779, 6029, 6045`). Expansion size sent to the client = `inventory_slots - 100` (see packets file, `0x0b18`).
* **Other containers (not covered):** storage `MAX_STORAGE = 600`, cart `MAX_CART = 100`.
* **Server index vs wire index:** the server array index `n` (0-based) goes on the wire as `n + 2` (`client_index`, `clif.cpp:122-124`); inbound client indexes are decremented by 2 (`server_index`). Storage uses +1 instead (`:130-136`). The client reserves 0/1 for internal use. Index is a slot position, NOT stable across relogs (see 7).
* **Inbound bound check:** `clif_parse_UseItem` rejects `n < 0 || n >= MAX_INVENTORY` (`clif.cpp:12097-12100`); `pc_dropitem` rejects the same (`pc.cpp:6139`); `pc_delitem` only rejects `n < 0` (callers must range-check).
* **Runtime companion:** `sd->inventory_data[n]` is a cached pointer to the item DB entry (null for empty slots; refilled from `nameid` after load, `pc_setinventorydata`, `pc.cpp:1500`). Unknown ids resolve to the dummy item (`itemdb_search`).

## 2. Stackable vs non-stackable

`item_data::isStackable()` (`itemdb.cpp:4932-4942`): **not stackable** iff `Type` is `Weapon`, `Armor`, `PetEgg`, `PetArmor` or `ShadowGear`. Everything else (`Healing`, `Usable`, `Etc`, `Card`, `Ammo`, `DelayConsume`, `Cash`) is stackable. `flag.guid` (`UniqueId` flag) forces a separate slot per add even for stackables (`inventorySlotNeeded`, `:4944`). Consequences: equipment/pet eggs get one slot each (unique_id assigned per item); stackables merge by key (3).

* Slot count to add `q` items: `q` if non-stackable or UniqueId-flagged, else 1.
* Optional per-item cap: `Stack.Amount` with `Stack.Inventory = true` (item_db). Absent `Stack` node = only the global 30000 cap. See [db/item_db.md](../db/item_db.md).

## 3. `pc_additem`

Source: `pc.cpp:5991-6100`.

Signature: `(sd, item*, amount, log_type, favorite=false)` -> `e_additem_result` (`pc.hpp:113-122`):

| Value | Name | Meaning (source comment `pc.cpp:5976-5989`) |
| :--- | :--- | :--- |
| 0 | `ADDITEM_SUCCESS` | ok |
| 1 | `ADDITEM_INVALID` | `nameid == 0` or `amount <= 0` |
| 2 | `ADDITEM_OVERWEIGHT` | would exceed max weight |
| 3 | `ADDITEM_ITEM` | unused by `pc_additem` |
| 4 | `ADDITEM_OVERITEM` | no free slot (within `inventory_slots`) |
| 5 | `ADDITEM_OVERAMOUNT` | stack would exceed 30000 / `Stack.Amount` |
| 6 | `ADDITEM_REFUSED_TIME` | not returned here; used by `clif_parse_TakeItem` as a generic "pickup failed" code |
| 7 | `ADDITEM_STACKLIMIT` | `amount > Stack.Amount` for a single add |

Check order (first failure wins, nothing is modified on failure):
1. `nameid == 0 || amount <= 0` -> 1.
2. `amount > MAX_AMOUNT` -> 5.
3. `Stack.Inventory && amount > Stack.Amount` -> 7.
4. `w = item.Weight * amount`; `sd->weight + w > sd->max_weight` -> 2. (Limit is 100% of max weight; the 50/70/90% thresholds are status effects only, section 8.)
5. If item DB `UniqueId` flag and `item.unique_id == 0`: generate one (section 5).
6. **Merge path** (only if stackable AND `item.expire_time == 0`): scan all `MAX_INVENTORY` slots for one with equal `nameid`, `bound`, `unique_id`, `expire_time == 0` and all four `card[]` equal. Identify/refine/attribute/options are NOT part of the key. On a match: `amount > 30000 - stack.amount` (or beyond `Stack.Amount`) -> 5; matching slot index `>= inventory_slots` -> 5; else `stack.amount += amount`, send `clif_additem(slot, amount, 0)`.
7. **New-slot path** (no merge, non-stackable, or rental): first empty slot (`pc_search_inventory(sd, 0)`, lowest index with `nameid == 0`); none -> 4; slot index `>= inventory_slots` -> 4. Copy the whole `struct item`, then zero `equip` and `equipSwitch`, set `amount`, set `favorite` from parameter, cache `inventory_data[i]`, remember `last_addeditem_index`. Non-stackable or UniqueId items get `unique_id = item.unique_id ? item.unique_id : generate`. Send `clif_additem(slot, amount, 0)`.
8. Log, then `sd->weight += w` and `clif_updatestatus(SP_WEIGHT)` (this send also re-evaluates the overweight status, section 8). Note the add packet is sent BEFORE the weight packet.
9. If the item DB has the autoequip flag (script-set, `script.cpp:17003`), call `pc_equipitem`. If `expire_time` set: already expired -> send rental-expired + delete; else send rental time and arm the rental timer.
10. Achievement / questinfo hooks (out of scope).

* **`pc_checkadditem`** (`pc.cpp:5729-5762`): dry-run capacity check used by shops/scripts. Returns `CHKADDITEM_EXIST(0)` (merges into an existing stack), `CHKADDITEM_NEW(1)` (needs a new slot: non-stackable, UniqueId, or no match), `CHKADDITEM_OVERAMOUNT(2)`. Matches the FIRST slot with the same `nameid` only (known FIXME: ignores cards).
* **`pc_inventoryblank`** (`:5772-5783`): count of empty slots among the first `inventory_slots`.
* **`pc_search_inventory(nameid)`** (`:5968-5973`): lowest index with `nameid` matching and `amount > 0` (or `nameid == 0`); -1 if none.

## 4. `pc_delitem`

Source: `pc.cpp:6103-6132`.

`(sd, n, amount, type, reason, log_type)` returns 0 ok / 1 invalid. Fails (1, no change) if `n < 0`, slot empty, `amount <= 0`, `amount > stack.amount`, or `inventory_data[n]` is null.

Effects: `stack.amount -= amount`; `sd->weight -= Weight * amount`; if the stack reaches 0: if `equip != 0` call `pc_unequipitem(n, 2 | (type&4 ? 0 : 1))`, then zero the whole slot and clear `inventory_data[n]`. The slot becomes a hole (later adds reuse the lowest empty index).

`type` bit flags: `&1` do NOT send the delete packet, `&2` do NOT send the weight update, `&4` do not recalc status. `reason` goes to `ZC_DELETE_ITEM_FROM_BODY.deleteType` (0 normal, 1 skill, 2 refine fail, 3 material changed, 4 moved to storage, 5 moved to cart, 6 sold, 7 four-spirit analysis; `clif.cpp:2906-2916`). Without `&1` the client gets `0x07fa` (index, amount, reason); without `&2` it gets `SP_WEIGHT`.

**Worn stack** (verified `pc.cpp:6103-6132`, `d4b8e7b8f`): only when the stack reaches 0. The order inside `pc_delitem` is: amount and `sd->weight` reduced -> `pc_unequipitem(n, 2|1)` (flag `1` unless `type&4`; flag `2` = forced, ignores `sc.cant.unequip`) -> slot memset -> `0x07fa` (unless `&1`) -> `SP_WEIGHT` (unless `&2`). So the unequip packets (`0x01d7` looks, `0x099a`, ammo `0x099a` for a weapon, status diff pushes from one `status_calc_pc`) precede `0x07fa`, which precedes `SP_WEIGHT`. A partial delete of a worn stack leaves it worn and sends no equip packets. Detail: [packets/equipment.md](../packets/equipment.md#take-off--drop-order-pc_unequipitem).

## 5. `unique_id`

`pc_generate_unique_id` (`pc.cpp:15334-15337`): `(char_id << 32) | uniqueitem_counter++`, where `uniqueitem_counter` is a persisted `u32` column on the `char` table (`main.sql:277`, saved with the char row, `char.cpp:297, 313`). Assigned to every non-stackable item and every `UniqueId`-flag item on add if it has none; also backfilled on login for non-stackable items with `unique_id == 0`, but only when the `item_check` battle option has bit `ITMCHK_INVENTORY` set (`pc_check_available_item`, `pc.cpp:12672-12695`; default `item_check = 0`, `battle.cpp:8378`, so the backfill and the disabled-item purge are OFF by default).

## 6. Drop (`CZ_ITEM_THROW` -> `pc_dropitem`)

Source: `pc.cpp:6137-6177`, `clif.cpp:12042-12072`.

Parser (`clif_parse_DropItem`): `index = wire_index - 2`, `amount` = u16. Aborts to the failure path if: player dead, `pc_cant_act2` (e.g. in a state that blocks actions) or an NPC script is attached (`sd->npc_id`), or `sc.cant.drop`.
`pc_dropitem` returns false if: `n` out of `[0, MAX_INVENTORY)`; `amount <= 0`; slot empty / `amount < requested` / `inventory_data` null; `state.trading` or `state.vending`; `equipSwitch != 0`; mapflag `nodrop` (message 271); `!pc_candrop` (message 263; false for `sc.cant.drop`, hatched pet egg, and item DB `Trade.NoDrop` unless the GM group overrides; `pc.cpp:11356`).
On success: `map_addflooritem(item, amount, map, x, y, ...)` at the player's own cell (floor item; same `item` struct incl. cards/options), then `pc_delitem(n, amount, type=1)` (no `0x07fa`) and `clif_dropitem` -> `ZC_ITEM_THROW_ACK 0x00af {index+2, amount}`. The floor-item appearance packet is separate (`dropflooritem`, not covered here).
**Worn items** (`pc_dropitem` has no equipped check; `pc.cpp:6137-6177`): the floor copy is made first and keeps the `equip` bits (cleared again by `pc_additem` on pickup, `pc.cpp:6049-6052`). Full-stack drop of a worn item: unequip packets (see §4), then `SP_WEIGHT` (the drop passes `type=1`, so no `0x07fa`), then `0x00af {idx+2, amount}` last. Partial drop of a worn stack (e.g. 5 of 100 arrows): stays worn, packets are `SP_WEIGHT` then `0x00af`. Sitting does not block drop (`clif_parse_DropItem` gates: dead, `pc_cant_act2`, `npc_id`, `cant.drop`).
**Failure path:** the parser still sends `clif_dropitem(index, 0)` i.e. `0x00af` with `count = 0` ("the client does not like being ignored"). Verified `clif.cpp:12071`.

## 7. Use (`CZ_USE_ITEM` -> `pc_useitem`)

Source: `pc.cpp:6450-6575`, `clif.cpp:12080-12101`.

Parser: if dead -> send `CLR_DEAD` clear; if (not in NPC context and trading) or in a chat room or `PCBLOCK_USEITEM` -> `clif_msg(MSI_BUSY)`, stop. Else `n = wire_index - 2`; range check; `pc_useitem(n)`; on false return the parser sends an empty ack `clif_useitemack(n, 0, false)` "or the client gets stuck".

`pc_isUseitem` (`pc.cpp:6277+`) gate, in order (all return false unless noted): item exists; `Type` must be `Healing`, `Usable` or `Cash` (DelayConsume is remapped to `Usable` at load, `itemdb.cpp:1122`); GM `ITEM_UNCONDITIONAL` permission bypasses the rest; mapflag `noitemconsumption`; use-delay timers (`canuseitem_tick` = now + `item_use_interval` 100 ms, `conf/battle/items.conf:68`; cash-food interval); `NoUse.Sitting` while sitting (`MSI_CANT_USE_WHEN_SITDOWN`, bypass if GM level >= `NoUse.Override`); storage open (non-Cash); dead-branch/nobranch; teleport/return item groups on notp/noreturn maps; party-call item needs party leader + other member; mercenary items; `Neuralizer` needs `reset` mapflag; for `Cash` or group-container items: fails if weight percent `>= open_box_weight_rate` (renewal 90, `battle.cpp:8392`) or free slots `<= 10` (message MSI_PICKUP_FAILED_ITEMCREATE); gender mismatch; `EquipLevelMin`/`EquipLevelMax` vs base level; job/class (`pc_job_can_use_item`, `pc_isItemClass`); `sc.cant.consume`.
`pc_useitem` additionally: refuses if `mail_writing`; if NPC attached and not allowed `MSI_BUSY`; if `sc.opt1 > 0` (stone/freeze/stun/sleep etc., except `OPT1_STONEWAIT`/`OPT1_BURNING`) unless the item is Nauthiz; delayed-consume items refuse while sitting / mounted (except Reins) / casting; `Delay.Duration` item cooldown (`pc_itemcd_check`); restricted-map `noitemconsumption`-style check (`itemdb_isNoEquip`) -> `MSI_IMPOSSIBLE_USEITEM_AREA`; pet-capture item on `nopetcapture` map.

Consumption ordering when it passes:
* Normal (non-delay) items: `clif_useitemack(n, amount-1, ok=true)` (`0x01c8`, sent to `AREA`, i.e. observers too), then `pc_delitem(n, 1, type=1, reason 0)` (so NO `0x07fa`; weight packet is sent). Rental items (`expire_time != 0`) and the Reins item are NOT deleted: ack is `(n, 0, false)`.
* `DelayConsume` items: ack `(n, amount, true)` and the item is consumed later by the skill/target-selection flow.
* Then: `canuseitem_tick = now + item_use_interval`, run the item's `Script` with the player attached (restoring any previously attached script state afterwards).
* `clif_useitemack` with `ok == false` is sent to `SELF` only; `ok == true` to `AREA`.

### 7.1 Wire layouts for potion use (20260219)

* **Provenance:** `rathena` @ `d4b8e7b8f`, source, confirmed by capture in section 7.5. All `20260219`.
* **`CZ_USE_ITEM` `0x0439`, 8 bytes** (`clif_packetdb.hpp:1151` `parseable_packet(0x0439,8,clif_parse_UseItem,2,4)`; not overridden in `clif_shuffle.hpp`, which only lists it for older versions): `index` u16 @2 (= inventory slot + 2), `AID` u32 @4 (read by the client, ignored by the server; the parser reads only `pos[0]`, `clif.cpp:12112`). Layout comment `clif.cpp:12080`.
* **`ZC_USE_ITEM_ACK` `0x01c8`, 15 bytes** (`packets_struct.hpp:2577-2589`, `useItemAckType = 0x1c8` for `PACKETVER >= 3`, `:308-316`). Because `PACKETVER_MAIN_NUM >= 20181121`, `itemId` is u32: `0x01c8 | index u16 @2 (slot+2) | itemId u32 @4 | AID u32 @8 | amount u16 @12 | result u8 @14`. `amount` is the stack count REMAINING after use (`amount - 1`); `result` 1 = used, 0 = refused. Builder `clif_useitemack`, `clif.cpp:4468-4495`.
* **Scope:** `result == 1` goes to `AREA` (observers receive it too); `result == 0` goes to `SELF` only (`clif.cpp:4488-4492`). The builder returns silently (sends NOTHING) when the slot's `nameid == 0` or `inventory_data[index]` is null, so a failure ack for an already-emptied slot is never sent.
* **`ZC_RECOVERY`** (`clif_heal`, `clif.cpp:6840`): for `PACKETVER >= 20141022` it is `0x0a27`, 8 bytes: `type` u16 @2 (5 = HP, 7 = SP), `amount` i32 @4 (`packets.hpp:1210-1222`; older form `0x013d` has i16 amount). SELF only. Potions and `percentheal` do NOT use it (see 7.3).
* **HP/SP push for potions:** `ZC_PAR_CHANGE` `0x00b0` (`clif.cpp:3550`, 8 bytes: `varID` u16 = 5 for `SP_HP` / 7 for `SP_SP`, `count` i32 = the NEW ABSOLUTE value, not the delta), SELF. `SP_HP` of a dead (hp 0) player is sent as 1 (max_hp/2 for Novice), `clif.cpp:3683-3690`.

### 7.2 Gate order and what each refusal sends (potion path)

Source: `clif.cpp:12080-12106`, `pc.cpp:6276-6575`. "Failure ack" = `clif_useitemack(n, 0, false)`, i.e. `0x01c8` SELF with `amount 0, result 0`.

| # | Check (in order) | On refusal |
| :-- | :-- | :-- |
| 1 | Parser: player dead | `CLR_DEAD` clear packet to AREA; NO ack (`:12084-12087`) |
| 2 | Parser: (trading and no NPC) or chat room or `PCBLOCK_USEITEM` | `clif_msg(MSI_BUSY)`; NO ack |
| 3 | Parser: `n = index - 2` outside `[0, MAX_INVENTORY)` | silent return, no ack (`:12102`) |
| 4 | `pc_useitem`: `mail_writing` | return 0 -> failure ack |
| 5 | NPC script attached and (progress bar or no `npc_item_flag`) | return 0 -> failure ack (renewal adds `MSI_BUSY`) |
| 6 | slot empty / `amount <= 0` | failure ack builder returns silently (nameid 0) |
| 7 | `pc_isUseitem`: no `inventory_data`; `Type` not Healing/Usable/Cash | failure ack |
| 8 | GM `ITEM_UNCONDITIONAL` permission | skips the rest of `pc_isUseitem` AND the `Delay` check |
| 9 | mapflag `noitemconsumption` | failure ack |
| 10 | `canuseitem_tick - now > 0` (i.e. used less than `item_use_interval` ago; default **100 ms**, `battle.cpp:8379`, `conf/battle/items.conf:68`; the tick is set AFTER the ack at `pc.cpp:6545`, so it is per-player, shared by all items) | failure ack, no message |
| 11 | per-item `NoUse.Sitting` flag AND sitting AND GM level below override | `MSI_CANT_USE_WHEN_SITDOWN` + failure ack. Red/Blue Potion do not set it, so **sitting does not block potions by default**; only the flagged items |
| 12 | storage open (non-Cash) | red message 388 + failure ack |
| 13 | teleport/return/party-call item groups, Neuralizer, mercenary items | not relevant to potions |
| 14 | Cash or group-container items: weight% `>= open_box_weight_rate` or free slots `<= 10` | message + `return 0` -> failure ack. **Not applied to normal potions** (type Healing) |
| 15 | gender, `EquipLevelMin` > base level, `EquipLevelMax` < base level, `pc_job_can_use_item` (job), `sc.cant.consume`, `pc_isItemClass` | failure ack, no message |
| 16 | `pc_useitem`: `sc.opt1` set (stone/freeze/stun/sleep, except STONEWAIT/BURNING) unless Nauthiz | failure ack |
| 17 | `Delay.Duration` item cooldown (`pc_itemcd_check`, skipped for GM bypass) | failure ack (the check sends its own cooldown message) |
| 18 | restricted-area no-equip lists (`itemdb_isNoEquip`) | `MSI_IMPOSSIBLE_USEITEM_AREA` + return 0 -> failure ack |

Not gates for potions: weight (only Cash/groups), job-level, and `Delay:` unless the item sets it. `pc_useitem` never checks that the player is walking or attacking: using a potion neither stops movement nor the auto-attack loop in the source.

### 7.3 Exact order for a successful potion (`itemheal`)

1. `clif_useitemack(n, amount-1, true)` -> `0x01c8` AREA (`pc.cpp:6535`). The item id and index are read before deletion.
2. `pc_delitem(n, 1, type=1, reason 0)` (`:6536`): `type&1` suppresses `0x07fa`, `type&2` is not set, so the only packet is `SP_WEIGHT` `0x00b0 {24, weight}` SELF. The ack's `amount` is what updates the client stack count; the client removes the slot itself at 0. (Order inside `pc_delitem`: see section 4.)
3. Fame-potion flag: `potion_flag = 2` when `card[0] == CARD0_CREATE` and the creator ranks in the Alchemist fame list (`:6541`); normal potions leave it 0.
4. `canuseitem_tick = now + item_use_interval` (`:6545`).
5. `run_script` (`:6558`): `itemheal` -> `pc_itemheal` (7.4) -> `status_heal(sd, hp, sp, 1)`.
6. `status_heal` (`status.cpp:1803-1888`): returns 0 for a dead target; clamps hp/sp to `max - current`; **if the clamped hp, sp and ap are all 0 (and flag&4 unset) it returns 0 and sends NOTHING**. Otherwise adds to the status and calls `pc_heal` (`pc.cpp:10663`). With flag `1` (bit 2 clear) `pc_heal` takes the `else` branch: `clif_updatestatus(SP_HP)` (`0x00b0` var 5) if hp != 0, then `SP_SP` (var 7) if sp != 0 (then AP). `clif_updatestatus` also runs `clif_update_hp` (`clif.cpp:3909`, `3539`): hpmeter packet only on `hpmeter_visible` maps, party HP packet if in a party (unless `party_hp_mode`), BG HP; nothing is sent to ordinary observers.
7. `clif_heal` (`0x0a27`) is only used when flag bit 2 is set (`status_heal(..., 2|...)`, e.g. `status_set_hp` with flag 2, skill heals with `show_hp_sp_gain`, `skill.cpp:1784`). **Item and `percentheal` heals use flag 1 / 0, so no `0x0a27`**: the client sees the new HP via `0x00b0` only.
8. Result at full HP: the potion is already consumed (steps 1-2 precede the script), the heal clamps to 0, and no HP/SP packet is sent. Only `0x01c8` (AREA) and `SP_WEIGHT` go out.

Total for a normal Red Potion at reduced HP: `0x01c8` (AREA) -> `0x00b0` weight -> `0x00b0` HP. Blue Potion: `0x01c8` -> weight -> `0x00b0` SP. A potion with both values sends HP then SP.

### 7.4 `itemheal` and `percentheal` formulas (renewal)

Source: `script.cpp:6038-6095`, `pc.cpp:10695-10776` (itemheal), `pc.cpp:10792-10820` (percentheal), `status.cpp:1905-1973`, `utils.hpp:23`.

* **`itemheal hp,sp{,char_id}`**: with `potion_flag == 1` (potion-pitcher mode) only stores the values; otherwise `pc_itemheal(sd, sd->itemid, hp, sp)`. The args are evaluated once per use, so `rand(a,b)` is rolled per drink.
* **HP bonus (percent)** `b = 100 + 2*VIT + 10*SM_RECOVERY lv + 5*AM_LEARNINGPOTION lv`, with `VIT = battle_status.vit` (the total, including gear/buffs). Then in order: fame flag (`potion_flag == 2`: `b += b*50/100`; Rogue Soul Link adds `+b`, HP only); `b += bonus.itemhealrate2`; `b += b * itemgroup_rate/100`; `b += b * per_item_rate/100`; `b += b * SC_INCHEALRATE.val1/100`. Applied as `tmp = hp*b/100; if (b != 100 && tmp > hp) hp = tmp`.
* **SP bonus** identical with `INT` (`battle_status.int_`), `MG_SRECOVERY`, `AM_LEARNINGPOTION`, `itemsphealrate2`, group/per-item SP rates; no Rogue bonus, no SC_INCHEALRATE.
* **Then status modifiers** (only if `sc` non-empty): penalty = `SC_CRITICALWOUND.val2` + 20 if Death Hurt (`val3 == 1`); `SC_NORECOVER_STATE` sets penalty 100; `SC_VITALITYACTIVATION` hp += hp/2; Water Insignia lv 2 hp/sp += 10 %; renewal `SC_APPLEIDUN` hp += val3/100; `penalty > 0`: `hp -= hp*penalty/100`, same for sp; renewal `SC_EXTREMITYFIST` sp = 0; `SC_BITESCAR` hp = 0.
* **Result** `status_heal(sd, hp, sp, 1)` (7.3). Worked example: Red Potion roll 50, VIT 1, no skills: `b = 102`, `50*102/100 = 51`. With VIT 0 and no skills `b = 100`, the multiply is skipped.
* **`percentheal hp,sp{,char_id}`**: no VIT/INT/skill scaling. Script layer: renewal `SC_EXTREMITYFIST` -> sp 0; `SC_NORECOVER_STATE` -> hp and sp 0; `SC_BITESCAR` -> hp 0. `pc_percentheal` clamps both rates to `[-100, 100]`; both >= 0 -> `status_percent_heal(rate)` = `status_percent_change(.., -hp_rate, -sp_rate, 0, flag 0)`: heal = `apply_rate(max_hp, rate)` = `max_hp*rate/100` (`max_hp/100*rate` when max_hp > 100000), rate >= 100 = full max; a non-zero rate yields at least 1; then `status_heal(target, hp, sp, ap, 0)` (flag 0, so `0x00b0` pushes, no `0x0a27`). Negative rates damage (percent of MAX), and a crossed-sign pair is split into one heal and one damage call.
* **`potion_flag`**: global in `script.cpp:365`. 0 normally; 2 for fame potions set in `pc_useitem`; 1 only from skill-launched "potion pitcher" (`script.cpp:12441`, `skill.cpp`), where `itemheal`/`percentheal` record `potion_hp/sp/per_hp/per_sp` instead of healing. Reset to 0 at the end of `pc_useitem` (`pc.cpp:6574`).

### 7.5 Capture: scripting_1.txt (potion use)

Source: [captures/scripting_1.txt](../captures/scripting_1.txt) lines 7732-8230 (proxy #7722 to the end; file line = proxy `#` + 10), rAthena `d4b8e7b8f`, `20260219`. A = `skilltest2` (map session #7, AID 2000000 = `80 84 1e 00`, a `group_id` GM), B = `skilltest1` (#4, observer, also GM). A's base stats: VIT 1, INT 1 (then VIT 51 after `@vit 50`). Everything below is captured fact unless marked "source".

* **`0x0439` request (client to map):** payload `index u16 | AID u32`; e.g. `07 00 80 84 1e 00`. `index` 7 = slot 5 + 2 (Red Potion, nameid 501, `0x01f5`); 8 = Blue Potion (505, `0x01f9`). The `AID` is A's own AID. Confirms 7.1.
* **`0x01c8` ack (15 bytes)** `07 00 | f5 01 00 00 | 80 84 1e 00 | 04 00 | 01` = index, nameid u32, AID u32 (the user, not the item), `amount` = stack REMAINING after the use (`amount-1`, confirmed: 5 potions gave 4, 3, 2; 52 gave 51 down to 33), `result` 1. **B (observer) receives the identical `0x01c8`** (AREA scope confirmed). Send order between A's copy and B's copy varies per use (A first in some, B first in others), so do not rely on it.
* **Order for a heal that changes HP (A's connection):** `0x01c8` -> `0x00b0` var 24 `SP_WEIGHT` -> `0x00b0` var 5 `SP_HP` (new absolute value) -> `0x080e {AID, hp, maxhp}`. Blue Potion: `0x01c8` -> `SP_WEIGHT` -> `0x00b0` var 7 `SP_SP`; **no `0x080e`** and nothing for B except the `0x01c8`. Weight dropped 70 per Red Potion (`18 00 18 06 00 00` = 1560, then 1490, 1420) and 150 per Blue Potion in this DB (1420 to 1270).
* **`0x080e` correction to 7.3 step 6:** B received `0x080e {AID, hp, maxhp}` (14 bytes) for every HP change of A, and A received its own. No party exists in the capture; the cause is `clif_update_hp` -> `clif_hpmeter` (`clif.cpp:3539`) which sends it to every nearby player whose group has `PC_PERM_VIEW_HPMETER` (`clif.cpp:11085`, set at map entry; both test accounts are GMs) on maps with a viewer (`mapdata->hpmeter_visible`). A normal player gets no `0x080e` about others and, with no party, not about itself either. For Metaling: gate `0x080e` on that permission (or party), not on "everyone nearby".
* **No `0x0a27` and no `0x07fa` for potions** (source claim 7.3 confirmed: none appeared in any of the ~25 uses). `0x0a27` does exist in this capture, from skill steps (13 times, see stats.md); it is not part of the item path.
* **Red Potion at reduced HP, VIT 1:** HP 87 to 137 (= MaxHP 137, clamped), so only a lower bound: `roll * 102 / 100 >= 50`. Not informative.
* **After `@vit 50` (VIT 51, MaxHP 205), HP 92 to 186 = +94:** with `b = 100 + 2*51 = 202` the roll must satisfy `roll*202/100 = 94`, i.e. roll 47 (inside `rand(45,65)`). Without the VIT bonus 94 would be impossible. **The 7.4 HP formula (`2*VIT` percent, integer multiply then divide) is confirmed.** Next potion 186 to 205 (+19, clamped).
* **Blue Potion (`itemheal 0, rand(40,60)`), INT 1:** SP 2 to 60 = +58: `b = 102`, roll 57 gives `57*102/100 = 58`, roll 56 would give 57. Consistent with `2*INT` percent. Second potion 60 to 75 (MaxSP 75, clamped). A third at full SP sent only `0x01c8` + weight.
* **At full HP (or SP):** the potion is still consumed. Each use sent `0x01c8` (to A and B) and `0x00b0` `SP_WEIGHT` only, no HP/SP/`0x080e` (about 20 uses in the spam burst). Confirms 7.3 step 8.
* **Use rate / spam:** the burst `15:47:07.096` to `:09.884` has 19 `0x0439` requests, gaps 119 to 192 ms (client-enforced pacing; typical 130 to 180 ms), stack 51 down to 33 with **every request acknowledged with `result` 1; no `result` 0 ack and no dropped request.** The server limit `item_use_interval` is 100 ms (`conf/battle/items.conf:68`), so this capture never tested a use under 100 ms. The refusal behaviour for faster uses is source-only (7.2 row 4: `canuseitem_tick` gate returns a failure ack).
* **`@item 501 5`:** `0x0b41` (70 bytes) `07 00 | 05 00 | f5 01 00 00 | 01 ...`: index 7, `count` 5 (the amount ADDED), nameid, `is_identified` 1, rest zero; then `0x00b0` var 24 weight (880 = 530 before + 5 * 70), then the confirmation message (`0x008e`). Order: ack, weight, message. `@item 505 5` same, with six `0x0a24` achievement updates between weight and message (first Blue Potion in the inventory; achievements are out of scope).
* **`@item 501 50`** (stack was 2): `0x0b41` index 7, `count` 0x32 = 50 (added, not the new total), weight 4470, message. No failure.
* **`@item 501 500` failure:** a single `0x0b41` and **no weight push and no message**: payload `02 00 | 00 00 ...` with `index` = 2, `count` 0, all zero except the byte at frame offset 33 = `02` (`ADDITEM_OVERWEIGHT`; 500 * 70 = 35000 exceeds the room left). This is the first captured failure form and matches the layout in [packets/inventory.md](../packets/inventory.md#0x0b41-zc_item_pickup_ack-add-item--pickup-result) (index 2, count 0, `result` at offset 33).
* **Natural regen visible in the burst:** HP +1 every 6.0 s, SP +1 every 8.0 s while standing, each as `0x00b0`; after `@vit 50` HP +11 per tick (`51/5 + max(1, 205/200) = 11`). See [stats.md](./stats.md#capture-scripting_1txt-regen-and-0x0a27).

## 8. Weight

> Open findings: [PF-19](../../../PACKET_FINDINGS.md#pf-19).

Units: all weights are integers in 0.1 weight units (`Weight: 70` = 7.0; `item_db.yml` header: "Each 10 is 1 weight"). The same units go on the wire (`SP_WEIGHT`=24, `SP_MAXWEIGHT`=25, `0x00b0`; see [map_units.md#0x00b0-zc_par_change](../packets/map_units.md#0x00b0-zc_par_change)).

* **Total weight** `sd->weight` = sum(`Weight` x `amount`) over inventory. Maintained incrementally by add/delete; fully recomputed by `status_calc_weight(CALCWT_ITEM)` (`status.cpp:3653-3702`), which runs after inventory load and sends `SP_WEIGHT` if changed.
* **Max weight** (`status.cpp:3663-3690`): `job_db.maxWeight(job) + STR * 300` + (with `CALCWT_MAXBONUS`): `bAddMaxWeight` item/script bonus; `+2000 * MC_INCCARRY lv` (Enlarge Weight Limit); `+10000` if riding with `KN_RIDING` learned; Dragon riding `+5000 + 2000 * RK_DRAGONTRAINING lv`; `SC_KNOWLEDGE` adds `max_weight * val1 / 10`; `+2000 * ALL_INCCARRY lv`; Mado Gear `+15000`. Base per job: `MaxWeight` in `db/re/job_stats.yml` (default 20000; ~line 107 sample 28000). Sends `SP_MAXWEIGHT` and re-runs the overweight evaluation when it changes.
* **Percent:** `weight * 100 / max(max_weight, 1)` integer (`pc_getpercentweight`, `pc.cpp:3012`).
* **Thresholds** (`pc_updateweightstatus`, `pc.cpp:3026-3051`; invoked from `clif_updatestatus(SP_WEIGHT)` and from the max-weight change): percent `>= major_overweight_rate` (90, `battle.cpp:8906`) -> `SC_WEIGHT90`; else `>= natural_heal_weight_rate` (**renewal 70**, pre-renewal 50; `battle.cpp:8390-8394`, commented out in `conf/battle/player.conf:69`) -> `SC_WEIGHT50`; else none. Only changes trigger status start/end. Both statuses are permanent (no timer, `status.cpp:10818`) and show the 50%/90% client icons. `regen.state.overweight = (new != 0)`.
* **Effects found in the server source:**
  * Overweight at either level: natural HP/SP regen is disabled (`status.cpp:15521`: `if (flag && regen->state.overweight) flag = RGN_NONE`), sitting-regen bonus runs at half rate (`:15495-15509`). (Skill-based/other regen via `pc_regen` is separate.)
  * `SC_WEIGHT90` (>= 90%): skill use fails with `USESKILL_FAIL_WEIGHTOVER` (9) in `skill_check_condition_castbegin` and `..._castend` (`skill.cpp:8312, 9374`).
  * `Cash`/container item use blocked at `>= open_box_weight_rate` (90 renewal), section 7.
  * Add fails at `weight + w > max_weight` (100%), section 3.
  * **No server-side basic-attack block at 90% was found** (grep of `battle.cpp`/`unit.cpp`/`skill.cpp` shows only the two skill checks); the "cannot attack / cannot pick up" behaviour at 90% is therefore client-enforced or unimplemented server-side. Do not invent it without a capture.
  * Pickup at the add check uses 100% only, not 90%.

## 9. Pickup (`CZ_ITEM_PICKUP` -> `pc_takeitem`) summary

`clif_parse_TakeItem` (`clif.cpp:12002-12036`): resolve object id to a floor item on the same map; checks dead, `pc_cant_act`, then `pc_takeitem` (`pc.cpp:6181`): distance `<= 2` (unless Greed), `sc.cant.pickup`, loot-priority timers (first/second/third attacker, party share), then `party_share_loot` -> `pc_additem`. If it returns nonzero `flag`: `clif_additem(sd, 0, 0, flag)` (a failure `0x0b41` carrying the `e_additem_result` as `result`), and the floor item stays. On success: pickup animation `ZC_NOTIFY_ACT` type `DMG_PICKUP_ITEM` to `AREA` (`clif_takeitem`, `clif.cpp:5315`), floor item removed; the item arrives via the `clif_additem` inside `pc_additem`. If ANY earlier gate fails the parser sends `clif_additem(sd, 0, 0, 6)` ("Client REQUIRES a fail packet or you can no longer pick items"). Packet layouts: [packets/inventory.md](../packets/inventory.md).

## 10. When the inventory list is sent

* **Login / every map entry:** inside `clif_parse_LoadEndAck` (`clif.cpp:10748+`, inventory at `:10782`), right after look/costume/clothcolor and BEFORE `pc_checkitem` ("otherwise deleted items in pc_checkitem show up as unknown"), then equip-switch list, cart list, `SP_WEIGHT`, `SP_MAXWEIGHT`. Full order and the rest of the entry sequence: [map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack](./map_entry_actions.md#map-entry-cz_notify_actorinit-clif_parse_loadendack). That is the same for the first login and for map changes (inventory list is resent on every `LoadEndAck`, not only the first).
* **Also resent** by `clif_refresh` (`clif.cpp:9840-9851`, e.g. `@refresh`): changemap, inventory, equip-switch, cart, weight.
* **Before that,** at auth (`pc_authok`, `pc.cpp:2238`): `clif_inventory_expansion_info` (`0x0b18`) is sent immediately before `clif_authok` (`0x02eb`).
* **Server-side load:** after the char-server answers the storage request (`intif_parse_StorageReceived`, `intif.cpp:3421-3520`), the map-server: fills `inventory_data`, `pc_setequipindex`, expiry check, `pc_check_available_item` (no-op unless `item_check` bit 1 is set: removes disabled items, backfills `unique_id`), item-cooldown restore, bound-party cleanup, `status_calc_pc(SCO_FIRST)` and `status_calc_weight(CALCWT_ITEM|CALCWT_MAXBONUS)`. The wire index of an item is its position in the loaded array.

## 11. Persistence

* **Load:** `char_memitemdata_from_sql` (`char.cpp:751-850`): `SELECT ... FROM inventory WHERE char_id=? ORDER BY id`; rows are written into the in-memory array at positions `0..n-1` consecutively (holes are NOT preserved across a save/load cycle; wire indexes compact on relog). Max rows read = `MAX_INVENTORY` (200).
* **Save:** `char_memitemdata_to_sql` (`char.cpp:533-740`): diff against the table, matching rows by `(nameid, card0..3, unique_id)` then comparing the rest; UPDATE all fields on any difference, DELETE rows not present, INSERT unmatched items as new rows (new `id` auto-increment). Persisted fields: `nameid, amount, equip, identify, refine, attribute, card0-3, option_id/val/parm 0-4, expire_time, favorite, bound, unique_id, equip_switch, enchantgrade`. NOT persisted: runtime `id` (regenerated), `inventory_data`, rental timers.
* **When saved:** the map-server sends the whole inventory (`intif_storage_save`, packet `0x308b`, `intif.cpp:3652`) whenever `chrif_save` is called with `CSAVE_INVENTORY` (`chrif.cpp:299`): periodic autosave (`pc_autosave`, `pc.cpp:13002`, passes `CSAVE_INVENTORY|CSAVE_CART`, `pc.cpp:13030`; `autosave_time` 300 s spread across online players, min gap `minsave_time` 100 ms, `conf/map_athena.conf:91,96`), quit/map-server change (`CSAVE_QUIT|CSAVE_INVENTORY|CSAVE_CART`, `chrif.cpp:520, 752`; `pc.cpp:7072`), inventory expansion (`clif.cpp:23183`), and event triggers from `save_settings` (default 4095 = trade, vending, storage close, pet hatch, mail, auction, quest, bank, attendance; `map_athena.conf:114`, `map.hpp:1122-1132`). Zeny is NOT in the inventory table; it is the `char.zeny` column saved with the char row.
* **New characters:** `start_items` (`conf/char_athena.conf:124-126`, format `nameid,amount,equip_pos:...`) inserted directly into `inventory` with `identify=1` (`char.cpp:1519`).
* **Delete char:** `DELETE FROM inventory WHERE char_id` (`char.cpp:1678`).

## 12. Design hints for Metaling (not source facts)

* Keep inventory as a dense `Vec<Option<Item>>`/slot array of `inventory_slots` entries; derive wire index as `slot + 2`; assign DB `id` only at save.
* Implement add as: validate -> weight -> merge key `(nameid, bound, unique_id, card[4])` for stackables with `expire_time == 0` -> first empty slot; treat `ADDITEM_*` codes as the exact wire `result` values (section 3 / packets file).
* Weight pipeline: integer 0.1 units; derive percent once per change; start/stop `WEIGHT50`/`WEIGHT90` statuses only on transitions.
