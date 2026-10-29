# Specification: Skills (learn only): tree, skill-up, skill list, resets, persistence

**Scope:**
- **New skill data, imported from rAthena:**
  - The skill registry: id, aegis name, max level, target type (`inf`), per-level SP cost and range, and the learnability flags.
  - Every job's skill tree, with inheritance resolved: max level per job, prerequisites, base/job level gates.
- **Each character's learned skills** (`id`, level, flag) are kept in memory, persisted, and sent in the real `0x010f` list. This replaces the `NV_BASIC` level 0 placeholder.
- **Which skills are visible** follows rAthena's `pc_calc_skilltree`:
  - prerequisites;
  - level gates;
  - the `player_skillup_limit` downgrade, where points must be spent in the previous job before the next job's skills appear;
  - the quest/wedding/spirit filters.
- **Skill-up** with `0x0112`: silent refusals, and replies `0x010e`, `SP_SKILLPOINT`, `0x010f` in the captured order.
- **Skill reset** (`pc_resetskill`) through `@skreset`, `@reset` (stats + skills) and `@jlvl −n`, plus `@allskill`.
- **`@job`** sends the new class's real list and records the class-change job levels, and `@blvl`/`@jlvl` re-check the tree.
- **Skill effects are placeholders.**
  - Learned passive skills give no bonuses.
  - Nothing can be cast.
  - The `NV_BASIC` level gates (sit, trade, chat rooms, …) are not enforced.

  The learned set is the input those later features read (`crates/ro-engine/DESIGN.md` §9.9, §10, §11).

Out of scope (see §5):
- casting (`0x0113`/`0x0438`/`0x0af4`), SP/AP costs, cast and after-cast delay, cooldowns
- passive bonuses (DESIGN.md §9.9) and weapon masteries
- quest/platinum skill grants (they need NPC scripting)
- item-granted (TEMPORARY) and plagiarised skills, `0x0441` for item skills
- guild and homunculus skills, Taekwon Ranker, Soul Linker spirit skills, Bard/Dancer change-sex reset
- option stripping on reset or job change (cart, falcon, peco, mado, homunculus)
- hotkeys naming skills

**Prerequisite:** the **traits** feature (`specs/traits/`) lands first. `@reset` calls its full `pc_resetstate`. Its `CharacterProgress`, `save_play_state` and `CharacterSummary` changes (the char-select `sppoint` included) are the base this feature extends.

**Ground truth:** rAthena `d4b8e7b8f` at `PACKETVER = 20260219`, renewal, as recorded in the catalog:
- **`reference/catalog/db/skills.md`:**
  - `skill_db.yml` fields;
  - `TargetType` → `inf`;
  - per-level array expansion;
  - flags;
  - range2;
  - the `skill_tree.yml` schema and load-time inheritance;
  - runtime visibility (`pc_calc_skilltree`);
  - points counted;
  - reset rules;
  - the battle-config knobs;
  - the SQL table and flags.
- **`reference/catalog/mechanics/skills.md`:**
  - `pc_skillup` checks and replies (§2);
  - visible-tree computation (§3);
  - `0x0441` (§5);
  - `pc_resetskill` (§6);
  - job change (§7);
  - job levels, atcommands, persistence and login (§8);
  - message ids (§9);
  - **§10, the capture write-up, which gives every order this spec cites.**
- `reference/catalog/packets/settings.md`: `0x010f` entry, `0x010e`, `0x0441` and "0x010f / 0x010e field details". `mechanics/job_change.md` step 1 (class-change job levels).
- **`reference/catalog/captures/rathena_skills_sequence.txt`** (2026-10-29). It covers:
  - two fresh Novices: `NV_BASIC` 1..9;
  - `@job` 1/7/2;
  - SM and KN skill-ups with unlocks;
  - `@jlvl -5`, `@skreset`, `@reset`, `@allskill`;
  - a relog after a job change (hidden levels lost);
  - the `skillup_limit` refusals.

  `equip_sequence_4.txt` (Archer/Dancer skill-ups, Dancer → Assassin `0x0441`) adds to it.
