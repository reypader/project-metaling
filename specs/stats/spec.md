# Specification: Character stats (attributes, derived status, stat-up, level and job commands)

**Scope:**
- Every derived stat in the status window is computed the way rAthena computes it for players (renewal):
  - status ATK and weapon ATK
  - status MATK and weapon MATK
  - soft and hard DEF, soft MDEF
  - HIT, FLEE, CRIT, perfect dodge
  - ASPD
  - MaxHP and MaxSP
  - max weight
- The inputs are:
  - allocated stats
  - base and job level
  - the job's tables (job-level stat bonus, base HP/SP, ASPD bases, MaxWeight, max levels, status points)
  - worn gear (ATK, MATK, DEF, refine)
  - a typed battle config
- `job_level` reaches the engine. `0x0141` sends real bonuses. `0x00bd`, the login push and every recalc push send real values.
- Recalcs follow the tick order decided in `crates/ro-engine/DESIGN.md` §5/§9: a change marks the player dirty, and one recalc per tick runs after all mutations and before HP/SP are committed.
- Players raise stats with `0x00bb`, and the stats persist.
- GM commands: `@blvl`, `@jlvl` and `@job`, with the level-max and job gear re-checks and force-unequip.
- New datasets: job data (`tools/job-import`) and refine tables (`tools/refine-import`).

Out of scope (see §5):
- item, card and combo bonus scripts, so gear hard MDEF stays 0 (DESIGN.md §9.8 ruling)
- passive skills, skill trees, the skill-list re-send on `@job`
- status effects (DESIGN.md §10)
- auto-attack and damage (DESIGN.md §11)
- mob stat blocks
- natural regen
- trait stats (POW..CRT, trait points, AP, `0x0b24`: stay 0)
- enchant grade
- overweight statuses
- exp gain (only GM level changes)

**Ground truth:** rAthena commit `d4b8e7b8f` at `PACKETVER = 20260219`, renewal, as recorded in the catalog:
- `reference/catalog/mechanics/stats.md`: every formula, refine application ("Refine bonuses"), stat costs and limits, status points per level.
- `reference/catalog/mechanics/battle_config.md`: knobs, shipped defaults, ASPD cap conversion, max-stat selection order.
- `reference/catalog/db/job_stats.md`, `db/exp_tables.md` (`statpoint.yml`, max levels), `db/refine.md`.
- `reference/catalog/packets/status.md`: `0x00bd`, `SP_*` ids, stat-up order, recalculation push order, entry order.
- `reference/catalog/mechanics/atcommands.md` "@baselevelup / @joblevelup", `mechanics/experience.md` "Level-up" / "Direct set", `mechanics/job_change.md`.
- `reference/catalog/mechanics/equipment.md` §3a (`pc_isequip` job/class checks, mapid), §4 (unequip), §6.
- Captures 3 and 4 (`reference/catalog/captures/equip_sequence_3.txt`, `equip_sequence_4.txt`) give stat values: ASPD 440 ↔ 590, CRITICAL 2 ↔ 4 with a katar, Rod +10 `SP_MATK1` 50 / `SP_MATK2` 27, Knife +10 ATK2 37, armor +10 hard DEF +18.
- **Still source-only:** `@blvl`, `@jlvl` and `@job` packet order; most derived values (no status-window capture yet).

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline

```text
$RATHENA_PATH/db/re/job_stats.yml, job_aspd.yml, job_exp.yml, job_basepoints.yml, statpoint.yml
        ──► tools/job-import ──► crates/ro-db/data/job/{job_db.yml, statpoint.yml} (GPL)  ──► ro_db::JobDb    (JOB_DATA_DIR)
$RATHENA_PATH/db/re/refine.yml
        ──► tools/refine-import ──► crates/ro-db/data/refine/refine.yml (GPL)           ──► ro_db::RefineDb (REFINE_DATA_DIR)
config/battle.yml (ours, MIT, rAthena shipped defaults) ──► ro_mechanics::BattleConfig   (BATTLE_CONFIG; unset = built-in defaults)

ro_engine: BaseStats + GearStatus + JobDb/RefineDb rows + BattleConfig ──► ro_mechanics::calc_status ──► Status ──► diff pushes
PostgreSQL characters (str..luk, points, levels, job_class, exp, hp/sp, max_hp/max_sp: columns exist) ◄──► ro_persistence
```

