# Item Database (`item_db`) Fields Relevant to Inventory and Equipment

Target client version: `PACKETVER = 20260219`, Renewal (`db/re/`).

* **Provenance (all sections):** `rathena` @ `d4b8e7b8f`: `db/re/item_db.yml` (header comment `:1-81`), loader `src/map/itemdb.cpp` (`ItemDatabase::parseBodyNode` `:49-1100`, `loadingFinished` `:1117-1215`), struct `src/map/itemdb.hpp` (`item_data` ~`:3455-3550`), enums `src/common/mmo.hpp`. Applies to `20260219` (`#ifdef RENEWAL` branches taken). Higher-level overview of the YAML family: [yaml_schemas.md](./yaml_schemas.md). Inventory behaviour that consumes these fields: [mechanics/inventory.md](../mechanics/inventory.md). Wire use: [packets/inventory.md](../packets/inventory.md).
* **Licensing reminder:** the YAML files are GPL-3.0 rAthena data; they may only be copied through an importer into `crates/ro-db/data/` (see `CLAUDE.md`). This file records facts only.

## Files and loading

* `db/re/item_db.yml` contains only the schema comment, `Header: {Type: ITEM_DB, Version: 3}` and a `Footer.Imports` list that pulls in, in order: `db/re/item_db_usable.yml` (Healing, Usable, DelayConsume, Cash; 6415 entries), `db/re/item_db_equip.yml` (Armor, Weapon, ShadowGear, PetArmor, PetEgg; 12982 entries), `db/re/item_db_etc.yml` (Etc, Card, Ammo; 9959 entries). Entry counts are `grep -c "^  - Id:"`; Type counts across the three files (case variants merged): Armor 8954, Card 5593, Etc 4240, Weapon 2806, Usable 2800, Cash 2410, DelayConsume 742, ShadowGear 1028, Healing 463, Ammo 123, PetEgg 149, PetArmor 45.
* Server operators override/extend via `db/import/item_db.yml` (not present in the checkout; the `import` directory only exists in templates). An entry with an existing `Id` is MERGED field-by-field into the earlier one (`parseBodyNode`: `exists` branch only overwrites keys that are present); a new `Id` requires `AegisName` and `Name`.
* `Id` is `u32`; `AegisName` must be unique (case-insensitive), max `ITEM_NAME_LENGTH-1` chars; `Name` is the English display name. The loader always creates a dummy item `ITEMID_DUMMY = 499` ("UNKNOWN_ITEM", weight 1, sell 1, `view_id` 512 = apple sprite) used for unknown ids (`itemdb.cpp:1203-1214`, `UNKNOWN_ITEM_ID = 512`, `itemdb.hpp:22`).

## Field reference

Weights, prices are integers. "Default" is what an omitted key becomes for a NEW entry.

