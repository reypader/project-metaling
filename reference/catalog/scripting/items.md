# Item Scripts (`Script`, `EquipScript`, `UnEquipScript`)

Target client version: `PACKETVER = 20260219` (script semantics are version independent; renewal build).

* **Provenance:** `rathena` @ `d4b8e7b8f` (2026-10-06), paths relative to `$RATHENA_PATH`. Census numbers come from a regex-based read-only census over `db/re/item_db_{usable,equip,etc}.yml` and `db/re/item_combos.yml` (counts are approximate to a few percent where noted "~"; the heal-shape counts in "Importer notes" are exact). Semantics were checked in `src/map/{pc,status,script}.cpp`. Item-use packet flow is in [mechanics/inventory.md section 7](../mechanics/inventory.md#7-use-cz_use_item---pc_useitem).
* **Licensing reminder:** the script texts are rAthena data (GPL). They may only reach us through `tools/item-import` into `crates/ro-db/data/`.

## 1. Keys and census

The YAML keys are `Script` (use effect for usable items, passive bonus for equipment), `EquipScript` (runs once on equip) and `UnEquipScript` (runs once on unequip). They are NOT named `OnEquip_Script`/`OnUnequip_Script`. Combos (`item_combos.yml`) carry a `Script` per set.

| Block | Count |
| :-- | --: |
| `Script` total | 25151 |
| - usable (`item_db_usable.yml`) | 5945 |
| - equip (`item_db_equip.yml`) | 8922 |
| - etc (`item_db_etc.yml`) | 5170 |
| - combos | 5114 |
| `EquipScript` | 19 |
| `UnEquipScript` | 343 (mostly `heal -N,0`-style HP/SP loss) |

Top builtins by number of scripts using them (statement position):

| Group | Builtin: count |
| :-- | :-- |
| usable | `getgroupitem` 2768, `getitem` 1242, `sc_start` 454, `itemheal` 248, `percentheal` 237 (234 in the first census, regex difference), `itemskill` 119, `sc_end` 28, `warp` 26 |
| equip | `bonus` 7928, `bonus2` 4925, `if` 3855, `skill` 384, `bonus3` 366, `autobonus` 226 |

Shape statistics (strict = only constant-argument `bonus*` statements, no `if`, no calls): equip 37 %, etc 54 %, combos 45 %, usable about 0 %. Loops (`for`/`while`) are almost absent. `.@r = getrefine()` threshold ladders dominate the conditionals. Only 4 usable items open a dialog (`input`/`select`); none use `mes`/`sleep`.

Condition reads (scripts mentioning each): `getrefine` ~5900, `BaseLevel` ~1100, `getskilllv` ~880, job/class (`BaseClass`, `eaclass`, `Class`) ~800, `readparam` ~700. `readparam(bStr)`-style reads return the raw allocated stat via `pc_readparam` (`script.cpp:2780`, `:8842-8867`), not the bonus-accumulated one.

## 2. Effect shapes of usable items

* About 480 usable items contain `itemheal` or `percentheal`; 288 of them consist of that single statement (see Importer notes). Red Potion 501 `itemheal rand(45,65),0;`, Blue Potion 505 `itemheal 0,rand(40,60);`.
* Cures, food and ASPD potions need statuses (`sc_start`, `sc_end`): 454 `sc_start` scripts. Next plan, not this one.
* Fly Wing / Butterfly Wing are `itemskill AL_TELEPORT` (and similar, 119 scripts). `itemskill` sets `sd->skillitem`, sends `0x0147`, the client re-sends `0x0438`.
* Boxes: `getgroupitem` (2768 scripts) resolves through `db/re/item_group_db.yml` (504 groups). `getitem` 1242.

## 3. Execution context

* **Usable `Script`:** `run_script(script, 0, sd->id, fake_nd->id)` at `pc.cpp:6558`, after the `0x01c8` ack and `pc_delitem` (so heal packets follow the ack; order in [inventory.md 7.3](../mechanics/inventory.md#73-exact-order-for-a-successful-potion-itemheal)). The player is the attached RID; a previously attached script state is detached and restored around the call (`pc.cpp:6547-6570`). `sd->itemid`/`sd->itemindex` are set first so `itemheal` knows which item (for per-item heal-rate bonuses). `DelayConsume` items ack with the amount unchanged and are consumed later by the skill flow.
* **Equip `Script`:** not run on equip. It runs inside every `status_calc_pc` pass (`status.cpp:3993-4048` for weapons/armor slots, `:4125-4198` for data-driven equipment) after the bonus tables are reset, so bonuses are re-derived from scratch each recalculation. Skipped when the item is restricted on the map unless the GM permission `USE_ALL_EQUIPMENT` is set. Combo scripts are run in the same pass.
* **`EquipScript`:** once on equip (`pc.cpp:12263-12276`, plus card/option sub-items) and once at login during the first calc (`SCO_FIRST`, `status.cpp:3920-3922`, `:4125`). **`UnEquipScript`:** once on unequip (`pc.cpp:12361-12375`; also `:1211`). These are for one-shot actions (e.g. HP loss), not bonuses.
* **Recursion guard:** `status_calc_pc` keeps a static depth counter and aborts beyond 10 nested calls (`status.cpp:3756-3763`); a `run_script` that re-triggers a calc returns early (`:3997`, `:4030`, `:4036`, `:4050`).

## 4. Bonus re-evaluation semantics

* Because the equip `Script` runs on every recalculation, any condition it reads is re-read each time: refine level (`getrefine`), base level, skill levels (`getskilllv`), job/class, stats, other equipped items (`isequipped`). A script is therefore a pure function of (equipment, character) producing a bonus list; it must be re-run whenever one of its inputs changes.
* `readparam` inside an equip script reads the raw allocated stat (before this calc's bonuses), so stat-threshold items do not feed back into themselves.
* A script that is unconditional and bonus-only (equip 37 %, etc 54 %, combos 45 %) can be evaluated once at load into a static list; the rest depend on the reads above and must be re-evaluated on refine/level/skill/job changes. (This is the static/dynamic split also used by rust-ro, see [rust-ro-script-architecture.md](./rust-ro-script-architecture.md).)
* Usable scripts are evaluated per use with no caching; `rand()` is rolled per use.

## 5. Importer notes (for `tools/item-import`, M1)

Source: `db/re/item_db_usable.yml`, 5945 `Script` blocks; all are `Script: |` literal blocks (no inline scalars). Statement text is indented 6 spaces and ends with `;`. Counts below were produced by normalising digits to `N` after stripping `/* */` and `//` comments, on the same commit.

**Scripts that are exactly one `itemheal` or one `percentheal` statement: 288** (212 itemheal, 76 percentheal). Distinct shapes:

| Shape (digits as `N`) | Count | Example ids |
| :-- | --: | :-- |
| `itemheal rand(N,N),N;` | 101 | 501, 502, 503, 504, 507, 508 |
| `percentheal N,N;` | 75 | 500, 607, 608, 663, 669, 680 |
| `itemheal N,N;` | 37 | 535, 558, 559, 560, 561, 564 |
| `itemheal N,N;` followed on the same line by a `/* ... */` comment (e.g. `/* average value */`) | 32 | 11606, 11611, 11612, 11625, 11627, 11628 |
| `itemheal rand(N,N),rand(N,N);` | 22 | 518, 552, 576, 582, 587, 11506 |
| `itemheal N,rand(N,N);` | 19 | 505, 510, 514, 533, 548, 568 |
| `itemheal rand(N,N),N;` + a separate-line `/*TODO*/` comment | 1 | 11605 |
| `percentheal N,N;` + a separate-line `/* TODO ... */` comment | 1 | 22945 |

Observations for the parser:
* Spacing is uniform in the corpus: one space after the command, no spaces after commas, `rand(` with no space. No `//` comments appear in any heal script. Accept optional whitespace anyway.
* Only decimal non-negative integer literals appear in the 288; no negative values, no variables, no third (`char_id`) argument, no `rand` with a single argument. Negative heals appear only inside the 2 conditional scripts (678, 22549), which are not liftable.
* `rand(a,b)` is inclusive on both ends in rAthena script (Red Potion 45-65); `a <= b` in all rows.
* Trailing block comments must be stripped; treat the script as liftable only if, after stripping comments, exactly one statement remains.
* A `percentheal` pair is `(hp%, sp%)`; an `itemheal` pair is `(hp, sp)` where each side is a constant or `rand(min,max)`.

**Scripts that mix a heal with other statements: 196** (not liftable as whole-script `Use:`). Of these, 36 contain `itemheal` and 158 contain `percentheal` (1 contains both, 11613: `percentheal 3,0;` then `itemheal 0,100;`). By companion statements:

| Companion statements | Count | Example ids |
| :-- | --: | :-- |
| `percentheal` + `sc_start` | 124 | 12027, 12034, 12041, 12042 |
| `itemheal` + `sc_end` (cures: poison/silence/blind/...) | 12 | 526, 566, 573, 583 |
| `itemskill` + `percentheal` | 10 | 12200, 12236, 12376, 12377 |
| `percentheal` + `sc_start2` | 8 | 12274, 12275, 12422, 12423 |
| `itemheal` + `sc_start` | 7 | 536, 549, 554, 11551 |
| `percentheal` + `sc_start` + `skilleffect` | 7 | 12235, 12290, 12291, 12405 |
| `itemheal` inside `if` (e.g. `checkmadogear()`), one heal only | 5 | 11515, 11563, 12392, 12393 |
| `itemheal` + `showscript` + `specialeffect2` + `transform` | 4 | 11592, 11593, 11594, 11595 |
| `percentheal` + `specialeffect2` | 4 | 23260, 100848, 103071, 103432 |
| `percentheal` + `sc_start` + `specialeffect2` | 3 | 22621, 23259, 23261 |
| `callfunc` + `itemheal` | 2 | 11536, 11537 |
| `itemheal` + `specialeffect2` | 2 | 11601, 11602 |
| single-instance mixes: `itemheal`+`itemskill` (585), `itemheal`+`percentheal` (11613), `itemheal`+`sc_start2` (12436), `MDiceCone` custom call + `percentheal` (12133), `bonus2`+`bonus_script`+`itemheal`+`specialeffect2` (12574), `if` chains whose only heal text is in a branch (12396) | 1 each | as listed |
| `if` + `sc_start` with a non-literal condition and heal-like text inside (`eaclass()` tests) | 2 | 678, 22549 |

(The `itemheal` + `sc_end` shape therefore covers 12 scripts, all cure potions such as 526, a heal rand plus several `sc_end SC_*;` lines.)

Cross-references: runtime formulas in [mechanics/inventory.md 7.4](../mechanics/inventory.md#74-itemheal-and-percentheal-formulas-renewal); packet flow in [7.1-7.3](../mechanics/inventory.md#71-wire-layouts-for-potion-use-20260219).