## 1. Data

### 1.1 `JOB_DATA` (`crates/ro-db/data/job/`)
- `job_db.yml`: one entry per job **id**, with the importer's merge already applied. Merge rules are in catalog `db/job_stats.md` "Load order and merge semantics": later keys override, absent keys keep earlier values, and new jobs get the defaults.
  - Fields: `Id`, `Name` (the rAthena `JOB_` suffix, e.g. `Swordman`), `MaxWeight`, `MaxBaseLevel`, `MaxJobLevel`.
  - `BonusStats`: sparse per-level gains, as in rAthena. `ro-db` accumulates them at load into one row per job level.
  - `BaseAspd`: weapon type → value, plus `Shield`. Missing weapons mean 200.
  - `BaseHp` / `BaseSp`: dense arrays indexed by `level - 1`, length `MaxBaseLevel`. The importer fills slots the table omits with the fallback formula (catalog `mechanics/stats.md` MaxHP/MaxSP) and records that in the attribution header.
  - The importer drops `AlternateOutfits`, `BaseAp`/AP fields and exp tables (exp gain is out of scope). It keeps the trait `BonusStats` keys (they stay unused).
- `statpoint.yml`: `Level` → cumulative `Points` (levels 1–275). Drop `TraitPoints`.
- Job **names → ids**: rAthena's `e_job` constants are facts. Put the table in the importer and emit `Id` and `Name` together.
- Header, attribution, `NOTICE.md` section and a load+header test (`DB-JOB-01`), like `item`.

### 1.2 `REFINE_DATA` (`crates/ro-db/data/refine/refine.yml`)
- Groups `Armor` (levels 1–2) and `Weapon` (levels 1–5). Each `(group, level)` has `Bonus[1..20]` and `RandomBonus[1..20]` in 1/100 units. Entries are **cumulative**, as in rAthena: the entry for +N is the whole bonus at +N.
- Shadow groups are dropped (no `Bonus` in the data). So are chances and costs.
- Header, `NOTICE.md` section, test `DB-REF-01`.

### 1.3 `BattleConfig` (`config/battle.yml`, ours)
- A typed struct in `ro-mechanics`, `serde::Deserialize`, every field optional, defaulting to the **shipped** rAthena value. The field list is in catalog `mechanics/battle_config.md` "Metaling `BattleConfig` fields implied". Renewal-ignored knobs are not fields.
- `ASPD` caps are converted once at load, `(2000 − aspd×10)×2`, and clamped as `amotion ≥ cap/2`.
- Per-job max stat: when the job's `MaxStats` is empty (always, in this DB), the class-group default applies, chosen in the catalog's first-match order: baby-third, baby, summoner, extended, 4th, third-trans, third, trans, default. Use the engine's mapid table (`crates/ro-engine/src/job.rs`) for the class tests.
- `start_status_points` (48) lives here too, though rAthena keeps it in `inter_athena.conf`.
- The map-server reads `BATTLE_CONFIG` (a path). If it's unset, the built-in defaults apply. A file that fails to parse aborts startup with the path in the error.

## 2. Behaviour

### 2.1 Model (DESIGN.md §9)
- Per player:
  - `BaseStats`: allocated STR..LUK, base and job level, job id, status and skill points, exp. Persisted.
  - `GearStatus`: per-hand weapon records and hard DEF. Rebuilt on gear change.
  - `Status`: the computed snapshot, replacing `PlayerInfo.derived` / `DerivedStatus`.
  - The dirty set.
  - `base_bonus` / `battle_bonus` exist as types but stay zero (no sources yet).
