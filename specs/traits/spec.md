# Specification: Trait stats, AP, stat-point grants, stat reset and char-select values

**Scope:**
- **Trait stats** POW, STA, WIS, SPL, CON, CRT for the trait jobs (the 4th classes and the "upper expanded second" set). Each trait value is the allocated amount plus the job's trait `BonusStats` at the current job level. Players raise them with `0x0b24`, and they persist.
- **Trait-derived values** P.ATK, S.MATK, RES, MRES, H.PLUS and C.RATE, plus the trait terms that feed existing stats (status ATK +5×POW, HIT and FLEE +2×CON).
- **AP:** MaxAP from the job's AP table, and current AP persisted and clamped. Nothing grants or spends AP yet.
- **Trait points:** granted from `statpoint.yml` `TraitPoints` (base 201 and up) and by `@job` into a trait job (+7). Every place that sends `SP_TRAITPOINT` sends the running total.
- **Stat-point grants on `@job`:** transcendent ±52 into or out of an upper or primary 4th class, the trait +7 or zeroing, and the stat resets these trigger (including the captured double reset).
- **`pc_resetstate`, complete:** the base pools include the +52 and +7 and allocated traits go to 0. `@blvl −n` also checks the trait pool. New command `@streset` (alias of `@resetstat`).
- **Char select** (`0x006b`/`0x0b72` `CHARACTER_INFO`) sends real HP/SP, max HP/SP, status points, skill points, STR..LUK and exp instead of placeholders.

Out of scope (see §5):
- item, card and status-effect sources of traits or AP (`bPow`.., `bMaxAP`, SC_BENEDICTUM, ...)
- anything that grants or spends AP: skills, AP regen, AP loss on death or on VS maps
- the battle consumers of P.ATK/S.MATK/RES/MRES/H.PLUS/C.RATE (damage, §11)
- 4th-job skill ratios
- the skill tree (the `skills` feature, which follows this one)

**Ground truth:** rAthena `d4b8e7b8f` at `PACKETVER = 20260219`, renewal, as recorded in the catalog:
- `reference/catalog/mechanics/trait_stats.md`: effects, MaxAP, current-AP rules, trait points, `pc_traitstatusup`, persistence, `@resetstat`. Section 7 holds the captured values.
- `reference/catalog/packets/status.md`: "Trait stat-up (0x0b24)", the diff order under "Recalculation pushes", and **"Capture: traits (rathena_traits_sequence.txt)"**, which gives every order this spec cites.
- `reference/catalog/mechanics/job_change.md` steps 7–10 (level cap, trans points, `0x00be` ×6, trait points). `mechanics/atcommands.md`: "@baselevelup / @joblevelup" and "@resetstat / @streset / @reset (stat side)".
- `reference/catalog/db/exp_tables.md` `statpoint.yml`: `TraitPoints` totals 3, 6, 9, 12, 19 for B201..205, then +3 per level and +7 every fifth (corrected 2026-10-29). `db/job_stats.md`: `BaseAp`, `ApFactor`/`ApIncrease`.
- `reference/catalog/packets/char.md`: "Status fields in CHARACTER_INFO" and "Captured select-screen values".
- **`reference/catalog/captures/rathena_traits_sequence.txt`** (2026-10-29). Actor: Knight, then Dragon Knight (4252). It covers:
  - trait-ups;
  - the B200/B201/B205 tails;
  - `@blvl -10` with the trait-triggered reset;
  - `@streset`;
  - `@job` into and out of a trait job (the forced double reset);
  - the login block;
  - char select.
- Earlier captures: `attribute_sequence_2.txt` (Novice High +52 / Super Novice −52, Hyper Novice +7) and `attribute_sequence_3.txt` (Hyper Novice J52 trait bonuses and PATK..CRATE 4/3/9/9/3/1, MAXAP 200 at B201).
- **Still source-only:**
  - the `0x0b24` failure form. The client blocks the request at 0 points (user, 2026-10-29), so the reply only matters against a modified client;
  - amount > 1;
  - leaving a trait job with ≥ 7 unspent trait points;
  - the trait cap (110);
  - SMATK/MRES with real SPL/WIS;
  - AP > 0;
  - trait job → trait job.

When this spec and the catalog disagree, the catalog wins and this spec gets fixed.

