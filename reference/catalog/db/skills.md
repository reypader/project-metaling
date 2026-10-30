# Skills Database: skill_db.yml, skill_tree.yml, SQL `skill` table

Target `PACKETVER = 20260219`, renewal. Source: `$RATHENA_PATH` commit `d4b8e7b8f`. All data is GPL-3.0 (import tool only; see CLAUDE.md). Packet side (0x010f / 0x010e / 0x0112): [../packets/settings.md](../packets/settings.md). Job names/ids: [job_stats.md](./job_stats.md). SQL overview: [sql_schemas.md](./sql_schemas.md).

## Files and layout

| File | Size | Notes |
| :--- | :--- | :--- |
| `db/re/skill_db.yml` | 50782 lines, 1635 `- Id:` entries | `Header: {Type: SKILL_DB, Version: 4}`, `Body:` list. No `Footer`. Renewal only: there is a pre-renewal `db/pre-re/skill_db.yml`; renewal build loads `db/re/` then `db/import/skill_db.yml` (header only here, empty). Later entries with an existing `Id` MERGE (only present keys override). A NEW id must carry `Name`, `Description`, `MaxLevel`. |
| `db/re/skill_tree.yml` | 8760 lines, 175 `- Job:` entries, 1425 tree entries | `Header: {Type: SKILL_TREE_DB, Version: 1}`. `db/import/skill_tree.yml` is header only. |
| `db/re/guild_skill_tree.yml` | | guild skills only, out of scope (guild ids 10000+). |

Provenance: `src/map/skill.cpp:14775` (`SkillDatabase::parseBodyNode`), `src/map/pc.cpp:13422-13701` (`SkillTreeDatabase`), file header comments `db/re/skill_db.yml:19-139`, `db/re/skill_tree.yml:19-37`.

## skill_db.yml fields needed for learn-only

| Key | Meaning |
| :--- | :--- |
| `Id` | u16 skill id. `MAX_SKILL` = 1641 index slots (`src/common/mmo.hpp:92`), ids map to a dense index internally; we can key by id. |
| `Name` | Aegis name, max 40 chars (`SKILL_NAME_LENGTH`). This string is what 0x010f `name[24]` carries (NUL padded; names are <= 23 chars in practice, rAthena copies with `safestrncpy` to 24). |
| `Description` | display name, max 40 chars. Not sent in 0x010f. |
| `MaxLevel` | 1..13 (`MAX_SKILL_LEVEL` = 13). The tree entry `MaxLevel` is capped to this. |
| `Type` | `BF_` suffix: `None`(1 entry, rest default None), `Weapon` 544, `Magic` 431, `Misc` 79. Default None. Not relevant to learning. |
| `TargetType` | suffix of `INF_<X>_SKILL`: `Passive`(default, 0x00), `Attack` 0x01, `Ground` 0x02, `Self` 0x04, `Support` 0x10, `Trap` 0x20 (0x08 unassigned). Counts in the file (explicit): Self 661, Attack 495, Ground 175, Support 118, Trap 3. **This value is stored as `inf` and sent verbatim as the i32 `inf` in 0x010f** (`skill_get_inf`, `skill.cpp:177`, `skill.hpp:78-85`). Passive = 0 also means "recalc status on level change" (`pc.cpp:9154`). |
| `Range` | scalar OR list of `{Level, Size}`. Scalar fills all 13 slots. Negative = "weapon range" (see range2). Default 0. |
| `Requires.SpCost` | scalar-or-list `{Level, Amount}`; absent = 0 at all levels. |
| `Flags` | map `FlagName: true/false`, suffix of `INF2_<UPPER>`; unknown names are skipped with a warning. See flags table. |

List form expansion (`SkillDatabase::parseNode`, `skill.cpp:14705-14763`; `MAX_SKILL_LEVEL` = 13 is the slot count `S` of every per-level array, `skill.hpp:43,269`). Applies to every per-level key (Range, SpCost, CastTime, ...), commit `d4b8e7b8f`, valid for 20260219. Exact behaviour:

