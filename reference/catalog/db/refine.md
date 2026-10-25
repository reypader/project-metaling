# `refine.yml` and `enchantgrade.yml` (bonus data)

Source: rAthena `d4b8e7b8f`, `$RATHENA_PATH/db/re/refine.yml` (2639 lines, header `REFINE_DB` version 2), `db/re/enchantgrade.yml`; loader `RefineDatabase::parseBodyNode` `src/map/status.cpp:140-410`, lookups `status.cpp:415-488`, `EnchantgradeDatabase` `status.cpp:586-985`. Applies to 20260219 (renewal). How the bonuses are used in the stat pass: [../mechanics/stats.md](../mechanics/stats.md#refine-bonuses-weapon-atk-weapon-matk-armor-def).

## Schema (`refine.yml`)

`Body:` list of `Group` entries: `Armor`, `Weapon`, `Shadow_Armor`, `Shadow_Weapon` (any other name is skipped with a warning, `status.cpp:144`). Each group has `Levels:` (item level = `WeaponLevel` for Weapon, `ArmorLevel` for Armor, always 1 for shadow gear, `status.cpp:465-488`), each level has `RefineLevels:`, one entry per refine level 1..`MAX_REFINE` (**20** in renewal, `status.hpp:34`; a `Level` of 0 or above MAX_REFINE is skipped).

Per refine-level fields (`refine.yml:25-45`): `Bonus` (u32, default 0), `RandomBonus` (u32, default 0), `BlacksmithBlessingAmount`, `BroadcastSuccess`, `BroadcastFailure`, and `Chances` (list per cost `Type` Normal/HD/Enriched with `Rate` /10000, `Price` zeny, `Material` ore, `BreakingRate` /10000, `DowngradeAmount`). Success rates, costs, breaking and downgrade are refine-NPC territory and are not catalogued.

* **Units:** `Bonus` and `RandomBonus` are 1/100 of a point (Bonus 100 = 1 ATK/DEF point). Consumers divide by 100.
* **Cumulative, not incremental.** The entry for refine level N already holds the total bonus of an item at +N (Armor L1: 100, 200, 300, ...). The stat pass looks up exactly the entry for the item's current refine (`findCurrentLevelInfo` asks for `refine`, stored internally at key `refine-1`, `status.cpp:457-462`); nothing is summed over lower levels.
* Lookup returns "no bonus" when: the item is `no_refine` (item_db `Refineable` false or absent; absent defaults to not refineable, `itemdb.cpp:570-582`), refine is 0, the item type is not Weapon/Armor/ShadowGear, or no entry exists for that item level. A refine above MAX_REFINE is clamped to 20 (also clamped in `status_calc_pc`, `status.cpp:3927`).
* Merging: re-loading an existing group/level/refine key overwrites only fields present (import-over-existing semantics); a new key defaults missing `Bonus`/`RandomBonus` to 0.
* **Shadow groups carry no `Bonus`/`RandomBonus` at all** (only Chances); shadow gear refine gives no stat bonus here.

## Bonus table (Bonus, 1/100 units; identical in all rows below for refine 1..20)

`Armor` (refine level 1..20):

| Item level | +1..+4 | +5 | +6 | +7 | +8 | +9 | +10 | +11 | +12 | +13 | +14 | +15 | +16 | +17 | +18 | +19 | +20 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | 100,200,300,400 | 600 | 800 | 1000 | 1200 | 1500 | 1800 | 2100 | 2400 | 2800 | 3200 | 3600 | 4000 | 4500 | 5000 | 5500 | 6000 |
| 2 | 120,240,360,480 | 720 | 960 | 1200 | 1440 | 1800 | 2160 | 2520 | 2880 | 3360 | 3840 | 4320 | 4800 | 5400 | 6000 | 6600 | 7200 |

(Armor level 2 = level 1 x 1.2. Only armor levels 1 and 2 exist.)

`Weapon`, `Bonus` per refine level (`RandomBonus` is 0 where blank):

| Wlv | +1 | +2 | +3 | +4 | +5 | +6 | +7 | +8 | +9 | +10 | +11 | +12 | +13 | +14 | +15 | +16 | +17 | +18 | +19 | +20 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | 200 | 400 | 600 | 800 | 1000 | 1200 | 1400 | 1600 | 1800 | 2000 | 2200 | 2400 | 2600 | 2800 | 3000 | 4800 | 5100 | 5400 | 5700 | 6000 |
| 2 | 300 | 600 | 900 | 1200 | 1500 | 1800 | 2100 | 2400 | 2700 | 3000 | 3300 | 3600 | 3900 | 4200 | 4500 | 8000 | 8500 | 9000 | 9500 | 10000 |
| 3 | 500 | 1000 | 1500 | 2000 | 2500 | 3000 | 3500 | 4000 | 4500 | 5000 | 5500 | 6000 | 6500 | 7000 | 7500 | 11200 | 11900 | 12600 | 13300 | 14000 |
| 4 | 700 | 1400 | 2100 | 2800 | 3500 | 4200 | 4900 | 5600 | 6300 | 7000 | 7700 | 8400 | 9100 | 9800 | 10500 | 16000 | 17000 | 18000 | 19000 | 20000 |
| 5 | 800 | 1600 | 2400 | 3200 | 4000 | 4800 | 5600 | 6400 | 7200 | 8000 | 8800 | 9600 | 10400 | 11200 | 12000 | 12800 | 13600 | 14400 | 15200 | 16000 |

Weapon `RandomBonus` (over-refine; the stat pass uses it as the maximum, see stats.md), by refine level:

| Wlv | non-zero entries |
| :--- | :--- |
| 1 | +8:300, +9:600, +10:900, +11:1200, +12:1500, +13:1800, +14:2100, +15:2400, +16:2700, +17:3000, +18:3300, +19:3600, +20:3900 (0 for +1..+7) |
| 2 | +7:500, +8:1000, +9:1500, +10:2000, +11:2500, +12:3000, +13:3500, +14:4000, +15:4500, +16:5000, +17:5500, +18:6000, +19:6500, +20:7000 |
| 3 | +6:800, +7:1600, +8:2400, +9:3200, +10:4000, +11:4800, +12:5600, +13:6400, +14:7200, +15:8000, +16:8800, +17:9600, +18:10400, +19:11200, +20:12000 |
| 4 | +5:1400, +6:2800, +7:4200, +8:5600, +9:7000, +10:8400, +11:9800, +12:11200, +13:12600, +14:14000, +15:15400, +16:16800, +17:18200, +18:19600, +19:21000, +20:22400 |
| 5 | none (all 0) |

Extracted by a one-off parse of the file (Group / Level / RefineLevels Level / Bonus / RandomBonus). Importer should keep the structure and re-derive; do not hand-copy.

## `enchantgrade.yml` (grade bonus)

Types `Armor` (Level 2 only) and `Weapon` (Level 5 only) have `Grades` None, D, C, B (and further) with per-grade `Bonus` (percent) plus `Chances`/`Options` (upgrade data, not catalogued). Bonus: None 10, D 30, C 50, B 100 (`enchantgrade.yml:88,128,166,205` armor; `252,292,330,369` weapon). Constants `ENCHANTGRADE_NONE=0, D=1, C=2, B=3, A=4` (`mmo.hpp:125-129`). Lookup (`status.cpp:961-985`) is by `item.enchantgrade - 1` and runs only when `enchantgrade > 0`, and only for Weapon level 5 / Armor level 2, so lower-level items get no grade bonus. Note the off-by-one: item field 1 resolves to grade entry `None` (10%), 2 to D (30%), 3 to C (50%), 4 to B (100%); the A entry's Bonus was not read. Verify against a real-client capture before relying on the mapping.