| Key | Type / values | Default | Notes (load-time rules) |
| :--- | :--- | :--- | :--- |
| `Id` | u32 | required | |
| `AegisName` | string | required (new) | script/lookup name; duplicates skipped |
| `Name` | string | required (new) | |
| `Type` | `Healing, Usable, Etc, Armor, Weapon, Card, PetEgg, PetArmor, Ammo, DelayConsume, ShadowGear, Cash` (case-insensitive; resolved as constant `IT_<Type>`) | `Etc` | invalid -> `Etc`. `DelayConsume` is rewritten to `Usable` + `delay_consume` TEMP flag in `loadingFinished` (`:1122`). |
| `SubType` | Weapon: `W_` names (below); Ammo: `AMMO_`; Card: `CARD_` (`Normal, Enchant` seen) | 0 | Only legal for Weapon/Ammo/Card types, else the entry is rejected (`:300+`). |
| `Buy` | u32 zeny, capped `MAX_ZENY = INT_MAX` | 0 | If only `Sell` is given: `Buy = 2 * Sell`. |
| `Sell` | u32 zeny | 0 | If only `Buy` is given: `Sell = Buy / 2`. After defaults, if `Buy/124 < Sell/75` (discount/overcharge exploit) then `Sell = 1` (`:1190`). |
| `Weight` | u32, **10 = 1.0 weight** | 0 | e.g. Red Potion 70, Knife 400, Guard 300, Jellopy 10. Used raw (0.1 units) by inventory weight math and on the wire. |
| `Attack` | u32 | 0 | Cleared (warning) unless Type is Weapon or Ammo (`:1166`). |
| `MagicAttack` | u32 (Renewal only) | 0 | Cleared unless Weapon. |
| `Defense` | u32, capped `DEFTYPE_MAX` | 0 | Cleared unless Armor or ShadowGear (`:1161`). |
| `Range` | u16, capped `AREA_SIZE` (14) | 0 | Cleared unless Weapon. |
| `Slots` | u16, capped `MAX_SLOTS` (4) | 0 | Card slots (`itemdb_slots`). |
| `Jobs` | map of job name -> bool; `All: true` | all jobs | Names resolved as `EAJ_<Name>` (names seen in the data include `Novice, Swordman, Mage, Archer, Acolyte, Merchant, Thief, Knight, Priest, Wizard, Blacksmith, Hunter, Assassin, Crusader, Monk, Sage, Rogue, Alchemist, BardDancer, StarGladiator, SoulLinker, Gunslinger, Ninja, KagerouOboro, Rebellion, SuperNovice, Spirit_Handler`). Stored as three `u64` base-class bitsets `class_base[3]`. |
| `Classes` | map of upper-class -> bool; `All: true` | all | `ITEMJ_` bits (`itemdb.hpp:213-225`): `Normal 0x01, Upper 0x02, Baby 0x04, Third 0x08, Third_Upper 0x10, Third_Baby 0x20, Fourth 0x40`; group names `All_Upper, All_Baby, All_Third`. Stored as `class_upper` (u16). |
| `Gender` | `Female, Male, Both` | `Both` | Forced: wedding rings M/F, Musical instruments (`W_MUSICAL`) = Male, Whips (`W_WHIP`) = Female (`defaultGender`, `:1227`). |
| `Locations` | map of `EQP_` position names -> bool | none | See "Equip location bits". **For Weapon, Armor, Ammo and ShadowGear a missing/empty `Locations` demotes the item to `Etc`** (`:476-484`, `itemdb_isequip2`). Position names containing `Shadow_*` are only valid for ShadowGear (else item becomes `Etc`). |
| `WeaponLevel` | 1-4 | 0 | Only valid for Weapon; Weapon with 0 is set to 1 at finish (`:1129-1134`). |
| `ArmorLevel` | 1-? | 0 | Only valid for Armor; Armor with 0 -> 1 (`:1139-1144`). |
| `EquipLevelMin` | u16 <= `MAX_LEVEL` | 0 | Base-level gate (`item.elv`), checked on equip and use. |
| `EquipLevelMax` | u16 | `MAX_LEVEL` | Must be `>= EquipLevelMin`. |
| `Refineable` | bool | false | `no_refine = !Refineable`. |
| `Gradable` | bool | false | Whether item can receive a `grade` (`enchantgrade`). |
| `View` | u32 | 0 | Sprite/view id (`item.look`): weapon sprite type, headgear/garment/costume sprite id, shield id (a Shield with `View: 0` is forced to 1 "Guard", `:1196`). Sent as `look`/`wItemSpriteNumber`. |
| `AliasName` | AegisName of another item | none | Sets `view_id`: the client is sent THAT item's id instead (`client_nameid`). Unknown alias is a load warning. |
| `Flags` | map | | below |
| `Delay` | `Duration` (seconds), `Status` (`SC_` name, shared-cooldown group) | none | Item use cooldown (`pc_itemcd_check`). |
| `Stack` | `Amount` (u16), `Inventory`, `Cart`, `Storage`, `GuildStorage` (bools) | none | Only legal for stackable types. **The code defaults `Inventory`/`Cart`/`Storage`/`GuildStorage` to false when a `Stack` node exists without that key** (`itemdb.cpp:822-880`), although the header comment says `Inventory` defaults to true; write `Inventory: true` explicitly in data. No `Stack` node = `amount 0`, no per-item cap (global 30000 only). |
| `NoUse` | `Override` (GM level, default 100), `Sitting` (bool) | none | `Sitting` blocks use while sitting (`MSI_CANT_USE_WHEN_SITDOWN`) unless GM level >= Override. |
| `Trade` | `Override` (GM level, default 100) + bools `NoDrop, NoTrade, TradePartner, NoSell, NoCart, NoStorage, NoGuildStorage, NoMail, NoAuction` | all false | `NoDrop` blocks `CZ_ITEM_THROW`. Most frequent flags in the data: NoDrop ~7.3k, NoAuction ~7k, NoMail ~7k, NoGuildStorage ~7k, NoCart ~7k, NoTrade ~6.9k, NoSell ~4.6k. |
| `Script` | script text (`|` block) | none | Run on use (consumables) and, for equipment, on every status recalculation while worn. See [scripting/items.md](../scripting/items.md). |
| `EquipScript` | script | none | Run once when equipped. |
| `UnEquipScript` | script | none | Run once on unequip / rental expiry. |

