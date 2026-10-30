# Implementation Plan: Skills (learn only)

Spec: `specs/skills/spec.md`. Design: `crates/ro-engine/DESIGN.md` §5, §9 (§9.5 dirty bits, §9.9 passives). Planned on 2026-10-28/29 together with `specs/traits/`. M0 was done in the planning session. **Start every session by reading this whole file**: the notes carry context that isn't in the code.

**Prerequisite:** `specs/traits/` must be **finished and committed** before M1 here. This plan uses its:
- full `pc_resetstate` (`reset_stats` in `crates/ro-engine/src/level_command.rs`), for `@reset`;
- extended `CharacterStatus` / `CharacterProgress`;
- `save_play_state` with trait columns;
- `CharacterSummary` with `sppoint`.

Line anchors below are from `0e0baab` (before traits). Re-grep them, because traits will have moved some.

## Decisions (settled with the user, 2026-10-28)
| Topic | Decision |
| :--- | :--- |
| Scope | **Learn only**: data, tree, skill-up, real list, resets, persistence. Skill effects are placeholders: no casting, no passive bonuses, no `NV_BASIC` gates. |
| GM commands | `@skreset` (`resetskill`), `@reset`, `@allskill` (+ aliases). |
| Captures | Recorded by the user on 2026-10-29 after research (`rathena_skills_sequence.txt`). The spec was finished from it. |
| *Planning defaults (user may revisit)* | The importer resolves tree inheritance and expands the per-level arrays, so `ro-db` stays a plain reader. Skill-up replies are sent immediately, not as follow-ups: there's no stat recalc, so per-request grouping in a burst matches rAthena. `@job` sends one final list: rAthena's two-pass `0x0441` form for some 2nd/3rd-class changes is a deviation. The class-change job levels are `characters` columns (rAthena: char vars). No dead/sitting check on skill-up (rAthena has none). Our own message wording. |

## Notes for the implementer

### Captures and catalog
- **`reference/catalog/captures/rathena_skills_sequence.txt`** (2026-10-29). File line = proxy # + 10.
  - **Sessions:** actor `#6` (relog `#8`), observer `#5`. The `0x00f3` chat payloads are empty in the proxy output, so the commands were identified by their effects.
  - **Write-up:** catalog `mechanics/skills.md` §10, with the step map in §10.1.
  - **Extra steps the user took:** `@jlvl 2` at 1020-1035, and the observer's `@job 7` / `@jlvl 20`.
  - **The client blocks `0x0112`** at 0 points and at max level (user, 2026-10-29). The server's silent refusal there only guards against a modified client. Test it, but there is nothing to capture. Hidden ids at 0 points are still sent and get no reply (`equip_sequence_4.txt` 2114-2153).
- **`equip_sequence_4.txt`** (session `#3`, Archer → Dancer) has more skill-ups and the Dancer → Assassin `0x0441` form. See catalog `mechanics/skills.md` §4, which uses the capture's own line labels.
- **The catalog corrects some earlier source-only notes:**
  - job changes don't always send `0x0441`;
  - list order is ascending id;
  - resets send a fresh list when the tree shrinks.

  `mechanics/skills.md` §10.8 lists them.

### Existing code anchors (as of `0e0baab`; re-grep after traits)
- **ro-core:**
  - `SkillInfo` is at `message.rs:321`.
  - `ServerEvent::SkillList` is at `message.rs:604`.
  - `ClientIntent` is at `message.rs:34`.
  - `CharacterProgress` is at `character.rs:59`.
- **ro-engine:**
  - `settings.rs:49` holds `nv_basic()`. It is used at login (`settings.rs:144-153`) and in `@job` (`level_command.rs:384-389`).
  - `stats.rs` has the dirty flags at `:26-30` (only `DIRTY` / `STALE_GEAR`), `recalc` at `:300`, and the `FollowUp` queue at `:47`.
  - `job.rs` has the masks at `:9-21`, `JOB_MAPIDS` at `:59`, `job_mapid` at `:217` and `job_for_sex` at `:225`.
  - `warp.rs:43` holds `PlaySnapshot`.
  - `command.rs:84-93` holds the GM `match`.
  - `level_command.rs` holds `command_job_level` (`:218`) and `command_job` (`:303`).
- **ro-packets:**
  - `0x0112` is only in `opcode.rs:109` / `table.rs:129`.
  - `encode_zc_skillinfo_list` is at `outgoing/settings.rs:37`.
  - The `ServerEvent` mapping is in `outgoing/encoder.rs:293`.
  - Check whether `0x010e` and `0x0441` already have rows in `table.rs`; the proxy frames them as KNOWN.
