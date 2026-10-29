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

List form expansion (applies to every per-level array, `skill.cpp:14705-14763`): entries set `arr[Level-1]`. If the listed levels form a linear sequence (tested with steps 1..max_level/2 over the whole listed range), the remaining levels up to 13 are extrapolated linearly (clamped to 1 if it would go below 1 after being >= 0); otherwise all unlisted higher levels repeat the last listed value. Unlisted LOWER levels stay 0. Scalar form fills all slots with the value.

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
      MaxLevel: n            # required; capped to skill_db MaxLevel; 0 = remove skill (only used once: Summoner removes WE_CALLBABY)
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
4. Finally every entry with `MaxLevel == 0` is deleted from every tree (so Summoner's `WE_CALLBABY: 0` removes it even though it would be inherited).
5. Within a single `Tree`, a skill listed twice in the same job (or the same job listed twice in the file) merges into the same entry (only given keys overwrite; `Requires` merges by name).

`MAX_SKILL_TREE` does not exist as a compile limit in this commit (trees are hash maps).

Trans classes inherit the same way (Lord_Knight lists `Novice, Swordman, Knight`; Swordman_High lists `Novice, Swordman`). The Baby tree's own entries: NV_BASIC 9, NV_FIRSTAID 1, NV_TRICKDEAD 1 (Exclude), WE_BABY, WE_CALLPARENT, WE_CHEERUP.

### Max level lookups

`skill_tree_get_max(skill, class_)` (`skill.cpp:286`) = tree entry `MaxLevel` for `status.class_` if the skill is in that job's tree, else skill_db `MaxLevel`. Used for `pc_skillup` limit and `upFlag`.

## Runtime: which skills a player "has" (`pc_calc_skilltree`, `pc.cpp:2601-2795`)

Per character there is `skill[MAX_SKILL]` of `{id:u16, lv:u8, flag:u8}`. `id != 0` means the skill is VISIBLE in the list (level may be 0); that is exactly what `clif_skillinfoblock` sends (level-0 visible entries ARE sent). Algorithm (rules):

1. Effective tree job: `pc_calc_skilltree_normalize_job`. If `player_skillup_limit` (default yes) is off, or the player has the all-skill permission, the real class is used. Otherwise the class is DOWNGRADED while the player has not spent enough points:
   - let `used` = sum of learned levels (see "points counted" below).
   - Non-summoner: if `used < (Novice max job level - 1)` -> effective job NOVICE (must put 9 points into the Basic Skill first, Novice MaxJobLevel 10).
   - subtract `NoviceMaxJobLv - 1` from `used`.
   - 2nd class (JOBL_2, not Super Novice): `change_level_2nd` = job level the character had when leaving the 1st job (stored in char var `JOBCHANGE2ND_VAR`; if unset, defaults to the 1st job's max job level and is saved). If `used < change_level_2nd - 1` -> effective job is the 1st class (e.g. Knight with unspent 1st-class points sees the Swordman tree). Then subtract `change_level_2nd - 1`.
   - 3rd class (JOBL_THIRD): same with `change_level_3rd` against the 2nd class, subtract; 4th class (JOBL_FOURTH): `change_level_4th` against the 3rd class.
   - Result keeps JOBL_UPPER / JOBL_BABY if the reduced class still exists with that bit (so a Baby/trans Novice-level class maps to Baby / Novice_High tree); 4th-class primaries that were reduced to non-4th get JOBL_UPPER (trans trees).
   - The class id handed to the tree is `pc_mapid2jobid(mapid, sex)` (sex only matters for Bard/Dancer family).
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