- **Scalar form** (key is not a YAML sequence): all 13 slots = the value.
- **List form**: slots start at 0 for a new skill (rAthena memsets/zeroes the arrays when the skill is new; a merged re-definition keeps old values in unlisted slots). Each entry sets slot `Level` (1-based) to its value; `Level > 13` aborts the whole skill row. `M` = highest listed Level (not MaxLevel, not entry count; entry order irrelevant; duplicates: last wins). Entries missing Level or value are skipped.
- **Linear test** (1-indexed levels, `v[k]`): for `step = 1 .. floor(M/2)` in increasing order, let `d = v[M] - v[M-step]`. The step is a level distance (period), not a value step. The test passes if `v[k] - v[k-step] == d` for EVERY `k` in `step+1 .. M`. This reads all slots 1..M including UNLISTED ones, which hold 0 (gaps and levels below the first listed). First passing `step` wins. Consequences: `M = 1` -> no steps tried -> repeat rule. `M = 2` -> step 1 trivially passes (any two values are linear, if both levels listed). A single entry at level L>=2 (rest 0) fails unless its value is 0 (the zeros below break the difference), so it repeats. Gaps generally break linearity because the gap slots are 0. step > 1 accepts periodic patterns (e.g. 8,8,8,8,8,15,15,15,15,15 passes at step 5 with d = 7).
- **If linear** (step s, diff d): for `k = M+1 .. 13`: `v[k] = v[k-s] + d` (plain i32 add, no division, no rounding). Immediately after computing, if `v[k] < 1 AND v[k-1] >= 0`: `v[k] = 1`, `d = 0`, `s = 1` (so all following levels are 1). The clamp floor is **1, not 0** (the source comment says "0" but code stores 1). It also fires for a flat-zero series (v[k] = 0 < 1 and v[k-1] = 0 >= 0 -> becomes 1), e.g. a list `[{1,0},{2,0}]` expands to 0,0,1,1,... If `v[k-1]` is already negative no clamp occurs and the series keeps going.
- **If not linear** (or `M = 1`): `v[k] = v[M]` for `k = M+1 .. 13` (repeat last LISTED value).
- **Unlisted levels BELOW M** (gaps and below the first listed level) are never filled: they stay 0 (they were only used as zeros in the linear test).
- Expansion always runs to slot 13, not to the skill's `MaxLevel`. For data this means levels above MaxLevel hold extrapolated junk that is never used (`skill_get_lv` clamps level to 13, not MaxLevel), so a clean-room implementation only needs correct values for 1..MaxLevel unless it supports levels above MaxLevel.
- Empty list (`M = 0`): rAthena reads `arr[-1]` (UB); ignore, does not occur in the data.

Data check (python simulation of the above over `db/re/skill_db.yml`, 1635 skills; 1142 distinct skills are in some `skill_tree.yml` job tree): **0 skills** (in trees or anywhere) have an `SpCost` or `Range` list that omits any level in 1..MaxLevel. Every list is complete for 1..MaxLevel, so within 1..MaxLevel the expansion is the identity and a loader can simply read `[{Level, Amount}]` into slots; only levels > MaxLevel differ. Worked examples (listed -> slots 1..13):

| Skill | Listed | Expanded 1..13 | Rule |
| :--- | :--- | :--- | :--- |
| SM_PROVOKE SpCost (MaxLevel 10) | 4,5,6,7,8,9,10,11,12,13 | 4,5,6,7,8,9,10,11,12,13 **,14,15,16** | step 1, d = 1 |
| SM_BASH SpCost (MaxLevel 10) | 8 x5 (L1-5), 15 x5 (L6-10) | 8,8,8,8,8,15,15,15,15,15 **,22,22,22** | step 1 fails (d = 0 vs 7), step 5 passes (d = 7) |
| AL_TELEPORT SpCost (ML 2) | 10, 9 | 10,9 **,8,7,6,5,4,3,2,1,1,1,1** | step 1, d = -1, clamp to 1 at L10+ |
| SL_SKA SpCost (ML 3) | 100,80,60 | 100,80,60 **,40,20,1,1,...** | d = -20; 0 would be next, clamped to 1 |
| TK_STORMKICK SpCost (ML 7) | 14,12,10,8,6,4,2 | ...,2 **,1,1,1,1,1,1** | next would be 0 -> 1 |

Hypothetical (no such data): listed `{L3: 30}` only -> 0,0,30,30,... (repeat, lower stay 0); listed `{L1:10, L3:30}` -> M = 3, step 1: d = 30 - 0 = 30, check v2 - v1 = -10 != 30 -> fail -> 10,0,30,30,...

