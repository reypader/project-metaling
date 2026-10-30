# Implementation Plan: Combat (player auto-attack, damage, mob death, exp)

Spec: `specs/combat/spec.md`. Design: `crates/ro-engine/DESIGN.md` §5, §9, §11. Planned on 2026-10-30. **Start every session by reading this whole file**: the notes carry context that isn't in the code.

## Decisions (settled with the user, 2026-10-30)
| Topic | Decision |
| :--- | :--- |
| Scope | Player auto-attack on mobs, damage, mob death. **No mob AI**: it is a separate later feature (spec "Why mob attacks are not in this feature"). |
| Exp | Included: damage share, attacker bonus, renewal level penalty, real NEXT exp, natural base and job level-up. |
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
  - **Open:** the LOS case at capture lines 4195-4199. Recheck against the real `prontera` `.gat` in M4.
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

## Session split (recommended)
| Session | Milestones | Why this cut |
| :--- | :--- | :--- |
| **A: data and formulas** | M1, M2 | Leaf crates (importers, `ro-db`, `ro-mechanics`), fully testable alone. |
| **B: wire and loop** | M3, M4, early review | Packets, then the attack loop and timer heap in the map tick. |
| **C: hits, death, exp, persistence** | M5, M6 | Builds on the loop; level-up packet order from the capture. |
| **D: finish** | Finish | `reviewer` over the full diff (fresh context), fixes, docs, DEFERRED, smoke, commit. |

**Early review:** run `reviewer` after M4. It adds new shared state to the map tick: the combat timer heap, `AttackState`, and the dying-mob entity lifetime, which M5 builds on (CLAUDE.md).

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [ ] **M0 catalog, capture, spec, plan**
  - [x] `metaling-explorer` filled the gaps (ranged/ammo, LOS, refusal, skip rules, infinite defense, mob stats, stepaction, exp lookup, level-up order, save timing).
  - [x] Spec and plan drafted.
  - [x] The user recorded `combat_sequence_1.txt` (2026-10-30).
  - [x] Wrote up the capture: catalog sections, PF-09/15 resolved, PF-13/14 updated, README row, PINs and message text redacted. The spec's capture marks are settled and the MECH-CMB-03 vectors are in.
  - [x] A size-table check found the full renewal table (the base file plus the overlay). The user kept 100 % (deferred).
  - [ ] Commit M0 docs. **User checkpoint.**
- [ ] **M1 data**
  - `tools/mob-import`: new fields and the mode bit-set (spec §0.1).
  - `tools/job-import`: `job_exp.yml` and `level_penalty.yml` (§0.2).
  - Importer unit tests on small hand-written YAML: defaults, `AttackDelay ≥ AttackMotion`, Ai + Modes bits, exp group expansion, penalty fill.
  - Regenerate the data and update `NOTICE.md`.
  - `ro-db`: `MobData` fields, `JobDb::next_base_exp/next_job_exp`, `LevelPenaltyDb`, root re-exports.
  - Map-server config loads the penalty data (follow `JOB_DATA_DIR`).
  - `spawn_mobs` uses real HP, level, speed and `is_boss`.
  - Tests DB-CMB-01..03, plus mob-spawning tests updated for real HP.
- [ ] **M2 mechanics**
  - `Status.adelay`/`dmotion` (players too); `calc_mob_status`.
  - `AttackerView`/`DefenderView`/`DamageResult`, `calc_weapon_attack`, `check_distance_client`, exp share.
  - `BattleConfig`: `delay_battle_damage`, `arrow_decrement`, `exp_bonus_attacker`, `exp_bonus_max_attacker`, `monster_hp_bars_info`, `display_skill_fail`; `config/battle.yml`.
  - Tests MECH-CMB-01..06 (05's LOS part lands in M4 with `path.rs`).
- [ ] **M3 core and packets**
  - `ServerEvent` variants: `Damage` (full `0x08c8`), `MobHpBar` (`0x0977`), `AttackOutOfRange` (`0x0139`), `ExpGained` (`0x0acc`), `SkillFail` (`0x0110`).
  - Check that `0x07fa`/`0x013b`/`0x099a` events exist from items/equipment.
  - `VANISH_DIED`.
  - `ClientIntent::CancelAttack` and the `0x0118` decoder.
  - Opcode names, and `table.rs` rows if any are missing.
  - Test PKT-CMB-01.
- [ ] **M4 engine attack loop**
  - `AttackState`, the combat timer heap and its place in `step_tick`.
  - Request/cancel handling (§2.2).
  - `path_search_long` in `path.rs`; range (§2.3).
  - The swing check, swing and ammo (§2.4–2.6), with `0x08c8` to the target's area.
  - Pending hits are queued; the apply is a stub until M5.
  - Step action.
  - Tests ENG-CMB-01..06, MECH-CMB-05 (LOS).
  - **Early `reviewer`.**
- [ ] **M5 hits, death, exp**
  - Hit apply with skip rules, the damage log and `0x0977` (§2.7).
  - Death (§2.8): grid removal, the vanish timer, despawn.
  - `gain_exp` and the base/job level-up loops in the captured order (§2.9).
  - Real NEXT exp everywhere; drop the placeholders.
  - Tests ENG-CMB-07..10, with existing login/`@blvl`/`@jlvl`/`@job` tests updated.
- [ ] **M6 persistence and bin**
  - Make sure an exp, level or point change reaches the autosave and logout save (revision bump).
  - Tests PER-CMB-01 (DB up: `docker compose -f tools/docker/docker-compose.yml up -d db`) and BIN-CMB-01.
- [ ] **Finish**
  - `./verify.sh --smoke` green.
  - `reviewer` over `git diff HEAD` plus untracked files; apply its fixes.
  - Docs: `crates/ro-engine/DESIGN.md` §1a (combat bullet), §5.1, §11 (what is implemented); `crates/ro-db/DESIGN.md`.
  - `DEFERRED.md`:
    - Add a "combat" section from spec §5.
    - Remove mob-spawning's "mob_db stats" item.
    - Mark stats' "Exp tables … exp gain and natural level-up" as done.
    - Update `drops.rs`'s note (still uncalled).
  - Commit.
  - **Manual client check (the user, after the commit),** on `prt_fild08` against metaling:
    1. bare-hand and Knife Poring kills;
    2. an out-of-range click (the client walks, then attacks);
    3. Esc cancel;
    4. an Archer running out of arrows;
    5. base and job level-ups from kills;
    6. relog (exp kept).

    Record a metaling capture and compare it with `combat_sequence_1.txt`.
