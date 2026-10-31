# Specification: Combat (player auto-attack, normal-attack damage, mob death with exp and level-up)

Status: **final for M0** (2026-10-30). Captures: `reference/catalog/captures/combat_sequence_1.txt` (this feature), with write-ups in catalog `auto_attack.md` §8, `combat.md` §7 and `experience.md` "Capture: combat_sequence_1.txt"; and `item_drop_pickup_sequence.txt` (an earlier kill).
- **Also captured** in `combat_sequence_2.txt` (catalog `auto_attack.md` §8.6): the straight-line +1 range (PF-14, settled) and the gun no-ammo `0x0110` cause 84.
- **Source-only, not captured:** the re-click inside `canact` (silent drop; the client seems to debounce fast clicks), a base-only level-up and exp at max job level.
- Where the spec says "captured", the capture is the ground truth.

Design: `crates/ro-engine/DESIGN.md` §5 (tick), §9 (status), §11 (damage). Catalog:
- `mechanics/auto_attack.md`: request, range, timer loop, swing, §5.3 ranged and ammo, §7 delayed-hit skip rules, stepaction and stop effects.
- `mechanics/combat.md`: hit, crit, damage parts, DEF, §1.1 mob target stats, §1.2 infinite defense.
- `mechanics/movement_aoi.md` §5: line of sight.
- `mechanics/mobs.md` §2 (HP reduction, damage log) and §3 (death).
- `mechanics/experience.md` and `db/exp_tables.md`: exp gain, natural level-up order, lookup rule.
- `packets/combat.md` and `packets/map_units.md` `0x08c8`.

## Scope

**In:**
- A player auto-attacks a mob with `0x0437` action 0 or 7. Melee and ranged; bows and guns need and spend ammo.
- Renewal normal-attack damage: hit, crit, the damage parts, DEF.
- The delayed HP change, the mob damage log and the `0x0977` HP bar.
- Mob death: exp to the damage-log members, natural base and job level-up, and the death vanish.
- Real mob stats at spawn.

**Out** (§5): mob AI of any kind, which covers retaliation, aggro, roaming, chase and mob → player hits. Also player death, drops, respawn, elements and sizes (all 100 %), dual wield and katar `damage2`, Double Attack, cards and item bonuses, masteries, status effects, MVP and parties.

### Why mob attacks are not in this feature
rAthena runs mob and player swings through the same attack timer and the same `battle_calc_weapon_attack`; mob stats are just data (DESIGN §11.5). What differs is who starts an attack and how the attacker gets in range:
- the AI's aggro scan;
- `mob_attacked` retaliation at hit + dmotion;
- chase with `unit_walktobl`;
- random walk;
- a square range check.

A mob hit also needs player HP pushes, death and respawn. None of that exists. This feature builds the swing → `DamageResult` → delayed apply core so that it doesn't care which side attacks, but only players start attacks. The mob-AI feature reuses the core.

## 0. Data pipeline

### 0.1 `tools/mob-import` (extend)
- **New `RawMob` / `MobData` fields:**
  - `BaseExp`, `JobExp`, `Attack`, `Defense`, `Str`, `Agi`, `Vit`, `Int`, `Dex`, `Luk`, `Resistance`, `AttackRange`;
  - `Size`, `Race`, `Element`, `ElementLevel`;
  - `AttackDelay`, `AttackMotion`, `DamageMotion`;
  - a `modes` bit-set built from `Ai` and `Modes`. This feature uses only `IgnoreMelee` (catalog `combat.md` §1.2), but the importer keeps the whole set.
- Default values follow catalog `db/mob_db.md`. `AttackDelay` is raised to at least `AttackMotion`, as at rAthena's load.
- Exp rates are 100, so `BaseExp` and `JobExp` are stored verbatim.
- Regenerate `crates/ro-db/data/mob/mob_db.yml` and update the `NOTICE.md` "mob" section.

### 0.2 `tools/job-import` (extend)
- Read `db/re/job_exp.yml` and write the per-job base and job exp tables. Use the existing `job_ids.rs` table. The 27 mounted "2" names are skipped as today; catalog `db/exp_tables.md` "Lookup rule".
- Add headers and attribution, and update `NOTICE.md`.

