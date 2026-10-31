# `mob_db.yml` fields used by mob spawn, damage and death

Target `PACKETVER = 20260219`, renewal data file `$RATHENA_PATH/db/re/mob_db.yml` (commit `d4b8e7b8f`; header `Type: MOB_DB`, `Version: 5`; 120900 lines, 2675 live entries (`^  - Id:`; 3009 more are commented out, so a plain `grep -c -- "- Id:"` gives 5684); there is no `db/import` directory in this checkout). Loader: `MobDatabase::parseBodyNode` (`src/map/mob.cpp:4977-5530`), post-processing `MobDatabase::loadingFinished` (`mob.cpp:5537-5596`), drop post-processing `mob_drop_ratio_adjust` (`mob.cpp:6773-6935`). Generic YAML schema overview: [yaml_schemas.md](./yaml_schemas.md). How the fields are used at runtime: [mechanics/mobs.md](../mechanics/mobs.md).

This is GPL-3.0 rAthena data: only read it from disk via an import tool under `tools/`, never `include_str!` it (see repo `CLAUDE.md` licensing rules).

## Id rules

`Id` must satisfy `1000 < id < 3999` or `20020 < id < 31999` (`mob.cpp:61-64`); anything else is rejected. An entry without `AegisName` + `Name` for a new id is rejected. A later entry with an existing `Id` overrides only the fields it lists (merge). `Name` and `AegisName` are capped at 23 chars. Spawn lines may refer to the mob by id or by `AegisName` (`mobdb_search_aegisname`).

## Fields

All fields optional after the first load of an id unless noted. "Default" is the value the C++ struct constructor uses (`s_mob_db::s_mob_db`, `mob.cpp:4938-4970`), applied when the key is absent.