### `Flags` map

| Flag | Meaning |
| :--- | :--- |
| `BuyingStore` | Allowed in buying stores; only valid for stackable items. |
| `DeadBranch` | Dead Branch behaviour; blocked on `nobranch` maps; logged. |
| `Container` | Item-group container (`flag.group`): use is blocked at weight percent `>= open_box_weight_rate` or with `<= 10` free slots (renewal). |
| `UniqueId` | Always its own slot with a `unique_id` (`flag.guid`); only valid for stackable items; never merges. |
| `BindOnEquip` | Becomes `bound` (character) when equipped; shown as `bindOnEquipType = 1`. |
| `DropAnnounce` | Broadcast on obtain (`flag.broadcast`). |
| `NoConsume` | Item is not consumed by use (`delay_consume |= NOCONSUME`). |
| `DropEffect` | Floor-drop effect for monster drops: `NONE, CLIENT, BLUE_PILLAR, YELLOW_PILLAR, PURPLE_PILLAR, GREEN_PILLAR, RED_PILLAR` for `PACKETVER >= 20200304` (WHITE and ORANGE were removed/replaced then and are kept only as legacy enum tail values, `itemdb.hpp:3263-3290`). `CLIENT` (client-chosen by item) is the only value seen in the shipped data. |

(`flag.autoequip` has no YAML key: it is set at runtime by the `autoequip` script command, `script.cpp:~16990-17004`; `flag.available` is true for every loaded entry.)

## Type enum

`enum item_types` (`mmo.hpp:223-237`). "Wire type" is what `itemtype()` puts in packet `type` bytes (`clif.cpp:109-118`).

| `IT_*` | Value | YAML `Type` | Stackable | Equipable (`itemdb_isequip2`) | Wire type |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `IT_HEALING` | 0 | `Healing` | yes | no | 0 |
| `IT_UNKNOWN` | 1 | (unused) | yes | no | 1 |
| `IT_USABLE` | 2 | `Usable` (also `DelayConsume` after load) | yes | no | 2 |
| `IT_ETC` | 3 | `Etc` | yes | no | 3 |
| `IT_ARMOR` | 4 | `Armor` | no | yes | 4 |
| `IT_WEAPON` | 5 | `Weapon` | no | yes | 5 |
| `IT_CARD` | 6 | `Card` | yes | no | 6 |
| `IT_PETEGG` | 7 | `PetEgg` | no | no | **4** (as Armor) |
| `IT_PETARMOR` | 8 | `PetArmor` | no | no | 8 |
| `IT_UNKNOWN2` | 9 | (unused) | yes | no | 9 |
| `IT_AMMO` | 10 | `Ammo` | **yes** | yes | 10 |
| `IT_DELAYCONSUME` | 11 | `DelayConsume` | yes | no | n/a (becomes 2 at load) |
| `IT_SHADOWGEAR` | 12 | `ShadowGear` | no | yes | **5** if location has `EQP_SHADOW_WEAPON`, else **4** |
| `IT_CASH` | 18 | `Cash` | yes | no | 18 |

* **Stackable rule:** non-stackable = Weapon, Armor, PetEgg, PetArmor, ShadowGear (`itemdb.cpp:4932`). Ammo/Card/Cash/Etc/Usable/Healing stack. Non-stackable items are sent in the equipment list packet (`0x0b39`), stackable in the normal list (`0x0b09`).
* **Consumable use gate:** only `Healing`, `Usable`, `Cash` may be used via `CZ_USE_ITEM` (`pc_isUseitem`).

## Equip location bits (`enum equip_pos`, `mmo.hpp:336-362`; YAML names `EQP_<Name>`)