## 1. Data API (`ro-db`, root re-exports)
- `MobData` gains the fields of §0.1, plus a `has_mode(MobMode)` helper.
- `JobDb::next_base_exp(job, level) -> u64` and `next_job_exp(job, level) -> u64`:
  - The value is the row `Level: L`, the exp needed for L → L+1.
  - Level 0 gives 0.
  - At or above the class max, or for a job with no table, they return the constants 99_999_999 and 999_999_999.
  - A missing row gives 0, which stops the level-up loop.

## 2. Behaviour

### 2.1 Mob status at spawn
- **Spawn wiring:** `spawn_mobs` reads `MobDb`.
  - HP and max HP come from `Hp`.
  - The level is the spawn-line level, else `Level`.
  - Speed comes from `WalkSpeed`.
  - The `0x09fe` / `0x09ff` `is_boss` is 1 for `Class: Boss` and 2 for `Mvp`.
  - This closes the DEFERRED "mob_db stats" item.
- **Stats:** a pure `ro_mechanics::calc_mob_status(&MobStatusInputs, &BattleConfig) -> Status` gives `hit`, `flee`, `flee2` = 0, `cri` = 0, `hard_def`, `soft_def` = `(L + vit)/2`, `res`, `amotion`, `adelay`, `dmotion`, and `battle` primaries (LUK for crit reduction). Catalog `combat.md` §1.1 has the formulas. They use the **db `Level`** (`mobs_level_up` off). In rAthena a spawn-line level changes only the level-penalty difference, which we don't apply (§2.9).
- `MobInfo` stores the `Status`, the damage log, and a `dying` flag.
- `ro_mechanics::Status` gains `adelay` and `dmotion`.
  - Player: `adelay = 2 × amotion`, `dmotion = clamp(800 − AGI × 4, 400, 800)`.
  - Neither field is pushed to the client.

### 2.2 Request (`0x0437` action 0 / 7) and cancel (`0x0118`)
Order, per catalog `auto_attack.md` §1:
1. Self dead → `0x0080` type 1 to the area, return.
2. Stop the current attack (no packet).
3. `now < canact_at` → silent drop. The default `display_skill_fail: 2` sends nothing (`packets/combat.md` `0x0110`).
4. Target must resolve by gid to a live, non-`dying` mob on this map; otherwise silent. An NPC target is a no-op (no `npc_click` yet).
5. Set `target`, `continuous = (action == 7)`, `chase_range = weapon range`.
6. Mid-walk and not within `range − 1` → store a step action (§2.4) and return.
7. `attackable_at > now` → schedule a swing at `attackable_at` and send nothing now (captured: a deferred swing is a bare `0x08c8` at the previous swing + adelay, `auto_attack.md` §8). Otherwise send `0x0088` and run the swing check now (§2.4).

Other notes:
- **What the client sends** (captured, `auto_attack.md` §8):
  - A plain click sends action 0 (one swing, no loop) unless `/noctrl` is on.
  - Ctrl-click, a held click, and `/noctrl` clicks send 7 once; the client never repeats it.
  - Releasing a held click, or re-clicking the current target, sends `0x0118` about 100 ms after a 7. The request must still swing at once; the `0x0118` then ends the loop. A hit already swung still lands.
- `0x0118` (new `ClientIntent::CancelAttack`): stop the attack and clear `continuous`. No reply. A pending step action is left as rAthena leaves it.
- Other cancels: a move request (`0x035f`), any other `0x0437` action (sit/stand already stop walking), a warp and a logout all stop the attack. Pending hits are **not** cancelled (catalog §7.1).

### 2.3 Range and line of sight (catalog `auto_attack.md` §2, `movement_aoi.md` §5)
- **Distance:** `check_distance_client`: `d = floor(max(sqrt(dx² + dy²) − 0.1, 0))`, in range iff `d ≤ range`.
- **`range`:**
  - It is the weapon `Range`, with a minimum of 1.
  - Add +1 when the swing came from a step action.
  - The +1 for a walking target can't happen yet, because mobs don't walk.
