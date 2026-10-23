# Player Inventory Mechanics

Target client version: `PACKETVER = 20260219` (main-server numbering: `PACKETVER_MAIN_NUM`; `PACKETVER_RE` is NOT defined because 20260219 is outside both `PACKETVER_RE` ranges, `src/config/packets.hpp:20-30`). Renewal build.

* **Provenance (all sections):** `rathena` @ `d4b8e7b8f` (2026-10-06), paths relative to `$RATHENA_PATH`. Line numbers are for that commit. No proxy captures were available in the workspace when this was written (`tools/proxy/packet_captures/` is absent); every behaviour below is source-derived, so verify against a capture before relying on a detail the client observes.
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

## 5. `unique_id`

`pc_generate_unique_id` (`pc.cpp:15334-15337`): `(char_id << 32) | uniqueitem_counter++`, where `uniqueitem_counter` is a persisted `u32` column on the `char` table (`main.sql:277`, saved with the char row, `char.cpp:297, 313`). Assigned to every non-stackable item and every `UniqueId`-flag item on add if it has none; also backfilled on login for non-stackable items with `unique_id == 0`, but only when the `item_check` battle option has bit `ITMCHK_INVENTORY` set (`pc_check_available_item`, `pc.cpp:12672-12695`; default `item_check = 0`, `battle.cpp:8378`, so the backfill and the disabled-item purge are OFF by default).

## 6. Drop (`CZ_ITEM_THROW` -> `pc_dropitem`)

Source: `pc.cpp:6137-6177`, `clif.cpp:12042-12072`.

Parser (`clif_parse_DropItem`): `index = wire_index - 2`, `amount` = u16. Aborts to the failure path if: player dead, `pc_cant_act2` (e.g. in a state that blocks actions) or an NPC script is attached (`sd->npc_id`), or `sc.cant.drop`.
`pc_dropitem` returns false if: `n` out of `[0, MAX_INVENTORY)`; `amount <= 0`; slot empty / `amount < requested` / `inventory_data` null; `state.trading` or `state.vending`; `equipSwitch != 0`; mapflag `nodrop` (message 271); `!pc_candrop` (message 263; false for `sc.cant.drop`, hatched pet egg, and item DB `Trade.NoDrop` unless the GM group overrides; `pc.cpp:11356`).
On success: `map_addflooritem(item, amount, map, x, y, ...)` at the player's own cell (floor item; same `item` struct incl. cards/options), then `pc_delitem(n, amount, type=1)` (no `0x07fa`) and `clif_dropitem` -> `ZC_ITEM_THROW_ACK 0x00af {index+2, amount}`. The floor-item appearance packet is separate (`dropflooritem`, not covered here).
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