| YAML key | Type | Default | Notes / runtime use |
| :--- | :--- | :--- | :--- |
| `Id` | u32 | required | see above; also the sprite id sent in `job` of `0x09ff/0x09fe` |
| `AegisName` | string | required (new id) | spawn-line / script reference |
| `Name` | string | required (new id) | English name; used when `override_mob_names: 1`; spawn-line name is used at the default `0` |
| `JapaneseName` | string | = `Name` | |
| `Level` | u16 (no range check, `mob.cpp:5044`) | 1 | `md->level` on spawn unless the spawn line overrides; level penalty input; `clevel` in the entry packets |
| `Hp` | u32 | 1 | `status.max_hp`; scaled at load by `monster_hp_rate` (non-MVP) or `mvp_hp_rate` (MVP) (both default 100); `hp = max_hp` on every spawn |
| `Sp` | u32 | 1 | `max_sp`, minimum 1 |
| `BaseExp` | u64 (`asUInt64`, `mob.cpp:5071`) | 0 | multiplied at load by `base_exp_rate` (100); the full-share value before damage-share and bonuses |
| `JobExp` | u64 (`mob.cpp:5080`) | 0 | same with `job_exp_rate` (100) |
| `MvpExp` | u64 | 0 | same with `mvp_exp_rate` (100); only paid to the MVP player when the mob has `Modes: Mvp` |
| `Attack` | u16 | 0 | renewal: base ATK (`rhw.atk`) |
| `Attack2` | u16 | 0 | renewal: base MATK (`rhw.matk`) (pre-renewal: max ATK) |
| `Defense` | i16 (renewal `defType`) | 0 | read as u16 (`mob.cpp:5120-5131`), then clamped to `[DEFTYPE_MIN, DEFTYPE_MAX]` = `[-32768, 32767]` under RENEWAL (`config/const.hpp:46-51`; pre-renewal is `signed char` -128..127). Because it is parsed as u16 a negative YAML value fails to parse (entry rejected); only values above 32767 get capped |
| `MagicDefense` | i16 | 0 | `status.mdef`, same u16 parse + clamp (`mob.cpp:5134-5145`) |
| `Resistance`, `MagicResistance` | u16 | 0 | renewal `res` / `mres`; stored as read, no clamp (`mob.cpp:5148-5163`) |
| `Str`,`Agi`,`Vit`,`Int`,`Dex`,`Luk` | u16 | 1 each | parsed as u16, `max(0, x)` is a no-op; no upper clamp (`mob.cpp:5166-5217`). Negative YAML fails to parse |
| `AttackRange` | u16 | 0 | melee/auto-attack range in cells (`rhw.range`); no clamp (`mob.cpp:5220`) |
| `SkillRange` | u16 | 0 | `range2`: view/aggro range for skill/AI (scaled by `view_range_rate`, default 100) |
| `ChaseRange` | u16 | 0 | `range3`: chase range (>= `range2` after scaling) |
| `Size` | enum | Small | `Size_<name>`: `Small`=0, `Medium`=1, `Large`=2 (`SZ_BIG`). Used for damage tables only; the *rendered* size effect comes from the spawn line, not this field |
| `Race` | enum | Formless | `Formless 0, Undead 1, Brute 2, Plant 3, Insect 4, Fish 5, Demon 6, Demihuman 7, Angel 8, Dragon 9, Player_Human 10, Player_Doram 11` (`Player` alone is NOT valid: `RC_Player` is unknown -> Formless). See "Enum strings" below |
| `RaceGroups` | map name->bool | none | secondary groups (`RC2_*`, e.g. `Goblin`, `Treasure`); `Treasure` switches drop rates to `item_rate_treasure` |
| `Element` | enum | Neutral | `Neutral, Water, Earth, Fire, Wind, Poison, Holy, Dark, Ghost, Undead` |
| `ElementLevel` | u16 1..4 | 1 | `MAX_ELE_LEVEL` 4 (`map.hpp:422,1280`); out of range -> 1 with warning |
| `WalkSpeed` | u16 ms/cell | 150 (`DEFAULT_WALK_SPEED`) | becomes `status.speed`, sent verbatim in the entry packets' `speed`; clamped to `[MIN_WALK_SPEED 20, MAX_WALK_SPEED 1000]`. Examples: Poring 400, Scorpion 200, Osiris 100. Smaller = faster; step time = `speed` ms (x14/10 on diagonals) |
| `AttackDelay` | u16 | 100 (`MAX_ASPD_NOPC`) | `adelay` = time between attacks; clamped to `[MAX_ASPD_NOPC 100, MIN_ASPD 8000]` (`status.hpp:44,55`; `mob.cpp:5372-5379`); raised to at least `AttackMotion` after load |
| `AttackMotion` | u16 | `MAX_ASPD_NOPC/AMOTION_DIVIDER_NOPC` | `amotion`; clamped `[1, MIN_ASPD/AMOTION_DIVIDER_NOPC]` = `[1, 8000]` (`AMOTION_DIVIDER_NOPC` = 1; `mob.cpp:5381-5392`) |
| `ClientAttackMotion` | u16 | = `AttackMotion` | client-side damage frame; used by `synchronize_damage` for the `0x08c8` `src_speed` |
| `DamageMotion` | u16 | 0 | `dmotion` (no clamp), scaled by `monster_damage_delay_rate` (100); flinch time, sent as `dmg_speed` in `0x08c8` |
| `DamageTaken` | u16 (percent) | 100 | multiplier on all damage received (`md->damagetaken`; e.g. Osiris 10) |
| `GroupId` | u16 | 0 | sent as `group_id` in `0x0adf` name reply |
| `Title` | string | empty | sent as `title` in `0x0adf` |
| `Ai` | string | `06` | `MONSTER_TYPE_<Ai>` lookup -> initial mode mask; full table in "Mode bits and Ai table" below. Unknown or mask > 0xFFFF -> `06` (mask 0) with a warning |
| `Class` | enum | Normal | `Normal, Boss, Guardian, Battlefield, Event`. `Boss` adds `Detector\|StatusImmune\|KnockbackImmune` modes and makes `is_boss = 1` in entry packets; `Guardian` adds StatusImmune; `Battlefield` adds StatusImmune\|SkillImmune; `Event` adds `FixedItemDrop` (no level-penalty on drops). Counts (live entries, checked by `tools/mob-import` 2026-10-23): Boss 732, Event 69, Battlefield 25, Guardian 4; 190 have `Mvp: true` |
| `Modes` | map `<Mode>: bool` | none | `MD_<Key>` lookup (case-insensitive); true sets, false clears that bit, applied after `Ai`. Class-added modes and the `monster_active_enable` clear come later in `loadingFinished`. See below. `Mvp: true` -> `is_boss = 2` (bosstype MVP) |
| `MvpDrops` | list, max `MAX_MVP_DROP` = **3** | none | `Item` (AegisName), `Rate` (1..10000 = 0.01%..100%), `RandomOptionGroup`, `Index` |
| `Drops` | list, max `MAX_MOB_DROP` = **10** | none | `Item`, `Rate` (same scale), `StealProtected` (default false), `RandomOptionGroup`, `Index`. Unknown item names are skipped with a warning; a list longer than the max drops the extras; `Index` overwrites an existing slot. At load each rate is adjusted by `drops.conf` (identity at defaults) and entries with rate 0 are removed |