- **tools:**
  - `tools/job-import` (`job_ids.rs` has the job name → id table, with mounted and dummy jobs skipped) is the model for `tools/skill-import`.
  - The `Schema` header check lives in `crates/ro-db/src/yaml.rs`.
- **map-server:** `config.rs` (`JOB_DATA_DIR` pattern) and `main.rs:41` (required-dir error). `run_metaling_servers.sh` sets the data dirs.

### Pitfalls
- **Two different levels for the tree:** visibility uses the **effective** (normalised) job, while max level and `upFlag` use the **real** class.
- **NV_BASIC is kept** on reset for non-Novices, and its 9 points still count toward `P`.
- **Hidden levels** (a skill from a previous class) stay in memory and count toward nothing, because only visible slots count. They vanish at the next save. Don't save them, and don't drop them before the save (a quick job swap back shows them again; source-only).
- **Inline recomputation** in skill-up and `@allskill` must store the visible set, so Stage 3 finds no change and doesn't send a second list. Only the `SKILLS` dirty bit triggers Stage 3's diff. `@job` is different (review fix): its inline pass is rAthena's first pass, and it also marks `SKILLS` so Stage 3 sends the second pass.
- **Burst handling:** many `0x0112` arrive in one millisecond. Process them in order, each against the state the previous one left (captured 707-720). The client can request a skill unlocked by an earlier request in the same burst.
- **The 0x010f entry `name` is 24 bytes,** NUL-padded, from the aegis name. Names longer than 23 bytes are cut (rAthena `safestrncpy`).
- **Hot path:** the `SkillSet` is a small sorted `Vec`. Tree lookups go through `SkillTreeDb` slices. The visible-set computation should allocate at most once per call; it runs only on skill events, never per tick.
- **Normalisation edge cases:** Super Novice is not a second class for this rule. Summoner skips the Novice step. Baby and upper bits are kept only if the reduced job exists. Check each against catalog `db/skills.md` "Runtime" step 1 in tests.

## Session split (recommended)
| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: data** | M1 | A new importer, `ro-db` registries and map-server config. Leaf crates, fully testable alone. |
| **B: engine core** | M2, M3 | `SkillSet`, visibility, list building, login/`@job` list, Stage 3 tree diff, skill-up. These need one coherent context. |
| **C: flows and persistence** | M4, M5 | Resets, `@allskill`, schema, snapshot, BIN tests. |
| **D: finish** | Finish | `reviewer` over the full diff (fresh context), fixes, docs, DEFERRED, smoke, commit. Then the user's manual check. |

**Early review:** consider one `reviewer` pass after M2. M3–M5 build on the new per-player `SkillSet` state and the Stage 3 tree diff in the map tick (CLAUDE.md: shared state in the map-server). Otherwise review once at Finish.

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 catalog, captures, spec, plan** (2026-10-28/29, with traits)
  - `metaling-explorer` wrote `db/skills.md` and `mechanics/skills.md`, plus additions to `packets/settings.md` and `yaml_schemas.md`. The user recorded `rathena_skills_sequence.txt`, and it was written up (`mechanics/skills.md` §10). INDEX and README rows were added.
  - This spec and plan. Committed with traits' M0. **User checkpoint.**