- **Still source-only:**
  - `0x0112` refusal at max level. The client blocks the request at 0 points and at max (user, 2026-10-29), so these refusals only guard against a modified client. Hidden ids at 0 points *are* sent and get no reply (`equip_sequence_4.txt` 2114-2153);
  - `@jlvl −n` when the visible tree does not shrink;
  - whether hidden levels survive a job detour without a save;
  - the exact `change_level_2nd` threshold of 18 (17 was tested);
  - `@skreset` as a Novice;
  - `BaseLevel`/`JobLevel`-gated unlocks.

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline (`tools/skill-import`, new)
- **Sources:** `db/re/skill_db.yml` and `db/re/skill_tree.yml`. The `db/import/` files are header-only and are ignored.
- **The importer is clean-room code** written from catalog `db/skills.md`, with the job name → id mapping shared with `tools/job-import`. Depend on its lib, or move the table into a small shared module under `tools/`. It writes `crates/ro-db/data/skill/`:
  - **`skill_db.yml`** (`SKILL_DATA` v1): one row per skill the trees reference, plus every skill a tree requires. Fields:
    - `id`, `name` (aegis), `max_level`;
    - `inf` (the `TargetType` code: Passive 0, Attack 1, Ground 2, Self 4, Support 16, Trap 32);
    - `sp_cost` and `range` as **expanded per-level lists up to `max_level`** (catalog "List form expansion": linear extrapolation or repeat-last; scalar fills);
    - `flags` limited to `quest`, `wedding`, `spirit`, `npc`, `guild`, `alter_range_{vulture,snake_eye,radius,research_trap,shadow_jump}`.
  - **`skill_tree.yml`** (`SKILL_TREE_DATA` v1): **inheritance resolved by the importer** (catalog "Load-time inheritance", rules 1–5: flat non-transitive `Inherit`, `Exclude`, child entry wins whole, later parent wins, BaseLevel/JobLevel capped to the child's max levels, `MaxLevel: 0` removes). Keyed by numeric job id. Each entry has `skill_id`, `max_level` (capped to the skill's), `base_level`, `job_level` and `requires: [{skill_id, level}]` (level-0 requirements dropped). Entries are sorted by skill id. Mounted and dummy jobs are skipped as `tools/job-import` skips them.
- **Each generated file** starts with the SPDX line and the attribution (rAthena URL, commit, source files, "Modified: trimmed to learn-only fields, per-level arrays expanded, tree inheritance resolved, names resolved to ids by `tools/skill-import`", and the `NOTICE.md` pointer). Add a `NOTICE.md` "skill" section.
- **No new reference source**, so no README credit change.

## 1. Data API (`ro-db`)
- **`SkillDb`:** `get(id)`, `by_name(&str)` (case-insensitive, for `@allskill` debugging and tests), and per skill `sp(level)` / `range(level)` (0 for level 0).
- **`SkillTreeDb`:** `tree(job_id) -> &[TreeEntry]` (sorted by skill id; empty for an unknown job) and `max_level(job_id, skill_id)`. That is `skill_tree_get_max`: the tree entry's max, or else the skill's `max_level`.
- **Loaded** from `SKILL_DATA_DIR` (new map-server config, normally `crates/ro-db/data/skill`; required, like `JOB_DATA_DIR`) and added to the engine registries.

## 2. Behaviour

### 2.1 Model
- **Per player, a `SkillSet`:** a small `Vec<SkillSlot>` sorted by id, where `SkillSlot { id: u16, level: u8, flag: SkillFlag, visible: bool }`. `SkillFlag` is `Permanent` (0) or `Granted` (3); rows with other flags aren't produced yet.
  - **A slot exists** for every visible tree skill (level 0 allowed) and for every skill with level > 0.
  - **Hidden slots with a level** (from a previous class) stay in memory until the next save drops them (§2.9; captured).
