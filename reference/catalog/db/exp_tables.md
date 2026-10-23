# Exp Tables: job_exp.yml, statpoint.yml, level_penalty.yml

Target `PACKETVER = 20260219`, renewal. Paths under `$RATHENA_PATH/db/re/` (renewal DB; the loader reads `db/re` when `RENEWAL` is defined, with `db/import/` overrides). Gameplay use: [../mechanics/experience.md](../mechanics/experience.md). Data is GPL-3.0 (rAthena); per project rules it may only be copied by an import tool into `crates/ro-db/data/`, never into code.

## job_exp.yml

Source: `db/re/job_exp.yml`, 3816 lines, header `Type: JOB_STATS`, shared with the other job files read by `JobDatabase`.

Structure (field doc at lines 22-86, `Body:` at 86): `Body` is a list of group entries. Every entry is one of two shapes:

* Base-exp group: `- Jobs:` (map of `JobName: true`), `MaxBaseLevel: N`, `BaseExp:` list of `- Level: L` / `Exp: X`.
* Job-exp group: `- Jobs:` (map), `MaxJobLevel: N`, `JobExp:` list of `- Level: L` / `Exp: X`.

Semantics (`pc.cpp:8640-8676`, `JobDatabase::get_baseExp/get_jobExp`): `Exp` at `Level L` is the exp needed to go from level L to L+1 (array index `L-1`). The entry at the max level is never consumed as a requirement: at max level `pc_nextbaseexp` returns the constant 99999999 and `pc_nextjobexp` 999999999 regardless of the table's last value (`config/const.hpp:73-75`), and stored exp is clamped to that. Exp values are 64-bit (`t_exp`); the Doram/4th tables exceed 32 bits. A class's max levels come from its group (`pc_maxbaselv/pc_maxjoblv`, via `job_db`, keyed by class id).

### Base-exp groups (line = start of group)

| Line | Jobs (count) | MaxBaseLevel | Level 1 | Level 2 | Level 10 | Level 50 | Last (max) |
| ---: | :--- | ---: | ---: | ---: | ---: | ---: | ---: |
| 88 | Novice, 1st, 2nd, Super Novice, Gunslinger, Ninja, Taekwon family, Baby variants, etc. (61) | 99 | 548 | 894 | 5337 | 42165 | L98 1252761; L99 9999999 |
| 350 | Novice_High, 1st_High, Lord_Knight and other transcendent (22) | 99 | 658 | 1073 | - | - | L98 1258714; L99 9999999 |
| 573 | Rune_Knight and other 3rd classes, Kagerou/Oboro, Rebellion, Star Emperor, Soul Reaper etc. (65) | 200 | 658 | 1073 | 6404 | 59031 | L99 1272747; L100 1354202; L150 48508204; L199 651094164; L200 999999999 |
| 1041 | Dragon_Knight, Meister, Shadow_Cross, Arch_Mage, Cardinal, Windhawk, ... 4th classes (25) | 275 | 658 | 1073 | 6404 | 59031 | L199 651094164; L200 653047446; L274 430803653341; L275 999999999999 |
| 1619 | Summoner (Doram), Baby_Summoner (2) | 200 | 55 | 90 | 600 | 43200 | L199 708463491; L200 999999999 |

Because the 3rd and 4th class tables share the first 199 values, base exp does NOT restart when changing class; but note transcendent and Summoner tables differ at low levels.

### Job-exp groups

| Line | Jobs | MaxJobLevel | Level 1 | Level 2 | Last two (L-1, L) |
| ---: | :--- | ---: | ---: | ---: | :--- |
| 2024 | Novice, Baby | 10 | 10 | 18 | 340, 999 |
| 2049 | Novice_High | 10 | 11 | 20 | 374, 999 |
| 2073 | 1st classes + Baby 1st (15) | 50 | 60 | 130 | 97180, 99999 |
| 2191 | 1st_High (6) | 50 | 100 | 200 | 61300, 99999 |
| 2300 | 2nd classes (incl. alt "2" jobs, Baby 2nd; 34) | 50 | 2500 | 4200 | 345400, 999999 |
| 2437 | Transcendent 2nd (Lord_Knight, Paladin ... 15) | 70 | 1354 | 1624 | 974343, 999999 |
| 2595 | 3rd classes and trans-3rd, Baby 3rd (51) | 70 | 12800 | 16384 | 924220092, 999999999 |
| 2789 | 4th classes (25) | 60 | 4700000 | 5522500 | 13194183036, 99999999999 |
| 2937 | Gunslinger, Ninja, Baby variants (4) | 70 | 200 | 300 | 308000, 999999 |
| 3084 | Star_Gladiator (+2, Baby) (4) | 50 | 50700 | 50700 | 690800, 999999 |
| 3191 | Super_Novice, Super_Baby (2) | 99 | 60 | 130 | 1035400, 9999999 |
| 3394 | Super_Novice_E, Super_Baby_E (2) | 70 | 12800 | 16384 | 924220092, 999999999 |
| 3539 | Kagerou, Oboro, Rebellion, Star_Emperor, Soul_Reaper + Baby (12) | 70 | 12800 | 16384 | 924220092, 999999999 |
| 3694 | Summoner, Baby_Summoner (2) | 60 | 60 | 130 | 132528619, 999999999 |