---

## 0. Data pipeline

`tools/job-import` already reads every source file this feature needs. It stops dropping two things:
- **`statpoint.yml` `TraitPoints`**: kept as an optional cumulative column (`trait_points`, absent = 0) beside `points`.
- **AP:** `job_basepoints.yml` `BaseAp` plus the `ApFactor`/`ApIncrease` fallback from `job_stats.yml`. Each job gets a full `base_ap` table (one value per base level up to `MaxBaseLevel`), fallback-filled with `calc_baseap` (catalog `trait_stats.md` §2) wherever the source has no entry or a 0, exactly as the HP/SP tables are filled today. Jobs whose table is all zero may omit it.

Regenerate `crates/ro-db/data/job/` and update its headers and the `NOTICE.md` job section ("Modified": trait points kept; AP tables fallback-filled). The data format version goes up by one, and `ro-db` rejects the old version.

## 1. Data API (`ro-db`)
- `StatPointRow` gains `trait_points`. `JobDb::trait_points(base_level) -> u32` reads the cumulative table, with 0 below 201 and past the table.
- `Job::base_ap(base_level) -> u32`, with 0 if the job has no table.
- `JobBonus` already carries pow..crt (`crates/ro-db/src/job.rs:50-60`); nothing changes.

## 2. Behaviour

### 2.1 Model
- **`ro_core::CharacterStatus`** gains:
  - `traits: [u16; 6]`: allocated POW, STA, WIS, SPL, CON, CRT, in this order everywhere;
  - `trait_point: u16`;
  - `ap: u32`.

  The `Default` (a fresh Novice) has all three at 0. `CharacterProgress` carries them, so the save, the transfer and the recent-write record carry them too.
- **`ro_mechanics`:**
  - New input `StatusInputs.traits` (allocated) and `JobRow.trait_bonus` (from `Job::bonus(job_level)`), with the same layering as the six primary stats: base = job bonus + allocated (+ `base_bonus`, zero today), battle = base (+ `battle_bonus`, zero today).
  - Use a `Traits` struct mirroring `Primary`, named pow..crt. The implementer may reuse one generic struct if it reads better.
  - `JobRow.base_ap` comes from `Job::base_ap(base_level)`.
- **`ro_mechanics::Status`** gains:
  - base and battle traits;
  - `patk`, `smatk`, `res`, `mres`, `hplus`, `crate_` (clamped 0..32767);
  - `max_ap`.

  The diff and the pushes read these.
- **`ClassFlags.trait_job`** is already computed (`crates/ro-engine/src/job.rs`). Before relying on it, check it against catalog `trait_stats.md` §1 "Trait job set" (primary 4th plus Hyper Novice, Sky Emperor, Soul Ascetic, Night Watch, Shinkiro/Shiranui, Spirit Handler). The stats plan left this unverified, so fix it here if it's wrong.
- **`ClassFlags.fourth`** (primary 4th, already used for the ×1.25 HP rule) and `upper` decide the transcendent +52.

### 2.2 Formulas (catalog `trait_stats.md` §1–§2)
- `batk += 5 × battle POW` (inside the existing status-ATK formula, after the `/10`).
- HIT and FLEE get `+2 × battle CON`.
- PATK = POW/3 + CON/5. SMATK = SPL/3 + CON/5. RES = STA + (STA/3)×5. MRES = WIS + (WIS/3)×5. HPLUS = CRT. CRATE = CRT/3. All use integer division on the battle traits.
- None of the traits feed MaxHP, MaxSP, ASPD, CRIT, DEF or MDEF.
- **MaxAP** = `base_ap`. If `ap_rate != 100`, apply `ap_rate`. Clamp to `BattleConfig.max_ap`.
- New `BattleConfig` knobs `max_ap` (shipped 1000) and `ap_rate` (100), with entries in `config/battle.yml`. `max_trait_parameter` (110) already exists.
- **Trait cost and cap:**
  - cap = `max_trait_parameter` for a trait job, else 0;
  - cost per point = 1;
  - the "next point" cost shown in `0x00be` `SP_U<trait>` = 1 below the cap, 0 at the cap (always 0 for non-trait jobs).
  - Add `next_trait_cost(current, cap) -> u8` and `raise_trait(current, requested, points, cap) -> StatRaise` to `ro_mechanics::points` next to the stat versions.