- **`skill_point`** stays in `CharacterStatus`.
- **`ro_core::CharacterProgress`** gains the class-change job levels `change_level: [u16; 3]` (2nd, 3rd, 4th; 0 = unset).
- **`PlaySnapshot`** gains `skills: Vec<LearnedSkill>`: the persisted rows, so `(id, level, flag)` with level > 0 and visible. It also gains a skills revision, so autosave writes only when the skills changed.
- **`BattleConfig`** gains the knobs `player_skillfree` (false), `player_skillup_limit` (true), `quest_skill_learn` (false) and `quest_skill_reset` (false), plus `config/battle.yml` (catalog `db/skills.md` "Battle config defaults").

### 2.2 Visible tree (catalog `db/skills.md` "Runtime", `mechanics/skills.md` §3)

`compute_visible(class, skills, base_level, job_level, change_levels, config)` is pure, so it can be unit-tested with hand-written trees.

**1. Effective tree job** (only when `player_skillup_limit` is on), from `P` = the points counted (catalog "Points counted": visible, level > 0, not quest or wedding, `Permanent` only):
- Non-Summoner: `P < 9` (Novice max job level − 1) → Novice. Otherwise `P −= 9`.
- A second class (not Super Novice): `P < change_level_2nd − 1` → its 1st class. Otherwise subtract.
- Then the same for 3rd (`change_level_3rd`, against the 2nd class) and 4th (`change_level_4th`, against the 3rd class).

The reduction works on the mapid: drop the 2nd/3rd/4th bits with the masks in `crates/ro-engine/src/job.rs`, keep the upper/baby bits when that job exists, and map back to a job id. That reverse lookup is new: scan `JOB_MAPIDS`, then `job_for_sex` for Bard/Dancer.

**Unset `change_level_*`** defaults to the previous class's `MaxJobLevel`, and is stored. The 3rd/4th defaults must be checked against the catalog (`mechanics/skills.md` §3). If the catalog doesn't settle them, ask `metaling-explorer`.

**2. Visibility reset:** clear `visible` on every `Permanent` slot. `Granted` slots stay visible.

**3. Fixed-point loop** over the effective job's tree. A skill becomes visible when all of these hold:
- every `requires` skill is visible and learned at ≥ the level;
- `base_level ≥ base_level`;
- `job_level ≥ job_level`.

`player_skillfree` skips these checks. A level-0 skill that is `quest` (unless `quest_skill_learn`), `wedding` or `spirit` never becomes visible.

**4. Result:** the visible set, sorted by id. Returning whether it changed, and which ids vanished, lets the callers below decide what to send.

**Max level for skill-up and `upFlag`** uses the **real** class (`SkillTreeDb::max_level(class, id)`), not the effective one (catalog).

### 2.3 The list (`0x010f`) and `0x010e`
- **Entries:** every visible slot, in ascending id order (captured; `WE_CALLPARTNER` is never visible here). Each entry has:
  - `inf` from the data. Level-0 entries carry the real `inf` too (captured).
  - `level`.
  - `sp` = `SkillDb::sp(level)`, 0 at level 0.
  - `range2` = `|range(level)|` plus the `alter_range_*` bonuses from the player's learned Vulture's Eye / Snake Eye / Radius / Research Trap levels, with `shadow_jump` replacing it with Shadow Jump's range (catalog "range2"). It is 0 at level 0, and players with a final 0 get 0.
  - `name`.
  - `upFlag` = `Permanent && level < max_level(class, id)`. It stays 1 with 0 free points (captured).
- **Captured values for tests:**
  - Bash lv 1-5: sp 8, range2 1. Bash lv 10: 15/1.
  - Provoke lv 1-5: sp 4..8, range2 9.
  - Magnum: 30/0.
  - Endure: 10/0.
  - Passives: 0/0.
- **`0x010e`** = `id, level, sp, range2, upFlag` with the same values: a new `ServerEvent::SkillUpdate` and its encoder.
- **`0x0441`** = `ServerEvent::SkillDelete { id }` and its encoder. The proxy table already frames both.
- **The 8192-byte cap** (n ≤ 221; rAthena overflows to `0x0111`) is not reachable by any tree. The builder asserts it in debug and notes it.