Not in the file as keys but derived: `bosstype` (0 none, 1 `Class: Boss`, 2 `Modes: Mvp`) from `Class` + `Mvp`; `status_calc_misc` fills hit/flee/crit/etc. from stats and `Level` after load.

## Example entries (verbatim facts from `db/re/mob_db.yml`)

* Poring (`Id: 1002`, line 136): `Level 1`, `Hp 55`, `BaseExp 150`, `JobExp 40`, `Attack 1`, `Attack2 1`, `Defense 2`, `MagicDefense 5`, `Str 6 Int 0 Dex 6 Luk 5` (Agi/Vit default 1), `AttackRange 1`, `SkillRange 10`, `ChaseRange 12`, `Size Medium`, `Race Plant`, `Element Water 1`, `WalkSpeed 400`, `AttackDelay 1872`, `AttackMotion 672`, `ClientAttackMotion 288`, `DamageMotion 480`, `Ai 02`; `Drops`: Jellopy 7000, Knife_ 100, Sticky_Mucus 400, Apple 1000, ... (rates are 1/10000).
* Osiris (`Id: 1038`): `Hp 1175840`, `MvpExp 61380`, `DamageTaken 10`, `Ai 21`, `Class Boss`, `Modes: Mvp: true`, 3 `MvpDrops` (5000/3000/500) and 8 `Drops`.

## Gotchas

* `Hp` for the first kill target: Poring `55`; the renewal db already holds renewal values, no per-version conversion needed.
* Rates in the db are per 10000; `base_exp_rate` etc. are applied once at load, not at kill time.
* Drop and MVP-drop arrays stay in file order; the roll order is that order (MVP: first success wins at default `item_drop_mvp_mode: 0`).
* `Size` here is NOT what the client renders; the 0x09fe/0x09ff packets carry no size field for mobs (the `x_size/y_size` bytes are 0). Big/medium look comes from the spawn-line size parameter via a special-effect packet.
* `AttackRange`/`SkillRange`/`ChaseRange` are measured in cells (Chebyshev); the AI itself is out of scope for the death milestone.

## Mode bits and Ai table

Source: `src/map/mob.hpp:150-182` (`MONSTER_TYPE_*`), `src/common/mmo.hpp:243-271` (`e_mode`), `src/map/script_constants.hpp:3556-3578,10254-10280` (exported names), loader `mob.cpp:5446-5525`, `loadingFinished` `mob.cpp:5537-5560`. Applies to 20260219 renewal.

**Order of mode computation**: (1) `Ai` sets `mode` wholesale (replaces, default mask 0 for a new entry; a re-listed id without `Ai` keeps its old mode); (2) each `Modes:` key sets (`true`) or clears (`false`) one bit in file order, so `X: false` clears even a bit `Ai` set; (3) in `loadingFinished`, `Class` adds modes with OR: Boss = Detector|StatusImmune|KnockbackImmune (0x2000000|0x4000000|0x200000), Guardian = StatusImmune, Battlefield = StatusImmune|SkillImmune, Event = FixedItemDrop (confirmed); Class cannot be overridden by `Modes: false` because it runs after; (4) if `monster_active_enable` is 0 the `Aggressive` (0x4) bit is cleared from every mob. Nothing is derived from `Mvp` beyond `bosstype` (a read of the `Mvp` bit; `Class: Boss` gives `BOSSTYPE_MINIBOSS` 1, Mvp bit gives 2).

**Name matching**: all of `Ai`, `Modes` keys, `Size`, `Race`, `Element`, `Class`, `RaceGroups` are looked up as script constants with a prefix (`MONSTER_TYPE_`, `MD_`, `Size_`, `RC_`, `ELE_`, `CLASS_`, `RC2_`); the constant table hash lookup uses `strcasecmp` (`script.cpp:694`), so matching is case-insensitive (the file spells `KnockBackImmune`, the constant is `MD_KNOCKBACKIMMUNE`). Unknown `Modes` key: warning, skipped. Mask range check for a mode key is `0..MD_SKILLIMMUNE`.

### Ai values (`Ai:` string -> mask; live-entry counts in `db/re/mob_db.yml`)