### 2.3 Pushes
- **Diff order** (catalog "Recalculation pushes"; captured): after the existing `… MAXHP, MAXSP, HP, SP` come:
  1. `0x0141` POW..CRT (`base` = allocated, `bonus` = battle − allocated), only the changed ones; an allocated change also counts, as for STR..LUK.
  2. `0x00b0` `SP_PATK`, `SP_SMATK`, `SP_RES`, `SP_MRES`, `SP_HPLUS`, `SP_CRATE` (225–230), changed only.
  3. `SP_MAXAP` (233), then `SP_AP` (232), changed only.

  Captured:
  - trait-up 488-541 (`ATK1`/`HIT`/`FLEE1` before the trait's `0x0141`, PATK/RES/HPLUS/CRATE after);
  - B200 451 (`MAXAP` 200 after `MAXSP`);
  - `@blvl -10` 762-777 (ends `MAXAP` 0 after `SP`);
  - capture 3 lines 962-973.
- **AP clamp:** when MaxAP drops below current AP, set AP to MaxAP and push `SP_AP` with the HP/SP clamp pushes, before the diff (catalog §2 "Clamp when max shrinks"). It is unreachable in practice today, but implement and test it.
- **Login block after `0x00bd`** (`send_initial_status`, `crates/ro-engine/src/status.rs:303`). The values become real, with the order unchanged:
  1. `0x0141` POW..CRT;
  2. PATK..CRATE;
  3. `SP_TRAITPOINT`, `SP_AP`, `SP_MAXAP`;
  4. `0x00be` `SP_UPOW..SP_UCRT` with the next-point costs.

  Captured 1451-1471. `SP_AP` is sent even at 0.
- **Pre-ready login push** (`send_login_status_push`): rAthena's pre-`0x007d` diff carries only non-zero trait values (POW, then PATK at 1401-1402). Append the trait couples and trait-derived values to the end of our push, non-zero only, in diff order.
- **Map change** (`send_map_change_stats`): the trait `0x0141` ×6 carry real values.
- **`0x00be` trait costs** everywhere they are sent today as 0 (`reset_stats`, `@job`) carry the real next-point cost.

### 2.4 Trait stat-up (`0x0b24`, catalog `packets/status.md` "Trait stat-up (0x0b24)", `trait_stats.md` §4)
- Parse `CZ_ADVANCED_STATUS_CHANGE` (`var:i16`, `amount:i16`) into `ClientIntent::RaiseTrait { var: u16, amount: i16 }`. The opcode and length are already in `ro-packets`.
- **Negative amount:** drop the request with no reply (rAthena logs and ignores).
- **Failure**: only `0x00bc` with ok 0, value 0 and `sp` = the requested var, sent at once. The cases:
  - var outside 219..224;
  - amount 0;
  - not a trait job (cap 0);
  - nothing affordable;
  - the result would exceed the cap.

  This form is source-only. The client blocks the request itself at 0 points (user, 2026-10-29), so the reply only guards against a modified client. It is tested but can't be captured.
- **Refused while dead,** as for stat-up (our ruling).
- **Amount > 1** is capped to `min(amount, trait_point, cap − current)`, not rejected (source-only; the client always sent 1).
- **Success:** apply, take the points and mark dirty. Then queue the follow-ups, which mirror `raise_stat`:
  - `0x00be` `SP_UPOW + n` with the next-point cost;
  - `0x00b0` `SP_TRAITPOINT`;
  - `0x00bc` ok 1, `sp` = var, `value` = `min(allocated, 255)`. The value is the allocated stat, not including the job bonus (captured).
  - The trailing `0x0141` for a value over 255 can't happen with a cap of 110. Skip it, and note this in a comment.
- **Wire order** (captured 488-493):
  1. the tick's diff (derived first, the trait's `0x0141`, trait-derived);
  2. `0x00be`;
  3. `SP_TRAITPOINT`;
  4. `0x00bc`.
- **The observer** gets nothing.