### 2.4 Login and map change
- **At spawn**, compute the visible set synchronously, with no output. The login burst sends the real `0x010f` where `nv_basic()` is sent today (`crates/ro-engine/src/settings.rs:144-153`). `nv_basic()` is deleted.
- **Map change** sends no list (as today; captured in earlier work).
- **A cross-map transfer** carries the `SkillSet` (the persisted rows plus the hidden levels) and `change_level`.

### 2.5 Skill-up (`0x0112`, catalog `mechanics/skills.md` §2, §10.3)
- **Parse** `CZ_UPGRADE_SKILLLEVEL` (`skillId:u16`) into `ClientIntent::UpgradeSkill { id }`. The opcode and length already exist.
- **Silent refusal** (no packet at all) unless all of these hold:
  - the id is known;
  - `skill_point > 0`;
  - the slot is visible;
  - the slot is `Permanent`;
  - `level < max_level(class, id)`.

  Guild and homunculus ids are refused (out of scope). There is no dead/sitting check (rAthena has none; ruling: accept).
- **Success:**
  - `level += 1`, `skill_point −= 1`.
  - Recompute the visible set at once. This is not a stat recalc, so running it inline is allowed. It is cheap and keeps a burst correct: later requests in the same tick see earlier unlocks (captured 707-720).
  - Then send **at once**, in this order:
    1. **If the skill is passive (`inf == 0`) and the visible set grew,** a `0x010f` with the new set and the raised level (captured: SM_SWORD → +Two-Handed at 595, KN_SPEARMASTERY → +Pierce at 1009, NV_BASIC 9 on a Swordman → +2,4,5,6 at 1436).
    2. `0x010e`.
    3. `0x00b0` `SP_SKILLPOINT`.
    4. `0x010f`.

    An active skill never sends the first list. Its unlocks appear only in the trailing list (Bash 5 → Magnum, 626).
- **No stat recalc:** passives are placeholders. When passive bonuses arrive, a passive skill-up marks the stats dirty and its diff comes from Stage 3. rAthena sends that diff *first*, which is a known future deviation, recorded in DESIGN.md §9.9.
- **No save** (rAthena saves on autosave or logout).
- **The observer** gets nothing (captured).

### 2.6 Tree re-check in the recalc pass
- **New dirty bit `SKILLS`** (DESIGN.md §9.5). Stage 3 recomputes the visible set for flagged players. If the set changed, it sends `0x0441` for each id that went from visible to hidden (ascending), then one `0x010f`. This goes after `SP_MAXWEIGHT` and before the HP/SP clamp pushes and the diff. It matches rAthena, whose tree diff sits inside `status_calc_pc` before the stat pushes (captured `@jlvl -5` 650-663; `equip_sequence_4` Dancer → Assassin 2608-2612).
- **Set by:** `@blvl` (BaseLevel gates), `@jlvl` (JobLevel gates), and the resets (§2.7). `@job` and `@allskill` compute the set inline instead (§2.8), so the pass finds no change.
- **`@jlvl +n`** with an unlock therefore sends a list before the diff. That case is source-only (`mechanics/skills.md` §8).

### 2.7 Resets (catalog `mechanics/skills.md` §6, §10.5)

**`reset_skills(e, send: bool) -> u32`** (`pc_resetskill`) refunds every slot with level > 0, except:
- wedding and spirit skills;
- `Granted` slots;
- `NV_BASIC` unless the class is Novice-line (`mapid & SECOND_MASK == Novice`, incl. baby). NV_BASIC is kept for a Swordman (captured).

Quest skills: when `quest_skill_learn` is off they are never refunded, and set to 0 only if `quest_skill_reset` is on.

Refunded slots go to level 0 and `Permanent`. The refund total (the sum of levels) is added to `skill_point`.

