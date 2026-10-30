# Exp Tables: job_exp.yml, statpoint.yml, level_penalty.yml

Target `PACKETVER = 20260219`, renewal. Paths under `$RATHENA_PATH/db/re/` (renewal DB; the loader reads `db/re` when `RENEWAL` is defined, with `db/import/` overrides). Gameplay use: [../mechanics/experience.md](../mechanics/experience.md). Data is GPL-3.0 (rAthena); per project rules it may only be copied by an import tool into `crates/ro-db/data/`, never into code.

## job_exp.yml

Source: `db/re/job_exp.yml`, 3816 lines, header `Type: JOB_STATS`, shared with the other job files read by `JobDatabase`.

Structure (field doc at lines 22-86, `Body:` at 86): `Body` is a list of group entries. Every entry is one of two shapes:

* Base-exp group: `- Jobs:` (map of `JobName: true`), `MaxBaseLevel: N`, `BaseExp:` list of `- Level: L` / `Exp: X`.
* Job-exp group: `- Jobs:` (map), `MaxJobLevel: N`, `JobExp:` list of `- Level: L` / `Exp: X`.

Semantics (`pc.cpp:8640-8676`, `JobDatabase::get_baseExp/get_jobExp`): `Exp` at `Level L` is the exp needed to go from level L to L+1 (array index `L-1`). The entry at the max level is never consumed as a requirement: at max level `pc_nextbaseexp` returns the constant 99999999 and `pc_nextjobexp` 999999999 regardless of the table's last value (`config/const.hpp:73-75`), and stored exp is clamped to that. Exp values are 64-bit (`t_exp`); the Doram/4th tables exceed 32 bits. A class's max levels come from its group (`pc_maxbaselv/pc_maxjoblv`, via `job_db`, keyed by class id).

### Lookup rule: (class id, level) to required exp

Source: `pc.cpp:8566-8676` (`get_maxBaseLv`, `get_maxJobLv`, `pc_is_max*lv`, `get_baseExp`, `get_jobExp`, `pc_nextbaseexp`, `pc_nextjobexp`), loader `JobDatabase::parseBodyNode` `pc.cpp:13819-13870, 14029-14110`; applies to 20260219.

* Each `Jobs:` key becomes a class id by the constant `JOB_<Name>` (upper-cased by the script constant table, e.g. `Rune_Knight_T` is `JOB_RUNE_KNIGHT_T`); an unknown name aborts the entry. The group's `MaxBaseLevel` / `MaxJobLevel` and the `Level`/`Exp` list are copied to EVERY listed class's own record (`base_exp[level-1] = Exp`, `job_exp[level-1] = Exp`). There is no shared group at runtime: the lookup key is the character's own `status.class_`, a flat per-class array of `MAX_LEVEL` (275) 64-bit slots, zero-initialised.
* `pc_nextbaseexp(sd)`: `base_level == 0` returns 0; else if `base_level >= max_base_level(class)` returns `99999999`; else `base_exp[class][base_level - 1]`. `pc_nextjobexp` is identical with `job_level`, `max_job_level`, `999999999` and `job_exp`. So the requirement for L to L+1 is `Exp` of the row `Level: L`, i.e. array index `L-1`, and it is only read for `1 <= L < Max`.
* Level 0: returns 0 (no level-up possible; the guard exists so index -1 is never read). A level at or above the max: the constants above. A level above the table but below the max cannot happen (tables are full length), and a class with no record (not in the file, e.g. 22 `WEDDING`, 26-30 costumes, or any unmapped id) has max level 0, so it counts as "at max" and returns the constants; `get_*Exp` returns 0 for an unknown class. A missing row inside a record stays 0, and a next-exp of 0 stops the level-up loop (`pc_check*levelup`: `!next` returns 0).
* Rows whose `Level` exceeds the group's Max are skipped (`continue`); `Level` 0 or above 275 aborts the entry. `Max*Level` 0 or above 275 is capped to 275 with a warning; if absent, default 275 (only for a class first seen in this entry).
* After the player's class changes the lookup simply uses the new class's arrays with the (reset) levels; at login a `base_level` above the class max is clamped to the max (`pc.cpp:9351-9354`) and max-level exp above the constant is clamped (`pc.cpp:2294-2300`).
* `client_exp(x)` (`clif.cpp:69-71`): `(int64) min(x, MAX_EXP)` with `MAX_EXP = INT64_MAX` (`config/const.hpp:65`); it is the only clamp on the wire value (the 32-bit variant, not used at 20260219, casts to int32).
* Job names covered. The file lists 175 distinct names; each name appears once among the 5 base-exp groups and once among the 14 job-exp groups. 148 of them are in the importer's name table `tools/job-import/src/job_ids.rs` (`JOB_IDS`, 154 names). The other 27 are the mounted / alternate "2" variants, which `JOB_IDS` leaves out on purpose: `Knight2`(13), `Crusader2`(21), `Lord_Knight2`, `Paladin2`, `Baby_Knight2`, `Baby_Crusader2`, `Star_Gladiator2`, `Baby_Star_Gladiator2`, `Rune_Knight2`, `Rune_Knight_T2`, `Royal_Guard2`, `Royal_Guard_T2`, `Ranger2`, `Ranger_T2`, `Mechanic2`, `Mechanic_T2`, `Baby_{Rune_Knight,Royal_Guard,Ranger,Mechanic}2`, `Star_Emperor2`, `Baby_Star_Emperor2`, `Windhawk2`, `Meister2`, `Dragon_Knight2`, `Imperial_Guard2`, `Sky_Emperor2` (ids: [../mechanics/equipment.md](../mechanics/equipment.md) unmapped list). These ids are never a stored class id (login resets them to Novice), so for exp they are unreachable and skipping them is correct: each "2" variant sits in the same group as its base class (same Max, same values). The 6 names in `JOB_IDS` but absent from the file (`Wedding`, `Xmas`, `Summer`, `Hanbok`, `Oktoberfest`, `Summer2`) get no exp record (the loader exempts them from the missing-table warning, `pc.cpp:14274`); the importer falls back to `MAX_LEVEL` 275 for them. So the same name table is sufficient for `job_exp.yml`; nothing is missing for reachable classes.

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