### 2.5 Trait points across levels (`@blvl`, catalog `trait_stats.md` §3)
- **The base tail** sends the real `SP_TRAITPOINT` (the running total).
- **`@blvl +n`:** `trait_point += trait_points(new) − trait_points(old)`, alongside the status-point delta.
- **`@blvl −n`:**
  - `remove_trait = trait_points(old) − trait_points(new)`.
  - **The reset runs when `status_point < remove || trait_point < remove_trait`** (captured 720-782: the trait pool alone triggered it).
  - After the optional reset, subtract each pool and floor it at 0.
  - The existing deviation stays: the reset's stale `0x0141` ×12 and its own recalc fold into the tick's single diff.
  - The order the stats feature already sends stays as it is, with real values in the reset pushes and the tail.
- **MAXAP** changes come from the diff (B200: 200; down to B195: 0). Level changes never push `SP_AP`, because the heal is HP and SP only.

### 2.6 `pc_resetstate` (the existing `reset_stats`, `crates/ro-engine/src/level_command.rs:179`)

**New pools:**
- `status_point` = `JobDb::status_points(base_level)`, plus `transcendent_status_points` (52, new `BattleConfig` knob) if the class is upper or a primary 4th class.
- `trait_point` = `JobDb::trait_points(base_level)`, plus `trait_points_job_change` (7, new knob) if it is a trait job.
- The six stats go to 1 and the six traits to 0.

**The class it reads is the one in effect when it runs.** In `@job` that is already the new class (captured, §2.7).

**Immediate pushes, in this order:**
1. `0x00be` ×6 for `SP_USTR..SP_ULUK`.
2. `0x00be` ×6 for `SP_UPOW..SP_UCRT`, with real costs.
3. `SP_STATUSPOINT`.
4. `SP_TRAITPOINT`.

Then mark dirty, so the `0x0141`s and derived values come from the diff. rAthena's stale `0x0141` ×12 is skipped, the same deviation the stats feature already ruled.

**No AP push.**

### 2.7 `@job` point grants (catalog `job_change.md` steps 7–10; captured)

The new steps sit around the existing step 4 (`0x00be` ×6), in this order:
1. **Level cap** (existing step 3). If the base level exceeds the new job's max, reset (§2.6, new class), then the base tail. In the captured B195 case the tail is `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, exp pair.
2. **Transcendent points** (new):
   - Into an upper or primary 4th class from a class that is neither: `status_point += 52` and push `SP_STATUSPOINT`.
   - Out of one into neither: if `status_point < 52`, reset (§2.6); otherwise subtract 52 and push `SP_STATUSPOINT`.
   - Captured: 1273 ↔ 1325 (DK); 1221 after reset #1 (B195 case, line 1025); Novice High +52 and Super Novice −52 in capture 2.
3. `0x00be` ×6 (existing step 4).
4. **Trait points** (new):
   - Non-trait → trait: `trait_point += 7`, then push `SP_TRAITPOINT` + `0x00be` `SP_UPOW..SP_UCRT`.
   - Trait → non-trait: if `trait_point < 7`, reset (§2.6, new class, so the pool becomes `trait_points(level)` + 0, normally 0). Otherwise set `trait_point = 0` and push the same seven. Allocated traits are **not** zeroed on that path; they keep counting with a cap of 0 (rAthena does the same).
   - Trait → trait: nothing.
5. The rest of `@job` as today: `SP_JOBLEVEL` and onward.

**Captured double reset** (B195 DK → Knight, 956-1073) is the order these steps must reproduce:
1. Reset #1 (level cap).
2. The base tail.
3. `SP_STATUSPOINT` −52.
4. `0x00be` ×6.
5. Reset #2, because the trait pool is now 0 < 7.
6. The job pushes.

From B99 (1185-1249) it is −52, `0x00be` ×6, one reset, then the job pushes. Each of our resets sends §2.6's immediate pushes; the derived diff comes once, at tick end. Into DK (352-399): `SP_STATUSPOINT` +52, `0x00be` ×6, `SP_TRAITPOINT` 7, trait `0x00be` ×6 (value 1).

### 2.8 `@streset` / `@resetstat` (catalog `trait_stats.md` §6)
- Group 99. Names: `resetstat` and `streset`.
- Run §2.6, mark dirty, then queue the message (our wording) as a follow-up, so it comes after the diff.
- Captured order (870-922):
  1. `0x0141` ×12 (we skip these);
  2. `0x00be` ×12;
  3. `SP_STATUSPOINT`;
  4. `SP_TRAITPOINT`;
  5. the HP/SP clamp pushes;
  6. the diff;
  7. the message.