**With `send`:**
1. push `SP_SKILLPOINT`;
2. push a `0x010f` of the **current visible set** with the zeroed levels (nothing is deleted yet);
3. mark `SKILLS` dirty, so Stage 3 sends the deletes and the second list.

**Without `send`** (`@jlvl −n`): only mark dirty.

The Taekwon Ranker early return and the option stripping are out of scope.

**`@skreset`** (name `skreset`, canonical `resetskill`; group 99):
- `reset_skills(send)`, then the message as a follow-up.
- Captured 766-773:
  1. `SP_SKILLPOINT 44`;
  2. `0x010f` (ids 1–8, all level 0);
  3. `0x0441` 3, 7, 8;
  4. `0x010f` (5 entries);
  5. the message.

**`@reset`:**
- traits' `pc_resetstate` (its immediate pushes), then `reset_skills(send)`, then the message as a follow-up.
- Captured 859-892:
  1. `0x0141` ×12 (skipped by the stats ruling);
  2. `0x00be` ×12;
  3. `SP_STATUSPOINT`;
  4. `SP_TRAITPOINT`;
  5. `SP_SKILLPOINT`;
  6. `0x010f`;
  7. `0x0441` …;
  8. `0x010f`;
  9. the message.

  Our diff (stats) comes after the second list.

**`@jlvl −n`:**
- When `skill_point < n`: `reset_skills(no send)`. Then if still `< n` set the points to 0, else subtract.
- The existing pushes (message, `SP_JOBLEVEL`, exp, `SP_SKILLPOINT`) stay. Stage 3 adds the deletes and list before the diff.
- Captured 650-663:
  1. message;
  2. `SP_JOBLEVEL 5`;
  3. exp ×2;
  4. `SP_SKILLPOINT 4`;
  5. `0x0441` 3, 7;
  6. `0x010f`;
  7. stat diffs.
- If the visible set does not change, rAthena sends no list and the client keeps stale levels (source-only quirk). We match it: no change, no list.

**The observer** gets nothing for any of these.

### 2.8 `@job` and `@allskill`
- **`@job`:**
  - **Step 1, new:** record the class-change job level (catalog `job_change.md` step 1). To a 4th class it sets `change_level_4th`, to a 3rd `change_level_3rd`, and when the 2nd-class bit newly appears `change_level_2nd`, each = the job level before the change. Persist with the progress.
  - **Step 8:** replace the `nv_basic()` re-send with the real list. Compute the visible set inline for the new class and send **one** `0x010f` at the same position (after the looks).
  - Skill levels and `skill_point` are untouched, and `SP_SKILLPOINT` is not sent. Skills outside the new tree become hidden but keep their level in memory.
  - Captured on Novice → Swordman → Knight → Mage → Knight: one list, no `0x0441`.
  - rAthena's two-pass form ("list, `SP_MAXWEIGHT`, `0x0441`…, list") for some 2nd/3rd-class changes (Dancer → Assassin, Rune Knight → Shadow Cross) is a deliberate deviation: we send only the final list.
- **`@allskill`** (names `allskill`, `allskills`, `skillall`, `skillsall`; group 99):
  - Every entry of the **real** class's tree (not normalised) is set to its tree max. Quest skills (unless `quest_skill_learn`), wedding, spirit and `SG_DEVIL` are skipped.
  - `skill_point = 0`.
  - Compute the visible set inline. With everything spent, normalisation exposes the whole tree.
  - Send at once `0x010f`, then `SP_SKILLPOINT 0`, then the message as a follow-up. No deletes, no diff (captured 895-898).