| Ai | Mask | Count | | Ai | Mask | Count |
| :-- | :-- | --: | :-- | :-- | :-- | --: |
| `01` | 0x81 | 164 | | `13` | 0x308D | 67 |
| `02` | 0x83 | 75 | | `17` | 0x91 | 108 |
| `03` | 0x1089 | 32 | | `19` | 0x3095 | 137 |
| `04` | 0x3885 | 771 | | `20` | 0x3295 | 101 |
| `05` | 0x2085 | 51 | | `21` | 0x3695 | 614 |
| `06` | 0 | 311 | | `24` | 0xA1 | 24 |
| `07` | 0x108B | 26 | | `25` | 0x1 | 0 |
| `08` | 0x7085 | 2 | | `26` | 0xB695 | 20 |
| `09` | 0x3095 | 0 | | `27` | 0x8084 | 5 |
| `10` | 0x84 | 64 | | `ABR_PASSIVE` | 0x21 | 1 |
| `11` | 0x84 | 0 | | `ABR_OFFENSIVE` | 0xA5 | 3 |
| `12` | 0x2085 | 6 | | | | |

Not defined (unknown -> `06`): 14, 15, 16, 18, 22, 23, and any other string. The counts sum to 2582 of 2675 live entries; the other 93 omit `Ai` and keep the struct default, mask 0 (same as `06`). Every defined mask fits the `<= 0xFFFF` check, so none are rejected as "invalid".

### MD_* bits and accepted `Modes:` keys

| Bit | `Modes:` key | Live count |
| :-- | :-- | --: |
| 0x1 | `CanMove` | 0 |
| 0x2 | `Looter` | 0 |
| 0x4 | `Aggressive` | 0 |
| 0x8 | `Assist` | 0 |
| 0x10 | `CastSensorIdle` | 0 |
| 0x20 | `NoRandomWalk` | 0 |
| 0x40 | `NoCast` | 0 |
| 0x80 | `CanAttack` | 0 |
| 0x100 | (free, no key) | |
| 0x200 | `CastSensorChase` | 0 |
| 0x400 | `ChangeChase` | 0 |
| 0x800 | `Angry` | 0 |
| 0x1000 | `ChangeTargetMelee` | 0 |
| 0x2000 | `ChangeTargetChase` | 0 |
| 0x4000 | `TargetWeak` | 0 |
| 0x8000 | `RandomTarget` | 0 |
| 0x10000 | `IgnoreMelee` | 145 |
| 0x20000 | `IgnoreMagic` | 144 |
| 0x40000 | `IgnoreRanged` | 142 |
| 0x80000 | `Mvp` | 190 |
| 0x100000 | `IgnoreMisc` | 138 |
| 0x200000 | `KnockBackImmune` (any case) | 30 |
| 0x400000 | `TeleportBlock` | 158 |
| 0x800000 | (free, no key) | |
| 0x1000000 | `FixedItemDrop` | 323 |
| 0x2000000 | `Detector` | 424 |
| 0x4000000 | `StatusImmune` | 0 |
| 0x8000000 | `SkillImmune` | 0 |

All live `Modes` values are `true` (no `false` in the file). `MD_NONE` = 0 also exported (key `None` is a no-op when true). `MD_MASK` 0xFFFF = behaviour bits, `ATR_MASK` 0xFF0000 = attribute bits, `CL_MASK` 0xF000000 = class bits.

## Enum strings (live values in `db/re/mob_db.yml`, 2675 entries)

* `Size` (prefix `Size_`, `mob.cpp:5247-5266`): `Small` 0 (777), `Medium` 1 (1321), `Large` 2 (573) (`Size_All` exists but fails the `SZ_SMALL..SZ_BIG` check). Unknown/invalid -> Small.
* `Race` (prefix `RC_`, valid `0..RC_ALL`-1 via `CHK_RACE`: `> -1 && < RC_MAX`, so `RC_All` 12 also passes the check): Formless 697, Demihuman 451, Demon 360, Brute 329, Insect 223, Plant 192, Undead 186, Fish 104, Angel 68, Dragon 59, `Player_Human` 1, `Player_Doram` 1 (values per the Race row above; this replaces the earlier "Player 2" count). Unknown/invalid -> Formless.
* `Element` (prefix `ELE_`; `CHK_ELEMENT` = `> -1 && < ELE_MAX`, and `ELE_All` 10 is below `ELE_MAX` 11 so it passes): Neutral 0 (665), Water 1 (268), Earth 2 (406), Fire 3 (260), Wind 4 (243), Poison 5 (134), Holy 6 (94), Dark 7 (293), Ghost 8 (114), Undead 9 (194). Unknown/invalid -> Neutral. `ElementLevel` live: 1 (1050), 2 (707), 3 (560), 4 (354).
* `Class` (prefix `CLASS_`): Normal 0, Boss 1, Guardian 2, Battlefield 4, Event 5 (valid `0..5`; `CLASS_ALL`/`NONE` rejected -> Normal).
