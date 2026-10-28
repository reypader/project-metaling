# Implementation Plan: Character stats

Spec: `specs/stats/spec.md`. Design: `crates/ro-engine/DESIGN.md` §5 (revised tick order) and §9–§11. Planned on 2026-10-25. M0 was done in the planning session; M1 onwards is for new sessions (see "Session split"). **Start every session by reading this whole file**: the notes below carry context that isn't in the code.

## Decisions (settled with the user, 2026-10-25)
| Topic | Decision |
| :--- | :--- |
| Implementation scope | **Core player stats only**: job data, `job_level`, every derived stat, refine, stat-up, real `0x0141`, `@blvl`/`@jlvl`/`@job`. Out: auto-attack, skill casting, scripting, item/card bonuses and any equip effect beyond ATK/MATK/DEF/MDEF, status-effect runtime, damage code, mob stat blocks. DESIGN.md describes those so later plans plug in. |
| Classes | Job tables and stat caps for every job, including 3rd/4th. Trait stats (POW..CRT, AP, `0x0b24`) stay 0. |
| Recalc timing | **Tick order:** (1) timers/intents/hits → (2) effects emit modifiers / mark dirty → (3) recalc dirty actors, primary → secondary → (4) HP/SP deltas, clamps to the new max, follow-ups (heal after level-up) → (5) emit. All simulation reads the snapshot committed last tick. **Recalc never runs inline in a flow** (only spawn/login compute synchronously, without a diff). The user asked whether deferring to the *next* tick was fine; we settled on same-tick so clamps and follow-ups see the new max. |
| Modifier model | Named rAthena-style bonus slots with a source (gear, card, passive, status). Full rebuild by a pure function (no incremental edits). Base and battle layers; each secondary formula reads the layer rAthena reads. Gear sums cached, re-summed only on a gear change. |
| Item bonuses (future) | Compiled to declarative data by the importer; conditional scripts flagged for the VM. Not in this plan. |
| Hard MDEF | Stays 0 here. The user suggested promoting script MDEF to sit with DEF; ruling recorded in DESIGN.md §9.8 (`MagicDefense` item field lifted from unconditional `bonus bMdef` by the future item-bonus importer). |
| Status effects (future) | Data-driven registry in our own format (DESIGN.md §10), Rust hooks only for odd cases. |
| Config knobs | Typed `BattleConfig` in `ro-mechanics`, our own YAML (`config/battle.yml`), rAthena **shipped** defaults, `BATTLE_CONFIG` env var. |
| GM commands | `@blvl`, `@jlvl`, `@job` (with the job gear re-check and force-unequip). |
| *Planning defaults (user may revisit)* | Stat-up refused while dead (rAthena doesn't check). Our own message wording for the GM commands. `@job` skips the skill tree, option stripping, trans/trait point grants (spec §5). ~~Stat-up and level pushes come after the acks at tick end~~ **superseded 2026-10-26 by the capture, confirmed by the user the same day:** flows queue their trailing pushes as Stage 4 follow-ups, so the wire order matches rAthena (spec §2.1 "Follow-up queue"). M3 must also flush or drop a player's queue when they leave the map or disconnect in the same tick. |

## Notes for the implementer

### Git state at hand-off
- M0 (DESIGN.md, catalog, this spec and plan) is committed on its own as "Stats: M0 design, catalog, spec and plan" (`3e03ebb`; CLAUDE.md allows M0 docs on their own). M1 onwards stays uncommitted until Finish; generated data may commit early (items precedent).
- The `0x00bc` proxy framing fix is committed on its own (`5317367` "Proxy: frame ZC_STATUS_CHANGE_ACK"). Everything else since `3e03ebb`/`1ec97c0` (M1-M6 code, regenerated `crates/ro-db/data/{job,refine}/`, captures 2-3 and their catalog write-ups, spec fixes, `config/battle.yml`, the compose `name: metaling` and the CLAUDE.md crate-graph/DB-command edits) is **uncommitted** on `main`. The Finish reviewer diff is `git diff HEAD` plus untracked files.
- **Ledger:** `.superpowers/sdd/plan/progress.md` (git-ignored) holds every `Ruling:` line and the M3 review outcome. Read it at session start; the final message at Finish lists its rulings and deferred minors.

### Captures (recorded 2026-10-26, processed in session A)
`reference/catalog/captures/attribute_sequence.txt` is redacted (PIN, `0x008e`, `0x01c3`) and written up:
- catalog `packets/status.md` "Captured stat-up, level and job sequences";
- catalog `mechanics/stats.md` "Captured status values";
- confirmation notes in `atcommands.md` and `job_change.md`;
- an INDEX row and a captures README row.

The spec was fixed where the capture disagreed. What it settled:
- **Stat-up:** recalc diff first (`SP_MAXWEIGHT` before the stat's `0x0141` for STR), then `0x00be`, `SP_STATUSPOINT`, `0x00bc`. Over-affordable amounts raise as far as possible. No failure was captured: the client doesn't send one.
- **`@blvl +n`:** diff, heal (`SP_HP`, `SP_SP`), `0x019b`, message, then `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, exp. **`@blvl −n`:** message, clamp pushes, diff, tail. The spec had the tail first; that was wrong.
- **`@jlvl`:** up is effect, message, pushes, diff; down is message, pushes, clamp, diff.
- **`@job`:**
  - `0x010f` **is** re-sent (after the looks). The spec now does that; it is cheap.
  - A forced unequip of a two-hander sends `LOOK_WEAPON` twice, then `0x099a`, right after the job-level pushes.
  - The trans ±52 and trait +7 grants, and the stat resets they trigger, were captured but stay out of scope.
- **Recalc details:**
  - A clamped HP/SP is pushed before the diff and again in it.
  - The diff compares internal units, so LUK re-sends `FLEE2`/`CRITICAL`.
- **Values:** Novice B1 with Knife + Cotton Shirt (amotion **590**, not the bare-hand 440), Novice B11/J10, Bard B99, Swordman, Knight. Also ASPD steps, shield +50, Blade/Guard +6 refine. They are MECH-01..04 vectors, and ENG-STAT-11 checks them against the shipped data.
- **Not captured** (an observer, the `@blvl −n` reset, `elvmax`, a stat-up failure): listed in spec §5.
- **Left open on purpose:** a Knight wearing a katar (can't use it) shows amotion **1250**. The catalog's "unlisted weapon = base 200" predicts 2040. It only shows up because rAthena recalcs before the forced unequip. Our single recalc runs after it, so the engine never computes ASPD for an unusable weapon.
- **4th-class HP/SP confirmed:** `@job 4254` in the capture is **Shadow Cross** (not Dragon Knight). Its MaxHP/MaxSP at two VIT/INT values match the catalog fallback formula with ×1.25 exactly (catalog stats.md "4th-class MaxHP/MaxSP"). A stopped explorer noted that Dragon_Knight *does* have `job_basepoints` rows. Check catalog `db/job_stats.md`'s "4th classes have no tables" against the import report in M1.

### Catalog facts gathered in M0 (all written into the catalog)
- `mechanics/battle_config.md` (new): every knob the formulas read, conf file + line, default, read site. **Shipped conf values differ from compiled defaults** for `max_sp` (1000000 vs 32500), `max_ap`, and the 3rd/4th/extended/summoner/baby-3rd `max_*_parameter` (shipped 130/130/130/130/130/117). Use shipped. `natural_heal_weight_rate` is commented out in the conf, so the compiled 70 applies. ASPD caps convert `(2000 − aspd×10)×2` at load; clamp `amotion ≥ cap/2`. Max-param selection order: baby-third, baby, summoner, extended, 4th (trait), third-trans, third, trans, default. The per-job `MaxStats` overrides would win, but it's empty in the DB. `start_status_points` lives in `inter_athena.conf`. `atcommand_max_stat_bypass` doesn't exist (group permission `bypass_max_stat`). `critical_rate` is skipped for players. ~~Open: whether 4th classes carry the third-class flag~~ settled by the capture: they do (193 cap).
- `db/refine.md` (new) + `mechanics/stats.md` "Refine bonuses": `Bonus`/`RandomBonus` are 1/100 units and **cumulative per level** (the +N entry is the whole bonus). MAX_REFINE 20. No bonus unless `Refineable` and refine > 0. Weapon `atk2 += Bonus/100`; weapon MATK `+= Bonus/100` except bows; armor `refinedef += Bonus` over **every worn Armor-type item, any location**, then once `def += (refinedef + 50)/100` (rounded on the sum). Overrefine `= RandomBonus/100`, used only in damage. Shadow groups have no bonus. Grade bonus only for weapon level 5 / armor level 2 (deferred; its `enchantgrade − 1` indexing is flagged unverified). The captured +10 values match.
- `mechanics/atcommands.md` "@baselevelup / @joblevelup": spec §2.5 is a faithful summary. Note the `elvmax` check runs on level down too (harmless), there is **no heal or effect on level down**, and points are removed after an optional `pc_resetstate` *before* the decrement (status points = table at the **old** level).
- `mechanics/job_change.md` (new): the 22-step `pc_jobchange`; spec §2.6 keeps steps 5–7, 9, 11, 12, 14, 19–20 and the save. **Correction made in M0:** the explorer first wrote the skill list as `0x0b32`. The login capture and `packets/settings.md` show `0x010f` (the `0x0b32` gate is false for our client); job_change.md and INDEX.md were fixed.
- Already catalogued and reused: stat-up order (`packets/status.md`), recalc push order, job data (`db/job_stats.md`), statpoint (`db/exp_tables.md`), formulas (`mechanics/stats.md`).

### Existing code anchors (line numbers as of 2026-10-25, commit `dd8efca`)
- **`crates/ro-engine/src/status.rs`:** `DerivedStatus` :77 (becomes / is replaced by the `ro-mechanics` `Status` + an engine wrapper), `derived_diff` :141 (extend; keep ENG-STAT-01's order test), `stat_raise_cost` :178 (moves to `ro-mechanics`, cap per job), `status_window` :197, `send_login_status_push` :232 (`0x0141` bonus is 0 today), `send_initial_status` :286, `current_derived` :363, `init_derived` :375, `recalc` :383 (becomes "mark dirty"; the actual pass runs in `step_tick`).
- **`crates/ro-engine/src/wear.rs`:** inline `self.recalc(e)` at :181 (equip) and :339 (unequip, `how.recalc`). Both become mark-dirty. The per-item recalc of unequip-all collapses into one.
- **`crates/ro-engine/src/equipment.rs`:** `GearStatus` :288, `gear_status` :302, `hands()` (the two-hander-on-`HAND_L` fix; DEFERRED "Per-hand values"). Extend `GearStatus` to per-hand records (item ATK, refine `atk2`, MATK, wlv, overrefine, weapon type) plus Σ armor DEF, Σ refine-def, and shield presence.
- **`crates/ro-engine/src/engine.rs`:** `step_tick` :1021 (insert the recalc pass and the Stage 4 follow-ups between `plan_moves` and `out.append`).
- **`crates/ro-engine/src/world.rs`:** `PlayerInfo` :32 (`job`, `clevel`, `status: CharacterStatus`, `derived`). Add job level, the `Status` snapshot and the dirty set (a `Vec<Entity>` + per-entity flag, or a bit in `PlayerInfo`; keep it allocation-free per tick).
- **`crates/ro-engine/src/job.rs`:** job id → mapid table (class tests for the stat cap and the ASPD cap; `pc_isequip` for `@job`).
- **`crates/ro-engine/src/command.rs`:** GM dispatch, `GM_GROUP` 99, message constants (`MSG_ITEM_USAGE` style; our own wording).
- **`crates/ro-engine/src/inventory.rs`:** max weight is `20000 + 300 × STR`. Switch to the job's `MaxWeight` + allocated STR × 300; push `SP_MAXWEIGHT` on change.
- **`crates/ro-engine/src/warp.rs:24`** `MapTransfer` carries `spawn: CharacterSpawn` (gets `job_level` and the progress fields).
- **`crates/ro-core/src/character.rs`:** `CharacterStatus` :13 (allocated stats, points, exp, hp/sp), `CharacterSpawn` :55 (add `job_level`; `CharacterSummary` already has it). `message.rs:34` `ClientIntent` (add `RaiseStat`); events `CoupleStatus`, `StatusCost`, `ParameterChanged`, `LongLongParameterChanged` exist. Check for a `0x00bc` ack event and a `0x019b` effect event; add them if missing.
- **`crates/ro-packets`:** `0x00bb` is in `opcode.rs`/`table.rs` (Fixed 5) but has no decoder. `0x00bc` (6) and `0x019b` (10): check the table.
- **`crates/ro-persistence`:** `repository/inventory.rs` `PlayState` :11 / `load_play_state` :22 / `save_inventory` :27; `postgres/inventory.rs` :166/:235; `postgres/character.rs` loads `job_level` (:142). Columns for every saved field already exist (`sql/01_schema.sql` :23–45).
- **`bins/map-server/src/config.rs`:** `MapConfig` (env/clap style; add `JOB_DATA_DIR`, `REFINE_DATA_DIR`, `BATTLE_CONFIG`). `claim.rs` `WriteKind::Inventory` is the ordered writer to extend. `run_metaling_servers.sh` sets the data dirs; add the new ones.
- **`tools/item-import`** is the importer pattern (header, attribution, `--rathena` path, NOTICE). `ro_db::ItemDb` is the registry pattern (`Arc`, `set_registries`).

### Pitfalls
- **Captured interleave tests:** several equipment engine tests assert a status push between a displaced item's `0x099a` and the new `0x0999`. Under the new tick order they must expect one net diff after all acks. Change only the push placement; keep ack and look order exactly.
- **No diff at spawn:** compute `Status` synchronously at spawn and at cross-map arrival, store it, and push nothing beyond the existing login push and `0x00bd`. A diff against zeros would duplicate the login push.
- **Login status push today sends every field once in captured order** (DEFERRED "Fix 2 simplifications"). Keep that; only the values become real.
- **`0x0141` bonus = battle − allocated**, so the job bonus shows as "bonus" (Novice job 10: `+1`).
- **Max weight uses allocated STR only**, never the job bonus or gear.
- **MaxHP's VIT term:** the % uses battle VIT (job bonus included); the flat +1 per point uses only gear VIT (0 today, no bonuses). Keep the two separate so item bonuses can plug in.
- **Float vs double:** follow the C types in the catalog formulas. Pin MECH-01's hand-computed values.
- **ASPD push value is `amotion`**, not the displayed ASPD (`status.rs` already sends amotion).
- **Weapon type for ASPD and for status ATK** reads the right hand (after the `hands()` fix). Dual wield adds the left weapon's base / 4; a shield adds the job's `Shield` base.
- **`@job` unequips go through the normal unequip path**, which marks dirty. Don't add a recalc per item; the tick's pass covers them.
- **`elvmax` check** uses `EquipLevelMax` (`ItemData.equip_level_max`, 0 = none).
- **Job names in `@job`** come from `JobDb` (`Name` field from the importer's `e_job` table). Mounted and dummy ids are refused (catalog deny list).
- **Data regeneration:** pass an expanded path to `--rathena` (`$RATHENA_PATH` may hold `~`). Never hand-edit generated files.
- **Never paste rAthena message text**; use our own wording.
- **DB tests:** fixed 2026-10-27. The metaling compose file now names its project `metaling`, so `up -d db` no longer touches rAthena's containers. Before that, the shared project "docker" made it remove `rathena-db`.

### Capture 3 (recorded and processed 2026-10-27, session C)
`attribute_sequence_3.txt` is redacted and written up (catalog `packets/status.md` "Capture 3", `atcommands.md`, `stats.md` "Capture 3 Hyper Novice values", INDEX, captures README). No buffs were active. What it settled:
- **`elvmax` unequip runs last**, after the base tail (`pc_baselevelchanged` ends the command). Per slot it's the existing `wear.rs` order: shoes look then `0x099a`; head top `0x099a` then look. Spec §2.5 now queues the check as a follow-up after the tail. Its own diff (if the item had stats) goes out next tick: the pass doesn't re-run a player listed by its own follow-up. The recorder clamped at Novice 99, so the unequips happened on a Hyper Novice at 100 and 101.
- **Observer:** `@blvl` gives it `0x080e` (party HP bar, out of scope) and `0x019b` effect 0; `@jlvl` only `0x019b` effect 1; an unequip only the `0x01d7`.
- **Hyper Novice MaxHP/MaxSP match** the generated data and the formula (no ×1.25, +10 stats) at B99 J1, B99 J52, B150, B201. No data change.
- Out of scope, now in spec §5: `0x0a24` achievements after level effects; `0x0446` quest marker at a level change; `SP_TRAITPOINT` is the running trait total in rAthena (we send 0).

### Start of session D: Finish (next: 2026-10-28)
Session C finished M4-M6 and stopped at the Finish boundary. Everything after `5317367` is uncommitted on `main`.
1. **Read** this whole file, the ledger (`.superpowers/sdd/plan/progress.md`; its session-C lines hold every ruling since M4), spec §2 and §5, and `DEFERRED.md`. Run `git status` and check it matches "Git state" above.
2. **DB:** `docker compose -f tools/docker/docker-compose.yml up -d db`. It is safe beside rAthena now (the compose file names its project `metaling`). Baseline: full `./verify.sh` was 754/754 and `--smoke` ok at the end of session C; rerun it before reviewing.
3. **Reviewer** (one `reviewer` agent, CLAUDE.md "Review once per feature"):
   - Diff: `git diff HEAD` plus the untracked files (`git status --porcelain | grep '^??'`). Pass `specs/stats/spec.md` and this plan.
   - Tell it to skip the bodies of the generated files under `crates/ro-db/data/{job,refine}/` (check only their headers and the `NOTICE.md` sections) and the redacted captures under `reference/catalog/captures/`.
   - M3 already had an early review (ledger "M3 review"). Point the reviewer at those settled points so it doesn't re-raise them, and name the risky parts added since: the Stage 4 follow-up queue re-listing a player (`stats.rs` `settle_stats`/`run_stat_pass`); `FollowUp::ToArea` events built at flow time (`@job`'s equip looks could go stale if another flow changes gear in the same tick); the `elvmax` unequip's diff landing next tick; the save path (`PlaySnapshot`, `RecentPlay`, autosave on revision **or** progress, `save_requested` dedupe after the hand-offs, the boxed `Claim.play`); `save_play_state` column ranges in one transaction; `@job`'s step order and the sex pick (`job::job_for_sex`).
   - Apply its fixes with the `receiving-code-review` rigor (verify each claim first), with tests, `./verify.sh` green.
4. **Docs:**
   - `crates/ro-engine/DESIGN.md` §1a ("not implemented" list: stats now are), plus §5/§9.6/§9.7 against what landed (the follow-up queue, `LevelGearCheck`, the next-tick `elvmax` diff).
   - `crates/ro-db/DESIGN.md` (job/refine registries, from M1; check it's current).
   - `crates/ro-persistence/DESIGN.md` (`save_play_state`; done in M6, check it).
   - `DEFERRED.md`:
     - Add a "stats" section from spec §5 and the ledger's deferred minors: M-3 (`status_diff` allocates two Vecs per recalc), M-7 (`DISPLACED` == `CASCADED_AMMO`), flow-time `ToArea` events, the next-tick `elvmax` diff, `LOOK_CLOTHES_COLOR` always 0, the `@job` ammo-refusal `0x0291` being source-only, and two `elvmax` items coming off in one command (not captured).
     - Remove the equipment section's "Needs the stats feature" bullets that landed, and the map-entry-settings placeholders that landed (listed in the Finish item below).
     - The M1 Hyper Novice caveat is **resolved** (capture 3 matched the data); don't add it.
5. `./verify.sh --smoke`.
6. **Commit** once: the whole plan, after the reviewer's fixes (CLAUDE.md "Commit once per plan"). Suggested subject: "Stats: job/refine data, formulas, stat model, stat-up, @blvl/@jlvl/@job, persistence". The final message lists the ledger's rulings and the deferred minors.
7. **Then hand the manual check to the user** (the Finish item below; the user records it). Anything it turns up is a fix in its own commit.

Housekeeping the user knows about: rAthena is stopped and its `rathena-db` container was removed (volume `docker_rathenadb` intact; rAthena's compose recreates it). `docker_metalingdb` is an old unused volume.

## Session split (recommended)
| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: data and formulas** | M1, M2 | Leaf crates only (importers, ro-db, ro-mechanics). Pure and fully unit-testable; no engine context needed. |
| **B: engine model** | M3 | The core change: tick order, `Status`, dirty set, diff, the equipment-test rewrite. Needs a fresh context and the DESIGN.md sections. |
| **C: flows and wiring** | M4, M5, M6 | Stat-up, GM commands, persistence and map-server wiring build on M3's API. Done 2026-10-27 (capture 3 processed first). |
| **D: finish** | Finish | Reviewer over the full diff (fresh context), fixes, docs, DEFERRED, smoke, commit; then the user's manual check. Starts 2026-10-28; see "Start of session D". |

If a session runs short, cut at a milestone boundary; never mid-milestone.

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 design, catalog, spec, plan** (2026-10-25)
  - Interview with the user (decisions above).
  - `metaling-explorer` ×4: `mechanics/job_change.md`, `atcommands.md` "@baselevelup / @joblevelup", `db/refine.md` + stats "Refine bonuses", `mechanics/battle_config.md`; INDEX rows. Fixed the `0x0b32` → `0x010f` error.
  - `crates/ro-engine/DESIGN.md`: §2 Tier 2 columns, §3.1 bitflags row, §5 tick order rewritten, §6 speed accessor, §7/§8 stage refs, §1a "not implemented", new §9 (stats), §10 (status effects), §11 (damage).
  - This spec and plan. Committed on their own. **User checkpoint.**
- [x] **M1 data: job and refine importers, ro-db registries** *(session A, done 2026-10-26)*
  - Done: `tools/job-import`, `tools/refine-import`, `ro_db::{JobDb, Job, JobBonus, RefineDb, RefineGroup, RefineBonus}`, data in `crates/ro-db/data/{job,refine}/`, NOTICE sections, ro-db DESIGN.md.
  - Import result: 148 jobs. 27 mounted/dummy names skipped (`Knight2`, ...). 72 jobs fallback-filled: every 4th class (all 275 levels), 3rd classes at 1-98, and Gangsi/Death Knight/Dark Collector. The special-case jobs (Super Novice, Summoner, Ninja, Gunslinger) have full tables.
  - **Caveat:** Hyper Novice (4307) is fallback-filled. rAthena's Super Novice HP adjustments might apply to it too; unverified. Note it in DEFERRED at Finish.
  - Data API for M3: `Job::bonus(job_level) -> JobBonus` (cumulative, 12 stats), `base_hp/base_sp(base_level)`, `aspd_base(WeaponType)` (200 if unlisted), `shield_aspd()`, `max_weight`, `max_base_level`, `max_job_level`. Also `JobDb::status_points(base_level)` and `RefineDb::bonus(group, item_level, refine) -> {bonus, random_bonus}` (1/100 units, cumulative).
  - `tools/job-import`: merge the five renewal files per catalog `db/job_stats.md` (later key wins; defaults), the `e_job` name/id table, fallback-fill the HP/SP tables, keep `BonusStats` sparse, drop outfits, AP and exp tables; `statpoint.yml` with `Points` only. Header + attribution, `NOTICE.md` section.
  - `tools/refine-import`: Armor 1–2, Weapon 1–5, `Bonus`/`RandomBonus` per refine 1–20.
  - `ro-db`: `JobDb` (accumulate `BonusStats` into per-job-level rows at load; lookups by id and by case-insensitive name), `RefineDb`. DB-JOB-01..05, DB-REF-01. `crates/ro-db/DESIGN.md`.
  - Generate `crates/ro-db/data/{job,refine}/` (may be its own commit, like the items data).
- [x] **M2 `ro-mechanics` formulas** *(session A, done 2026-10-26)*
  - Done: `ro_mechanics::{calc_status, StatusInputs, JobRow, GearInput, HandInput, RefineInput, WeaponClass, Primary, Status, ClassFlags, BattleConfig, BonusSlot, BonusTotals, stat_cost, next_stat_cost, raise_stat, StatRaise}`, plus `config/battle.yml` (every default written out). The crate depends only on `serde`; `serde-saphyr` is a dev-dependency, so the map-server parses `BATTLE_CONFIG` itself (M6). `ro-engine` now depends on `ro-mechanics`, unused until M3.
  - Tests: MECH-01..06 (18) with the captured windows: Novice B1/B11/J10/B6, Swordman B37, Knight B99, Bard B99, Shadow Cross and Lord/Rune Knight MaxHP. Mutation-checked: dropping the 4th-class ×1.25 or integer soft DEF fails them.
  - **Mapping notes for M3** (the engine builds the inputs):
    - `JobRow.bonus` = `Job::bonus(job_level)` (first six fields).
    - `GearInput.right` is the right-hand weapon, with a two-hander on `HAND_L` counting here (the `hands()` fix). Bare hands: `HandInput::bare(job.aspd_base(Fist))`.
    - `left` only for a *different* second weapon (dual wield).
    - `shield` = a shield is worn (ASPD adds `job.shield_aspd()`).
    - `WeaponClass` from `WeaponType`: Katar → `Katar`; Bow → `Bow`; Musical, Whip, Revolver, Rifle, Gatling, Shotgun, Grenade → `Ranged`; other weapons → `Melee`.
    - `HandInput.refine` = `RefineDb::bonus(Weapon, item.weapon_level, refine)` for `Refineable` items, else zero. `armor_refine` = Σ `RefineDb::bonus(Armor, armor_level, refine).bonus` over worn Armor-type `Refineable` items.
    - `item_def` = Σ item `Defense` (the existing `gear_status` loop). `range` = the existing gear range (bare 1).
    - `ClassFlags` from the mapid: `upper` = `JOBL_UPPER`, `baby` = `JOBL_BABY`, `third` = `JOBL_THIRD`, `fourth` = `JOBL_FOURTH`, `trait_job` = `pc_is_trait_job`: the 4th classes, probably also the expanded 4th lines without the `JOBL_FOURTH` bit (Night Watch, Shinkiro/Shiranui, Spirit Handler). **Unverified:** check the exact set in the catalog first, or ask `metaling-explorer`. `summoner` = Summoner line; `extended` = Kagerou/Oboro/Rebellion lines (second mask `0x109` / `0x108`).
    - Stat cap = `BattleConfig::max_stat(&class)`. `0x00be`/`0x00bd` cost = `next_stat_cost(stat, cap)` (replaces `status.rs` `stat_raise_cost`).
  - `Status` is the new snapshot. Push values: `SP_ATK1` batk, `SP_ATK2` watk, `SP_MATK2` status_matk, `SP_MATK1` weapon_matk, `SP_DEF1` soft_def, `SP_DEF2` hard_def, `SP_MDEF1` soft_mdef, `SP_MDEF2` hard_mdef, `SP_FLEE2` `flee2_shown()`, `SP_CRITICAL` `critical_shown()`, `SP_ASPD` amotion. Diff on the raw `cri`/`flee2` (captured re-send).
  - Input view structs (no `ro-db` dependency), `BattleConfig` (serde, shipped defaults, ASPD conversion, max-param selection by a class-group enum the engine supplies), bonus-slot types (`BonusSlot`, `BonusTotals`) with zero sources, `calc_status` (base → battle primary → every secondary of spec §2.2), refine application, stat cost and cap. `config/battle.yml` with every default written out.
  - MECH-01..06. Add `ro-mechanics` deps (serde) and `ro-engine → ro-mechanics`.
  - `Status` keeps `cri` and `flee2` in rAthena's 1/10 units (the diff compares them; the push divides by 10).
- [x] **M3 engine stat model and tick order** *(session B, done 2026-10-26)*
  - Done: `crates/ro-engine/src/stats.rs` (new) + `status.rs` rewrite. `PlayerInfo.derived: ro_mechanics::Status`, `PlayerInfo.gear: GearStatus` (cached, rebuilt when `STALE_GEAR`), `stat_flags`, `followups: Vec<FollowUp>`; `MapInstance::stat_queue`, `save_requests`. `ro_engine::Registries { items, mobs, jobs, refine, battle }` replaces the map-server's struct; `set_registries(Registries)`. The map-server still fills only items/mobs (`..Registries::default()`): M6 loads the rest.
  - **API for M4/M5:** `self.mark_dirty(e, gear: bool)`, `self.queue_followup(e, FollowUp::{ToSelf(ev), ToArea(ev), HealToMax, Save})` (remove the two `#[allow(dead_code)]` in `stats.rs` once a flow uses them), `MapInstance::drain_save_requests(&mut Vec<CharId>)` (public; the map-server must call it, M6). Base level is `PlayerInfo.clevel`, job level `PlayerInfo.status.job_level` (**`job_level` lives in `CharacterStatus`**, ruling; `CharacterSpawn`/`MapTransfer` carry it through `status`, and `load_play_state` already selects it). Status points/costs: `BattleConfig::max_stat(&class_flags(job_mapid(job)))`, `next_stat_cost`, `raise_stat` (ro-mechanics). `status_window(&status, &derived, cap)`.
  - Behaviour: per listed player `SP_MAXWEIGHT` (if changed), clamp pushes, `status_diff`, follow-ups. Spawn/arrival: `init_status` (clamps HP/SP silently). Cross-map warp: `settle_stats(e)` before the vanish; logout drops. Map change: `0x0141` ×12 (capture 2). A job missing from `JobDb` keeps saved MaxHP/MaxSP, MaxWeight 20000, ASPD bases 200.
  - Capture 2 (`attribute_sequence_2.txt`) was redacted and catalogued this session; spec fixed (map change ×12, failure ack captured, `@blvl -n` reset order, Super Novice +10, primary-4th ×1.25). `ClassFlags.fourth` now means `pc_is_primary_fourth`. Super Novice +10 is in (`job::super_novice_bonus`, die_counter assumed 0). `tools/job-import` gained the Super Novice line HP adjustment; `job_db.yml` regenerated (the M1 Hyper Novice caveat is resolved).
  - Tests: `stats_tests.rs` (ENG-STAT-02, -03, -09, -11 incl. Hyper Novice), `stats.rs` unit tests (ENG-STAT-04, -10, warp flush, logout drop), `status.rs` (ENG-STAT-01), `job.rs` (class flags, SN bonus), `equipment.rs` (per-hand gear, refine). Equipment/settings tests rewritten to one net diff (a Knife over a Knife now sends no push).
  - **Early review done** (no Critical; fixes applied: map-server fmt, `0x0141` also on an allocated change, Summoner/extended masks per `battle_config.md` §4/§8, `HealToMax` pushes only the raised parts and skips the dead). **Carry into M4–M6:**
    - M4: `FollowUp::ToSelf` is built at flow time; the trailing `0x0141` for a stat > 255 needs the post-recalc bonus, so add a send-time variant (e.g. `FollowUp::CoupleStat(offset)` reading `info.derived`). Unreachable with the shipped caps (≤ 130).
    - M5: the stat-reset's `0x0141`s now come from the diff even when a battle value is unchanged (allocated compared too). The heal follows `status_heal` (catalog `atcommands.md` "Heal pushes after the level-up").
    - M6: `Registries.battle` is `Arc<ro_mechanics::BattleConfig>`, so the map-server needs a `ro-mechanics` dependency: update CLAUDE.md's crate graph ("map adds db + engine") with the user, or re-export. Make a missing `JOB_DATA_DIR` a startup error (production has an empty `JobDb` until then). The `drain_save_requests` consumer must dedupe and tolerate a character already handed off (a warp flushes `Save`).
    - Deferred minors: `status_diff` allocates two Vecs per recalc (dirty players only); `DISPLACED` and `CASCADED_AMMO` in `wear.rs` are now identical.
  - The late login `SP_ASPD` push in both captures (Knight 270 → 230, Bard 420 → 410) is the recorder's Berserk Potion buff (user, 2026-10-26): a status effect, out of scope. Our values match the unbuffed first push. **DB tests not run in session B** (rAthena containers were up); M6 must run them.
  - `job_level` in `CharacterSpawn` and `PlayerInfo`; `GearStatus` per hand + the `hands()` fix; the `Status` snapshot replaces `DerivedStatus`; the dirty set; `recalc` → mark dirty; the recalc pass + Stage 4 follow-ups (clamp, heal-to-max hook) in `step_tick`; extended `derived_diff` (MAXHP/MAXSP/HP/SP, `0x0141` bonus); job-based max weight + `SP_MAXWEIGHT` push; login push / `0x00bd` real values; `0x0141` ×6 at a changemap actor init; synchronous compute at spawn and transfer arrival.
  - `set_registries` gains `JobDb`, `RefineDb`, `BattleConfig`.
  - Rewrite the equipment tests' push expectations to the single net diff (ENG-STAT-02).
  - The per-player **follow-up queue** (spec §2.1): typed entries (ready packet, heal-to-max, save request), drained in Stage 4 after that player's diff. The Stage 3 order is `SP_MAXWEIGHT`, clamp pushes, diff. M4 and M5 queue their trailing pushes here; this is how the captured orders come out. DESIGN.md §5 gets a line on it, and §9.6/§9.7 (which still say "clamps push from Stage 4" and "stat-up acks sent in the flow") change to the captured order.
  - ENG-STAT-01..04, ENG-STAT-09, ENG-STAT-10, ENG-STAT-11 (shipped data reproduces the captured windows). DESIGN.md §1a.
  - **Early review?** Yes, one `reviewer` pass on M3 alone: later milestones build on the tick-order change and the shared status state in the map-server (CLAUDE.md's early-review rule).
- [x] **M4 stat-up** *(session C, done 2026-10-27)*
  - Done: `ClientIntent::RaiseStat`, `ServerEvent::{StatusChangeAck, SpecialEffect}` (`0x00bc`, `0x019b`; `PacketOpcode::ZC_NOTIFY_EFFECT` added), `CzStatusChangePacket` (the amount is unsigned: captured `0xe7` = 231 honoured; catalog line fixed). Engine `stat_up.rs` (`raise_stat`), `FollowUp::CoupleStat(offset)` (send-time `0x0141` for a stat > 255, review M-1). `status.rs` exports `SP_STATUSPOINT`, `SP_STR`, `SP_LUK`, `SP_USTR`, `couple`, `param`, `battle_stats`; `wear.rs` `is_dead` is `pub(crate)`.
  - Tests: PKT-STAT-01 (`ro-packets/tests/status_codec_tests.rs`), ENG-STAT-05 (`stats_tests.rs` order/over-affordable/LUK/failures; `stat_up.rs` dead, > 255, two raises in a tick).
  - ro-core `ClientIntent::RaiseStat`, `0x00bc` and `0x019b` events if missing; ro-packets `0x00bb` decoder, `0x00bc`/`0x019b` encoders (+ table rows). PKT-STAT-01.
  - Engine flow per spec §2.4. ENG-STAT-05.
- [x] **M5 GM commands** *(session C, done 2026-10-27)*
  - Done: `level_command.rs` (`@blvl`/`@jlvl`/`@job` with the catalog aliases, our wording), `FollowUp::LevelGearCheck` (the `elvmax` check runs after the base tail, capture 3), `wear.rs` `unequip_over_level` / `unequip_unusable` (the `pc_isequip` re-check; an ammo refusal sends its `0x0291` first, as `pc_isequip` does) / `equip_look_events` (`pc_equiplookall`; shoes carry the worn item id, captured), `job::job_for_sex` (every shared mapid is a male/female pair, male = lower id), public `MapInstance::progress_snapshot` + `ro_engine::ProgressSnapshot` (job, base level, `CharacterStatus`) for M6's save. All `#[allow(dead_code)]` on `FollowUp` are gone.
  - Tests: `tests/level_command_tests.rs` ENG-STAT-06..08 (10 tests, two observers' views, mutation-checked: `elvmax` before the tail and a dropped `Save` both fail), `job.rs` sex-pair test, `level_command.rs` argument test.
  - Choices (ledger): `LOOK_CLOTHES_COLOR` is 0 (no cloth colour stored yet; rAthena sent 0 in the captures too). A missing `JobDb` entry refuses `@blvl`/`@jlvl` with our message. `@job` takes a `u16` id or the whole argument as a name; a 5-digit non-job number is "no job matches".
  - `@blvl`/`@jlvl` (aliases from catalog `atcommands.md`), `@job`/`@jobchange` per spec §2.5–§2.6, with the `elvmax` and `pc_isequip` re-checks through the `wear.rs` unequip path, looks to the area, the heal follow-up. The engine signals "save now" after `@job` (e.g. a flag the map-server drains, like `drain_transfers`).
  - ENG-STAT-06..08.
- [x] **M6 persistence and map-server wiring** *(session C, done 2026-10-27)*
  - User answers: rAthena stopped for the DB; the map-server depends on `ro-mechanics` directly (a re-export would break "one import path per item"); CLAUDE.md's crate graph updated.
  - Done: `ro_core::CharacterProgress` (+ `CharacterSpawn::progress()`); `InventoryRepository::save_inventory` → `save_play_state(char_id, inventory, looks, progress)` (one transaction; too-wide values refused); engine `InventorySnapshot` → `PlaySnapshot { inventory, looks, progress }`, `inventory_snapshot` → `play_snapshot`, `progress_snapshot -> CharacterProgress` (the M5 `ProgressSnapshot` is gone). Map-server: `WriteKind::Play`, `RecentPlay` (a re-entry racing the write gets the progress too), `Claim::{set_play, save_play}` (the claim's snapshot is boxed: clippy `large_enum_variant` on `WorldCommand`), autosave on inventory revision **or** progress change, `save_requested()` after the hand-offs (sorted + deduped; a character that warped out is skipped since its hand-off saved it). `MapConfig` `JOB_DATA_DIR`/`REFINE_DATA_DIR`/`BATTLE_CONFIG` (+ CLI flags, `registry_paths()`), `load_registries(RegistryPaths)` → `anyhow::Result`, `load_battle_config` (unknown key / bad YAML / out of range → error with the path), `main` refuses to start without `JOB_DATA_DIR`. `run_metaling_servers.sh` sets the three. ro-persistence DESIGN.md updated.
  - Tests: PER-STAT-01 (`per_inv_04` now saves the progress through `save_play_state`), BIN-STAT-01 ×2 (relog keeps STR/points; `@job` saves while online, once), BIN-STAT-02. Mutation-checked: no save drain, and a play snapshot without the status, each fail. Full `./verify.sh` with the DB: 754/754; `--smoke` ok (148 jobs loaded).
  - Repository: load `job_level`; save the progress fields with the inventory in one transaction (extend the ordered writer); immediate save after `@job`. `ro-persistence/DESIGN.md`.
  - `MapConfig`: `JOB_DATA_DIR`, `REFINE_DATA_DIR`, `BATTLE_CONFIG`; load at startup (bad config → error with path); `run_metaling_servers.sh`.
  - BIN-STAT-01, BIN-STAT-02.
- [ ] **Finish** *(session D)*
  - Session C ended here (2026-10-27). The metaling DB is up as container `metaling-db`. The compose file now names its project `metaling` (user, 2026-10-27), so CLAUDE.md's `up -d db` is safe beside rAthena. rAthena is stopped and its `rathena-db` **container** was removed by an accidental compose recreate (volume `docker_rathenadb` intact): the user recreates it from rAthena's compose.
  - `reviewer` over `git diff HEAD` plus untracked files (or from the M0 commit if data was committed early); apply its fixes.
  - `DESIGN.md` for ro-engine (§1a), ro-db, ro-persistence. `DEFERRED.md`: a "stats" section from spec §5; remove the equipment section's "Needs the stats feature" bullets that landed (ASPD, status ATK/MATK, soft DEF/MDEF, HIT/FLEE/CRIT, MaxHP/SP, per-hand values, refine, job re-check, `@job`) and the map-entry-settings placeholders that landed (derived stats 0, stat cap 99, stat-up and `0x0141` map change).
  - `./verify.sh --smoke`.
  - Commit (CLAUDE.md: the plan is complete before the user's manual check; what the check finds are fixes in their own commits).
  - **Manual client check (the user, after the commit):** a fresh Novice's status window (ATK, MATK, DEF, MDEF, HIT, FLEE, CRIT, ASPD 156, HP/SP); stat-ups (window and weight update); relog keeps them; `@blvl 50` (full heal, effect, points), `@jlvl 10` (job bonus shows as `+`), `@job swordman` with a non-Swordman item worn (comes off; body changes); equip a Knife +10 / Guard (one diff, client fine with the order); record this metaling session through `tools/proxy` and diff its packet orders against captures 1-3 (stat-up, `@blvl ±n` with an `elvmax` item, `@jlvl ±n` with an observer, `@job` with an unusable item and a level cap). No new rAthena capture is needed (decided 2026-10-27): captures 1-3 cover every flow; only a stat above 255 is uncaptured, and it's unreachable with the shipped caps.