- `ro_mechanics::calc_status` is pure. `ro-engine` maps `ro-db` rows into its input views.
- **Tick order:** player flows (equip, stat-up, GM commands) only mark dirty. `MapInstance::step_tick` runs the recalc pass after intents, then HP/SP commit and follow-ups, then appends events. Recalc never runs inline in a flow.
- **Spawn and login** compute `Status` synchronously. There's no diff, and nothing is pushed beyond today's login push and `0x00bd`.
- **A cross-map transfer** carries `BaseStats` with the spawn, as it does the inventory. The destination recomputes at arrival, synchronously, without a diff, as a login does.

### 2.2 Formulas (`ro-mechanics`, catalog `mechanics/stats.md`)
- **Base primary** = job bonus row[job level] + allocated (+ base_bonus, 0 today). **Battle primary** = base (+ battle_bonus, 0 today). Super Novice +10 needs `die_counter`, which we don't have: deferred.
- Status ATK (ranged weapons swap STR/DEX), weapon ATK (both hands' item ATK + refine `atk2`), status MATK, weapon MATK (+ refine except bows), soft DEF/MDEF (`float`), hard DEF (Σ item DEF + `(Σ refine bonus + 50)/100` over every worn Armor-type item), HIT, FLEE, CRIT (×2 with a katar), perfect dodge, renewal ASPD, MaxHP/MaxSP (table × VIT/INT %, ×1.25 for upper classes, Taekwon ranker excluded, config rate, level-bracket caps), max weight (job `MaxWeight` + **allocated** STR × 300).
- **Refine** (catalog "Refine bonuses"): only `Refineable` items with refine > 0. Refine is clamped to 20. Weapon level picks the Weapon row, armor level the Armor row. Grade is ignored. Overrefine is stored per hand for combat but unused.
- **Precision:** use the C type rAthena uses (`float` vs `double`) wherever truncation could differ. Tests pin the catalog's worked values.
- **Stat cost and cap** (moved from `status.rs`): renewal cost table. The cap is the job's max stat (§1.3). The cost is 0 at the cap.

### 2.3 Recalc and pushes
- **Diff** the new `Status` against the stored one and emit in the catalog order:
  1. `0x0141` STR..LUK (`base` allocated, `bonus` battle − allocated)
  2. HIT, FLEE1, ASPD (amotion), SPEED
  3. ATK1, DEF1/DEF2 (hard change), ATK2, DEF2/DEF1 (soft change)
  4. FLEE2, CRITICAL, MATK2/MATK1, MDEF pairs
  5. ATTACKRANGE `0x013a`
  6. MAXHP, MAXSP, HP, SP

  Extend `derived_diff` in `crates/ro-engine/src/status.rs` and keep its existing order tests.
- **Max weight** is not part of the diff. When it changes (STR up, job change), push `SP_MAXWEIGHT` itself, after the diff.
- **HP/SP clamp (Stage 4):** when a max drops below current, clamp and push `SP_HP`/`SP_SP`. When the clamp happens inside the recalc diff (the HP field changed), it is part of the diff. Don't send it twice.
- **One net diff per tick.** An equip whose displaced items each pushed in rAthena (captured interleave) now sends one diff after all the acks. This is a deliberate deviation (DESIGN.md §5.1). Rewrite the equipment engine tests that assert the interleave to expect the net diff, and keep their ack and look order.
- **`Gear::hands` fix** (DEFERRED "Per-hand values"): a two-hander that holds only `HAND_L` counts as the right-hand weapon. ASPD reads the right weapon type, plus the shield or a quarter of a different left weapon.
- **Login push:** `send_login_status_push` sends the real `0x0141` bonus. **`0x00bd`** sends real values. **Map change** (`changemap`, not first login) sends `0x0141` STR..LUK at actor init (catalog `packets/status.md` "Where the status packets sit", step 3).

### 2.4 Stat-up (`0x00bb`, catalog `packets/status.md` "Stat-up request and acknowledgement")
- Parse `CZ_STATUS_CHANGE` (`var:u16`, `amount:u8`) into a new `ClientIntent::RaiseStat { var, amount }`. The opcode and length already exist in `ro-packets`.
- **Gate:** var must be 13..18 and amount > 0. Cap the amount to what the points and the job max allow. If that leaves nothing, fail.
- **Failure:** `0x00bc` with ok 0 and value 0, nothing else.
- **Success, in order:**
  1. Apply the raise and mark dirty.
  2. `0x00be` with `SP_USTR + offset` and the next-point cost.
  3. `0x00b0` `SP_STATUSPOINT`.
  4. `0x00bc` with ok 1 and `min(stat, 255)`.
  5. A trailing `0x0141` if the stat is above 255.
- rAthena's recalc pushes (including the stat's `0x0141`) come *before* `0x00be`. Ours come after `0x00bc`, at the end of the tick. Deliberate deviation, same as §2.3.
- Not refused while sitting. Refused while dead: our ruling, since rAthena's `pc_statusup` doesn't check `pc_isdead`.