- The observer gets nothing.
- `@reset` (stats and skills) belongs to the skills feature, which calls §2.6.

### 2.9 Persistence and char select
- **Schema:** guarded `ALTER TABLE characters ADD COLUMN` for `pow`, `sta`, `wis`, `spl`, `con`, `crt` (SMALLINT, default 0), `trait_point` (INTEGER, default 0) and `ap` (INTEGER, default 0). Follow the existing pattern in `crates/ro-persistence/sql/01_schema.sql:66-107`.
  - MaxAP is derived and not stored.
  - rAthena keeps `max_ap`. We don't need it, because char select doesn't show AP.
- **`save_play_state`** writes the new fields in the same transaction. **`load_play_state`** reads them.
- AP is loaded at login (`keep_ap_on_logout` is yes; we don't add the knob) and clamped to MaxAP silently at spawn, as HP/SP are today.
- **`CharacterSummary`** (`crates/ro-core/src/ids.rs:30`) gains `base_exp`, `job_exp`, `hp`, `max_hp`, `sp`, `max_sp`, `status_point`, `skill_point` and `stats: [u16; 6]`. `summary_columns!` selects them.
- **`encode_character_info`** sends them:
  - `exp` and `jobexp` as i64;
  - `jobpoint` = `status_point` capped at 32767;
  - `sppoint` = `skill_point` capped at 32767;
  - `hp` and `maxhp` uncapped i64;
  - `sp` and `maxsp` capped at 32767;
  - Str..Luk = the allocated values capped at 255.

  There are no trait or AP fields (captured).
- **The select screen shows the last saved values** (captured). Nothing extra is needed.

## 3. Scenarios

| ID | Scenario |
| :--- | :--- |
| DB-TRAIT-01 | Regenerated `job/*.yml` loads and keeps the GPL header. `trait_points`: B200 0, B201 3, B205 19, B210 38, B275 matches the source total. `status_points` unchanged. |
| DB-TRAIT-02 | `base_ap`: Dragon Knight B199 0, B200 200, B275 200. A job with only `ApFactor`/`ApIncrease` matches `calc_baseap` on a hand-computed level. Non-trait jobs return 0. |
| MECH-TRAIT-01 | Captured DK values (catalog `trait_stats.md` §7): batk +5 per POW (B200 57, B205 POW 4 73), HIT/FLEE +2 per CON (B205 CON 4: 389/314), PATK 1 at POW 3, RES 1,2,8,9,10,16,17 for STA 1..7, HPLUS = CRT, CRATE 1..5 at CRT 3..15. |
| MECH-TRAIT-02 | Capture 3 Hyper Novice J52 (job bonus only): POW..CRT bonus 9/4/4/8/8/3 gives PATK..CRATE 4/3/9/9/3/1. SMATK and MRES formulas on hand values (SPL 9 / WIS 6). |
| MECH-TRAIT-03 | MaxAP: 200 at B200+ for a DK, 0 at B199; `ap_rate` 50 halves it; `max_ap` 150 caps it. |
| MECH-TRAIT-04 | Trait cost/cap: cost 1 below 110, 0 at 110; non-trait job cap 0 → cost 0; `raise_trait` caps the amount by points and by cap. |
| PKT-TRAIT-01 | `0x0b24` parses into `RaiseTrait` (var 219, amount 1; amount −1 parses, the engine drops it); a truncated frame drops the session. |
| ENG-TRAIT-01 | Trait-up success (captured order): the diff (`ATK1` +5, POW `0x0141` base 1 bonus 1, `PATK` at the third), then `0x00be` `SP_UPOW` 1, `SP_TRAITPOINT`, `0x00bc` ok 1 value = allocated. CON pushes `HIT`, `FLEE1` before the CON `0x0141`. Amount 5 with 3 points raises 3. |
| ENG-TRAIT-02 | Trait-up failures: non-trait job, 0 points, at cap, var 13 (a primary stat on `0x0b24`), amount 0 → only `0x00bc` ok 0 value 0. Negative amount → nothing. Dead → ok 0. |
| ENG-TRAIT-03 | `@blvl` on a DK: 199→200 (diff ends `MAXAP` 200; tail `SP_TRAITPOINT` 7), then spend the 7 (POW ×3, CON ×4), 200→201 (tail 3: the unspent pool), 201→205 (tail 19, +16). `@blvl −10` from B205 with 2 trait points: reset first (trait-triggered), pools 3980 / 7 after, diff ends `MAXAP` 0. |
| ENG-TRAIT-04 | `@streset` on a DK with allocated stats and traits at B195: `0x00be` ×12 (trait costs 1), `SP_STATUSPOINT` 3980 (table + 52), `SP_TRAITPOINT` 7 (table + 7), clamp, diff (traits back to 0, PATK/RES etc.), then the message. The observer gets nothing. |
| ENG-TRAIT-05 | `@job 4252` from a Knight at B99: `SP_STATUSPOINT` 1325, `0x00be` ×6, `SP_TRAITPOINT` 7, trait `0x00be` ×6 = 1, then the existing job tail. Novice → Novice High +52; Novice High → Super Novice −52 (no reset with 64 left). |
| ENG-TRAIT-06 | `@job 7` from a DK at B195 with < 7 trait points: the captured double reset (reset #1 with the new class, base tail to B99, −52, `0x00be` ×6, reset #2, job pushes). From B99 with < 7: −52, `0x00be` ×6, one reset. With ≥ 7 unspent: `SP_TRAITPOINT` 0 + trait `0x00be` ×6 (0), allocated traits kept. |
| ENG-TRAIT-07 | Login of a DK with traits and points: pre-ready push appends only non-zero trait values; the block after `0x00bd` sends real POW..CRT, PATK..CRATE, `SP_TRAITPOINT`, `SP_AP`, `SP_MAXAP`, `SP_U*` = 1. Map change sends the real trait `0x0141` ×6. |
| ENG-TRAIT-08 | AP clamp: a stored AP above the new MaxAP after `@blvl −n` pushes `SP_AP` with the clamp pushes, then the diff carries `MAXAP` and `AP`. At spawn the clamp is silent. |
| BIN-TRAIT-01 | Trait-up, then relog: traits, `trait_point` and AP persist (in-memory repo). |
| PER-TRAIT-01 | `save_play_state` writes the trait columns, `trait_point` and `ap`; `load_play_state` reads them back; the summary returns HP/SP, max, points, stats and exp. |
| PKT-CHAR-01 | `encode_character_info` writes the real fields at their offsets (jobpoint 1325, sppoint 11, HP 8251, maxHP 11575, SP 315, maxSP 467, STR..LUK 1; captured `rathena_traits_sequence.txt` 1346/1497) and caps SP/maxSP/points at 32767 and stats at 255. |

## 4. Licensing
- **GPL data** goes only through `tools/job-import` into `crates/ro-db/data/job/`, with regenerated headers and an updated `NOTICE.md` job section.
- **Runtime:** read from `JOB_DATA_DIR`. No `include_str!`.
- **Test fixtures** in `ro-mechanics` and `ro-engine` are small hand-written rows. Tests may read the committed files from disk.
- **`config/battle.yml`** is ours. Default *values* are facts.
- **Formulas** are written from the catalog. No translation of rAthena code.

## 5. Deferred (move into `DEFERRED.md` at Finish)
- Item, card and SC trait/AP bonuses (`bPow..bCrt`, `bMaxAP`, `bMaxAPrate`, SC_BENEDICTUM, SC_MARINE_FESTIVAL).
- AP grant and spend (4th-job skills), AP regen, AP loss on death and on VS maps (`loose_ap_on_death`, `loose_ap_on_map`), `keep_ap_on_logout` off.
- The damage consumers of PATK/SMATK/RES/MRES/HPLUS/CRATE (§11), and 4th-job skill ratios.
- Refine P.ATK/S.MATK (weapon level 5) and RES/MRES (armor level 2) bonuses (catalog `stats.md` "Refine bonuses").
- rAthena's stale `0x0141` ×12 at the start of every reset (skipped by ruling, as in stats).
- Still uncaptured, so source-only:
  - the `0x0b24` failure form (the client blocks 0-point requests, so it can't be captured);
  - amount > 1;
  - trait job → non-trait job with ≥ 7 unspent;
  - trait job → trait job;
  - the trait cap;
  - SMATK/MRES;
  - AP > 0.
