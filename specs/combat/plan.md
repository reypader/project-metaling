# Implementation Plan: Combat (player auto-attack, damage, mob death, exp)

Spec: `specs/combat/spec.md`. Design: `crates/ro-engine/DESIGN.md` §5, §9, §11. Planned on 2026-10-30. **Start every session by reading this whole file**: the notes carry context that isn't in the code.

## Decisions (settled with the user, 2026-10-30)
| Topic | Decision |
| :--- | :--- |
| Scope | Player auto-attack on mobs, damage, mob death. **No mob AI**: it is a separate later feature (spec "Why mob attacks are not in this feature"). |
| Exp | Included: damage share, attacker bonus, real NEXT exp, natural base and job level-up. **No level penalty** (user, 2026-10-30, during M1): every kill pays full exp whatever the level difference. rAthena's `level_penalty.yml` is not imported. |
| Drops / respawn | **Neither.** A dead mob stays gone until the map world restarts. |
| Ranged | Included, with ammo spent per swing. |
| Element / size | **Deferred: 100 %**, with no `attr_fix`/`size_fix` import. This was re-confirmed by the user after the capture showed the full size table (Dagger vs Medium 75 % etc.). The damage function still takes `size_mod`/`attr_rate` inputs, so the import later is data-only. |
| Capture | `combat_sequence_1.txt`, recorded by the user on 2026-10-30, written up in M0. |
| *Planning defaults (user may revisit)* | `specs/combat/`. Damage is pure in `ro-mechanics` with a roll closure. Mobs get a `Status` from `calc_mob_status`. Swings are scheduled from the due time (no drift). One per-map combat timer heap. A dying mob leaves the grid at once and despawns after its +250 ms vanish. Level-up uses the flow + follow-up pattern. |

## Notes for the implementer

### Captures and catalog
- **`item_drop_pickup_sequence.txt` lines 1064-1118** (2026-10-23, attacker `#6`, observer `#3`) is a ground-truth kill:
  - The client sends `0x0437` action 7. Then `0x0088` ×2 and `0x08c8` go to **both** clients in the same ms.
  - `src_speed` 570, `dmg_speed` 480. Damage 40, then 41 at +1144 ms (= adelay 2 × 570).
  - `0x0977` hp 15/55 at +580 ms.
  - The kill tick: `0x0977` hp 0, `0x0acb`, `0x0acc`. Then `0x0080` type 1 at +246 ms.
  - The capture was in a party, so `0x080e` frames appear. Those are party HP, not ours. The mob's own `0x0088`/`0x08c8` is AI; ignore it.