### 2.5 `@blvl` / `@jlvl` (catalog `atcommands.md` "@baselevelup / @joblevelup")
- **Group 99 only**, as with `@item`. For anyone else the line is ordinary chat.
- **Argument:** a non-numeric, empty or 0 argument sends our own usage line.
- **Messages** are our own wording: raised, lowered, can't go higher, can't go lower.
- **`@blvl +n`:**
  - Clamp to the job's `MaxBaseLevel`. Already at max is a failure.
  - Add the `statpoint` deltas for each step.
  - Set `base_exp = 0`.
  - Push `SP_STATUSPOINT`, `SP_BASELEVEL`, then `0x0acb` `SP_BASEEXP` and `SP_NEXTBASEEXP`. NEXTBASEEXP stays the placeholder until exp tables exist.
  - Run the `elvmax` check, then mark dirty.
  - Follow-up: a full heal to the **new** max, then `0x019b` effect 0 to the area.
- **`@blvl −n`:**
  - Clamp to `base_level − 1`. Already at level 1 is a failure.
  - If unspent points are less than the removal, first reset stats: all stats to 1 and `status_point` = table[old level].
  - Subtract and floor at 0. Same pushes, no heal, no effect.
- **`elvmax` check:** every worn item whose `EquipLevelMax` is non-zero and below the base level is force-unequipped through the `wear.rs` unequip path (looks, `0x099a`, ammo cascade). `EquipLevelMin` is not rechecked.
- **`@jlvl ±n`:**
  - Clamp to `MaxJobLevel`, or to 1.
  - Skill points += n. On a decrease, floor at 0. There are no skills to reset, so skip `pc_resetskill`.
  - Set `job_exp = 0`.
  - Push `SP_JOBLEVEL`, `0x0acb` `SP_JOBEXP` and `SP_NEXTJOBEXP`, then `SP_SKILLPOINT`, and mark dirty.
  - `0x019b` effect 1 on increase only. No heal.

### 2.6 `@job <id|name>` (catalog `mechanics/job_change.md`, minimal step set)
- **Group 99.** The argument is a job id or a case-insensitive name from `JobDb`.
- **Refuse** these with our own messages, changing nothing:
  - unknown jobs
  - jobs with no `JobDb` entry
  - the mounted and dummy variants on the catalog's deny list
  - the current job

  Bard and Dancer swap to match the character's sex.
