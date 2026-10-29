# Implementation Plan: Trait stats, AP, stat-point grants, stat reset, char-select values

Spec: `specs/traits/spec.md`. Design: `crates/ro-engine/DESIGN.md` §5 and §9 (the stats model this extends). Planned on 2026-10-28/29. M0 was done in the planning session; M1 onward is for new sessions (see "Session split"). **Start every session by reading this whole file**: the notes carry context that isn't in the code.

The **skills** feature (`specs/skills/`) was planned in the same session and is implemented **after** this one. It calls this feature's `pc_resetstate` (for `@reset`) and builds on the `CharacterProgress` / `save_play_state` / `CharacterSummary` changes made here.

## Decisions (settled with the user, 2026-10-28)
| Topic | Decision |
| :--- | :--- |
| Feature split | Two features, each with its own spec, plan, review and commit: **traits first**, then skills (learn only). |
| Trait scope | The full set: POW..CRT allocation via `0x0b24`, trait points (`@job` +7, `statpoint.yml` from B201), derived PATK/SMATK/RES/MRES/HPLUS/CRATE, MaxAP and AP, persistence, real status-window values. |
| Stat-point work | `@job` trans ±52 and trait +7 grants with their resets, the full `pc_resetstate`, and real char-select values (HP/SP, points, stats). |
| GM commands | `@streset` (alias of `@resetstat`) here. `@skreset`, `@reset` and `@allskill` go in skills. |
| Captures | Recorded by the user on 2026-10-29 after catalog research (`rathena_traits_sequence.txt`). The spec was finished from it. |
| *Planning defaults (user may revisit)* | Our own message wording for `@streset`. Trait-up refused while dead (as stat-up). rAthena's stale `0x0141` ×12 at a reset stays skipped (the stats ruling). MaxAP not persisted (derived). `keep_ap_on_logout` behaviour is fixed on (no knob). New `BattleConfig` knobs: `max_ap` 1000, `ap_rate` 100, `transcendent_status_points` 52, `trait_points_job_change` 7, all with rAthena's shipped defaults. |

## Notes for the implementer

### Git state at hand-off
- M0 is committed on its own as "Traits and skills: M0 catalog, captures, specs and plans". It holds the catalog files `mechanics/trait_stats.md`, `mechanics/skills.md` and `db/skills.md`, the edits to `stats.md`, `status.md`, `char.md`, `settings.md`, `atcommands.md`, `exp_tables.md`, `yaml_schemas.md` and `INDEX.md`, the two captures, and both features' spec and plan.
- M1 onward stays uncommitted until Finish. Generated data may commit early (items precedent).