- **`combat_sequence_1.txt`** (2026-10-30, file line = proxy # + 10): A = `#3` (`combattest1`, Novice → Archer), B = `#6` (observer; attacks in step 8). They are **not** in a party. A's chat lines are rough markers only; the write-ups decoded events from the packets.
  - **Write-ups:**
    - `auto_attack.md` §8: action 0 vs 7, `0x0118`, `0x0088` counts, deferred swings, `0x0139`, the client chase, cancels.
    - `combat.md` §7: 70 damage vectors with attacker stats, miss/crit, `0x0977` timing, kill order, the ammo sequence, LOS refusals, Marine Sphere.
    - `experience.md` "Capture": no-level-up kill, base/job/double level-up tables, the step 8 share.
  - **Resolved:** PF-09 and PF-15. PF-13 is mostly settled. PF-14 is still open.
  - **Things the capture taught that are easy to miss:**
    - A plain click is action 0, a single swing.
    - The client sends `0x0118` about 100 ms after a 7 when the target is re-clicked or a held click is released.
    - A deferred swing sends nothing at request time.
    - A failed line-of-sight check gets one `0x0088` and nothing else.
    - `0x0977` goes only to damage-log members.
    - rAthena's natural level-up sends the tail **before** the diff, the reverse of `@blvl`.
    - A double level-up sends two diffs in rAthena; ours sends one (spec §2.9).
    - The size table is real but deferred: MECH-CMB-03 feeds `size_mod` 75 for the Dagger-vs-Poring vectors.
  - **Open:** the LOS case at capture lines 4195-4199, plus proxy #4362 and #4374 (likely LOS refusals too; `combat.md` §7.9). Recheck all three against the real `prontera` `.gat` in M4.
- **`combat_sequence_2.txt`** (2026-10-30, written up in catalog `auto_attack.md` §8.6, no step markers):
  - PF-14 is settled: range 1 hits 2 cells straight with A standing. For (2,1), (2,2) and 3 straight the client walks to an adjacent cell first, then sends the request during that 1-cell walk. The server swings on arrival with `0x0088` ×2.
  - The gun with no bullets gets `0x0088` ×2, then `0x0110` `{skill 0, cause 84}`, to self only. **The proxy has no `0x0110` row** (it misframes it): add it to `table.rs` in M3.
  - One captured gun shot follows the bow's ammo order (`0x07fa` reason 1, `SP_WEIGHT`, `0x08c8`).
  - The `canact` re-request is still uncaptured: the user saw the client debounce fast clicks.
- **Catalog sections added in M0:**
  - `auto_attack.md` §5.3 (ranged and ammo), §7.1 (delayed-hit skip rules), §7.2 (stepaction), §7.3 (stop has no packet);
  - `combat.md` §1.1 (mob stats, Poring worked example), §1.2 (`IgnoreMelee` infinite defense);
  - `movement_aoi.md` §5 (`path_search_long`);
  - `experience.md` (natural level-up order, double level-up, save timing, mob exp at load);
  - `exp_tables.md` "Lookup rule";
  - `packets/combat.md` `0x0110`.

### Existing code anchors (as of `ef2939c`)
- **ro-core:**
  - `ClientIntent::RequestAction` (`message.rs:90`).
  - `ServerEvent::EntityAction` (`message.rs:552`), used for sit, stand and pickup.
  - `UnitVanished` (`:516`), `AttackRange` (`:651`), `LongLongParameterChanged` (`:636`).
  - `CharacterStatus.base_exp/job_exp` (`character.rs:14-36`), already persisted.
- **ro-packets:**
  - `encode_zc_notify_act_modern` / `ActParams` (`outgoing/map.rs:344-386`) already carry every `0x08c8` field.
  - The `encoder.rs:224-270` arm handles only sit/stand/pickup.
  - `0x0977`/`0x0acc` exist only as `PACKET_0X0977`/`PACKET_0X0ACC` with lengths. `0x0139` has a length only.
  - `0x0118` falls to `Unhandled` (`incoming/frame.rs:142`).
- **ro-engine:**
  - `engine.rs:437-446` (action 0/7 ignored), `:1079-1097` (`step_tick` order: floor expiry, walkers, intents, plan moves, stat pass).
  - `engine.rs:109` + `floor_item.rs:126-160` is the heap pattern.
  - `rng.rs` `SplitMix64` (crate-private: `below(n)`).
  - `mob.rs:18-22, 239-266` (placeholders), `world.rs:93-104` (`MobInfo`), `world.rs:274-287` (`spawn_mob`), `:362` (`despawn`).
  - `emit_aoi` (`engine.rs:348`).
  - `level_command.rs:150-180` `raise_base_level` (the `@blvl` order: tail queued last), `:343` `send_job_level`, `:553` `base_tail`.
  - `status.rs:79-80` (NEXT placeholders).
  - `drops.rs` stays unused (no drops).
- **ro-mechanics:** `Status` (`status.rs:203-250`; no `adelay`/`dmotion` yet), `HandInput`/`RefineInput { bonus, random_bonus }` (overrefine is available), and `BattleConfig` (`config.rs`, already has `min/max_hitrate`, `enable_critical`, `enable_perfect_flee`, `multi_level_up*`).
- **ro-db:** `ItemData` has `weapon_level`, `ammo_type`, `attack` and `range`. `MobData` (`mob.rs:51-66`) uses `deny_unknown_fields`, so add fields to both `RawMob` (`tools/mob-import/src/lib.rs:127-140`) and `MobData`. `JobDb` has no exp tables yet.
- **map-server:** attack intents need no bin change (they fall through to `engine.push_intent`). New events need an encoder arm plus `is_implemented_outbound_opcode` (`encoder.rs:389`).

### Pitfalls
- **No drift:** compute `attackable_at` from the timer's due time. A test runs 10 swings at a 20 ms tick and checks the 10th lands at `first + 9 × adelay`.
- **Two `0x0088`s** precede the first swing of an immediate attack (both to `AREA`, attacker included). A deferred first swing (from `attackable_at`) has none from `unit_attack`. Check against the capture.
- **Pending hits survive a stop.** Only the skip rules drop them. The distance snapshot is taken at swing time.
- **The damage log is by char id** and counts overkill. `0x0977` goes to log members within 14 cells, not to the area.
- **A dying mob is not in the grid.** Every target lookup (requests, swing checks, step actions) must check `dying`, not just liveness. The vanish is sent around the death **cell**, because the entity is out of the grid.
- **Level-up order differs from `@blvl`:** the natural tail goes **first** and inline, then the recalc. Don't reuse `raise_base_level` as-is. Factor out the shared point math.
- **Both level-ups' diffs collapse** into one tick-end diff. Document it as a deviation if the capture shows two.
- **`SKILLS` dirty** on every base or job level-up (tree visibility depends on both).
- **Ammo packets come before the swing's `0x08c8`**, and the consuming swing still hits. When the last arrow goes, the unequip `0x099a` comes before `0x07fa`.
- **`ro-mechanics` stays dependency-free:** the engine builds `AttackerView`/`DefenderView` and `MobStatusInputs` from `ro-db` types.
- **Hot path:** the swing check and hit apply run per tick for active attackers only (heap-driven, no scans). The damage log is a small inline array (`SmallVec<[_; 4]>` growing to 20). Don't clone `Status`: borrow it.

## Capture script (M0, for the user)
Use rAthena via `tools/proxy` with `PROXY_FULL_PAYLOAD=1` on `prt_fild08`, with clients A (attacker) and B (observer, in view, **not partied**). Before each step A says `STEP n <what>`.
1. A logs in as a fresh, bare-handed Novice (`0x00bd`).
2. A plain-clicks a Poring and kills it. Then a ctrl-click (or `/noctrl`): is it action 0 or 7?
3. A attacks from 2 cells straight, then from (2,1).
4. A clicks a far mob (walk + step action); mid-walk, clicks the ground; locks on again and presses Esc.
5. A re-clicks within half a second of a swing.
6. Base and job level-ups from kills (ideally one kill doing both); `@jlevel` to 9, kill to 10, kill once more.
7. `@item 1201` (Knife), equip, attack. `@luk 98`, attack until crits. Optional 7b: hit a Green/Blue Plant (`prt_fild01`-`06`).
8. A and B hit one Poring, A doing most of the damage.
9. `@job 3`, `@item 1701`, `@item 1750 5`, equip, attack until the arrows run out; attack with none; shoot past a wall from 5+ cells.
10. Relog, then char select.

## Session A results (M1, M2; read before M3)
Uncommitted, by the commit-once rule. `./verify.sh` was green (853 tests) at the end of session A.
- **No level penalty** (user, mid-M1). There is no `LevelPenaltyDb` and no `level_penalty.yml`. `gain_exp` uses `shared_exp(db_exp, exp_share(..))` directly. The catalog keeps the rAthena facts.
- **Data API (ro-db):**
  - `MobData` gained `base_exp`, `job_exp`, `attack`, `defense: i16`, `resistance`, `str`..`luk: u16`, `attack_range`, `size: MobSize`, `race: MobRace`, `element: Element`, `element_level`, `attack_delay`, `attack_motion` and `damage_motion`. It also has `modes: MobModes`, a bit-set of `MobMode` with `has_mode()` and `is_mvp()`.
  - The old `mvp: bool` is gone (it is the `Mvp` bit now).
  - `MobData::default()` gives rAthena's struct defaults, for tests: `..MobData::default()`.
  - `MOB_DATA` is now version 2.
  - The exp tables are in `job/job_exp.yml` (`JOB_EXP_DATA`): groups of job ids that share a table. `JobDb::load_dir` needs all three job files. Lookups are `JobDb::next_base_exp(job, level)` and `next_job_exp(job, level)`, with `NEXT_BASE_EXP_AT_MAX` and `NEXT_JOB_EXP_AT_MAX`. Nothing calls them yet; M5 replaces the `status.rs` NEXT placeholders.
- **Spawn:** `spawn_mobs` takes HP, max HP, level (the spawn line first, then the db), walk speed and `is_boss` from `MobDb`. A mob id missing from the db spawns with `MobData::default()` and logs one warning per spawn line. `UnitAppearance` (ro-core) gained `is_boss: u8`, which the entry encoder writes.
  - `MobInfo` does **not** hold a `Status`, a damage log or `dying` yet (spec §2.1). Add them in M4/M5: call `calc_mob_status` once per spawn, or cache it per mob id.
- **Mechanics API (ro-mechanics):**
  - `Status.adelay` and `dmotion` exist for players and mobs. They are not in the push diff.
  - `calc_mob_status(&MobStatusInputs, &BattleConfig)`.
  - `AttackerView::from_status(&Status)`. Then set `weapon: Option<WeaponAttack { atk, refine_atk, level, overrefine, dex_based }>`, `equip_atk` (+ ammo `Attack`) and `long_range`.
    - `long_range` is for bow and guns only, from the `ro_db::WeaponType`. Don't use `WeaponClass::is_ranged()`: that also covers whip and instrument, which are short-range.
    - `dex_based` = `WeaponClass::is_ranged()` (now `pub`).
    - `size_mod` is 100 and `attr` is `AttrRates::default()`.
  - `DefenderView::from_status(&Status)`. Then set `ignore_melee` / `ignore_ranged` from `MobMode::IgnoreMelee` / `IgnoreRanged`. IgnoreRanged is included because it is the same rule.
  - `calc_weapon_attack(.., roll)` returns `DamageResult { damage, damage2, div, kind: HitKind }`. Use `kind.wire_type()` for `0x08c8`. On a miss, `dmg_speed` must be 0: the engine sends `dmotion` only when `kind.is_hit()`.
  - The rolls come in this order: lucky dodge `roll(1000)` (only when flee2 > 0), crit `roll(1000)` (only when cri − 2·LUK > 0), hit `roll(100)` (not after a crit), the variance `roll(span)`, then the overrefine `roll(n)`.
  - `check_distance_client(dx, dy, range)` and `client_distance`.
  - `exp_share(dmg, total, attackers, &cfg) -> f64` and `shared_exp(db_exp, share) -> u64`.
- **BattleConfig:** `delay_battle_damage`, `arrow_decrement` (0..=2), `exp_bonus_attacker`, `exp_bonus_max_attacker` (≥ 1), `monster_hp_bars_info` and `display_skill_fail`. They are also in `config/battle.yml`.
- **Facts from the explorer** (catalog `db/mob_db.md`): `AMOTION_DIVIDER_NOPC` is 1, so `AttackMotion` defaults to 100 and is clamped to 1..=8000. The older "1..4000" in `combat.md` §1.1 is superseded. `Race` uses `Player_Human` / `Player_Doram`. Enum and mode names match case-insensitively.

## Session split (recommended)
| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: data and formulas** | M1, M2 | Leaf crates (importers, `ro-db`, `ro-mechanics`), fully testable alone. |
| **B: wire and loop** (done 2026-10-30) | M3, M4, early review | Packets, then the attack loop and timer heap in the map tick. |
| **C: hits, death, exp, persistence** | M5, M6 | Builds on the loop; level-up packet order from the capture. |
| **D: finish** | Finish | `reviewer` over the full diff (fresh context), fixes, docs, DEFERRED, smoke, commit. |

**Early review:** run `reviewer` after M4. It adds new shared state to the map tick: the combat timer heap, `AttackState`, and the dying-mob entity lifetime, which M5 builds on (CLAUDE.md).

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 catalog, capture, spec, plan**
  - [x] `metaling-explorer` filled the gaps (ranged/ammo, LOS, refusal, skip rules, infinite defense, mob stats, stepaction, exp lookup, level-up order, save timing).
  - [x] Spec and plan drafted.
  - [x] The user recorded `combat_sequence_1.txt` (2026-10-30).
  - [x] Wrote up the capture: catalog sections, PF-09/15 resolved, PF-13/14 updated, README row, PINs and message text redacted. The spec's capture marks are settled and the MECH-CMB-03 vectors are in.
  - [x] A size-table check found the full renewal table (the base file plus the overlay). The user kept 100 % (deferred).
  - [x] Commit M0 docs. **User checkpoint.**
- [x] **M1 data** (session A, 2026-10-30)
  - `tools/mob-import`: new fields and the mode bit-set (spec §0.1).
  - `tools/job-import`: `job_exp.yml` (§0.2).
  - Importer unit tests on small hand-written YAML: defaults, `AttackDelay ≥ AttackMotion`, Ai + Modes bits, exp group expansion.
  - Regenerate the data and update `NOTICE.md`.
  - `ro-db`: `MobData` fields, `JobDb::next_base_exp/next_job_exp`, root re-exports.
  - `spawn_mobs` uses real HP, level, speed and `is_boss`.
  - Tests DB-CMB-01..03, plus mob-spawning tests updated for real HP.
- [x] **M2 mechanics** (session A, 2026-10-30)
  - `Status.adelay`/`dmotion` (players too); `calc_mob_status`.
  - `AttackerView`/`DefenderView`/`DamageResult`, `calc_weapon_attack`, `check_distance_client`, exp share.
  - `BattleConfig`: `delay_battle_damage`, `arrow_decrement`, `exp_bonus_attacker`, `exp_bonus_max_attacker`, `monster_hp_bars_info`, `display_skill_fail`; `config/battle.yml`.
  - Tests MECH-CMB-01..06 (05's LOS part lands in M4 with `path.rs`).
- [x] **M3 core and packets** (session B, 2026-10-30)
  - `ServerEvent` variants: `Damage` (full `0x08c8`), `MobHpBar` (`0x0977`), `AttackOutOfRange` (`0x0139`), `ExpGained` (`0x0acc`), `SkillFail` (`0x0110`).
  - Check that `0x07fa`/`0x013b`/`0x099a` events exist from items/equipment.
  - `VANISH_DIED`.
  - `ClientIntent::CancelAttack` and the `0x0118` decoder.
  - Opcode names, and `table.rs` rows if any are missing.
  - Test PKT-CMB-01.
  - **Results:**
    - `0x013b` (`ActionFailure`) and `0x099a` (`UnequipAck`) existed. `0x07fa` did not: added `ServerEvent::ItemDeleted { index, amount, reason }` (reason 1 for ammo).
    - Event shapes are trimmed to what combat sends: `ExpGained { gid, var_id, amount }` always writes `exp_type` 0; `SkillFail { skill_id, cause }` always writes `btype`, `item_id` and `flag` as 0. `MobHpBar` hp fields are `u32`.
    - The encoders are in `ro-packets/src/outgoing/combat.rs`; `Damage` reuses `encode_zc_notify_act_modern` (`is_sp_damage` 0).
    - Opcode names: `ZC_HP_INFO` and `ZC_NOTIFY_EXP2` replace `PACKET_0X0977`/`PACKET_0X0ACC`; `ZC_ACK_TOUSESKILL`, `ZC_ATTACK_FAILURE_FOR_DISTANCE` and `ZC_DELETE_ITEM_FROM_BODY` are new. `table.rs` gained the `0x0110` row (14); `0x0139` and `0x07fa` were already there.
    - `VANISH_DIED` is now one `pub(crate)` const in `engine.rs` (it was duplicated in `floor_item.rs` and `wear.rs`).
    - The engine still ignores `CancelAttack` (falls to the "not handled" debug arm) until M4.
- [x] **M4 engine attack loop** (session B, 2026-10-30)
  - `AttackState`, the combat timer heap and its place in `step_tick`.
  - Request/cancel handling (§2.2).
  - `path_search_long` in `path.rs`; range (§2.3).
  - The swing check, swing and ammo (§2.4–2.6), with `0x08c8` to the target's area.
  - Pending hits are queued; the apply is a stub until M5.
  - Step action.
  - Tests ENG-CMB-01..06, MECH-CMB-05 (LOS).
  - **Early `reviewer`.**
  - **Results (read before M5):**
    - Code: `ro-engine/src/combat.rs` (all of it), `path.rs` (`line_of_sight`, `battle_check_range`), `mob.rs` (`MobStats`), hooks in `engine.rs` (`step_tick`, `handle_intent`, `plan_moves`, `advance_walkers`) and `warp.rs` (`warp_same_map`). Tests: `tests/combat_tests.rs` (ENG-CMB-01..06, M4 parts) and `path.rs` unit tests `mech_cmb_05_*`.
    - **State:** `World.attacks: SparseSet<AttackState>`, created on the first request and kept after the attack stops (it holds `attackable_at`/`canact_at`); `world.despawn` removes it. `SparseSet::entry` (get or insert default) is new.
    - **Timers:** `MapInstance.combat_timers: CombatTimers`, a min-heap of `(due, seq)` with `CombatTimer::{Swing, StepAction} { attacker, generation }` and `Hit(PendingHit)`. Swing/step timers are stale when `AttackState.swing_gen`/`step_gen` moved on; a hit re-checks both units. `run_combat_timers(now)` runs right after `advance_walkers`. **M5 adds `CombatTimer::Vanish`.**
    - **Mob stats:** `MobInfo.stats: Arc<MobStats>` (`status` from `calc_mob_status`, `defender: DefenderView` with the `IgnoreMelee`/`IgnoreRanged` modes), cached per mob id in `MapInstance.mob_stats`. `MobInfo` still has no damage log or `dying` flag (M5).
    - **Target validity** is `MapInstance::attackable(t)`: alive, a mob, `in_grid`. M5's death takes the mob out of the grid, so a dying mob fails it everywhere with no extra flag needed for lookups (still add `dying` if death handling wants it).
    - **The hit stub** (`apply_hit`): skip rules (target attackable, attacker alive and a player on this map, Chebyshev ≤ snapshot), then `hp = hp.saturating_sub(damage)`. **M5 replaces the body after the skip rules** with the damage log, `0x0977` and death. `PendingHit` has no char id yet; M5 adds it for the log (use the attacker's char id at apply time, catalog §7.1).
    - **Weapon view:** `WeaponGear.level` (weapon level) is new. `refine_atk = refine.bonus / 100`, `overrefine = derived.overrefine[0]`, `dex_based = weapon_class(..).is_ranged()` (`stats::weapon_class` is now `pub(crate)`). `long_range` = the weapon needs ammo (bow → Arrow; revolver, rifle, gatling, shotgun, grenade → Bullet).
    - The test attacker is a level-99 Novice: amotion **450**, adelay 900 (not 440). The no-drift test ticks every 37 ms because 20 and 30 divide 900 and couldn't show drift (checked by mutation: using the tick's `now` fails it).
    - **LOS recheck done** (`metaling-explorer`, catalog `combat.md` §7.9): all three open single-`0x0088` requests are LOS refusals, and our `line_of_sight` agrees with the capture in all 8 cases. Use the Renewal cells in `$RATHENA_PATH/db/re/map_cache.dat`, never `db/map_cache.dat` (it's the pre-re cache, where the walls differ).
    - **Swing-check order fixed after the explorer's source check:** the line test (`line_of_sight`, no distance limit) now runs with the liveness checks, before the client-distance test, so out of range + behind a wall is a silent drop, not `0x0139` (ENG-CMB-03 has the case). The catalog had said `CELL_CHKNOPASS`; it is `CELL_CHKWALL` (catalog `auto_attack.md` §3 step 2 and `movement_aoi.md` §5 corrected).
    - **Early review done** (2026-10-30). Fixed:
      - LOS before the range test (above).
      - A move and an attack in the same tick: `request_attack` now calls `plan_move(e, now)` (factored out of `plan_moves`), so a move queued earlier in the tick walks first and the attack becomes a step action. Test `eng_cmb_05_move_then_attack_in_one_tick`.
      - A successful pickup calls `stop_attack` (`pc_takeitem`). Test `eng_cmb_04_pickup_stops_the_loop`.
      - `set_registries` clears `mob_stats`.
    - Not changed:
      - "Hidden unit sends `0x0088`" can't happen: `handle_intent` drops gameplay intents from units not in the grid.
      - Step actions starve for walkers faster than about 40 ms/cell (each step in a tick bumps `step_gen` before the timers run). Harmless at today's speeds; interleave walkers and timers by due time if speed buffs arrive.
      - Spammed requests between `canact` and `attackable` each push a stale `Swing`. Bounded by request rate × adelay.
      - Whether an equip/look change stops the attack (catalog §7.3 "look change") is unchecked.
    - **For M5 (from the review):**
      - Schedule `Vanish` from the hit timer's `due` (`run_combat_timers` has it), not `now`.
      - With `amotion ≤ 1` or `delay_battle_damage` off, `apply_hit` (so death, exp and level-up) runs inside `swing()`, before `swing_check` sets the delays and re-arms. The re-armed swing then fails `attackable`. Keep death free of borrows `swing_check` still holds.
      - Death must hide the mob (grid removal) before exp runs. Send the vanish and anything area-wide around the **death cell**, not `emit_aoi(mob)` (a hidden mob's area comes from its stale cell).
      - Read the attacker's char id in `apply_hit` from `players.get(hit.attacker)`; `PendingHit` doesn't need to store it.
  - **Rulings (M4):**
    - Ruling: any non-attack `0x0437` action (sit, stand, pickup…) and a same-map warp use `drop_attack` (rAthena `unit_unattackable`: also clears the step action), not the bare stop. rAthena leaves an already-armed step timer on sit; with the walk stopped it could still fire once. Cost if wrong: one missing stray swing after sit-while-walking.
    - Ruling: a move request drops the attack only when its path is found (in `plan_moves`), like `unit_walktoxy`; an unreachable click keeps the loop. Cost if wrong: an invalid ground click wouldn't cancel the attack.
    - Ruling: the in-loop `canact` re-arm case of `unit_attack_timer_sub` step 4 is not implemented: with `adelay = 2 × amotion` and the 100 ms floor, `attackable_at ≥ canact_at` always holds for players. Revisit for mob attackers.
    - Ruling: `amotion_floor` comes from `BattleConfig::amotion_floor(class_flags(job_mapid(job)))` (third classes get 70 ms, others 100).
    - Ruling: `attack()` (from a request or a step action) bumps `swing_gen`, cancelling any armed swing, where rAthena would "just retarget". The request path already stopped the attack, so only a step action firing during a loop differs, which can't happen (a walk drops the loop).
- [x] **M5 hits, death, exp** (session C, 2026-10-31)
  - Hit apply with skip rules, the damage log and `0x0977` (§2.7).
  - Death (§2.8): grid removal, the vanish timer, despawn.
  - `gain_exp` and the base/job level-up loops in the captured order (§2.9).
  - Real NEXT exp everywhere; drop the placeholders.
  - Tests ENG-CMB-07..10, with existing login/`@blvl`/`@jlvl`/`@job` tests updated.
  - **Results:**
    - Code:
      - `combat.rs`: `DamageLog` (`SmallVec<[DamageEntry; 4]>`, cap 20, by char id, raw damage), `apply_hit(hit, due)`, `send_hp_bar`, `kill_mob`, `CombatTimer::Vanish { mob, cell }`, `vanish_dead_mob`.
      - New `experience.rs`: `gain_exp`, the pure `level_up` loop (unit-tested with the captured clamp), and the base/job bursts.
      - `MobInfo` gained `damage_log` and `dying`. `MobStats` gained `base_exp`/`job_exp`. `attackable` also checks `!dying`.
      - `engine.rs` gained `recipients_at(cell, radius)` and `emit_to(chars, event)`.
    - The tails are now methods reading the player's current state: `base_tail(e)` and `job_level_tail(e)` in `level_command.rs`. They are shared by `@blvl`, `@jlvl`, `@job` and kills. The `PLACEHOLDER_NEXT_*` constants are gone; the login burst, the tails and `@job` use `JobDb::next_*_exp`.
    - Ordering mechanism: `gain_exp` keeps a `queued` flag. Once a level-up queues follow-ups, every later push of that kill (job tail, `SP_JOBEXP`, `0x0acc`) is queued too.
    - Tests: `combat_tests.rs` ENG-CMB-07..10, plus the `0x0977` check in ENG-CMB-01. ENG-CMB-02/06 now filter `0x0977` (`without_bars`), because their ticks also land the previous hit. `level_command_tests`/`trait_tests` expect the real NEXT values (`next_base`/`next_job` helpers). `./verify.sh` green: 874 tests.
  - **Rulings (M5):**
    - Ruling: the attacker `count` for the share bonus is the log entries whose player is on this map, dead or not; only the alive get exp (catalog `mobs.md` §3 step 2). The spec said "valid entries". Cost if wrong: none today, since players can't die yet.
    - Ruling: `LevelGearCheck` is queued right after effect 0, before the job burst (rAthena's base burst step 9), not last as spec §2.9 lists. The spec's own "in rAthena's order" says this. Cost if wrong: the rare `elvmax` unequip packets move ahead of the job tail.
    - Ruling: a job with no `JobDb` entry counts as at max level for exp (exp capped, `0x0acc` shows 0). Cost if wrong: only with broken data.
    - Ruling: `0x0977` recipients are the log members on this map within 14 cells of the mob, whether or not they're in the grid. Cost if wrong: a player who is loading might get one stray bar.
    - Not done (for DEFERRED): the `0x0977` rAthena sends to a log member when the mob comes into view (`clif_getareachar_unit`). (The two-kills-in-one-tick case was fixed after the final review; see Finish.)
- [x] **M6 persistence and bin** (session C, 2026-10-31)
  - Make sure an exp, level or point change reaches the autosave and logout save (revision bump).
  - Tests PER-CMB-01 (DB up: `docker compose -f tools/docker/docker-compose.yml up -d db`) and BIN-CMB-01.
  - **Results:**
    - No code change was needed. There is no play-state revision: the map-server's autosave compares the whole `progress_snapshot` (levels, exp, points) with the last save, and logout always saves `play_snapshot`.
    - Tests in `bins/map-server/tests/combat_tests.rs`, using the shipped data and a base-50 / job-9 Novice next to a Poring:
      - `bin_cmb_01_kill_over_tcp`: `0x08c8`, then `0x0977` hp 0, then `0x0acc` 150/40, then `0x0080` type 1.
      - `per_cmb_01_kill_exp_is_saved_and_survives_relog`: the autosave holds 150/40 while A is online, and the relog's `0x0acb`s carry it.
    - Ruling: PER-CMB-01 is a map-server test (kill, then save, then relog) on the in-memory repo. The Postgres round trip of exp and levels is already covered by `ro-persistence` PER-INV-04 / PER-STAT-01, since there are no new columns. Cost if wrong: none; no schema changed.
- [x] **Finish** (session C, 2026-10-31)
  - [x] `./verify.sh --smoke` green: 878 tests, smoke ok. The user's rAthena server containers were stopped for the run and restarted.
  - [x] Commit.
  - [x] `reviewer` over `git diff HEAD` plus untracked files; apply its fixes. **Done 2026-10-31.** No Critical findings. Fixed:
    - Important 1: `exp_share` now follows rAthena's float order: `per += per × ((n − 1) × 25) / 100`, with the integer product first. `metaling-explorer` confirmed it at `mob.cpp:3130` and wrote the exact order into catalog `mobs.md` §3 step 6 and `experience.md`. The rest of the exp formula already matched (`trunc(db × per)`, min 1; `bonus` and the map-flag rates are no-ops at 100). Test: MECH-CMB-06's 4 attackers at 3/5 of 40 give 42 (RED 41, then GREEN). The engine test helper `expected_exp` uses the same order now.
    - Important 2: queued pushes were snapshots, so a later push of the same var in the tick was overwritten on the client. Queued tails and exp pushes are now `FollowUp::Live(LivePush::{BaseTail, JobTail, Exp(var)})`, read when sent; only the `0x0acc` amounts stay snapshots. `@blvl`'s queued tail uses it too. `gain_exp` starts with `queued = !followups.is_empty()`, so a second kill in the tick queues behind the first, as in rAthena. Test: `eng_cmb_09_two_kills_in_one_tick_end_on_live_values` (RED: the last `SP_JOBEXP` was 25 with the server at 50, then GREEN).
    - Minor 7, graded up because NEXT 0 could divide by zero in the client: `db_cmb_03_shipped_bars_are_never_zero_below_the_max`. It passed at once (the shipped data is fine), so it stays as a guard.
    - Ruling: the reviewer agreed with every M4–M6 ruling.
    - Deferred minors (test strength and memory, not fixed):
      - ENG-CMB-07 ticks the kill at its exact due time, so "Vanish from the hit's due, not `now`" isn't pinned.
      - ENG-CMB-09 doesn't assert the `SKILLS` re-check on a level-up.
      - The 14-cell filter on `0x0977` is untested.
      - "Leaves the grid at once" is only checked through `attackable` (it could be checked on `MobView.visible`).
      - The inline `SmallVec<[DamageEntry; 4]>` adds about 72 B to every mob.
  - [x] Docs: `crates/ro-engine/DESIGN.md` §1a (combat bullet), §5.1, §11 (what is implemented); `crates/ro-db/DESIGN.md`.
  - [x] `DEFERRED.md`:
    - Add a "combat" section from spec §5.
    - Remove mob-spawning's "mob_db stats" item.
    - Mark stats' "Exp tables … exp gain and natural level-up" as done.
    - Update `drops.rs`'s note (still uncalled).
  - **Manual client check (the user, after the commit),** on `prt_fild08` against metaling:
    1. bare-hand and Knife Poring kills;
    2. an out-of-range click (the client walks, then attacks);
    3. Esc cancel;
    4. an Archer running out of arrows;
    5. base and job level-ups from kills;
    6. relog (exp kept).

    Record a metaling capture and compare it with `combat_sequence_1.txt`.