### 2.9 Persistence (catalog `db/skills.md` "SQL table", `mechanics/skills.md` §8)
- **Schema:** table `character_skills (char_id INTEGER REFERENCES characters ON DELETE CASCADE, skill_id SMALLINT, level SMALLINT, flag SMALLINT DEFAULT 0, PRIMARY KEY (char_id, skill_id))`. `characters` gains `change_level_2nd`, `change_level_3rd` and `change_level_4th` (SMALLINT, default 0). Both use the guarded pattern of `crates/ro-persistence/sql/01_schema.sql`.
- **`save_play_state`** rewrites the character's skill rows (delete then insert) in the same transaction, only when the skills revision changed. It writes only visible slots with level > 0, flag 0 or 3. **Hidden levels are therefore lost at the next save** (captured: Knight levels gone after Mage + relog).
- **`load_play_state`** returns the rows and the change levels. Visibility is rebuilt at spawn.
- **Saves happen** on autosave, logout and map transfer, and immediately after `@job` (existing). Skill-up and resets don't save.
- **Char select `sppoint`** is already real (traits feature).

## 3. Scenarios

| ID | Scenario |
| :--- | :--- |
| DB-SKILL-01 | Every committed `skill/*.yml` loads and starts with the GPL header and attribution. |
| DB-SKILL-02 | `SkillDb`: NV_BASIC max 9 inf 0; SM_BASH inf 1, sp lv1..5 = 8, lv10 = 15, range 1; SM_PROVOKE sp 4..8 at lv1..5, 13 at lv10, range 9; SM_MAGNUM inf 4, sp 30; `by_name` is case-insensitive. NV_FIRSTAID is `quest`. |
| DB-SKILL-03 | `SkillTreeDb`: Swordman tree has 1..8 with SM_TWOHAND requiring SM_SWORD 1, SM_MAGNUM requiring SM_BASH 5, SM_ENDURE requiring SM_PROVOKE 5; Knight inherits Novice+Swordman and adds 55..63 with KN_PIERCE requiring KN_SPEARMASTERY 1; Rune Knight has Novice+Swordman+Knight entries; Novice_High has no NV_TRICKDEAD from Novice (`Exclude`); Summoner has no WE_CALLBABY (`MaxLevel 0`); `max_level` falls back to the skill's. |
| ENG-SKILL-01 | `compute_visible` on hand trees: prerequisites (chain resolved in one call), base/job gates, `skillfree`, quest/wedding filters, a learned quest skill stays visible. |
| ENG-SKILL-02 | `skillup_limit`: a Swordman with NV_BASIC < 9 sees only NV_BASIC; at 9 the Swordman tree appears. A Knight with `change_level_2nd` 10 sees the Swordman tree until 18 points are spent (9 + 9), then the Knight tree. Unset `change_level_2nd` defaults to the 1st class's max job level. |
| ENG-SKILL-03 | List building: ascending ids, level-0 entries with real `inf` and sp/range2 0, `upFlag` 0 only at the class's tree max, captured sp/range2 values (Bash, Provoke, Magnum, Endure), range2 bonus from Vulture's Eye on Double Strafe. |
| ENG-SKILL-04 | Login of a fresh Novice: `0x010f` = NV_BASIC lv 0 `upFlag` 1 (41 bytes) at the existing burst position. A Swordman with learned skills: the real list. |
| ENG-SKILL-05 | Skill-up (captured): NV_BASIC ×9 on a Novice → 9 × (`0x010e`, `SP_SKILLPOINT` 8..0, `0x010f`), lv 9 `upFlag` 0, no pre-list. Swordman SM_SWORD 1 → pre-list (adds 3), `0x010e`, `SP_SKILLPOINT`, list. Bash ×5 → no pre-list; the 5th's trailing list adds 7. A burst of 14 requests in one tick: in order, later ones see earlier unlocks. |
| ENG-SKILL-06 | Refusals are silent: unknown id, 0 points, hidden id with points, at max, a `Granted` slot. |
| ENG-SKILL-07 | `@jlvl -5` on a Swordman at job 10 with 0 free and 9 spent (Sword 1, Bash 5, Provoke 3) and NV_BASIC 9: message, `SP_JOBLEVEL 5`, exp ×2, `SP_SKILLPOINT 4`, `0x0441` 3, `0x0441` 7, `0x010f` (1 lv 9, 2, 4, 5, 6 lv 0), diff. NV_BASIC kept. |
| ENG-SKILL-08 | `@skreset` (captured): `SP_SKILLPOINT 44`, `0x010f` (1–8 all 0), `0x0441` 3/7/8, `0x010f` (1,2,4,5,6), message. As a Novice NV_BASIC is reset and refunded. `@reset`: stat-reset pushes, `SP_SKILLPOINT`, list, deletes, list, diff, message. The observer gets nothing. |
| ENG-SKILL-09 | `@allskill` on a Swordman with 44 points: `0x010f` (2–8 at 10, `upFlag` 0; NV_BASIC 9), `SP_SKILLPOINT 0`, message; no quest skills, no Knight skills, no deletes. |
| ENG-SKILL-10 | `@job`: Novice (NV_BASIC 9) → Swordman: one list (1 lv 9, 2, 4, 5, 6) at the step-8 position, no `SP_SKILLPOINT`, no `0x0441`; `change_level_2nd` not set (1st class), Swordman → Knight sets `change_level_2nd` = old job level; Knight → Mage: one Mage list, Knight levels hidden but still in memory; back to Knight before a save shows them again. |
| ENG-SKILL-11 | `SKILLS` dirty from `@blvl`/`@jlvl` with a gated tree entry (hand data: BaseLevel 100): the list goes out in Stage 3 before the diff; no change, no list. |
| PKT-SKILL-01 | `0x0112` parses into `UpgradeSkill`; `0x010e` (11 bytes) and `0x0441` (4 bytes) encode as in the capture. |
| PER-SKILL-01 | `save_play_state` writes skill rows (only level > 0) and the change levels in the same transaction, rewriting on change; `load_play_state` reads them back. |
| BIN-SKILL-01 | Skill-ups, relog: levels persist. `@job 2` from a Knight then relog then `@job 7`: Knight levels gone, NV_BASIC 9 kept (captured). |
| BIN-SKILL-02 | `SKILL_DATA_DIR` unset or pointing at a bad dir fails map-server startup with the path. |