### Captures
`reference/catalog/captures/rathena_traits_sequence.txt` (2026-10-29) is redacted (PINs, `0x008e`) and written up in catalog `packets/status.md` "Capture: traits", `mechanics/trait_stats.md` §7 (values) and `packets/char.md` "Captured select-screen values". **File line = proxy # + 10.** Key facts the spec relies on:
- **Actor and observer:** the actor is `#6` (relog `#8`), a **Knight**, not a Novice. Its `@blvl 199` clamped to B99, so the DK work happens after `@job 4252` and `@blvl -199` / `@blvl 199`. The observer `#3` never sees trait or reset packets.
- **Trait-up:** the order is the diff (derived first, the trait's `0x0141`, trait-derived), then `0x00be` `SP_U*`, `SP_TRAITPOINT`, `0x00bc`. The ack value is the **allocated** stat.
- **`@blvl -10` at B205 with 2 trait points:** the reset is triggered by the trait pool alone. After the reset: status 4151 (4099 + 52), trait 26 (19 + 7). After the subtraction: 3980 and 7.
- **`@job` out of DK with < 7 trait points:** a **double reset**, because the first reset already uses the new (non-trait) class.
- **Login block:** `SP_TRAITPOINT`, `SP_AP`, `SP_MAXAP`, then `SP_U*`. `SP_AP` is sent even at 0.
- **Not captured** (so source-only, from catalog `trait_stats.md` §3–§4): the `0x0b24` failure (the client blocks the request at 0 points, user 2026-10-29; the reply only guards against a modified client), amount > 1, leaving a trait job with ≥ 7 unspent, trait job → trait job, the cap, SPL/WIS, AP > 0.

### Catalog fact corrected in M0
`db/exp_tables.md` claimed `TraitPoints` were "+3 per level". The capture and the source show 3, 6, 9, 12, **19** (+7 at every fifth level). Use the generated table, not a formula.

### Existing code anchors (as of `0e0baab`, 2026-10-28)
- **ro-core:** `CharacterStatus` / `CharacterProgress` are in `crates/ro-core/src/character.rs:14,59`; `CharacterSummary` is in `ids.rs:30`; `StatusWindow` and `SkillInfo` are in `message.rs:321-345`.
- **ro-mechanics:** `StatusInputs` / `JobRow` / `Status` are in `status.rs:144,60,160`; `calc_status` is at `:220`. `points.rs` holds the stat cost and raise. In `config.rs`, `max_trait_parameter` (`:66`) is unused so far.
- **ro-engine:**
  - `stats.rs`: `FollowUp` `:47`, `mark_dirty` `:153`, `compute_status` `:178`, `recalc` `:300` (`SP_MAXWEIGHT` → clamp → `status_diff`).
  - `status.rs`: SP ids `:17-50` (`TRAIT_PARAMS`, `SP_UPOW`), `status_diff` `:126`, `send_login_status_push` `:225`, `send_trait_couples` `:272`, `send_map_change_stats` `:288`, `send_initial_status` `:303` (trait zeros at `:340-351`).
  - `stat_up.rs:37`: `raise_stat` is the template for `raise_trait`.
  - `level_command.rs`: `reset_stats` `:179` (trait zeros `:202-212`), `command_job` `:303` (steps in comments), `base_tail` `:401` (`SP_TRAITPOINT` 0 at `:404`).
  - `command.rs:84-93`: the GM command `match`.
  - `job.rs:262`: `class_flags(mapid)`.
- **ro-packets:** `0x0b24` is only in `opcode.rs:984` / `table.rs:1014`. The decoder pattern is `incoming/map.rs:518-540` (`CzStatusChangePacket`), dispatched in `incoming/frame.rs:130`. `encode_character_info` is `outgoing/char.rs:13`.
- **ro-persistence:** the guarded-`ALTER` pattern is `sql/01_schema.sql:66-107`. `save_play_state` / `load_play_state` are `src/repository/inventory.rs:22,28` and `src/postgres/inventory.rs:182-300`. `summary_columns!` is `src/postgres/character.rs:18`.
- **tools/job-import:** `lib.rs` drops `TraitPoints` at `:147` and AP at `:137`. `RawEntry` is at `:201`.

### Pitfalls
- **The order of `0x00be` in `@job` matters** (spec §2.7): trans points, `0x00be` ×6, then trait points. Each reset sends its own immediate pushes; **don't** dedupe the double reset. Mirror the captured lines in a test.
- **A reset reads the class in effect.** `command_job` already sets the job before step 3, so pass the class (or read it from `PlayerInfo`) rather than caching the old one.
- **Allocated traits survive** a trait → non-trait change when ≥ 7 points are unspent. They keep feeding derived values with a cap of 0. Don't "fix" that.
- **`status_diff` already allocates two `Vec`s per recalc** (DEFERRED). Extend it without adding a third. Keep the hot path in line with CLAUDE.md.
- **`CharacterStatus` is `Copy`.** Adding `[u16; 6]` keeps that. Check the transfer struct (`MapTransfer`) and the recent-write record take the new fields through `CharacterProgress` with no extra plumbing.
- **The pre-ready login push** is a fixed list today. Append the trait values conditionally (non-zero), matching capture lines 1374-1403.
- **Char select:** the select-screen values are whatever was last saved. Map-server autosave (and the immediate `@job` save) is what makes them fresh. No extra save is needed.

## Session A hand-off (M1–M3, 2026-10-29)

Done and green (`./verify.sh`: 774 tests). Uncommitted, per the plan.

### Rulings
- **AP has no job guard.** A metaling-explorer pass checked the source: rAthena's AP fallback (`calc_baseap`, 200 from B200) fills every job, and MaxAP has no trait-job guard either. So 3rd classes at B200 (Rune Knight etc.) get MaxAP 200, as in rAthena. Spec §0 and DB-TRAIT-02 were fixed. The explorer also corrected catalog `trait_stats.md` §2 (`BaseAp` lives in `job_basepoints.yml`, trait jobs only, and is redundant with the fallback). 73 jobs now carry `BaseAp`.
- **`JobDb::trait_points` past the table** reads the last row, like `status_points` (the spec said 0). No level passes 275, so this can't happen in play, and a 0 would turn a level-up delta negative.
- **Pre-ready push placement:** the trait values go after `SP` and **before** `WEIGHT` (capture 1401-1403), not at the very end. Spec §2.3 was fixed.
- **Data versions:** both `JOB_DATA` and `STATPOINT_DATA` are now version 2. `TraitPoints` is omitted when 0, and `BaseAp` is omitted when all zero.
- **Config validation:** `max_ap` must be at least 100 (rAthena's floor) and `ap_rate` at least 1.

### Follow-up capture (2026-10-29)
`reference/catalog/captures/rathena_trait_sequence_2.txt` was recorded after M3, then redacted and written up (catalog `packets/status.md` "Capture: traits 2", `trait_stats.md` §7, captures README). File line = proxy # + 10. For session B:
- **Confirmed:**
  - MaxAP 200 on Rune Knight T at B200 (the no-guard ruling);
  - the login push order M3 implemented (2558-2565);
  - trait job → trait job sends nothing extra (DK ↔ Meister, 2029-2080). ENG-TRAIT-06 can cite it.
- **New `@job` case for M5:** DK (B99, 0 trait points) → Rune Knight T (416-489). There is no ±52, because both classes count as "upper or primary 4th". The single reset grants +52 because the new class is upper (1325). This is a good extra ENG-TRAIT-05/06 vector.
- **Still uncaptured:** trait → non-trait with ≥ 7 unspent. Both attempts hit the level-cap double reset (1074-1150, 1643-1764). These are more vectors for the captured double reset; implement the ≥ 7 branch from source.
- **AP > 0:** `@healap [<AP>]` sets it (it needs MaxAP > 0, so B200 or higher; source-verified 2026-10-29, catalog `trait_stats.md`). Not captured yet, so ENG-TRAIT-08 stays source-only for now.
- **Source-verified 2026-10-29:**
  - The clamp order is `[SP_MAXWEIGHT]`, `SP_HP`, `SP_SP`, `SP_AP`, then the diff, which re-sends each clamped value. This matches M3's `recalc`.
  - The ≥ 7 trait-job exit runs: level-cap reset/tail, ±52 (or a reset), `0x00be` ×6, then `trait_point = 0` + `SP_TRAITPOINT` + `SP_UPOW..SP_UCRT`, then the job pushes. Allocated traits are untouched.

### What session B builds on
- **ro-core:** `CharacterStatus` has `traits: [u16; 6]`, `trait_point: u16` and `ap: u32`, all 0 in `Default`.
  - **M6 must replace the placeholder** in `crates/ro-persistence/src/postgres/inventory.rs` `load_play_state`. Today it sets the three to 0, with a comment.
- **ro-mechanics:**
  - `Traits { pow..crt }`, `JobRow.trait_bonus`/`base_ap`, `StatusInputs.traits`.
  - `Status.trait_base`/`trait_battle`/`patk`/`smatk`/`res`/`mres`/`hplus`/`crate_`/`max_ap`.
  - `BattleConfig::max_trait(&ClassFlags)`, `transcendent_status_points`, `trait_points_job_change`.
  - `next_trait_cost`, `raise_trait` (returns `StatRaise`; cost = amount).
- **`class_flags(..).trait_job` is verified** against the catalog set for every shipped job (unit test `trait_job_matches_the_catalog_set` in `job.rs`). No fix was needed.
- **Engine diff model:**
  - `PlayerInfo.allocated` is `[u16; 12]` (STR..LUK, then POW..CRT).
  - `status_diff(old, new, (&Persisted, &Persisted))`, where `Persisted` holds allocated ×12 plus HP/SP/AP after the clamp. Still two `Vec`s.
  - `recalc` pushes the HP, SP, then AP clamp before the diff. `init_status` clamps AP silently.
- **Engine helpers:**
  - `status::trait_costs(&traits, cap)` gives the six `0x00be` `SP_UPOW..` events. `SP_AP` is `pub(crate)` in `status.rs`.
  - `base_tail(status_point, trait_point, level)` now sends the running `trait_point`. The `@blvl` trait-point **deltas** are still M5.
  - `reset_stats` already sends real trait costs and the real `SP_TRAITPOINT`. It doesn't yet zero traits or add +52/+7: that is M5's `pc_resetstate` rework.
- **Test harness for M4/M5:** `crates/ro-engine/tests/trait_tests.rs` (DK helpers, `World::command`, login burst/init capture). Add ENG-TRAIT-01..06 there.
- The existing ENG-STAT-01/09 tests needed no value changes, because every character in them has a non-trait job. ENG-STAT-01 gained `eng_stat_01_trait_block_after_sp`.

## Session B hand-off (M4–M6, 2026-10-29)

Done and green (`./verify.sh`: 797 tests, DB up). Uncommitted, per the plan. Not yet run: `./verify.sh --smoke` (Finish).

### Rulings and deviations
- **`RaiseTrait { var: u16, amount: i16 }`.** The wire `var` is `i16`; it is kept as its u16 bit pattern (only 219..224 are valid, and the refusal echoes it in a u16 field). The decoder is `CzAdvancedStatusChangePacket`.
- **The > 255 trailing `0x0141` is implemented for traits** (spec §2.4 fixed). `max_trait_parameter` validates up to 32767, so it is reachable by config. `FollowUp::CoupleStat(index)` now takes 0..12 (STR..LUK, then POW..CRT), and `status::couple(index, ..)` maps the index to its var.
- **`reset_stats(e)` takes no arguments.** It reads the class and `clevel` in effect, sets both pools (+52 / +7), zeroes traits, marks dirty and always pushes. The old `push = false` path (the `@job` level-cap reset in stats) is gone: the capture shows reset #1 sending its pushes. ENG-STAT-08's cap test was updated to match.
- **The ±52 test is `upper || fourth`** on both classes (`gets_transcendent_points`). So DK → Rune Knight T has no ±52, as captured.
- **`CharacterSummary.select_stats: CharacterSelectStats`** groups the nine select-screen fields (spec §2.9 fixed). Every summary literal takes `Default::default()`. The Postgres reader turns an out-of-range value into 0, like the looks, rather than failing the list.
- **Schema:** one guarded `DO` loop adds the eight columns (ALTER only, not in `CREATE TABLE`, like `unique_item_counter`). PER-TRAIT-01 has a migration test in its own Postgres schema, as PER-EQ-02.
- **Test placement:** the dead refusal and the > 255 couple are unit tests in `stat_up.rs` (they set `dead_sit` / a config directly). Everything else is in `crates/ro-engine/tests/trait_tests.rs`, which gained an observer (`World::observer`, `World::observed`). BIN-TRAIT-01 is `bins/map-server/tests/trait_tests.rs`. It uses a new `InMemoryRepo::set_progress` to start a DK at B200 with stored AP, since nothing grants AP yet. PKT-CHAR-01 is `crates/ro-packets/tests/char_codec_tests.rs`.
- **Mutation check done:** dropping reset #2 (the `trait_point < 7` reset in `take_job_trait_points`) fails three ENG-TRAIT-06 tests.
- **Capture line numbers** in the catalog, the spec and the tests are **file** lines (e.g. 956 is the `@job 7` chat, 1073 its message), not proxy `#`.

### For session C (Finish)
- The engine diff for these flows didn't change shape. `crates/ro-engine/DESIGN.md` §9 still needs the trait inputs and outputs, the diff order and the AP clamp (Finish item), plus a line on `@streset` and the new `@job` grant steps.
- The `@blvl −n` reset deviation stays: our reset's stale `0x0141` ×12 and its own recalc fold into the tick's single diff.
- The changed `level_command.rs` also affects the stats feature's flows (the `@job` cap reset now pushes). Point the reviewer at ENG-STAT-08 and ENG-STAT-06.

## Session C (Finish, 2026-10-29)
- The `reviewer` found nothing must-fix. Applied its nice-to-haves:
  - range checks for `transcendent_status_points` / `trait_points_job_change` (0..=32767);
  - a misplaced doc comment in `postgres/character.rs`;
  - the past-the-table ruling in spec §1 and `ro-db` DESIGN;
  - the superseded/landed notes in `specs/stats/spec.md`;
  - a capture line ref.
  
  The `status_diff` `STATS` const was left as is.
- `metaling-explorer` confirmed the +52 test is `JOBL_UPPER || pc_is_primary_fourth` in both `pc_jobchange` and `pc_resetstate`. That matches `upper || fourth`: expanded 4th classes and Hyper Novice get no +52. Recorded in catalog `mechanics/job_change.md` step 8.
- `./verify.sh --smoke` is green (797 tests). It needs the rAthena containers stopped, because they hold the same ports.

## Session split (recommended)
| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: data, formulas, model** | M1, M2, M3 | Importer and `ro-db` are leaf crates. `ro-mechanics` is pure. M3 extends the engine model and pushes, and needs the M2 types fresh. |
| **B: flows and persistence** | M4, M5, M6 | `0x0b24`, the `@blvl`/`@job`/`@streset` flows and persistence all build on M3's API. |
| **C: finish** | Finish | The `reviewer` over the full diff (fresh context), fixes, docs, DEFERRED, smoke, commit. Then the user's manual check. |

If a session runs short, cut at a milestone boundary, never mid-milestone. No early review: nothing here is an API between crates beyond added fields. The persistence change is additive columns, reviewed at Finish.

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 catalog, captures, spec, plan** (2026-10-28/29)
  - Interview with the user (decisions above). Three `metaling-explorer` research passes wrote `mechanics/trait_stats.md`, `mechanics/skills.md` and `db/skills.md`, plus additions to `atcommands.md`, `char.md`, `status.md` and `settings.md`.
  - The user recorded `rathena_traits_sequence.txt` and `rathena_skills_sequence.txt`. Both are redacted and written up, with INDEX and README rows.
  - This spec and plan, and the skills spec and plan. Committed on their own. **User checkpoint.**
- [x] **M1 data: trait points and AP tables** *(session A, 2026-10-29)*
  - `tools/job-import`:
    - Keep `TraitPoints` (optional, cumulative) in `statpoint.yml`.
    - Read `BaseAp` from `job_basepoints.yml` and `ApFactor`/`ApIncrease` from `job_stats.yml`.
    - Emit `base_ap` per job, fallback-filled with `calc_baseap` (catalog `trait_stats.md` §2) like the HP/SP tables.
    - Bump the data format version. Update the importer tests.
  - Regenerate `crates/ro-db/data/job/`. Update the `NOTICE.md` job section (what changed, date).
  - `ro-db`: `StatPointRow.trait_points`, `JobDb::trait_points`, `Job::base_ap`. Update `crates/ro-db/DESIGN.md`. Tests DB-TRAIT-01, DB-TRAIT-02, plus the existing header test.
- [x] **M2 `ro-mechanics` formulas** *(session A, 2026-10-29)*
  - `Traits` inputs/outputs, `JobRow.trait_bonus` and `JobRow.base_ap`. New `Status` fields: traits base/battle, `patk`, `smatk`, `res`, `mres`, `hplus`, `crate_`, `max_ap`.
  - Formulas per spec §2.2 (batk +5×POW, HIT/FLEE +2×CON, six trait-derived, MaxAP with `ap_rate` and the `max_ap` cap).
  - `points.rs`: `next_trait_cost`, `raise_trait`. A trait cap helper on `BattleConfig` (cap = `max_trait_parameter` if `ClassFlags.trait_job`, else 0).
  - `BattleConfig`: `max_ap`, `ap_rate`, `transcendent_status_points`, `trait_points_job_change`, plus `config/battle.yml`.
  - Tests MECH-TRAIT-01..04. Keep MECH-01..06 green: the added terms are 0 for non-trait jobs.
- [x] **M3 engine model and pushes** *(session A, 2026-10-29)*
  - `CharacterStatus` gains `traits`, `trait_point`, `ap` (ro-core). Fix every constructor and test helper.
  - Before relying on `ClassFlags.trait_job`, check `class_flags` against catalog `trait_stats.md` §1 "Trait job set" and fix it if needed (the stats plan flagged it as unverified).
  - `compute_status` feeds the allocated traits, the job's trait bonus and `base_ap`.
  - `status_diff`: trait block after SP (spec §2.3), `SP_MAXAP`, `SP_AP`. Add the AP clamp to `recalc` with the HP/SP clamps.
  - Real values in: `send_initial_status` (couples, PATK..CRATE, `SP_TRAITPOINT`/`SP_AP`/`SP_MAXAP`, `SP_U*` costs), `send_trait_couples` / map change, the pre-ready push (non-zero trait values appended), `StatusWindow` (unchanged; no trait fields in `0x00bd`), the `reset_stats` trait costs and `base_tail` `SP_TRAITPOINT`. Spawn clamps AP silently.
  - Tests: ENG-TRAIT-07, ENG-TRAIT-08, and the existing ENG-STAT-01/09 updated where trait zeros became values. Extend `status_diff`'s order test with the trait block.
- [x] **M4 `0x0b24` trait-up** *(session B, 2026-10-29)*
  - ro-core `ClientIntent::RaiseTrait { var, amount: i16 }`. ro-packets decoder and dispatch.
  - Engine `raise_trait` per spec §2.4 (failure immediate; success follow-ups `0x00be`, `SP_TRAITPOINT`, `0x00bc`; negative amount dropped; dead refused).
  - Tests PKT-TRAIT-01, ENG-TRAIT-01, ENG-TRAIT-02.
- [x] **M5 level, job and reset flows** *(session B, 2026-10-29)*
  - Rework `reset_stats` into the full `pc_resetstate` (spec §2.6): +52 for upper or primary 4th, `trait_points` + 7 for a trait job, traits to 0, immediate pushes `0x00be` ×12, `SP_STATUSPOINT`, `SP_TRAITPOINT`. Keep its call sites (`@blvl −n`, `@job` level cap).
  - `@blvl`: trait-point deltas both ways; the reset condition gains `trait_point < remove_trait` (spec §2.5).
  - `@job`: trans ±52 and trait +7/zero/reset steps around the `0x00be` ×6 (spec §2.7), reproducing the captured double reset.
  - `@streset` / `@resetstat` (spec §2.8): a new arm in `command.rs`, the body beside `reset_stats`, the message as a follow-up.
  - Tests ENG-TRAIT-03..06 in `tests/level_command_tests.rs` (or a new `trait_tests.rs`), with the captured line numbers in comments. Mutation-check the double reset (drop reset #2 → fail).
- [x] **M6 persistence and char select** *(session B, 2026-10-29)*
  - Schema: guarded `ALTER`s (spec §2.9). `save_play_state` / `load_play_state` gain the columns; the map-server spawn takes AP.
  - `CharacterSummary` gains the select fields. `summary_columns!` and every summary constructor are updated (char-server, tests). `encode_character_info` gets real values with the caps.
  - Update `crates/ro-persistence/DESIGN.md`.
  - Tests PER-TRAIT-01 (needs the DB: `docker compose -f tools/docker/docker-compose.yml up -d db`), BIN-TRAIT-01, PKT-CHAR-01.
- [x] **Finish** *(session C, 2026-10-29)*
  - `./verify.sh --smoke` green.
  - Run the `reviewer` over `git diff HEAD` plus untracked files (or from the M0 commit if data was committed early). Apply its fixes.
  - Docs: `crates/ro-engine/DESIGN.md` §9 (trait inputs and outputs, the diff order, AP clamp), `crates/ro-db/DESIGN.md`, `crates/ro-persistence/DESIGN.md`.
  - `DEFERRED.md`: a "traits" section from spec §5. Remove the landed stats items: "trait stats and AP", "`SP_TRAITPOINT` is sent as 0", the `@job` "transcendent (±52) and trait (+7) point grants and their stat resets", the char-select HP/SP/skill-point placeholder, and map-entry-settings' "Trait stats, their costs and AP are 0".
  - Commit (the plan is complete before the user's manual check; what the check finds are fixes in their own commits).
  - **Manual client check (the user, after the commit):**
    1. `@job 4252` at B99 shows 7 trait points and 1325 status points.
    2. Click + on each trait. The window shows P.ATK/S.MATK/RES/MRES/H.PLUS/C.RATE.
    3. `@blvl 101` to B200 shows MaxAP 200. `@blvl 5` gives 19 more trait points.
    4. `@blvl -10` resets.
    5. `@streset`.
    6. `@job 7` (the double reset).
    7. Relog keeps traits and points.
    8. Char select shows real HP/SP and stats.

    Record a metaling capture and compare it with `rathena_traits_sequence.txt`.
