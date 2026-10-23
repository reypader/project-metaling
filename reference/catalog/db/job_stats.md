# Job Stats Database: job_stats.yml, job_aspd.yml, job_basepoints.yml

Target `PACKETVER = 20260219`, renewal. Source: `$RATHENA_PATH/db/job_stats.yml` (loader entry) and the renewal files it imports from `db/re/` (commit `d4b8e7b8f`, 2026-10-06). Parsed by `JobDatabase::parseBodyNode` / `loadingFinished` (`src/map/pc.cpp:13700-14440`), type `JOB_STATS` version 4. The data is GPL-3.0 (rAthena): per project rules it may only be copied into `crates/ro-db/data/` by an import tool, never into code. Consumers: [../mechanics/stats.md](../mechanics/stats.md) (HP/SP, weight, ASPD, job bonuses), [../packets/status.md](../packets/status.md). Exp tables inside the same group (`BaseExp`/`JobExp`) and `statpoint.yml`: [exp_tables.md](./exp_tables.md). Generic YAML schema overview: [yaml_schemas.md](./yaml_schemas.md).

## Load order and merge semantics

`db/job_stats.yml` contains only `Header` plus a `Footer: Imports` list (`db/job_stats.yml`, bottom). They are loaded in this order for a renewal build (`Mode: Renewal`; the `Prerenewal` entries are skipped):

1. `db/re/job_stats.yml`: MaxWeight, HpFactor/HpIncrease, SpFactor/SpIncrease, ApFactor/ApIncrease, BonusStats (job-level stat bonuses).
2. `db/re/job_aspd.yml`: `BaseASPD` per weapon type (renewal ASPD is on; if `RENEWAL_ASPD` were off the pre-renewal file would be used, `database.cpp:200-204`).
3. `db/re/job_exp.yml`: `MaxBaseLevel`, `BaseExp`, `MaxJobLevel`, `JobExp`.
4. `db/re/job_basepoints.yml`: `BaseHp`, `BaseSp`, `BaseAp` tables (only when `HP_SP_TABLES` is defined, which it is: `config/core.hpp:49`).
5. `db/re/job_outfits.yml`: `AlternateOutfits`.
6. `db/import/job_stats.yml`: server overrides (absent in this checkout).

All five files share the same layout: `Body` is a list of entries, each with a `Jobs:` map (`JobName: true`, name resolves to constant `JOB_<Name>`) and any subset of keys. A job that appears in several entries (or several files) is MERGED: a key present later overrides that key only; absent keys keep earlier values. Defaults for a brand-new job: `MaxWeight` 20000, `HpFactor` 0, `HpIncrease` 500, `SpFactor` 0, `SpIncrease` 100, `ApFactor` 0, `ApIncrease` 0, `MaxBaseLevel`/`MaxJobLevel` = `MAX_LEVEL`, `BaseASPD` 2000 / 10 = 200 per weapon (a weapon absent from the table has ASPD base 200, effectively unusable).

## Per-job fields