- **Steps, in catalog order** (catalog steps 5–7, 9, 11, 12, 14, 19–20, and the save):
  1. Set the job, and the body to the class.
  2. Set `job_level = 1`, `job_exp = 0`.
  3. If the base level is above the new job's max: cap it, set `base_exp = 0`, reset stats, and send the base pushes.
  4. `0x00be` ×6 (`SP_USTR..SP_ULUK`) with the costs against the new cap.
  5. `SP_JOBLEVEL` and the job exp vars.
  6. **Re-check every worn item** with the existing `pc_isequip` subset against the new job. Force-unequip each failure (looks for weapon, shield and shoes, then `0x099a`, then the ammo cascade).
  7. `0x01d7` to the area: `LOOK_BASE`, `LOOK_HAIR`, `LOOK_CLOTHES_COLOR`, `LOOK_BODY2`. Update `PlayerInfo.job` and the cached looks.
  8. Mark dirty. Stage 3 pushes the diff, including `SP_MAXWEIGHT` from the new `MaxWeight`. Stage 4 clamps HP/SP (no heal).
  9. The equip-look re-send (`pc_equiplookall`).
  10. An **immediate save** (§2.7).
- **Skipped** (§5): class-change level vars, skill tree and `0x010f` re-send, option stripping, transcendent and trait point grants, party update.

### 2.7 Persistence
- Add one repository call that saves the character's progress in one transaction: str..luk, `status_point`, `skill_point`, `base_level`, `job_level`, `job_class`, `base_exp`, `job_exp`, `hp`, `sp`, `max_hp`, `max_sp`. **Prefer extending the existing ordered save** (the logout/autosave writer that runs `save_inventory`) so the status and the inventory go in the same transaction. That closes DEFERRED's "position and inventory in two transactions" gap for these fields.
- **When it saves:** logout, autosave and map transfer (as today for the inventory), plus an **immediate save after `@job`** (rAthena `chrif_save`).
- **What the map-server reads:** a status snapshot from the engine (like `inventory_snapshot`; a clone only on save).
- **Load:** `PlayState` / `CharacterSpawn` gain `job_level`. The `characters` columns already exist, so there's no schema change.

## 3. Scenarios

