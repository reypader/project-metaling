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
| *Planning defaults (user may revisit)* | Stat-up refused while dead (rAthena doesn't check). Our own message wording for the GM commands. `@job` skips the skill tree, option stripping, trans/trait point grants (spec §5). Stat-up and level pushes come after the acks at tick end, not inline (follows from the tick order). |

## Notes for the implementer

### Git state at hand-off
- M0 (DESIGN.md, catalog, this spec and plan) is committed on its own as "Stats: M0 design, catalog, spec and plan" (CLAUDE.md allows M0 docs on their own). M1 onwards stays uncommitted until Finish; generated data may commit early (items precedent).

### Catalog facts gathered in M0 (all written into the catalog)
- `mechanics/battle_config.md` (new): every knob the formulas read, conf file + line, default, read site. **Shipped conf values differ from compiled defaults** for `max_sp` (1000000 vs 32500), `max_ap`, and the 3rd/4th/extended/summoner/baby-3rd `max_*_parameter` (shipped 130/130/130/130/130/117). Use shipped. `natural_heal_weight_rate` is commented out in the conf, so the compiled 70 applies. ASPD caps convert `(2000 − aspd×10)×2` at load; clamp `amotion ≥ cap/2`. Max-param selection order: baby-third, baby, summoner, extended, 4th (trait), third-trans, third, trans, default. The per-job `MaxStats` overrides would win, but it's empty in the DB. `start_status_points` lives in `inter_athena.conf`. `atcommand_max_stat_bypass` doesn't exist (group permission `bypass_max_stat`). `critical_rate` is skipped for players. Open: whether 4th classes carry the third-class flag for `pc_maxaspd`. If not, they cap at 190.
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
- **DB tests:** the metaling and rAthena compose files share project "docker"; `up -d db` replaces a running `rathena-db`. Ask the user first if `rathena-*` containers are up.

## Session split (recommended)
| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: data and formulas** | M1, M2 | Leaf crates only (importers, ro-db, ro-mechanics). Pure and fully unit-testable; no engine context needed. |
| **B: engine model** | M3 | The core change: tick order, `Status`, dirty set, diff, the equipment-test rewrite. Needs a fresh context and the DESIGN.md sections. |
| **C: flows and wiring** | M4, M5, M6 | Stat-up, GM commands, persistence and map-server wiring build on M3's API. |
| **D: finish** | Finish | Reviewer over the full diff (fresh context), fixes, DEFERRED, smoke, manual check, commit. |

If a session runs short, cut at a milestone boundary; never mid-milestone.

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 design, catalog, spec, plan** (2026-10-25)
  - Interview with the user (decisions above).
  - `metaling-explorer` ×4: `mechanics/job_change.md`, `atcommands.md` "@baselevelup / @joblevelup", `db/refine.md` + stats "Refine bonuses", `mechanics/battle_config.md`; INDEX rows. Fixed the `0x0b32` → `0x010f` error.
  - `crates/ro-engine/DESIGN.md`: §2 Tier 2 columns, §3.1 bitflags row, §5 tick order rewritten, §6 speed accessor, §7/§8 stage refs, §1a "not implemented", new §9 (stats), §10 (status effects), §11 (damage).
  - This spec and plan. Committed on their own. **User checkpoint.**
- [ ] **M1 data: job and refine importers, ro-db registries** *(session A)*
  - `tools/job-import`: merge the five renewal files per catalog `db/job_stats.md` (later key wins; defaults), the `e_job` name/id table, fallback-fill the HP/SP tables, keep `BonusStats` sparse, drop outfits, AP and exp tables; `statpoint.yml` with `Points` only. Header + attribution, `NOTICE.md` section.
  - `tools/refine-import`: Armor 1–2, Weapon 1–5, `Bonus`/`RandomBonus` per refine 1–20.
  - `ro-db`: `JobDb` (accumulate `BonusStats` into per-job-level rows at load; lookups by id and by case-insensitive name), `RefineDb`. DB-JOB-01..04, DB-REF-01. `crates/ro-db/DESIGN.md`.
  - Generate `crates/ro-db/data/{job,refine}/` (may be its own commit, like the items data).
- [ ] **M2 `ro-mechanics` formulas** *(session A)*
  - Input view structs (no `ro-db` dependency), `BattleConfig` (serde, shipped defaults, ASPD conversion, max-param selection by a class-group enum the engine supplies), bonus-slot types (`BonusSlot`, `BonusTotals`) with zero sources, `calc_status` (base → battle primary → every secondary of spec §2.2), refine application, stat cost and cap. `config/battle.yml` with every default written out.
  - MECH-01..06. Add `ro-mechanics` deps (serde) and `ro-engine → ro-mechanics`.
- [ ] **M3 engine stat model and tick order** *(session B)*
  - `job_level` in `CharacterSpawn` and `PlayerInfo`; `GearStatus` per hand + the `hands()` fix; the `Status` snapshot replaces `DerivedStatus`; the dirty set; `recalc` → mark dirty; the recalc pass + Stage 4 follow-ups (clamp, heal-to-max hook) in `step_tick`; extended `derived_diff` (MAXHP/MAXSP/HP/SP, `0x0141` bonus); job-based max weight + `SP_MAXWEIGHT` push; login push / `0x00bd` real values; `0x0141` ×6 at a changemap actor init; synchronous compute at spawn and transfer arrival.
  - `set_registries` gains `JobDb`, `RefineDb`, `BattleConfig`.
  - Rewrite the equipment tests' push expectations to the single net diff (ENG-STAT-02).
  - ENG-STAT-01..04, ENG-STAT-09, ENG-STAT-10. DESIGN.md §1a.
  - **Early review?** Yes, one `reviewer` pass on M3 alone: later milestones build on the tick-order change and the shared status state in the map-server (CLAUDE.md's early-review rule).
- [ ] **M4 stat-up** *(session C)*
  - ro-core `ClientIntent::RaiseStat`, `0x00bc` and `0x019b` events if missing; ro-packets `0x00bb` decoder, `0x00bc`/`0x019b` encoders (+ table rows). PKT-STAT-01.
  - Engine flow per spec §2.4. ENG-STAT-05.
- [ ] **M5 GM commands** *(session C)*
  - `@blvl`/`@jlvl` (aliases from catalog `atcommands.md`), `@job`/`@jobchange` per spec §2.5–§2.6, with the `elvmax` and `pc_isequip` re-checks through the `wear.rs` unequip path, looks to the area, the heal follow-up. The engine signals "save now" after `@job` (e.g. a flag the map-server drains, like `drain_transfers`).
  - ENG-STAT-06..08.
- [ ] **M6 persistence and map-server wiring** *(session C)*
  - Repository: load `job_level`; save the progress fields with the inventory in one transaction (extend the ordered writer); immediate save after `@job`. `ro-persistence/DESIGN.md`.
  - `MapConfig`: `JOB_DATA_DIR`, `REFINE_DATA_DIR`, `BATTLE_CONFIG`; load at startup (bad config → error with path); `run_metaling_servers.sh`.
  - BIN-STAT-01, BIN-STAT-02.
- [ ] **Finish** *(session D)*
  - `reviewer` over `git diff HEAD` plus untracked files (or from the M0 commit if data was committed early); apply its fixes.
  - `DESIGN.md` for ro-engine (§1a), ro-db, ro-persistence. `DEFERRED.md`: a "stats" section from spec §5; remove the equipment section's "Needs the stats feature" bullets that landed (ASPD, status ATK/MATK, soft DEF/MDEF, HIT/FLEE/CRIT, MaxHP/SP, per-hand values, refine, job re-check, `@job`) and the map-entry-settings placeholders that landed (derived stats 0, stat cap 99, stat-up and `0x0141` map change).
  - `./verify.sh --smoke`.
  - **Manual client check:** a fresh Novice's status window (ATK, MATK, DEF, MDEF, HIT, FLEE, CRIT, ASPD 156, HP/SP); stat-ups (window and weight update); relog keeps them; `@blvl 50` (full heal, effect, points), `@jlvl 10` (job bonus shows as `+`), `@job swordman` with a non-Swordman item worn (comes off; body changes); equip a Knife +10 / Guard (one diff, client fine with the order); compare the window with the same character on rAthena through `tools/proxy` if possible, and record that capture under `reference/catalog/captures/` (it would be the first status-window capture).
  - Commit.