Notes: Novice max job level is 10 (job exp table has 10 rows; this is the "novice cap"), 1st classes 50. Exact row values for every level are in the file; table lengths equal the Max*Level of the group. Group line numbers are for the checked-out rAthena commit; re-extract with the import tool rather than relying on them.

The per-job non-exp fields (MaxWeight, HpFactor, BaseASPD, BonusStats, MaxStats, BaseHp/Sp/Ap, AlternateOutfits) are documented in the header of `job_exp.yml` (lines 22-86) but live in the sibling files `job_stats.yml`, `job_aspd.yml`, `job_basepoints.yml`; `job_exp.yml` itself holds only exp tables (this was checked by structure scan, not exhaustively).

## statpoint.yml

Source: `db/re/statpoint.yml`, 659 lines, `Type: STATPOINT_DB`, `Version: 2`.

* Fields: `Level`, `Points` (cumulative status points granted from base level 1 up to and including `Level`), optional `TraitPoints` (cumulative trait points, only present from level 201). 275 rows (levels 1-275).
* Points granted on a level-up from L to L+1 are `Points[L+1] - Points[L]` (`PlayerStatPointDatabase::pc_gets_status_point`, `pc.cpp:8755-8765`); `0` if the table does not increase. Same for trait points (`pc_gets_trait_point`, 0 below level 201).
* Samples: L1 = 48 (also the starting status-point budget used by stat reset), L2 = 51, L3 = 54, L4 = 57, L5 = 60, L6 = 64, L10 = 80, L99 = 1273, L100 = 1295, L175 = 3278, L176 = 3309, L200 = 4099, L201..L275 = 4099 (flat: no more status points above 200). `TraitPoints`: L201 = 3, L202 = 6, L203 = 9 (+3 per level), L275 = 285.
* `use_statpoint_table` / `use_traitpoint_table` (`exp.conf`, default yes) choose table vs formula for resets and level-up; the table path is the one documented here.
* Skill points: no table. +1 `skill_point` per job level gained (`pc_checkjoblevelup`, `pc.cpp:8332`). There is no skill-point grant on base level-up.

## Renewal level-difference exp penalty

`db/re/level_penalty.yml`, `Type: PENALTY_DB`, loader `PenaltyDatabase` (`pc.cpp:526-625`). Two types: `Exp` (applied to kill exp, percent) and `Drop` (item drop rate, not covered here; breakpoints 16:50, 13:60, 10:70, 7:80, 4:90, -4:90, -7:80, -10:70, -13:60, -16:50). `Difference` = `monster_level - player_base_level`.

`Exp` breakpoints (percent): 16:40, 15:115, 14:120, 13:125, 12:130, 11:135, 10:140, 9:135, 8:130, 7:125, 6:120, 5:115, 4:110, 3:105, -6:95, -11:90, -16:85, -21:60, -26:35, -31:10.

Fill rules after load (`loadingFinished`, `pc.cpp:593-619`): undefined differences from 0 upward inherit the previous defined rate going up (starting at 100 at difference 0), and from 0 downward inherit going down (starting at 100). Resulting effective table for `Exp`:

| Difference (mob lv - player lv) | Rate % |
| :--- | ---: |
| 0 to +2, and 0 to -5 | 100 |
| +3 | 105 |
| +4 | 110 |
| +5 | 115 |
| +6 | 120 |
| +7 | 125 |
| +8 | 130 |
| +9 | 135 |
| +10 | 140 |
| +11 | 135 |
| +12 | 130 |
| +13 | 125 |
| +14 | 120 |
| +15 | 115 |
| +16 and higher | 40 |
| -6 to -10 | 95 |
| -11 to -15 | 90 |
| -16 to -20 | 85 |
| -21 to -25 | 60 |
| -26 to -30 | 35 |
| -31 and lower | 10 |

Lookup index is `difference + MAX_LEVEL - 1` with `MAX_LEVEL = 275` (`map.hpp:78`), so |difference| beyond 274 would be out of range in the source; clamp. A type absent from the yml (e.g. MVP exp) returns 100.

## Persistence cross-link

Character exp/level columns: [sql_schemas.md](./sql_schemas.md) (`char.base_level`, `job_level`, `base_exp`, `job_exp`, `status_point`, `skill_point`, `trait_point`). `base_exp`/`job_exp` are `BIGINT UNSIGNED`, matching the 64-bit `t_exp`.