- [x] **M1 data: `tools/skill-import`, `ro-db` registries, config** *(2026-10-29)*
  - A new crate `tools/skill-import` (workspace member, `[lints] workspace = true`, MIT-compatible deps). It reads `$RATHENA_PATH/db/re/skill_db.yml` and `skill_tree.yml` per spec §0. The rules it implements:
    - per-level expansion (catalog "List form expansion");
    - tree inheritance (catalog "Load-time inheritance");
    - job names to ids (share `tools/job-import`'s table);
    - trimmed fields;
    - headers.

    Write importer unit tests on small hand-written YAML: expansion linear vs repeat, `Exclude`, child-wins, later-parent-wins, `MaxLevel 0`.
  - Generate `crates/ro-db/data/skill/`. Add a `NOTICE.md` "skill" section. Data may commit early, as with items.
  - `ro-db`: `SkillDb`, `SkillTreeDb`, `load_dir`, root re-exports. Add tests DB-SKILL-01..03 on the shipped files.
  - Map-server: `SKILL_DATA_DIR` (required), loaded into the engine registries. Update `run_metaling_servers.sh` and `crates/ro-db/DESIGN.md`. Add test BIN-SKILL-02.
- [x] **M2 engine model, visibility, list** *(2026-10-29)*
  - ro-core changes:
    - `SkillInfo` stays.
    - New `ServerEvent::SkillUpdate` (`0x010e`) and `SkillDelete` (`0x0441`), with their encoders (ro-packets) and `table.rs` rows if missing.
    - `CharacterProgress.change_level: [u16; 3]`.
  - `BattleConfig` knobs (spec §2.1) and `config/battle.yml`.
  - `crates/ro-engine/src/skills.rs` (new):
    - `SkillSet` / `SkillSlot`;
    - `compute_visible` (spec §2.2), including the mapid normalisation and the reverse mapid → job lookup;
    - `skill_list(&SkillSet, class, &SkillDb, &SkillTreeDb) -> Vec<SkillInfo>` (spec §2.3, range2 bonuses).
  - `PlayerInfo` gets a `SkillSet`. Spawn and arrival compute visibility silently.
  - The login burst and `@job` step 8 send the real list. `@job` also records the change levels. Delete `nv_basic()`.
  - Add the `SKILLS` dirty bit. In Stage 3, the tree diff (`0x0441`…, `0x010f`) goes after `SP_MAXWEIGHT` and before the clamps (spec §2.6). `@blvl` and `@jlvl` set the bit.
  - Tests: ENG-SKILL-01..04, ENG-SKILL-10, ENG-SKILL-11, and the existing `@job` / login tests updated from `nv_basic`.
  - **Early review** (optional, recommended): `reviewer` over M1–M2.
- [x] **M3 skill-up** *(2026-10-30)*
  - `ClientIntent::UpgradeSkill { id }`, the `0x0112` decoder and dispatch.
  - The engine flow per spec §2.5: silent refusals, inline visibility, the passive pre-list, then `0x010e`, `SP_SKILLPOINT` and the list sent immediately.
  - Tests PKT-SKILL-01, ENG-SKILL-05, ENG-SKILL-06. Mutation-check the pre-list rule: always sending it, or never, must fail.
- [x] **M4 resets and `@allskill`** *(2026-10-30)*
  - `reset_skills` (spec §2.7).
  - `@jlvl −n` uses it with no send.
  - `@skreset` / `resetskill`.
  - `@reset` (traits' `pc_resetstate`, then `reset_skills`).
  - `@allskill` and its aliases (spec §2.8).
  - Messages are follow-ups.
  - Tests ENG-SKILL-07..09, with the captured line numbers in comments.
- [x] **M5 persistence** *(2026-10-30)*
  - Schema: the `character_skills` table and the `change_level_*` columns (spec §2.9).
  - `save_play_state` rewrites the rows when the skills revision changed. `load_play_state` returns the rows and change levels.
  - `PlaySnapshot.skills` and the revision; the recent-write record and transfer carry them. Autosave triggers on a skills revision change.
  - Update `crates/ro-persistence/DESIGN.md`.
  - Tests PER-SKILL-01 (DB up: `docker compose -f tools/docker/docker-compose.yml up -d db`) and BIN-SKILL-01.
- [x] **Finish** *(2026-10-30; all milestones ran in one session at the user's request)*
  - `./verify.sh --smoke` green.
  - Run the `reviewer` over `git diff HEAD` plus untracked files (or from the data commit). Apply its fixes.
  - Docs: `crates/ro-engine/DESIGN.md` §9.5 (the `SKILLS` bit) and §9.9 (the learned set as input; the passive-diff ordering note); `crates/ro-db/DESIGN.md`; `crates/ro-persistence/DESIGN.md`.
  - `DEFERRED.md`:
    - Add a "skills" section from spec §5.
    - Remove the landed items: map-entry-settings' "A real skill list (NV_BASIC's real level and job skills) with the skill tree"; stats' `@job` "class-change level vars", "the skill tree and skill deletes"; `@jlvl` negative `pc_resetskill`.
  - Commit.
  - **Manual client check (the user, after the commit):**
    1. A fresh Novice: Basic Skill to 9.
    2. `@job 1`: the Swordman tree.
    3. Sword Mastery unlocks Two-Handed; Bash 5 unlocks Magnum.
    4. `@jlvl -5` resets.
    5. `@skreset`, `@reset`, `@allskill`.
    6. `@job 7` (Knight tree after enough points).
    7. `@job 2`, relog, `@job 7`: the Knight levels are lost.
    8. Char select shows the skill points.

    Record a metaling capture and compare it with `rathena_skills_sequence.txt`.
