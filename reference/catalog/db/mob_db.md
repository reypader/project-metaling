# `mob_db.yml` fields used by mob spawn, damage and death

Target `PACKETVER = 20260219`, renewal data file `$RATHENA_PATH/db/re/mob_db.yml` (commit `d4b8e7b8f`; header `Type: MOB_DB`, `Version: 5`; 120900 lines, 5684 `- Id:` entries; there is no `db/import` directory in this checkout). Loader: `MobDatabase::parseBodyNode` (`src/map/mob.cpp:4977-5530`), post-processing `MobDatabase::loadingFinished` (`mob.cpp:5537-5596`), drop post-processing `mob_drop_ratio_adjust` (`mob.cpp:6773-6935`). Generic YAML schema overview: [yaml_schemas.md](./yaml_schemas.md). How the fields are used at runtime: [mechanics/mobs.md](../mechanics/mobs.md).

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
| `Level` | u16 | 1 | `md->level` on spawn unless the spawn line overrides; level penalty input; `clevel` in the entry packets |
| `Hp` | u32 | 1 | `status.max_hp`; scaled at load by `monster_hp_rate` (non-MVP) or `mvp_hp_rate` (MVP) (both default 100); `hp = max_hp` on every spawn |
| `Sp` | u32 | 1 | `max_sp`, minimum 1 |
| `BaseExp` | u64 | 0 | multiplied at load by `base_exp_rate` (100); the full-share value before damage-share and bonuses |
| `JobExp` | u64 | 0 | same with `job_exp_rate` (100) |
| `MvpExp` | u64 | 0 | same with `mvp_exp_rate` (100); only paid to the MVP player when the mob has `Modes: Mvp` |
| `Attack` | u16 | 0 | renewal: base ATK (`rhw.atk`) |
| `Attack2` | u16 | 0 | renewal: base MATK (`rhw.matk`) (pre-renewal: max ATK) |
| `Defense` | int | 0 | `status.def`, clamped to `[DEFTYPE_MIN, DEFTYPE_MAX]` |
| `MagicDefense` | int | 0 | `status.mdef`, same clamp |
| `Resistance`, `MagicResistance` | u16 | 0 | renewal `res` / `mres` |
| `Str`,`Agi`,`Vit`,`Int`,`Dex`,`Luk` | int | 1 each | negative values become 0 |
| `AttackRange` | u16 | 0 | melee/auto-attack range in cells (`rhw.range`) |
| `SkillRange` | u16 | 0 | `range2`: view/aggro range for skill/AI (scaled by `view_range_rate`, default 100) |
| `ChaseRange` | u16 | 0 | `range3`: chase range (>= `range2` after scaling) |
| `Size` | enum | Small | `Size_<name>`: `Small`=0, `Medium`=1, `Large`=2 (`SZ_BIG`). Used for damage tables only; the *rendered* size effect comes from the spawn line, not this field |
| `Race` | enum | Formless | `Formless, Undead, Brute, Plant, Insect, Fish, Demon, Demihuman, Angel, Dragon, Player` (counts in file: Formless 736, Demihuman 459, Demon 368, Brute 335, Insect 230, Plant 198, Undead 186, Fish 110, Angel 68, Dragon 60, Player 2) |
| `RaceGroups` | map name->bool | none | secondary groups (`RC2_*`, e.g. `Goblin`, `Treasure`); `Treasure` switches drop rates to `item_rate_treasure` |
| `Element` | enum | Neutral | `Neutral, Water, Earth, Fire, Wind, Poison, Holy, Dark, Ghost, Undead` |
| `ElementLevel` | 1..4 | 1 | invalid -> 1 |
| `WalkSpeed` | u16 ms/cell | 150 (`DEFAULT_WALK_SPEED`) | becomes `status.speed`, sent verbatim in the entry packets' `speed`; clamped to `[MIN_WALK_SPEED 20, MAX_WALK_SPEED 1000]`. Examples: Poring 400, Scorpion 200, Osiris 100. Smaller = faster; step time = `speed` ms (x14/10 on diagonals) |
| `AttackDelay` | u16 | 100 (`MAX_ASPD_NOPC`) | `adelay` = time between attacks; clamped `[100, 8000]`; raised to at least `AttackMotion` after load |
| `AttackMotion` | u16 | `MAX_ASPD_NOPC/AMOTION_DIVIDER_NOPC` | `amotion`; clamped `[1, 8000 / AMOTION_DIVIDER_NOPC]` |
| `ClientAttackMotion` | u16 | = `AttackMotion` | client-side damage frame; used by `synchronize_damage` for the `0x08c8` `src_speed` |
| `DamageMotion` | u16 | 0 | `dmotion`, scaled by `monster_damage_delay_rate` (100); flinch time, sent as `dmg_speed` in `0x08c8` |
| `DamageTaken` | u16 (percent) | 100 | multiplier on all damage received (`md->damagetaken`; e.g. Osiris 10) |
| `GroupId` | u16 | 0 | sent as `group_id` in `0x0adf` name reply |
| `Title` | string | empty | sent as `title` in `0x0adf` |
| `Ai` | string `01`..`27`, `ABR_*` | `06` | mode bit-set via `MONSTER_TYPE_<ai>` (`mob.hpp:150-182`): `01` = `0x81` (CanMove\|CanAttack, passive), `02` = `0x83` (CanMove\|Looter\|CanAttack; Poring), `06` = `0` (cannot move or attack: plants), `21` = `0x3695`, `25` = `0x1`. Mode bits (`common/mmo.hpp:243-271`): CanMove 0x1, Looter 0x2, Aggressive 0x4, Assist 0x8, CastSensorIdle 0x10, NoRandomWalk 0x20, NoCast 0x40, CanAttack 0x80, Angry 0x800, IgnoreMelee 0x10000, IgnoreMagic 0x20000, IgnoreRanged 0x40000, Mvp 0x80000, IgnoreMisc 0x100000, KnockbackImmune 0x200000, FixedItemDrop 0x1000000, Detector 0x2000000, StatusImmune 0x4000000, SkillImmune 0x8000000. Unknown `Ai` -> `06` |
| `Class` | enum | Normal | `Normal, Boss, Guardian, Battlefield, Event`. `Boss` adds `Detector\|StatusImmune\|KnockbackImmune` modes and makes `is_boss = 1` in entry packets; `Guardian` adds StatusImmune; `Battlefield` adds StatusImmune\|SkillImmune; `Event` adds `FixedItemDrop` (no level-penalty on drops). Counts: Boss 732, Event 79, Battlefield 25, Guardian 4 |
| `Modes` | map `<Mode>: bool` | none | toggles single `MD_*` bits on top of `Ai`/`Class` (e.g. `Mvp: true`, `Detector: true`, `Looter`, `Aggressive`, `Assist`, `NoRandomWalk`, `IgnoreMelee`...). `Mvp` -> `is_boss = 2`, enables MVP reward and boss respawn scaling. If `monster_active_enable: no` the `Aggressive` bit is cleared at load |
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