Lookup of per-level data: `skill_get_lv` returns `arr[min(lv,13)-1]`; for `lv == 0` the index is -1 (out of bounds read in rAthena, effectively garbage/0). Metaling must send `sp = 0, range2 = 0` for level-0 entries (matches capture: NV_BASIC lv 0 sp 0 range2 0).

### `Flags` that matter (counts of `true` in the file)

| Flag (YAML) | Count | Effect on learning / list |
| :--- | :--- | :--- |
| `IsQuest` | 40 | "Platinum / quest" skills, e.g. NV_FIRSTAID, NV_TRICKDEAD, SM_FATALBLOW, SM_AUTOBERSERK, SM_MOVINGRECOVERY, AC_MAKINGARROW, AC_CHARGEARROW, TF_SPRINKLESAND/BACKSLIDING/PICKSTONE/THROWSTONE, MC_CARTREVOLUTION/CHANGECART/LOUD, AL_HOLYLIGHT, MG_ENERGYCOAT, AM_BIOETHICS, KN_CHARGEATK, CR_SHRINK, AS_SONICACCEL, AS_VENOMKNIFE, RG_CLOSECONFINE, WZ_SIGHTBLASTER, SA_CREATECON, SA_ELEMENT*, HT_PHANTASMIC, BA_PANGVOICE, DC_WINKCHARM, BS_UNFAIRLYTRICK, BS_GREED, PR_REDEMPTIO, MO_KITRANSLATION, MO_BALKYOUNG, ALL_INCCARRY, ALL_BUYING_STORE, MC_CARTDECORATE, MA_CHARGEARROW. Not learnable by skill points (hidden while level 0, cannot be raised from 0) unless `quest_skill_learn`; granted by NPC script at level N via permanent skill grant. Excluded from skill-point counting and from reset (see below). |
| `IsWedding` | 9 | Never learnable by points; hidden at level 0 always; not counted in used points; not reset. (e.g. WE_CALLBABY in Novice tree.) |
| `IsSpirit` | 7 | Soul Linker spirit skills; only appear (lv 1, TEMPORARY) while `SC_SPIRIT` is active; not reset. |
| `IsNpc` | 194 | "players can't have in their tree"; only matters for `@allskill`-style grant (`pc_grant_allskills` skips IsNpc/IsGuild). |
| `IsGuild` | 20 | guild skills; `pc_skillup` routes ids in the guild range to `guild_skillup`. |
| `IsSong`/`IsEnsemble`/`IsChorus`/`IsTrap`, `AllowWhenHidden`, `AllowWhenPerforming`, ranges `AlterRange*` | 11/10/11/27/6/3 | cast-time semantics only; irrelevant to learn-only except `AlterRange*` for range2 (below). |

Platinum skills are exactly the `IsQuest` ones; there is no other marker. In the tree they appear normally (e.g. NV_FIRSTAID in the Novice tree) but are filtered by the rules in "Visibility".

### range2 in 0x010f / 0x010e (player, `isServer=false`)

`skill_get_range2` (`skill.cpp:324-366`), as called by `clif_skillinfoblock` and `pc_skillup`:
1. `r = Range[level]`.
2. If `r < 0`: if battle conf `skillrange_from_weapon` includes players (default `BL_NUL`, i.e. NO) return the weapon attack range; otherwise `r = -r`. So default: absolute value (SM_BASH -1 -> 1).
3. The "cap to 14" applies only when `isServer` is true; the client list does not cap.
4. Add per-flag bonus (players only): `AlterRangeVulture` + AC_VULTURE level, `AlterRangeSnakeEye` + GS_SNAKEEYE level, `AlterRangeRadius` + WL_RADIUS level, `AlterRangeResearchTrap` + table `[0,1,1,2,2,3,3,4,4,5,5][RA_RESEARCHTRAP lv]`; `AlterRangeShadowJump` REPLACES range with NJ_SHADOWJUMP's Range at the player's NJ_SHADOWJUMP level.
5. Players with final range 0 return 0 (non-players get 9).
Learn-only simplification: `abs(Range[level])` plus the vulture/snake-eye/radius/research-trap bonuses from learned levels; recompute those entries when the governing skill levels up (rAthena does not: it re-sends the whole list via `clif_skillinfoblock` after each skill-up, which recomputes all).

## skill_tree.yml schema