| Key | Meaning | Used by |
| :--- | :--- | :--- |
| `MaxWeight` | base max carry weight in 0.1 units (28000 = 2800.0) | `max_weight = MaxWeight + status.str*300` ([stats.md](../mechanics/stats.md#max-weight-and-overweight)) |
| `HpFactor`, `HpIncrease`, `SpFactor`, `SpIncrease`, `ApFactor`, `ApIncrease` | fallback HP/SP/AP formula inputs | only for base levels with no `BaseHp/BaseSp/BaseAp` entry ([stats.md](../mechanics/stats.md#maxhp--maxsp)) |
| `BaseHp` / `BaseSp` / `BaseAp` | `- Level: L` / `Hp`/`Sp`/`Ap: N`: table of base HP/SP/AP at base level L | the `base_hp[level-1]` input of MaxHP/MaxSP |
| `BaseASPD` | weapon name -> base ASPD penalty value (smaller = faster), plus `Shield` | `status_base_amotion_pc` ([stats.md](../mechanics/stats.md#aspd-renewal)) |
| `BonusStats` | `- Level: L` plus any of `Str Agi Vit Int Dex Luk Pow Sta Wis Spl Con Crt` (int16, may be negative) | job-level stat bonuses |
| `MaxStats` | per-stat cap overrides; EMPTY in this DB, class caps come from `battle.conf` ([stats.md](../mechanics/stats.md#max-stat-per-class)) | `pc_maxparameter` |
| `MaxBaseLevel`, `BaseExp`, `MaxJobLevel`, `JobExp` | see [exp_tables.md](./exp_tables.md) | exp |
| `AlternateOutfits` | extra outfit job ids (cosmetic) | |

Level numbering: all `Level:` keys are 1-based. A `BaseHp` table longer than `MaxBaseLevel` is truncated. After load, every job with an unset (0) table slot is filled by the fallback formula; trailing slots above `MaxBaseLevel` are dropped.

`BonusStats` are stored per job level then ACCUMULATED at load (`pc.cpp:14310-14325`): `job_bonus[L-1].stat` is the sum of all entries with level <= L, for every level up to `MaxJobLevel`. So the YAML lists only the levels where a point is gained, and runtime reads one cumulative row indexed by the character's current job level. A Novice at job level 10 has +1 each of LUK, DEX, AGI, VIT, STR, INT.

BonusStats example, Novice group (`db/re/job_stats.yml:88-102`): Level 2 Luk+1, 3 Dex+1, 5 Agi+1, 6 Vit+1, 8 Str+1, 9 Int+1. Swordman first class gains 18 rows (job level 2 Str, 6 Vit, 10 Dex, 14 Str, 18 Vit, 22 Dex, 26 Luk, 30 Agi, 33 Str, 36 Dex, 38 Vit, 40 Str, 42 Vit, 44 Luk, 46 Agi, 47 Str, 49 Str, 50 Str). Row counts per group are in the next table. Trait stats (Pow..Crt) appear only in 4th-class groups.

## Job groups: weight and fallback HP/SP parameters

Generated from `db/re/job_stats.yml` (one row per entry; `+N` counts further aliases in the same entry; Baby_* variants of each job share the row; 2nd-class riding variants such as Knight2 share the base values). Rows with `20000 (default)` have no explicit `MaxWeight`.

| Jobs (first names of the group) | MaxWeight | HpFactor | HpIncrease | SpFactor | SpIncrease | BonusStats rows |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| Novice | 20000 (default) | 0 | 500 | 0 | 100 | 6 |
| Swordman, Swordman_High | 28000 | 70 | 500 | 0 | 200 | 18 |
| Mage, Mage_High | 22000 | 30 | 500 | 0 | 600 | 18 |
| Archer, Archer_High | 26000 | 50 | 500 | 0 | 200 | 18 |
| Acolyte, Acolyte_High | 24000 | 40 | 500 | 0 | 500 | 18 |
| Merchant, Merchant_High | 28000 | 40 | 500 | 0 | 300 | 18 |
| Thief, Thief_High | 24000 | 50 | 500 | 0 | 200 | 18 |
| Knight, Knight2 | 28000 | 150 | 500 | 0 | 300 | 30 |
| Priest | 26000 | 75 | 500 | 0 | 800 | 30 |
| Wizard | 24000 | 55 | 500 | 0 | 900 | 30 |
| Blacksmith | 30000 | 90 | 500 | 0 | 400 | 30 |
| Hunter | 27000 | 85 | 500 | 0 | 400 | 30 |
| Assassin | 24000 | 110 | 500 | 0 | 400 | 30 |
| Crusader, Crusader2 | 28000 | 110 | 700 | 0 | 470 | 30 |
| Monk | 26000 | 90 | 650 | 0 | 470 | 30 |
| Sage | 24000 | 75 | 500 | 0 | 700 | 30 |
| Rogue | 24000 | 85 | 500 | 0 | 500 | 30 |
| Alchemist | 30000 | 90 | 500 | 0 | 400 | 30 |
| Bard | 27000 | 75 | 300 | 0 | 600 | 30 |
| Dancer | 27000 | 75 | 300 | 0 | 600 | 30 |
| Super_Novice | 20000 (default) | 0 | 500 | 0 | 100 | 30 |
| Gunslinger | 28000 | 90 | 650 | 0 | 469 | 24 |
| Ninja | 26000 | 75 | 500 | 0 | 540 | 25 |
| Novice_High | 20000 (default) | 0 | 500 | 0 | 100 | 6 |
| Lord_Knight, Lord_Knight2 | 28000 | 150 | 500 | 0 | 300 | 45 |
| High_Priest | 26000 | 75 | 500 | 0 | 800 | 45 |
| High_Wizard | 24000 | 55 | 500 | 0 | 900 | 45 |
| Whitesmith | 30000 | 90 | 500 | 0 | 400 | 45 |
| Sniper | 27000 | 85 | 500 | 0 | 400 | 45 |
| Assassin_Cross | 24000 | 110 | 500 | 0 | 400 | 45 |
| Paladin, Paladin2 | 28000 | 110 | 700 | 0 | 470 | 45 |
| Champion | 26000 | 90 | 650 | 0 | 470 | 45 |
| Professor | 24000 | 75 | 500 | 0 | 700 | 45 |
| Stalker | 24000 | 85 | 500 | 0 | 500 | 45 |
| Creator | 30000 | 90 | 500 | 0 | 400 | 45 |
| Clown | 27000 | 75 | 300 | 0 | 600 | 45 |
| Gypsy | 27000 | 75 | 300 | 0 | 600 | 45 |
| Baby | 20000 (default) | 0 | 500 | 0 | 100 | 0 |
| Super_Baby | 20000 (default) | 0 | 500 | 0 | 100 | 30 |
| Taekwon | 28000 | 70 | 500 | 0 | 200 | 18 |
| Star_Gladiator, Star_Gladiator2 | 28000 | 90 | 650 | 0 | 470 | 30 |
| Soul_Linker | 24000 | 75 | 500 | 0 | 900 | 30 |
| Rune_Knight, Rune_Knight2, Rune_Knight_T, Rune_Knight_T2 +2 | 35000 | 150 | 500 | 0 | 300 | 43 |
| Warlock, Warlock_T | 30000 | 55 | 500 | 0 | 900 | 43 |
| Ranger, Ranger2, Ranger_T, Ranger_T2 +2 | 32000 | 85 | 500 | 0 | 400 | 43 |
| Arch_Bishop, Arch_Bishop_T | 30000 | 75 | 500 | 0 | 800 | 43 |
| Mechanic, Mechanic2, Mechanic_T, Mechanic_T2 +2 | 38000 | 90 | 500 | 0 | 400 | 43 |
| Guillotine_Cross, Guillotine_Cross_T | 32000 | 110 | 500 | 0 | 400 | 43 |
| Royal_Guard, Royal_Guard2, Royal_Guard_T, Royal_Guard_T2 +2 | 35000 | 110 | 700 | 0 | 400 | 43 |
| Sorcerer, Sorcerer_T | 30000 | 75 | 500 | 0 | 900 | 43 |
| Minstrel, Minstrel_T | 32000 | 75 | 300 | 0 | 400 | 43 |
| Wanderer, Wanderer_T | 32000 | 75 | 300 | 0 | 400 | 43 |
| Sura, Sura_T | 30000 | 90 | 650 | 0 | 400 | 43 |
| Genetic, Genetic_T | 32000 | 90 | 500 | 0 | 900 | 43 |
| Shadow_Chaser, Shadow_Chaser_T | 28000 | 85 | 500 | 0 | 400 | 43 |
| Super_Novice_E | 20000 (default) | 0 | 5000 | 0 | 500 | 44 |
| Super_Baby_E | 20000 (default) | 0 | 500 | 0 | 100 | 44 |
| Kagerou, Oboro | 26000 | 75 | 500 | 0 | 540 | 35 |
| Rebellion | 28000 | 90 | 650 | 0 | 469 | 43 |
| Summoner | 20000 (default) | 100 | 800 | 0 | 250 | 38 |
|  | 20000 (default) | 100 | 700 | 0 | 200 | 38 |
| Star_Emperor, Star_Emperor2 | 28000 | 90 | 650 | 0 | 470 | 43 |
| Soul_Reaper | 28000 | 75 | 500 | 0 | 900 | 43 |
| Dragon_Knight, Dragon_Knight2 | 45000 | 68 | 5828 | 7 | 14 | 51 |
| Meister, Meister2 | 48000 | 76 | 4073 | 3 | 552 | 56 |
| Shadow_Cross | 42000 | 64 | 5398 | 7 | 4 | 56 |
| Arch_Mage | 40000 | 91 | 1075 | 6 | 438 | 56 |
| Cardinal | 40000 | 94 | 1047 | 5 | 530 | 57 |
| Windhawk, Windhawk2 | 42000 | 93 | 1121 | 6 | 112 | 56 |
| Imperial_Guard, Imperial_Guard2 | 45000 | 94 | 2767 | 3 | 504 | 56 |
| Biolo | 42000 | 104 | 22 | 0 | 890 | 56 |
| Abyss_Chaser | 42000 | 64 | 5503 | 7 | 11 | 56 |
| Elemental_Master | 40000 | 104 | 40 | 9 | 61 | 57 |
| Inquisitor | 42000 | 90 | 2633 | 7 | 33 | 56 |
| Troubadour | 42000 | 95 | 1119 | 8 | 3 | 56 |
| Trouvere | 42000 | 95 | 1119 | 8 | 3 | 56 |
| Sky_Emperor, Sky_Emperor2 | 42000 | 72 | 6050 | 3 | 469 | 51 |
| Soul_Ascetic | 42000 | 94 | 1058 | 6 | 400 | 51 |
| Shinkiro | 45000 | 85 | 4496 | 3 | 474 | 58 |
| Shiranui | 45000 | 85 | 4496 | 3 | 474 | 57 |
| Night_Watch | 48000 | 66 | 5603 | 9 | 16 | 54 |
| Hyper_Novice | 40000 | 38 | 5584 | 6 | 40 | 57 |
| Spirit_Handler | 42000 | 16 | 11979 | 7 | 84 | 57 |

Observations: transcendent (High) classes share their base class row (Lord_Knight with Knight etc.) and get x1.25 MaxHP/MaxSP at runtime ([stats.md](../mechanics/stats.md#maxhp--maxsp)). Summoner (Doram) classes have default weight 20000 and their own HP/SP parameters. 4th classes carry large `HpIncrease` and non-zero `SpFactor` and have no `BaseHp/BaseSp` tables (see below), so for them the fallback formula IS the runtime HP/SP source.

## `job_basepoints.yml`: BaseHp / BaseSp / BaseAp tables

`db/re/job_basepoints.yml`: 89 entries. 45 entries carry `BaseHp`, 43 carry `BaseSp`, 1 carries `BaseAp`. Shape per entry: `Jobs`, then `BaseHp:` (list of `- Level: L` / `Hp: N`) and/or `BaseSp:` (`Sp: N`). Examples (value at level, from the file):

| Entry (first jobs) | Table range | HP L1 | HP L10 | HP L99 | HP L150 | HP last | SP L1 | SP L99 |
| :--- | :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Novice, Novice_High, Baby | HP 1..99 (SP 1..200 shared with Super Novice) | 40 | 85 | 530 | - | 530 | 11 | 109 |
| Super_Novice (+Baby, E variants) | HP 1..200 | 40 | 85 | 2530 | 4785 | 5035 (L200) | 11 | 109 |
| Swordman, High, Baby | 1..99 | 40 | 123 | 3999 | - | 3999 | 12 | 208 |
| Mage | 1..99 | 40 | 102 | 2020 | - | 2020 | 16 | 604 |
| Knight, Lord_Knight | 1..99 | 40 | 168 | 7978 | - | 7978 | 13 | 307 |
| Rune_Knight (3rd class family) | 99..200 | - | - | 8100 | 14883 | 22875 (L200) | - | 300 |
| Warlock (3rd class family) | 99..200 | - | - | 3200 | 7669 | 15625 | - | 900 |
| Summoner, Baby_Summoner | HP 1..200, SP 1..200 | 40 | 185 | 5999 | 15645 | 32955 (L200) | 8 | 253 |
| Gunslinger, Rebellion (SP only, shared) | SP 1..200 | - | - | - | - | - | 12 | 456 |

* First-job-to-second-job families: one entry covers 1st, 2nd, transcendent and baby versions of the same line; the 1.25x transcendent multiplier and the baby flag are applied in code, not in the table. 2nd-class entries cover base levels 1..99; for the High (upper) classes the same table is used with level <= 99.
* 3rd-class entries start at base level 99 and run to 200 (102 entries). For levels 1..98 of a 3rd class, no table slot exists and the fallback formula is used (practically unreachable: classes change at 99+).
* 4th classes (Dragon_Knight ... Spirit_Handler) have no `BaseHp`/`BaseSp`; `BaseAp` has one entry, level 200..275 AP = 200 (all 4th classes sharing it).

## `job_aspd.yml`: BaseASPD per weapon type (renewal)

`db/re/job_aspd.yml`, 624 lines. Value = base ASPD penalty (`aspd_base[weapon]`), subtracted (capped at 200) from the stat-derived ASPD ([stats.md](../mechanics/stats.md#aspd-renewal)). Smaller means faster. `Shield` is the extra penalty added when a shield is equipped. Weapons not listed for a job default to 200 (very slow). Weapon names are rAthena's `W_*` suffixes: `Fist, Dagger, 1hSword, 2hSword, 1hSpear, 2hSpear, 1hAxe, 2hAxe, Mace, 2hMace, Staff, Bow, Knuckle, Musical, Whip, Book, Katar, Revolver, Rifle, Gatling, Shotgun, Grenade, Huuma, 2hStaff`.

| Jobs (non-baby; aliases counted) | BaseASPD |
| :--- | :--- |
| Novice, Super_Novice, Novice_High +5 | Fist 40, Dagger 55, 1hSword 57, 1hAxe 50, Mace 50, 2hMace 55, Staff 65, 2hStaff 65, Shield 10 |
| Swordman, Swordman_High | Fist 40, Dagger 47, 1hSword 47, 2hSword 54, 1hSpear 57, 2hSpear 65, 1hAxe 55, 2hAxe 60, Mace 50, 2hMace 55, Shield 5 |
| Mage, Mage_High | Fist 50, Dagger 50, Staff 55, 2hStaff 55, Shield 10 |
| Archer, Archer_High | Fist 40, Dagger 55, Bow 50, Shield 9 |
| Acolyte, Acolyte_High | Fist 40, Mace 45, 2hMace 50, Staff 60, 2hStaff 60, Shield 7 |
| Merchant, Merchant_High | Fist 40, Dagger 52, 1hSword 52, 1hAxe 48, 2hAxe 55, Mace 50, 2hMace 55, Shield 5 |
| Thief, Thief_High | Fist 40, Dagger 48, 1hSword 50, 1hAxe 60, Bow 53, Shield 6 |
| Knight, Knight2, Lord_Knight +3 | Fist 40, Dagger 49, 1hSword 45, 2hSword 52, 1hSpear 55, 2hSpear 60, 1hAxe 50, 2hAxe 55, Mace 45, 2hMace 50, Shield 5 |
| Priest, High_Priest | Fist 40, Mace 43, 2hMace 48, Staff 60, Knuckle 60, Book 44, 2hStaff 60, Shield 5 |
| Wizard, High_Wizard | Fist 50, Dagger 54, Staff 53, 2hStaff 53, Shield 8 |
| Blacksmith, Whitesmith | Fist 40, Dagger 50, 1hSword 50, 1hAxe 46, 2hAxe 53, Mace 48, 2hMace 53, Shield 5 |
| Hunter, Sniper | Fist 40, Dagger 53, Bow 48, Shield 9 |
| Assassin, Assassin_Cross | Fist 40, Dagger 42, 1hSword 50, 1hAxe 51, Katar 42, Huuma 110, Shield 6 |
| Crusader, Crusader2, Paladin +3 | Fist 40, Dagger 48, 1hSword 43, 2hSword 55, 1hSpear 53, 2hSpear 52, 1hAxe 50, 2hAxe 55, Mace 45, 2hMace 50, Shield 5 |
| Monk, Champion | Fist 40, Mace 43, 2hMace 48, Staff 60, Knuckle 40, 2hStaff 58, Shield 5 |
| Sage, Professor | Fist 45, Dagger 53, 1hSword 60, Staff 55, Book 43, 2hStaff 55, Shield 5 |
| Rogue, Stalker | Fist 40, Dagger 45, 1hSword 50, Bow 50, Shield 5 |
| Alchemist, Creator | Fist 40, Dagger 50, 1hSword 45, 1hAxe 45, 2hAxe 52, Mace 45, 2hMace 50, Shield 4 |
| Bard, Clown | Fist 40, Dagger 53, Bow 48, Musical 45, Shield 7 |
| Dancer, Gypsy | Fist 40, Dagger 53, Bow 48, Whip 45, Shield 7 |
| Gunslinger | Fist 50, Revolver 45, Rifle 55, Gatling 50, Shotgun 90, Grenade 100, Shield 6 |
| Ninja | Fist 40, Dagger 43, Huuma 55, Shield 6 |
| Taekwon | Fist 40, Shield 6 |
| Star_Gladiator, Star_Gladiator2 +1 | Fist 40, Book 50, Shield 6 |
| Soul_Linker | Fist 50, Dagger 50, Staff 53, 2hStaff 55, Shield 8 |
| Rune_Knight, Rune_Knight2, Rune_Knight_T +5 | Fist 40, Dagger 50, 1hSword 52, 2hSword 55, 1hSpear 60, 2hSpear 58, 1hAxe 48, 2hAxe 52, Mace 45, 2hMace 52, Shield 5 |
| Warlock, Warlock_T, Arch_Mage +1 | Fist 45, Dagger 52, 1hSword 60, Staff 50, 2hStaff 56, Shield 5 |
| Ranger, Ranger2, Ranger_T +5 | Fist 40, Dagger 50, Bow 49, Shield 8 |
| Arch_Bishop, Arch_Bishop_T, Cardinal +1 | Fist 45, Mace 45, 2hMace 45, Staff 60, Knuckle 50, Book 44, 2hStaff 55, Shield 5 |
| Mechanic, Mechanic2, Mechanic_T +5 | Fist 40, Dagger 60, 1hSword 65, 1hAxe 45, 2hAxe 48, Mace 48, 2hMace 50, Shield 6 |
| Guillotine_Cross, Guillotine_Cross_T, Shadow_Cross +1 | Fist 40, Dagger 42, 1hSword 65, 1hAxe 80, Katar 42, Rifle 95, Gatling 120, Shotgun 90, Grenade 100, Huuma 110, Shield 9 |
| Royal_Guard, Royal_Guard2, Royal_Guard_T +5 | Fist 40, Dagger 47, 1hSword 45, 2hSword 53, 1hSpear 50, 2hSpear 50, 1hAxe 48, 2hAxe 52, Mace 44, 2hMace 50, Shield 5 |
| Sorcerer, Sorcerer_T, Elemental_Master +1 | Fist 40, Dagger 50, 1hSword 50, Staff 45, Book 45, 2hStaff 55, Shield 5 |
| Minstrel, Minstrel_T, Troubadour +1 | Fist 40, Dagger 52, Bow 49, Musical 44, Shield 7 |
| Wanderer, Wanderer_T, Trouvere +1 | Fist 40, Dagger 52, Bow 49, Whip 44, Shield 7 |
| Sura, Sura_T, Inquisitor +1 | Fist 38, Mace 43, 2hMace 45, Staff 48, Knuckle 39, 2hStaff 50, Shield 5 |
| Genetic, Genetic_T, Biolo +1 | Fist 40, Dagger 50, 1hSword 44, 1hAxe 48, 2hAxe 51, Mace 44, 2hMace 48, Shield 4 |
| Shadow_Chaser, Shadow_Chaser_T, Abyss_Chaser +1 | Fist 40, Dagger 43, 1hSword 47, Bow 47, Shield 4 |
| Kagerou, Oboro, Shinkiro +3 | Fist 40, Dagger 45, Huuma 50, Shield 3 |
| Rebellion, Night_Watch | Fist 45, Revolver 50, Rifle 55, Gatling 48, Shotgun 75, Grenade 80, Shield 10 |
| Summoner, Spirit_Handler | Fist 40, Staff 60, Shield 7 |
| Star_Emperor, Star_Emperor2, Sky_Emperor +3 | Fist 40, Book 45, Shield 3 |
| Soul_Reaper, Soul_Ascetic | Fist 45, Dagger 40, Staff 50, 2hStaff 52, Shield 5 |

## Open questions / not analysed

* Per-job `MaxBaseLevel`/`MaxJobLevel` come from `job_exp.yml`; read them via [exp_tables.md](./exp_tables.md).
* `job_outfits.yml` (`AlternateOutfits`) and `db/import/job_stats.yml` overrides are not catalogued.
* `statpoint.yml` content is in [exp_tables.md](./exp_tables.md#statpointyml); this file only records how it is combined with stat costs ([stats.md](../mechanics/stats.md#status-points-per-level-and-reset)).