| ID | Scenario |
| :--- | :--- |
| DB-JOB-01 | Every committed `job/*.yml` loads and starts with the GPL header and attribution. |
| DB-JOB-02 | Novice job-level bonus row at job level 10 = +1 each of STR..LUK. Swordman row at job 50 matches the catalog's 18 gains. |
| DB-JOB-03 | Base HP/SP tables have `MaxBaseLevel` entries. A slot the source omits equals the fallback formula. |
| DB-JOB-04 | ASPD bases: Novice Fist 40; a missing weapon gives 200; Shield present. `statpoint` L1 = 48, L99 = 1273. |
| DB-REF-01 | Refine data loads with its header. Weapon level 1 +10 Bonus = 2000; Armor level 1 +10 = 1800; Weapon level 1 RandomBonus +8 = 300. |
| MECH-01 | Bare level-1 Novice, all stats 1: amotion 440 (ASPD 156), and HIT/FLEE/CRIT/perfect dodge/soft DEF/soft MDEF/status ATK/status MATK equal the catalog formulas by hand. |
| MECH-02 | Refine: Knife +10 ATK2 37; Rod +10 weapon MATK 50; Hat +10 and Silk Robe +10 add 18 hard DEF each, rounded on the sum. A bow gets no refine MATK. |
| MECH-03 | ASPD with a shield and with dual wield (left weapon ¼). Captured 440 ↔ 590 on the capture-3 weapon swap. Caps per class (190 vs 193). |
| MECH-04 | MaxHP/MaxSP: table × VIT%, the upper-class ×1.25, the level-bracket caps; `hp_rate` from config. |
| MECH-05 | Stat cost table and per-class caps (99/80/130/117) in the catalog's selection order; cost 0 at the cap. |
| MECH-06 | `BattleConfig` defaults equal the shipped values. A partial YAML overrides only its keys. ASPD caps are converted. |
| ENG-STAT-01 | (existing) The diff pushes only changed fields, in order. Extended with MAXHP/MAXSP/HP/SP and the `0x0141` bonus. |
| ENG-STAT-02 | Equip a weapon over another: acks and looks as captured, then **one** net diff at the end of the tick (ATK2, ASPD, ...). Unequip-all of 3 items: one diff. |
| ENG-STAT-03 | A katar pushes CRITICAL ×2. A bow pushes ATK1 (DEX-based). Shield on/off pushes ASPD. |
| ENG-STAT-04 | Taking off a +MaxHP-relevant change that drops MaxHP below current HP clamps HP and pushes it once. |
| ENG-STAT-05 | Stat-up success order: `0x00be`, `SP_STATUSPOINT`, `0x00bc` ok, then the tick's diff (`0x0141` of the stat + derived). Failure: only `0x00bc` ok 0. An over-affordable amount raises as many as possible. Refused at the cap. |
| ENG-STAT-06 | `@blvl 10` on a level-1 Novice: points +, pushes in order, `elvmax` unequip, then the diff, full heal to the new MaxHP, `0x019b`. `@blvl -5` with spent points: stats reset first. |
| ENG-STAT-07 | `@jlvl` up and down: pushes, skill points, effect only on increase, no heal. |
| ENG-STAT-08 | `@job` Novice → Swordman wearing a Swordman-incompatible item: the item comes off (`0x099a`), `LOOK_BASE` etc. go to the area, job level 1, the diff includes `SP_MAXWEIGHT`, a save is requested. `@job` to the same job or an unknown name: our message, no change. |
| ENG-STAT-09 | Login: `Status` computed at spawn. The login push and `0x00bd` carry real values (Novice: ASPD 440). A cross-map warp recomputes without a diff push. Map change sends `0x0141` ×6 at actor init. |
| ENG-STAT-10 | Determinism: equip, then stat-up in the same tick: one recalc, and both changes are in its diff. |
| PKT-STAT-01 | `0x00bb` parses into `RaiseStat`; a truncated frame drops the session (existing framing rules). |
| BIN-STAT-01 | Stat-up then relog: stats and points persist. `@job` saves immediately (DB-backed test). |
| BIN-STAT-02 | `BATTLE_CONFIG` pointing at a bad file fails startup with the path; unset uses the defaults. |

## 4. Licensing
- **GPL data** goes only through `tools/job-import` and `tools/refine-import` into `crates/ro-db/data/{job,refine}/`, with headers, a `NOTICE.md` section each (source paths, commit, date, changes: merged, fallback-filled, dropped fields; the regenerate command), and header tests.
- **Runtime:** read from disk (`JOB_DATA_DIR`, `REFINE_DATA_DIR`). No `include_str!`.
- **Test fixtures** in `ro-mechanics`/`ro-engine` are hand-written small rows, not copied tables. Tests may read the committed files from disk.
- **`config/battle.yml`** is ours (MIT): default *values* are facts.
- **Formulas** are written from the catalog. No translation of rAthena code.

## 5. Deferred (move into `DEFERRED.md` at Finish)
- Item, card and combo bonuses, and gear hard MDEF (DESIGN.md §9.8). Passive skills (§9.9). Status effects (§10). Damage (§11). Mob `Status` from `MobDb`.
- `@job`: class-change level vars, skill tree and `0x010f` re-send, option stripping (cart, falcon, peco, mado, homunculus), transcendent (+52) and trait (+7) point grants, party class update, `@job` upper/baby flag argument.
- `@jlvl` negative: `pc_resetskill` (no skills yet).
- Super Novice +10 stats (`die_counter`), Taekwon ranker HP ×3, enchant grade, overrefine damage (combat).
- Exp tables, so NEXTBASEEXP/NEXTJOBEXP stay placeholders. Exp gain and natural level-up.
- Natural regen, overweight statuses, trait stats and AP.
- A capture of `@job`, `@blvl` and a stat-up on rAthena to confirm the packet orders (source-only today).