```
- Job: <JobName>            # constant JOB_<Name>; must be a valid player class (pcdb_checkid)
  Inherit: { <JobName>: true|false, ... }   # optional; false removes a previously inherited job (import override)
  Tree:                      # optional
    - Name: <SKILL_AEGIS_NAME>
      MaxLevel: n            # required; capped to skill_db MaxLevel; 0 = remove skill (only used once: Baby_Summoner removes the WE_CALLBABY it inherits from Summoner)
      Exclude: true          # optional, default false: do not pass this entry down to jobs that Inherit this job
      BaseLevel: n           # optional, default 0 (capped to the job's max base level)
      JobLevel: n            # optional, default 0 (capped to the job's max job level)
      Requires:              # optional
        - { Name: <SKILL>, Level: n }   # Level 0 removes an inherited/previous requirement
```
Usage in this file: `Exclude` 3 times (NV_TRICKDEAD in Novice, Novice_High, Baby), `BaseLevel` 13 (all Summoner SU_* skills, 100), `JobLevel` 6 lines but effectively only `LK_BERSERK` (Lord_Knight, job level 50) is non-zero, `MaxLevel: 0` once. There is NO `Hidden`, `Class`, `Skill Point cost`, etc.: one skill point per level, always.

Job names are the `JOB_<Name>` script constants (same set as `job_stats.md`): Novice, Swordman, ..., Knight2 (riding variant), Baby_*, *_High, *_T (trans 3rd), 4th classes (Dragon_Knight ... Sky_Emperor), Summoner, Hyper_Novice, Shinkiro, Shiranui, etc. Every job has its OWN entry (175 total, riding `*2` variants and all `Baby_*` included), so lookup is direct by the character's `class_`; the tree is not derived from a "base class" at runtime.

### Load-time inheritance (`SkillTreeDatabase::loadingFinished`, `pc.cpp:13640-13700`)