| YAML `Locations` key | Bit | Hex |
| :--- | :--- | :--- |
| `Head_Low` | 0 | `0x000001` |
| `Right_Hand` (`EQP_WEAPON`) | 1 | `0x000002` |
| `Garment` | 2 | `0x000004` |
| `Right_Accessory` | 3 | `0x000008` |
| `Armor` | 4 | `0x000010` |
| `Left_Hand` (`EQP_SHIELD`) | 5 | `0x000020` |
| `Shoes` | 6 | `0x000040` |
| `Left_Accessory` | 7 | `0x000080` |
| `Head_Top` | 8 | `0x000100` |
| `Head_Mid` | 9 | `0x000200` |
| `Costume_Head_Top` | 10 | `0x000400` |
| `Costume_Head_Mid` | 11 | `0x000800` |
| `Costume_Head_Low` | 12 | `0x001000` |
| `Costume_Garment` | 13 | `0x002000` |
| (reserved `Costume_Floor`) | 14 | `0x004000` (commented out) |
| `Ammo` | 15 | `0x008000` |
| `Shadow_Armor` | 16 | `0x010000` |
| `Shadow_Weapon` | 17 | `0x020000` |
| `Shadow_Shield` | 18 | `0x040000` |
| `Shadow_Shoes` | 19 | `0x080000` |
| `Shadow_Right_Accessory` | 20 | `0x100000` |
| `Shadow_Left_Accessory` | 21 | `0x200000` |

Combined names usable in YAML: `Both_Hand` = `Right_Hand|Left_Hand` (`0x22`, `EQP_ARMS`), `Both_Accessory` = `Right|Left_Accessory` (`0x88`, `EQP_ACC`), plus script-side groups `EQP_HELM` (`0x301`), `EQP_COSTUME` (`0x3C00`), `EQP_SHADOW_GEAR`, `EQP_SHADOW_ACC`, `EQP_SHADOW_ARMS` (`pc.hpp:1125-1134`). `EQP_VISIBLE` (positions that have a sprite): `HELM|GARMENT|COSTUME` for `PACKETVER >= 20110111` (`pc.hpp:1136-1140`). Multi-position items (e.g. `Head_Top|Head_Mid`) occupy all listed slots. Dual-wield promotion of one-hand dagger/sword/axe from `Right_Hand` to `Both_Hand` is done at inventory-list time, see `location` in [packets/inventory.md](../packets/inventory.md). Equip/unequip flows: another catalog section (equip agent).

## Weapon `SubType` names

`W_` constants: counts in the shipped data: Dagger, Staff, `1hSword`, Bow, Mace, Book, `2hSword`, Katar, `2hStaff`, Whip, Knuckle, `1hSpear`, Musical, `2hAxe`, `2hSpear`, Revolver, Huuma, `1hAxe`, Rifle, Grenade, Gatling, Shotgun, `Throwweapon`... Ammo: `Arrow, Bullet, Kunai, Cannonball, Shuriken`. Card: `Normal` (default), `Enchant` (4047 entries: enchant items). Numeric values come from script constants `W_*`, `AMMO_*`, `CARD_*` (not catalogued yet).

## Sample entries (facts, not copied rows)

* Consumable: `Id 501 Red_Potion`, `Type Healing`, `Buy 10`, `Weight 70`, `Script: itemheal rand(45,65),0;`. `Id 909 Jellopy`, `Type Etc`, `Buy 6`, `Weight 10`, `Flags.BuyingStore: true`.
* Weapon: `Id 1201 Knife`, `Type Weapon`, `SubType Dagger`, `Buy 50`, `Weight 400`, `Attack 17`, `Range 1`, `Slots 3`, `Locations.Right_Hand`, `WeaponLevel 1`, `EquipLevelMin 1`, `Refineable true`, long `Jobs` list.
* Armor/shield: `Id 2101 Guard`, `Type Armor`, `Buy 500`, `Weight 300`, `Defense 20`, `Locations.Left_Hand`, `ArmorLevel 1`, `Refineable true`, `View 1`.

## Notes for Metaling

* The registries `ro-db` loads are GPL-derived data read from disk at runtime (never embedded). A minimal item registry for inventory needs: `Id, AegisName, Name, Type, SubType, Weight, Slots, Locations, View, AliasName, EquipLevelMin/Max, Stack.Amount/Inventory, Flags.UniqueId/BindOnEquip, Trade.NoDrop, NoUse.Sitting, Delay.Duration, Script`.
* Treat `Weight` as already in 0.1 units everywhere; never divide until display.