## 4. Licensing
- **GPL data** goes only through the new `tools/skill-import` into `crates/ro-db/data/skill/`, with headers, a `NOTICE.md` section (source paths, commit, date, what changed, the regenerate command) and the DB-SKILL-01 header test.
- **Runtime:** read from `SKILL_DATA_DIR`. No `include_str!`, no codegen. `ro-db` keeps `exclude = ["data/"]`.
- **The importer and the tree code are clean-room**, written from catalog `db/skills.md` / `mechanics/skills.md`. They are not translated from `pc.cpp` / `skill.cpp`.
- **Test fixtures:** engine and `ro-db` unit tests use small hand-written trees. Tests may read the committed files from disk.
- **Skill names** (aegis) on the wire come from the data at runtime.

## 5. Deferred (move into `DEFERRED.md` at Finish)
- **Casting:** the use-skill packets, SP/AP costs, cast time, delays, cooldowns, effects (DESIGN.md §10/§11). Passive bonuses and weapon masteries (§9.9), including the stat diff *before* `0x010e` on a passive skill-up.
- **`NV_BASIC` level gates:** sit, trade, emotes, chat rooms, party and so on.
- **Quest/platinum skill grants** (NPC scripts, flag 3). Item-granted TEMPORARY skills and their `0x0441` on unequip (Bravery Bag). Plagiarism, guild and homunculus skills. Soul Linker spirit skills. Taekwon Ranker.
- **Option stripping** on reset and job change. The Bard/Dancer change-sex reset. `@skpoint`.
- **The two-pass job-change list** with `0x0441` (Dancer → Assassin, Rune Knight → Shadow Cross): we send one final list.
- **Skill hotkeys** (`0x0b20` naming skills).
- **Still uncaptured:**
  - the stale-list `@jlvl −n`;
  - hidden levels across a detour without a save;
  - the `change_level_2nd` threshold of 18;
  - `@skreset` as a Novice;
  - level-gated unlocks.