1. Each job's `Inherit` list is an explicit, flat list: inheritance is NOT transitive. Rune_Knight lists `Novice, Swordman, Knight`; Knight lists `Novice, Swordman`; Baby_Knight lists `Baby, Swordman, Knight`; Knight2 lists `Novice, Swordman, Knight`; Super_Novice_E lists `Novice, Supernovice`; Novice_High lists only `Novice` (+ Exclude'd TRICKDEAD of its own). Parents contribute only the skills in their OWN `Tree:` (never their inherited ones, since inheritance is applied to a snapshot after all parents' own trees are known and merged in afterwards).
2. For each parent in listed order, each parent entry is copied (entry shared by reference) unless it has `Exclude: true`, or the child already defines that skill in its own `Tree` (child entry wins entirely, no key-level merge: its BaseLevel/JobLevel/Requires/MaxLevel are what it declares). If two parents define the same skill, the LATER parent in the `Inherit` list wins.
3. Inherited `BaseLevel`/`JobLevel` are capped to the child job's max levels.
4. Finally every entry with `MaxLevel == 0` is deleted from every tree (so Baby_Summoner's `WE_CALLBABY: 0` removes the one it inherits from Summoner).
5. Within a single `Tree`, a skill listed twice in the same job (or the same job listed twice in the file) merges into the same entry (only given keys overwrite; `Requires` merges by name).

`MAX_SKILL_TREE` does not exist as a compile limit in this commit (trees are hash maps).

Trans classes inherit the same way (Lord_Knight lists `Novice, Swordman, Knight`; Swordman_High lists `Novice, Swordman`). The Baby tree's own entries: NV_BASIC 9, NV_FIRSTAID 1, NV_TRICKDEAD 1 (Exclude), WE_BABY, WE_CALLPARENT, WE_CHEERUP.

### Max level lookups

`skill_tree_get_max(skill, class_)` (`skill.cpp:286`) = tree entry `MaxLevel` for `status.class_` if the skill is in that job's tree, else skill_db `MaxLevel`. Used for `pc_skillup` limit and `upFlag`.

## Runtime: which skills a player "has" (`pc_calc_skilltree`, `pc.cpp:2601-2795`)

Per character there is `skill[MAX_SKILL]` of `{id:u16, lv:u8, flag:u8}`. `id != 0` means the skill is VISIBLE in the list (level may be 0); that is exactly what `clif_skillinfoblock` sends (level-0 visible entries ARE sent). Algorithm (rules):

1. Effective tree job: `pc_calc_skilltree_normalize_job` (`pc.cpp:2979-3005`) wrapping `_sub` (`pc.cpp:2893-2977`). Source rAthena `d4b8e7b8f`, applies to `20260219`. Runs on every `pc_calc_skilltree` call, for EVERY class including 1st classes and Novice, at the very start (before step 2 clears the visible ids). If `player_skillup_limit` (battle_config.skillup_limit) is off, or the player has the all-skill permission, the result is the real mapid unchanged. Otherwise (rules, `c` = running candidate mapid):
   - **Input `used`** (`pc_calc_skillpoint`, `pc.cpp:2518`): sum over all skill slots with `id != 0` and `lv > 0` (skills currently visible, every class's, not only the tree's) of: `lv` if flag is PERMANENT; the real learned level (`flag - REPLACED_LV_0`) if flag >= REPLACED_LV_0; nothing for TEMPORARY / PLAGIARIZED / PERM_GRANTED. A slot is skipped if it is `IsQuest` (unless `quest_skill_learn`) or `IsWedding` without `IsSpirit`. Signed 32-bit afterwards, may go negative after subtractions (irrelevant: every test is `used < threshold`).
   - **Tier 0, Novice** (skipped only when `mapid & 0xff == 0xa`, i.e. Summoner, Baby Summoner, Spirit Handler; NOT skipped for Super Novice, Taekwon, Gunslinger, Ninja or anything else): `nov = MaxJobLevel(job id 0 Novice) - 1` = 9, always the plain Novice entry (never Novice_High/Baby). If `used < nov` return mapid `0` (Novice, before the UPPER/BABY re-add below). Else `used -= nov`.
   - **Tier 1, "2nd class"**: applies if `mapid & JOBL_2 (0x300) != 0` AND `(mapid & 0xfff) != 0x100` (Super Novice / Super Novice_E / Hyper Novice / Super Baby are excluded; Spirit Handler `0x10a`, Rebellion, Kagerou/Oboro, Star Gladiator, Soul Linker, every 3rd and 4th class other than the SN line are included, since their mapids carry 0x100/0x200). `first = mapid & 0xff` (clears every other bit incl. UPPER/BABY/THIRD/FOURTH), `first_job = mapid2jobid(first, sex)`. If char var `jobchange_level` (`change_level_2nd`, unsigned char, loaded `pc.cpp:2360`) is 0: set it to `MaxJobLevel(first_job)` (the NON-trans, NON-baby 1st class, e.g. Swordman 50, Taekwon 50, Gunslinger 70, Ninja 70, Summoner 60) and WRITE IT BACK to the char var (`pc_setglobalreg`, persisted, so the default is frozen from then on). Then if `first_job > 0` (not Novice) and `used < change_level_2nd - 1` return `first`. Else `used -= change_level_2nd - 1`.
   - **Tier 2, "3rd class"**: applies if `mapid & JOBL_THIRD (0x1000)` (3rd and 4th classes incl. Super Novice_E, Hyper Novice, Star Emperor, Night Watch, Shinkiro). `second = mapid & 0xfff` (SECONDMASK), `second_job = mapid2jobid(second, sex)`. Var `jobchange_level_3rd` (`change_level_3rd`), if 0 -> `MaxJobLevel(second_job)` written back: the non-trans 2nd class (Rune Knight_T and Rune Knight both default to Knight 50, NOT Lord Knight 70; Super Novice_E/Hyper Novice default to Super Novice 99; Star Emperor to Star Gladiator 50). If `second_job > 0 && used < change_level_3rd - 1` return `second`. Else subtract `change_level_3rd - 1`.
   - **Tier 3, "4th class"**: applies if `mapid & JOBL_FOURTH (0x10000)`. `third = mapid & 0xffff` (THIRDMASK; keeps JOBL_THIRD), `third_job = mapid2jobid(third, sex)`. Var `jobchange_level_4th`, if 0 -> `MaxJobLevel(third_job)` written back (Dragon Knight -> Rune Knight non-T 70; Hyper Novice -> Super Novice_E 70; Sky Emperor -> Star Emperor 70). If `third_job > 0 && used < change_level_4th - 1` return `third`. Else subtract. If nothing returned early the real mapid is the result.
   - All comparisons are strict `<`, i.e. a character whose points equal the threshold exactly already has the higher class tree (Knight: 9 + (c2 - 1) points; with c2 = 10, 18 points, matching capture). The var holds a job level (1..99), so `c - 1` points is the full 1st-class spend.
   - **Post-processing** (in the non-sub function, applied to whatever `c` came back, including the unchanged real mapid): if real mapid has `JOBL_UPPER` and `mapid2jobid(c | JOBL_UPPER)` is a valid job, `c |= JOBL_UPPER`; likewise `JOBL_BABY`. Then, if the real class is a PRIMARY 4th (Dragon Knight .. Shadow Cross `0x11101-0x11106`, Imperial Guard .. Abyss Chaser `0x11201-0x11206`; NOT Hyper Novice / Sky Emperor / Soul Ascetic) and `c` is not primary 4th, `c |= JOBL_UPPER` (trans trees). UPPER and BABY are never both set. No `UPPERMASK` is used; only these three bits matter. Tree job = `pc_mapid2jobid(c, sex)` (the sex picks Bard/Dancer, Kagerou/Oboro, Shinkiro/Shiranui, Minstrel/Wanderer: male, then female).
   - **Where change vars are set** (`pc_jobchange`, `pc.cpp:10878-10893`, reads `status.job_level` BEFORE it is reset to 1 at `pc.cpp:10920`; exactly one var per job change, chain of `else if` in this order; "old" = current `class_`, "new" = target mapid after the upper/baby arg): (a) new has FOURTH and old lacks FOURTH -> `change_level_4th = job_level`; (b) else new has THIRD and old lacks THIRD -> `change_level_3rd`; (c) else new has any `JOBL_2` bit (`0x300`) and old lacks `JOBL_2` -> `change_level_2nd`. Note (c) is "has any of 0x300", so Novice -> Super Novice sets the 2nd var, Summoner -> Spirit Handler too; Knight -> Dragon Knight sets only the 4th var; Lord Knight -> Rune Knight_T sets 3rd; Novice_High -> Lord Knight sets 2nd. A jump that skips tiers leaves the skipped vars at their default (0, filled lazily as above). Changing down or sideways within the same tier sets nothing. Job-level source is the level in the old job at change time (job level of 10 for a Swordman, etc.).
   - **Worked examples** (P = `used` after counting; c2/c3/c4 = the vars, `n` = 9; defaults as above; result = tree job):
     - Swordman/Mage/etc. (1st class, `0x1`): `P < 9` -> Novice, else itself. Novice, Novice_High (`0x100000`), Baby: `P<9` -> Novice -> re-add -> itself; always itself. Taekwon, Gunslinger, Ninja: `P<9` -> Novice (UPPER/BABY absent) else itself.
     - Knight (`0x101`): `P<9` Novice; `P < 9+c2-1` Swordman; else Knight. Lord Knight (`0x100101`): Novice_High / Swordman_High / Lord Knight. Baby Knight (`0x200101`): Baby / Baby Swordman / Baby Knight.
     - Rune Knight (`0x1101`): Novice; `<9+c2-1` Swordman; `<9+(c2-1)+(c3-1)` Knight; else Rune Knight. Rune Knight_T (`0x101101`): Novice_High; Swordman_High; Lord Knight; Rune Knight_T (the UPPER bit re-add turns Knight into Lord Knight).
     - Dragon Knight (`0x11101`, primary 4th, no UPPER bit): Novice_High (`0|UPPER`), Swordman_High, Lord Knight, Rune Knight_T (`0x1101` is not primary 4th so UPPER is forced -> `0x101101`), else Dragon Knight. Tier thresholds cumulative: 9, +(c2-1), +(c3-1), +(c4-1).
     - Super Novice (`0x100`): `P<9` Novice, else itself (tier 1 skipped). Super Baby: Baby / itself. Super Novice_E (`0x1100`): Novice; `P < 9 + (c3-1)` (default c3 = 99) Super Novice; else itself. Hyper Novice (`0x11100`): Novice; `<9+c3-1` Super Novice; `<... + c4-1` (default 70) Super Novice_E; else Hyper Novice (no trans fallback: not primary 4th). Super Baby_E: Baby / Super Baby / itself.
     - Summoner (`0xa`), Baby Summoner (`0x20000a`): no Novice tier, no JOBL_2 -> always itself (Baby Summoner has no 2nd class). Spirit Handler (`0x10a`): `P < c2-1` (default Summoner 60 -> 59, counted WITHOUT the 9 Novice points) -> Summoner; else itself.
     - Star Gladiator (`0x107`) and Soul Linker (`0x207`): yes they carry `JOBL_2` (0x100 / 0x200). Novice; `<9+c2-1` (default Taekwon 50) Taekwon; else itself. Star Emperor (`0x1107`): Novice; Taekwon; Star Gladiator (c3 default SG 50); itself. Baby SG: Baby / Baby Taekwon / itself.
     - Ninja line: Kagerou/Oboro (`0x109`, same mapid, sex picks): Novice; `<9+c2-1` (default Ninja 70 -> 69) Ninja; else itself. Gunslinger line: Rebellion (`0x108`): Novice / Gunslinger / itself. Baby Kagerou -> Baby Ninja; Baby Rebellion -> Baby Gunslinger.
   - Note the defaults for trans lines are the NON-trans class max job level (50 vs the Lord Knight's real 70), so an unset-var trans/3rd character needs fewer points than its real history; the vars are normally filled by `pc_jobchange`. rAthena's default write-back means the first evaluation freezes the value.
2. All visible ids are cleared except `PLAGIARIZED` and `PERM_GRANTED` ones; `TEMPORARY`/REPLACED flagged ones restored to their real level and set PERMANENT (so item-granted bonus levels disappear and learned level returns).
3. Non-Taekwon classes lose Taekwon-ranker bonus skills (TK tree skills other than NV_BASIC, NV_FIRSTAID, WE_CALLBABY are made invisible). Out of scope here.
4. Fixed-point loop over the effective job's tree: for each skill not already visible, it becomes visible (id set, lv unchanged - 0 for new) when ALL hold (skipped entirely if `player_skillfree` = yes, which makes every tree entry visible with no prerequisite checks except the learnability filter in step 5):
   - every `Requires` skill is visible, not TEMPORARY/PLAGIARIZED, and its REAL learned level (REPLACED flag decoded) is >= required level;
   - base level >= `BaseLevel` and job level >= `JobLevel` (when the effective job differs from the real job the check still uses the raw base/job level of the character; net effect: plain comparison);
5. Learnability filter: when the skill's current level is 0 it is NOT made visible if it is `IsQuest` (and `quest_skill_learn` is off), or `IsWedding`, or `IsSpirit` (unless `SC_SPIRIT` active; in that case it is made visible at lv 1 flag TEMPORARY, not saved). An already-known (lv > 0) quest skill stays visible and can be raised: "this check allows raising already known skills". Repeat the loop until nothing changes (chains of prerequisites resolve in one call).
6. Taekwon Ranker bonus and Bard/Dancer spirit-link extras: out of scope.
`pc_check_skilltree` (after a successful skill-up) repeats step 4/5 on the effective-job tree to reveal newly unlocked skills (same filters; also compares plain base/job level).

Called from the full status recalculation (`status.cpp:3769`, i.e. login and level-ups) and from `pc_jobchange` (`pc.cpp:11007`, followed by a `0x010f` re-send).

### Skill-up (0x0112 -> `pc_skillup`, `pc.cpp:9129`)

Packet: `clif_parse_SkillUp` reads u16 skill id at the packet-db position (`clif.cpp:12757`). Accept only if ALL: skill id exists; `skill_point > 0`; entry `id != 0` (visible); entry `flag == PERMANENT` (granted/temporary/plagiarized refuse); `lv < skill_tree_get_max(skill, status.class_)`. Guild-range ids are routed to guild skill-up; homunculus ids to homunculus. Then `lv++`, `skill_point--`; passive skills (TargetType Passive, i.e. `inf == 0`) recalc stats; otherwise `pc_check_skilltree`. Replies: `0x010e` for that skill (sp = `skill_get_sp(id,newlv)`, range2, upFlag), `0x00b0` SP_SKILLPOINT, then full `0x010f` re-send (not sent if all-skill permission). Silent drop on failure (captured: hidden-id requests with free points get zero bytes back). Note it uses the REAL `status.class_` for the max, not the normalized job. Captured (`rathena_skills_sequence.txt`, see [../mechanics/skills.md](../mechanics/skills.md) section 10): unlock chains SM_SWORD 1 -> SM_TWOHAND, SM_BASH 5 -> SM_MAGNUM, SM_PROVOKE 5 -> SM_ENDURE, KN_SPEARMASTERY 1 -> KN_PIERCE; Novice with NV_BASIC < 9 sees only NV_BASIC after `@job 1`; a Knight whose 1st-to-2nd change was at job level 10 did not see Knight skills with 17 points spent.

### Points counted (`pc_calc_skillpoint`, `pc.cpp:2518`)

Sum over entries with `id != 0 && lv > 0`, excluding `IsQuest` (unless `quest_skill_learn`) and `IsWedding` (unless also `IsSpirit`): PERMANENT -> `lv`; REPLACED_LV_0+N -> N; others 0. Note PERM_GRANTED (flag 3) is also not counted.

### Skill reset (`pc_resetskill(sd, flag)`, `pc.cpp:9428`)

flag bit 1 = send updates (status point packet 0x00b0, skill list 0x010f, recalculation); bit 2 = dry run (just return refundable points, nothing changes); bit 4 = only Bard/Dancer gender-change skills. Skipped (not reset, not refunded): level 0; `IsWedding`; `IsSpirit`; flag PERM_GRANTED; NV_BASIC unless the character is a (baby) Novice (second-class mask == novice); ranked Taekwon (whole reset aborted). NV_TRICKDEAD for non novice: set to 0 without refund. `IsQuest` skills (when `quest_skill_learn` off): only wiped (lv 0, no refund) if `quest_skill_reset` on and not dry run; otherwise left untouched. Everything else: refund `lv` (PERMANENT) or decoded REPLACED level, set `lv=0`, `flag=PERMANENT`. Refund added to `skill_point`. Side effects (remove riding/falcon/dragon/wug/madogear/cart option, vaporize homunculus, end some SCs) are out of scope. Captured: NV_BASIC is kept (lv 9) when resetting as Swordman. After reset the visible set must be recomputed (`pc_calc_skilltree` via status recalc); ids stay `id != 0` with lv 0 until then.

### Battle config defaults (`conf/battle/player.conf:31-44`, `battle.cpp:8323-8347`)

| Option | Default | Meaning |
| :--- | :--- | :--- |
| `player_skillfree` | no (0) | all tree skills visible, no prerequisite/level checks |
| `player_skillup_limit` | yes (1) | the downgrade-to-lower-job rule (step 1 above) |
| `quest_skill_learn` | no (0) | IsQuest skills can be learned by points |
| `quest_skill_reset` | no (0) | reset also wipes IsQuest skills (no refund) |

## SQL table and flag semantics

`sql-files/main.sql:1045-1051` (MyISAM): `skill(char_id int unsigned, id smallint unsigned, lv tinyint unsigned, flag tinyint unsigned default 0, PRIMARY KEY(char_id, id))`.

`struct s_skill {u16 id; u8 lv; u8 flag}` (`mmo.hpp:394`); `enum e_skill_flag` (`mmo.hpp:381-392`): `NONE=-1`, `PERMANENT=0`, `TEMPORARY=1` (item/spirit bonus, id set, never saved), `PLAGIARIZED=2` (Rogue copy, not saved), `PERM_GRANTED=3` (script-granted permanent, ignores the tree, saved), `TMP_COMBO=4` (homunculus), `REPLACED_LV_0=10` (flag = 10 + real learned level while a temporary bonus overshadows it; not saved as such).

Save (`char.cpp:418-448`): whole table rewritten for the char (DELETE then multi-row INSERT) when the in-memory array differs. A row is written iff `id != 0`, flag != TEMPORARY, and NOT (`lv == 0` and flag in {PERMANENT, PERM_GRANTED}), and NOT (REPLACED with real level 0). Written `lv` = `lv` for PERMANENT/PERM_GRANTED, else `flag - 10` (the real level); written `flag` = 3 if PERM_GRANTED else 0. So DB rows only have flag 0 or 3, and level-0 rows are never stored. Before saving, `pc_clean_skilltree` converts TEMPORARY/PLAGIARIZED to cleared, REPLACED to real level (`pc.cpp:2877`).
Load (`char.cpp:1165-1190`): `SELECT id, lv, flag ... LIMIT MAX_SKILL`; flag kept only if 3, else PERMANENT; rows with id 0 ignored. After load the tree is recomputed (`pc_calc_skilltree`) which makes level-0 tree skills visible.

Metaling minimum: persist rows `(char_id, id, lv, flag)` for lv > 0 only; flag 0 normal, 3 for granted (e.g. platinum skills granted by NPC); visible-but-level-0 entries are derived at login from the tree.