Membership facts that the compact table above hides (checked by structure scan of `job_exp.yml`, rAthena checkout used for this catalog):

* The job-exp table is complete: 14 groups at lines 2024, 2049, 2073, 2191, 2300, 2437, 2595, 2789, 2937, 3084, 3191, 3394, 3539, 3694; counts 2, 1, 15, 6, 34, 15, 51, 25, 4, 4, 2, 2, 12, 2. Row counts equal each group's Max. Base-exp groups (5) at 88, 350, 573, 1041, 1619 with 61, 22, 65, 25, 2 names.
* The 15 names of the "1st classes" job group (max 50) are the six 1st jobs, their six Baby forms, plus `Taekwon`, `Gangsi` and `Baby_Taekwon`. Gunslinger, Ninja and their Baby forms are a separate 4-name group (max 70, 200 / 300 ..., 308000 / 999999). Star Gladiator (+`2`, Baby) is its own group (max 50, 50700 / 50700 ...), not part of the 2nd-class group.
* The "Transcendent 2nd" group (max 70) holds 15 names: `Lord_Knight`, `High_Priest`, `High_Wizard`, `Whitesmith`, `Sniper`, `Assassin_Cross`, `Lord_Knight2`, `Paladin`, `Champion`, `Professor`, `Stalker`, `Creator`, `Clown`, `Gypsy`, `Paladin2`.
* Group at line 3539 (max 70, same values as the 3rd-class table): `Kagerou`, `Oboro`, `Rebellion`, `Soul_Reaper`, `Star_Emperor`, `Star_Emperor2` and the Baby forms. `Super_Novice_E` / `Super_Baby_E` (3394, max 70, 12800 ...) match the 3rd-class values too; `Super_Novice` / `Super_Baby` (3191) are max 99.
* Summoner and Baby_Summoner are in both Doram groups (base: max 200, 55 / 90; job: max 60, 60 / 130, last 132528619 / 999999999). Spot check of first and last two values per group (levels 1, 2, L-1, L) matched the table above for all 19 groups.

Notes: Novice max job level is 10 (job exp table has 10 rows; this is the "novice cap"), 1st classes 50. Exact row values for every level are in the file; table lengths equal the Max*Level of the group. Group line numbers are for the checked-out rAthena commit; re-extract with the import tool rather than relying on them.

The per-job non-exp fields (MaxWeight, HpFactor, BaseASPD, BonusStats, MaxStats, BaseHp/Sp/Ap, AlternateOutfits) are documented in the header of `job_exp.yml` (lines 22-86) but live in the sibling files `job_stats.yml`, `job_aspd.yml`, `job_basepoints.yml`; `job_exp.yml` itself holds only exp tables (this was checked by structure scan, not exhaustively).

## statpoint.yml

Source: `db/re/statpoint.yml`, 659 lines, `Type: STATPOINT_DB`, `Version: 2`.

* Fields: `Level`, `Points` (cumulative status points granted from base level 1 up to and including `Level`), optional `TraitPoints` (cumulative trait points, only present from level 201). 275 rows (levels 1-275).
* Points granted on a level-up from L to L+1 are `Points[L+1] - Points[L]` (`PlayerStatPointDatabase::pc_gets_status_point`, `pc.cpp:8755-8765`); `0` if the table does not increase. Same for trait points (`pc_gets_trait_point`, 0 below level 201).
* Samples: L1 = 48 (also the starting status-point budget used by stat reset), L2 = 51, L3 = 54, L4 = 57, L5 = 60, L6 = 64, L10 = 80, L99 = 1273, L100 = 1295, L175 = 3278, L176 = 3309, L200 = 4099, L201..L275 = 4099 (flat: no more status points above 200). `TraitPoints`: L201 = 3, L202 = 6, L203 = 9, L204 = 12, L205 = 19, L206 = 22 (+3 per level, +7 on every fifth level: L210 = 38; confirmed by capture: B205 pool 19), L275 = 285.
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