- **`battle_check_range`:** same map; Chebyshev < 2 is OK; > 14 fails; otherwise `path_search_long` with `CELL_CHKWALL`. That test is asymmetric, never tests the end cells, and doesn't count cliffs (type 5) as walls.
- **Out of range at swing time:** send `0x0139` to self (range field = weapon range) and end the loop. The target stays set. From the loop, it comes at the due swing time with no `0x0088` (captured).
- **The client chases, not the server** (captured): the client answers `0x0139` with a `0x035f` to a cell next to the target, then sends `0x0437` again while walking (a step action). For a far target it walks first and asks mid-walk, so the server rarely needs `0x0139`.
- **In range but no line of sight:** the loop ends. No player chase, because `OFFICIAL_WALKPATH` is on.
  - Captured: a request that fails line of sight gets exactly one `0x0088` (from §2.2 step 7) and nothing else: no `0x0139`, no swing, no arrow spent.
  - One captured case (`combat.md` §7, lines 4195-4199) failed although our map-less reconstruction says the line is clear. Two more single-`0x0088` requests are likely LOS refusals (proxy #4362 and #4374, `combat.md` §7.9). Recheck all three against the real `prontera` `.gat` in M4.

### 2.4 Attack loop
- **Per-player state** (`AttackState`):
  - `target: Option<Entity>`, `continuous`, `step_attack`;
  - `attackable_at`, `canact_at`, which are kept after the attack stops;
  - the pending step action.
- **Timers:** one per-map min-heap of `(due_ms, seq, CombatTimer)`, with `CombatTimer = Swing(entity) | Hit(id) | Vanish(gid) | StepAction(entity)`.
  - Stale entries are skipped by a generation check, like `floor_expiry`.
  - The heap is popped in `step_tick` after `advance_walkers` and before intents.
- **Swing check** (catalog §3, in order):
  1. Abort silently if the attacker or target is gone, dead or dying, or if `path_search_long` with `CELL_CHKWALL` fails between them at any distance (rAthena's `OFFICIAL_WALKPATH` check, catalog `auto_attack.md` §3 step 2). So a target behind a wall is dropped silently even when it is also out of range.
  2. Run the range test (§2.3).
  3. If this came from the request path: `0x0088` again. This is captured: an immediate first swing is `0x0088` ×2 then `0x08c8` in the same ms, to the attacker and observers. A request that finds the target out of range sends one `0x0088` (step 7 of §2.2), then `0x0139`.
  4. If walking: stop with `0x0088`. A step action that fires mid-walk therefore sends three `0x0088` (captured).
  5. Swing (§2.5–2.6).
  6. Set `attackable_at = due + adelay` and `canact_at = due + amotion + amotion_floor`, where `amotion_floor` is `pc_maxaspd / 2`, 100 ms by default.
  7. If continuous: re-arm at `attackable_at`.
- **Due time:** times are taken from the timer's **due time**, not the tick's `now`, so the 20 ms tick never stretches the interval. A swing that runs immediately from a request uses `now`.
- **Step action** (catalog §7.2): after each completed step of a walking attacker with a pending step action, schedule `StepAction` at `step_end + speed/2`, or `speed × 14/10/2` for a diagonal step. When it fires and the target is still within 14 cells and visible, rerun §2.2 from step 5 with `step_attack` set.

### 2.5 Damage (`ro_mechanics::calc_weapon_attack`, catalog `combat.md`)
The signature is `calc_weapon_attack(&AttackerView, &DefenderView, &BattleConfig, roll: impl FnMut(u32) -> u32) -> DamageResult`. `roll(n)` returns `rnd() % n`. Order:
1. **Lucky dodge.** Only if the defender's `flee2 > 0`; mobs have 0.
2. **Crit.**
   - The attacker must be crit-enabled (`enable_critical`).
   - `cri − 2 × defender LUK`; roll per mille.
   - A crit always hits.
3. **Hit.** `clamp(hit − flee, min_hitrate, max_hitrate)`; roll per cent. A miss gives damage 0, type 0, `dmotion` 0.
4. **Infinite defense.** If the defender has `IgnoreMelee` and the attack is short-range, a landed hit (crit included) does 1. Stop here.
5. **Parts:**
   - status ATK ×2;
   - weapon ATK: the variance roll from item ATK and `wlv`, the refine `atk2`, the overrefine `roll(over) + 1`, and the STR/DEX bonus. For bow, instrument, whip and gun types, DEX replaces STR (§5.3). A crit takes `atkmax`. Bare hands give 0.
   - equip ATK: the ammo `Attack`, flat (§5.3).
   - percent ATK: 0 until item bonuses exist.
   - **Size and element are inputs, both 100 % for now** (user decision, 2026-10-30, made after the capture showed the full table).
     - `AttackerView` carries the weapon's size mod for the target (`size_mod: u16`), applied with truncation to the weapon part only: the roll, overrefine and the weapon damage rate. Not to status ATK or equip/arrow ATK.
     - There is an element rate (`attr_rate: u16`) for each part.
     - The engine passes 100 for both until `size_fix` / `attr_fix` are imported, which is a data-only change later.
     - The real renewal size table differs per weapon type: Dagger S/M/L 100/75/50, 1hSword 75/100/75, and so on (`combat.md` §3). So daggers, swords, axes, spears, maces, bows and katars **deal more than rAthena** against mobs of the wrong size until then. The element difference only affects Ghost mobs for a neutral attack.
6. `× (100 + patk)/100`, then the skill ratio 100.
7. **Res.** `damage −= damage × res/(res + 400) × 0.8` when `res > 0`.
8. **DEF.** `damage × (4000 + def)/(4000 + 10·def) − soft_def`, with `def = −400` clamped to −399. Then `max(damage, 1)`.
9. **Crit multiplier.** `× (1.4 + 0.01 × crate)`, floored.
10. **Result.** `DamageResult { damage, damage2: 0, div: 1, kind: Normal | Crit | Miss | LuckyDodge }`. The wire type is 0, 10, 0 or 11.

Precision follows rAthena's C types, as in `calc_status`.

### 2.6 Ammo (catalog `auto_attack.md` §5.3)
- **Which weapons:** bow, revolver, rifle, gatling, shotgun and grenade launcher check ammo at every swing, before the damage calc.
- **Bow:** no ammo, or ammo that isn't an arrow → `0x013b` type 0. The swing returns `ATK_NONE` and the loop ends with no re-arm and no delay update.
- **Guns:** need Bullet ammo, else `0x0110` with skill 0 and cause 84. `display_skill_fail` doesn't suppress this cause.
- **Consumption:** after the damage calc and before `0x08c8`, one ammo is spent, hit or miss (`arrow_decrement` 1).
  - Order: `0x07fa` (reason 1), then `SP_WEIGHT`.
  - When the last one goes: `0x099a` (the ammo coming off) first, then `0x07fa`, then `SP_WEIGHT`.
  - Then the swing's `0x08c8`. **Captured** (`combat.md` §7): per shot `0x07fa {wire idx, 1, reason 1}`, `SP_WEIGHT`, `0x08c8`. The last arrow sends `0x099a {idx, 0x8000, 0}` first; that shot still fires.
  - The next loop swing sends only `0x013b {0}` and the loop ends. A fresh click with no ammo sends `0x0088` ×2, then `0x013b {0}`.
  - The observer sees only the `0x08c8`.
  - Reuse `PlayerInventory` delete and `wear.rs` unequip.

### 2.7 Swing packet, delayed hit, damage log, HP bar
- **Swing packet:** `0x08c8` to the area around the **target**, attacker included, at swing time.
  - `src_speed` is the attacker's amotion; `dmg_speed` is the target's dmotion (0 on a miss).
  - `damage`, `div` 1, `type`, `damage2` 0.
  - The tick is the low 32 bits of the due time.
- **Pending hit:** the hit is queued at `due + amotion` with the attacker's gid and char id, the target entity and generation, and the distance snapshot `chebyshev(attacker, target) + 14`. With `amotion ≤ 1` it applies at once.
- **When it fires** (catalog §7.1), the hit is dropped silently unless all of these hold:
  - the target is present, alive and not dying;
  - the attacker still resolves on this map;
  - their Chebyshev distance is within the snapshot.

  A miss queues nothing.
- **Apply:**
  1. `hp = hp.saturating_sub(damage)`.
  2. Log `damage` (not clamped: overkill counts) under the attacker's char id. Same char: add. Otherwise append, evicting the oldest when 20 are held.
  3. Send `0x0977` (gid, hp, max_hp) to every log member online, on this map and within 14 cells of the mob (`monster_hp_bars_info`).
  4. If `hp == 0`: death (§2.8).

  Captured: `0x0977` reaches the attacker only, 588-600 ms after the swing. There is none on a miss and none for observers outside the log. A hit swung just before a `0x0118` still lands. A pending hit whose target died first is dropped (step 8: B's third hit).

### 2.8 Death (catalog `mobs.md` §3)
In the hit's tick:
1. Set `dying`, remove the mob from the spatial grid (no packet, no longer targetable), and stop attackers' loops lazily: their next swing check fails.
2. Exp (§2.9) per valid log entry.
3. Schedule `Vanish` at +250 ms (captured 245-273 ms, median about 252; it goes to the killer and the observers). When it fires, send `0x0080` type 1 (`VANISH_DIED`) to the area around the death cell, then despawn the entity, which frees the gid.

No drops, no respawn, no event labels (§5).

### 2.9 Exp and level-up (catalog `experience.md`)
- **Valid entries:** the player is online, on this map and alive. `count` = valid entries.
- **Share:** `per = dmg / total_damage` (as `f64`, clamped to 1). With `count > 1`: `per += per × (min(count, 12) − 1) × 25 / 100`.
- **Amount:** `exp = max(1, trunc(BaseExp × per))` for base, the same for job; a db exp of 0 stays 0.
  - **No level penalty** (user decision, 2026-10-30): full exp whatever the level difference. This is a deliberate deviation. rAthena's renewal `level_penalty.yml` scales it (catalog `db/exp_tables.md`), and the capture shows that (Archer on Pupa 95 %, on Marine Sphere 40 %).
  - Captured in step 8: A 107 and B 29 of 136 damage (the overkill counts) give A 154/54 and B 43/14. Ours gives B 41/14, because rAthena applied B's 105 % penalty rate.
- **`gain_exp`** (`pc_gainexp`, solo):
  - Record which of base and job were non-zero and whether each is at max level.
  - Clamp at max level.
  - Add base and run the base level-up loop. If no level-up, send `0x0acb SP_BASEEXP`.
  - Then the same for job.
  - Then `0x0acc` for base and for job (amount 0 at max level). Each is skipped if its gain was 0.
- **Base level-up loop:**
  - Per level: `exp −= next`; with `multi_level_up` off, clamp the leftover to `next − 1`; add the status- and trait-point deltas; level +1.
  - Then, in this order:
    1. `SP_STATUSPOINT`, `SP_TRAITPOINT`, `SP_BASELEVEL`, `0x0acb SP_BASEEXP`, `SP_NEXTBASEEXP`.
    2. The recalc diff (stat pass).
    3. Heal to full (`SP_HP`, `SP_SP`).
    4. `0x019b` effect 0 to the area.
    5. The `elvmax` check.

    Reuse the `FollowUp` queue: send the tail inline (before the recalc), then queue `HealToMax`, `ToArea(effect)` and `LevelGearCheck`.
  - Mark `DIRTY` and `SKILLS`.
  - **Captured** (`experience.md` "Capture: combat_sequence_1.txt"): the tail first (`SP_TRAITPOINT` is sent even when 0), then the diff, then `SP_HP`, `SP_SP`, then the effect. This is the reverse of `@blvl`, whose tail goes last.
  - rAthena sends `0x080e` (HP bar) after `SP_HP` even when solo. It stays deferred, as it is for `@blvl` (DEFERRED stats).
- **Job level-up:** `SP_JOBLEVEL`, `0x0acb SP_JOBEXP`, `SP_NEXTJOBEXP`, `SP_SKILLPOINT` (the new total), then the recalc diff (often empty, or the job-bonus stat), then `0x019b` effect 1. No heal. Mark `DIRTY` and `SKILLS`. Captured.
- **Both in one kill** (captured twice): the base burst (tail, diff, heal, effect 0), then the job burst (tail, diff, effect 1), then `0x0acc` base, then `0x0acc` job.
  - rAthena sends two diffs. Ours run once at tick end, so the base tail goes inline and **everything after it is queued** in rAthena's order: `HealToMax`, effect 0, the job tail, effect 1, `0x0acc` ×2, `LevelGearCheck`.
  - The job burst's diff therefore merges into the single diff before the heal. This is an accepted deviation of the same kind as DESIGN §5.1's.
  - When there is no base level-up, the job tail goes inline and effect 1 and `0x0acc` are queued.
  - A kill with no level-up sends `0x0acb` ×2 and `0x0acc` ×2 inline.
- **The leftover clamp is captured:** the leftover job exp was the consumed bar − 1.
- **Real NEXT values:** `SP_NEXTBASEEXP` / `SP_NEXTJOBEXP` everywhere, at login, after `@blvl`, `@jlvl` and `@job`, and here. They come from `JobDb`. Drop the placeholders in `status.rs`.

### 2.10 Persistence
- No new columns: `base_exp` and `job_exp` are already saved.
- Like rAthena, a kill or level-up doesn't save at once. It is saved at the next autosave and on logout or map change.
- The play-state revision must change on an exp, level or point change, so the autosave picks it up. M6 checks the current trigger.

## 3. Scenarios
Numbering is final once the capture is in. Each test names its ID.

| ID | Scenario |
| :--- | :--- |
| DB-CMB-01 | Every committed `mob/*.yml` and new `job/*.yml` file loads and has the GPL header and attribution. |
| DB-CMB-02 | Poring 1002: Hp 55, BaseExp 150, JobExp 40, Defense 2, AttackMotion 672, DamageMotion 480, AttackDelay 1872, not IgnoreMelee. Green Plant 1080: IgnoreMelee. |
| DB-CMB-03 | `next_base_exp(Novice, 1)` = 548, `(Novice, 99)` = 99_999_999; `next_job_exp` for Novice at 10 is the max constant. |
| MECH-CMB-01 | `calc_mob_status(Poring)`: hit 157, flee 102, def2 1, dmotion 480, adelay 1872. |
| MECH-CMB-02 | The worked skeleton (catalog `combat.md` §5) with fixed rolls: a miss, a hit, a crit (atkmax, ×1.4), DEF reduction and min 1, the infinite-defense 1. |
| MECH-CMB-03 | Captured vectors (`combat.md` §7 table): Knife ATK1 1/2 vs Poring 12/14; ATK1 15 STR 11 vs Lunatic 42..43; ATK1 48/49 vs Pupa 104..105/106..107, crit 149; Bow + Arrow ATK1 53 DEX 15 vs Pupa 135..136, ATK1 20 → 72..73, ATK1 10 DEX 5 → 52..54; vs Marine Sphere 16..18. Every captured value falls in the computed range. The Dagger-vs-Medium (Poring) vectors are checked with `size_mod` 75 passed in, the rest with 100. A second test pins that the engine's default (100) gives 16..17 for the ATK1 1 Knife vs Poring case: the known deviation. |
| MECH-CMB-04 | Ranged: DEX/STR swap for bow, whip and instrument; arrow ATK 25 into equip ATK, not varied. |
| MECH-CMB-05 | `check_distance_client` table: range 1 hits (1,0), (1,1), (2,0); misses (2,1). `path_search_long` on a hand-written grid: a wall blocks, a cliff doesn't, asymmetric rounding, end cells untested. |
| MECH-CMB-06 | Exp share: one attacker gets the whole share; two attackers at 70/30 get (0.7, 0.3) × 1.25; the cap at 12; the min 1. |
| PKT-CMB-01 | Encoders: `0x08c8` (all fields), `0x0977`, `0x0139`, `0x0acc`, `0x0110`, `0x07fa` reason 1; decoder `0x0118` → `CancelAttack`. Table lengths. |
| ENG-CMB-01 | Click (action 7) on an adjacent Poring: `0x0088` ×2 then `0x08c8` to attacker and observer; HP lands at swing + amotion; `0x0977` to the attacker; the next swing at + adelay; no drift over 10 swings at a 20 ms tick. |
| ENG-CMB-02 | Action 0 swings once. A re-request before `canact_at` is dropped silently and stops the loop. |
| ENG-CMB-03 | Out of range → `0x0139` once, loop ends. Range 1 straight at 2 cells hits; (2,1) doesn't. A wall between at 5 cells with a bow → the loop ends, no `0x0139`. |
| ENG-CMB-04 | A move request, `0x0118`, sit, warp and logout each stop the loop. A hit already pending still applies, except after logout or warp (skip rules). |
| ENG-CMB-05 | Step action: a request while walking out of `range − 1` attacks half a step after the next step ends, with range + 1. |
| ENG-CMB-06 | Bow with 2 arrows: the swings spend them in the captured packet order; the second takes the arrows off; the third swing sends `0x013b` 0 and stops. Gun without bullets: `0x0110` cause 84. |
| ENG-CMB-07 | Kill: the killing hit sends `0x0977` hp 0, then the exp packets in the same tick; the mob leaves the grid at once (a new attack on it is silent); `0x0080` type 1 at +250 ms to the area; the gid is free after. Overkill is logged. |
| ENG-CMB-08 | Exp: two attackers on one Poring, shares and the attacker bonus; one offline at death gets nothing and its share isn't redistributed; full exp at a level difference of −31 (no penalty). |
| ENG-CMB-09 | Base level-up from a kill: the captured order; HP/SP full; `0x019b` 0 to the area; points granted; `SKILLS` re-check. Job level-up: order, +1 skill point, no heal. Both in one kill. Max job level: `0x0acc` amount 0. |
| ENG-CMB-10 | Real `SP_NEXTBASEEXP` / `SP_NEXTJOBEXP` in the login burst, `@blvl`, `@jlvl` and `@job` (existing tests updated). |
| BIN-CMB-01 | Over TCP: log in, attack an adjacent mob until death, and receive `0x08c8`, `0x0977`, the exp packets and `0x0080` type 1. |
| PER-CMB-01 | Exp and levels from kills survive a logout and relog. |

## 4. Licensing
- The mob and exp data come in only through `tools/mob-import` and `tools/job-import` into `crates/ro-db/data/`, with headers, `NOTICE.md` and load tests. They are read from disk at runtime.
- Formulas and constants (the hit and crit clamps, 4000/10, ×1.4, the +250 ms vanish, the cap of 20, the 25 % attacker bonus) are facts written from the catalog. Code is clean-room.
- The new capture goes into `reference/catalog/captures/` with rAthena message text and PINs redacted.

## 5. Deferred (move into `DEFERRED.md` at Finish)
- **Mob AI (separate feature)**, for the reason given in the scope section:
  - retaliation, aggro scan and view range;
  - random walk (`MIN_RANDOMWALKTIME`), chase with `unit_walktobl`;
  - hard and lazy AI timers;
  - mob → player hits on this feature's core, with the square range and the 1000 ms delay cap for non-player attackers;
  - player HP pushes, player death (`0x0080` type 1 for a player, the death exp penalty), `CZ_RESTART` respawn.
  - The flinch walk-stop of a damaged mob comes with it.
- **Respawn** (`delay1` / `delay2` rules, 1000 ms floor, `0x09fe` to the area), and **event labels** run at death.
- **Drops** at death (`drop_mob_items` exists): owners from the damage log, `first_attack_loot_bonus`, after the exp packets.
- **The size table** (`size_fix`: the base `db/size_fix.yml` merged with the renewal overlay, `combat.md` §3) and **the element table** (`attr_fix`, plus weapon and ammo element): both are 100 % today.
  - The capture shows the size table matters, e.g. a Knife hits a Poring for 12 in rAthena and 16-17 here.
  - Both need an import tool (rAthena db YAML), then the engine feeds `size_mod` / `attr_rate` from them. No formula change.
- Dual wield and katar `damage2`, Double Attack and other passive procs, masteries, cards and item `bonus` rates, `CombatBonus` tables, status-effect modifiers.
- **Not planned** (user decision): the renewal level-difference exp penalty.
- MVP (`MvpExp`, `0x010c`/`0x010b`, MVP drops), party exp share and `0x0abd`/`0x080e`, KS protection, `pk_mode`, VIP, Rich Mankind and other exp bonuses, `max_exp_gain_rate`, `@showexp`.
- Exit refusal within `prevent_logout` of attacking (the exit-game DEFERRED item), and the delayed removal on close.
- Attacking NPCs (`npc_click`), hidden or cloaked targets, attack-blocking statuses, `pc_cant_act` states.
- Gun ammo subtypes beyond Bullet (the Shell quirk), Vulture's Eye and other range skills.
