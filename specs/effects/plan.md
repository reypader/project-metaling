# Implementation Plan: Effects foundation (skills and item effects, Rust hooks, Lua spike)

Spec: `specs/effects/spec.md`. Design: `crates/ro-scripting/DESIGN.md` §1-§5, `crates/ro-engine/DESIGN.md` §5.1 "Casting", §9.9, §11. Planned on 2026-10-31; M0 finished 2026-11-01. **Start every session by reading this whole file**: the notes carry context that isn't in the code.

## Decisions (settled with the user, 2026-10-31)
| Topic | Decision |
| :--- | :--- |
| Goal | Game rules move toward scripts (Lua later), so the game can fork from RO mechanics toward horizontal progression. NPCs are out of scope, but the interface is the one NPC and monster scripts will share (catalog `scripting/units.md`). |
| Model | **Composable effects.** A definition = engine-owned effect blocks with per-level numbers + an optional named hook. A hook reads a context and **returns effects**. It never mutates the world. |
| Definitions | **Rust code now** (`Definitions::builtin`). When Lua lands, they move into Lua tables/files. No new YAML format. |
| Lua | Designed for, not shipped. **M9 = a feature-gated `mlua` spike** (one skill from a `.lua` file + a benchmark). Hot reload: later. |
| Skills | First Aid (142), Bash (5) without stun, Mammonite (**42**), Heal (28) heal-only. |
| Items | Potion use for whole-script `itemheal`/`percentheal` items (288 in the data). |
| Statuses | Next plan. Nothing here starts or ends a status. |
| Capture | `scripting_1.txt`, recorded by the user 2026-10-31, written up in M0. Supplement `scripting_2.txt` (2026-11-01), written up in session B as catalog `scripting/skills.md` §15. |
| Deferred list | Everything postponed is in `DEFERRED.md` "effects" (user request). |
| Sources (2026-11-01) | `CastSource = Learned \| ItemSkill(ItemId) \| Mob \| ItemUse(ItemId)`. Potions are `ItemUse`; `ItemSkill`/`Mob` are rejected (`Unsupported`). |
| Undead Heal (2026-11-01) | A declarative target filter in `SkillDef`, checked at the `0x0438` request: silent, nothing sent or spent. |
| Cast-time capture (2026-11-01) | The user records one (steps written in session A after M1) to pin `cast_time` rounding. MECH-EFF-03 tests the catalog §4.1 worked examples until then. |
| *Planning defaults (my calls; the user may revisit)* | Item heal values come through `tools/item-import` (licensing). Effects apply inline at castend, like combat hits. Zeny plumbing and `@heal`/`@zeny` are added (Mammonite needs zeny; the manual check needs `@heal` because nothing damages players). Heal on Undead is dropped silently. |

## Notes for the implementer

### Captures and catalog
- **`scripting_1.txt`** (2026-10-31; file line = proxy `#` + 10): A = `skilltest2` map `#7` (AID 2000000), B = `skilltest1` map `#4` (AID 2000001, observer), **not partied** (both GMs, so B gets rAthena's `0x080e` HP meter, which we don't send). Write-ups:
  - `scripting/skills.md` §14: per-skill packet orders and fields, damage fits, the spam/`canact` evidence, step 3/4 analysis.
  - `mechanics/inventory.md` §7.5: potions, `@item` replies.
  - `mechanics/atcommands.md` "Capture: scripting_1.txt": zeny, `@heal`, `@revive`, `@vit`, `@questskill`.
  - `mechanics/stats.md`: regen as `0x00b0` every 6 s HP / 8 s SP (not `0x0a27`).
- **Encode tests can use real frames:** `#1669` `0x0b1a` First Aid, `#1984` `0x01de` Bash 237, `#5824` `0x00b1` 4000, `#6710` `0x0983`, `#6711` `0x09cb` 64, the `0x01c8` frames in step 8.
- **Things the capture taught that are easy to miss:**
  - `0x0b1a` goes out even with cast time 0, x = y = 0, Heal's element is **6**.
  - `0x0983` AID is the **caster** even when healing B, and only Heal sends it (delay > 0).
  - `0x09cb` carries the **unclamped** heal; at full HP no HP push follows.
  - SP is checked before zeny (cause 1 wins). Castend does **not** re-check SP or zeny.
  - A too-early cast is **silent** (no cause 4). A cast stops the auto-attack loop with no packet, and the client doesn't restart it.
  - The client walks into range itself and sends `0x0438` mid-walk; the server fires it at the step end (step action). Without a walk, out of range is a silent drop.
  - A miss is `0x01de` damage 0, ddelay 0, type 8, SP/zeny still spent, no `0x0977`.
  - The `0x01c8` success ack goes to the observer too. Potions at full HP are consumed with only the ack + `SP_WEIGHT`.
  - The capture's Knife hits use the size table (Dagger vs Medium 75 %), which metaling still keeps at 100 % (combat deviation). Use the **unarmed** rows for exact damage tests: Bash lv 1 = 11, Mammonite lv 10 = 46 vs a Poring at ATK1 5/4.
- `PACKET_FINDINGS.md` PF-21: `0x0111`, `0x0117`, `0x01b9`, `0x043d`, `0x08e2` and `0x0a27` rows were added in M0 (`82f8d4d` and the follow-up proxy commit).

### Existing code anchors (as of `82f8d4d`)
- **Combat:** `crates/ro-engine/src/combat.rs`.
  - `AttackState` at `:82` holds both `attackable_at` and `canact_at`, already separate like rAthena.
  - `request_attack` at `:199` (its `canact` gate is at `:213`).
  - `swing_check` at `:325` (gate updates at `:383-384`), `swing` at `:400`, `apply_hit` at `:528`.
  - `arm_step_action` at `:639`, `run_combat_timers` at `:683`.
  - `PendingHit` at `:102`, `CombatTimer` at `:111`.
- **Damage:** `ro_mechanics::calc_weapon_attack` (`crates/ro-mechanics/src/combat.rs:204`), with `AttackerView` at `:66` and `DefenderView` at `:125`. `BattleConfig` is at `crates/ro-mechanics/src/config.rs:21`; add `item_use_interval` there.
- **Skills:** `SkillSet` is in `crates/ro-engine/src/skills.rs`. `level()` at `:126` counts hidden slots, so check `visible` (`:61`).
- **Registries:** `crates/ro-engine/src/engine.rs:136`. Landed as `scripts: SkillScripts` (session B), already built by the map-server. The wire gid of an entity is `gid(e)` (`engine.rs:240`).
- **GM commands:** `crates/ro-engine/src/command.rs:70-110` dispatch (`@item`, `@blvl`, `@job`, `@allskill`, …). `@heal` and `@zeny` don't exist yet.
- **Events:** `ro_core::ServerEvent` (`crates/ro-core/src/message.rs`).
  - Existing: `ParameterChanged` (`0x00b0`), `MobHpBar` (`0x0977`), `SkillFail` (`0x0110`), `LongLongParameterChanged`.
  - Missing: `0x00b1`, `0x0b1a`, `0x01de`, `0x09cb`, `0x0983`, `0x01c8`.
- **Persistence:** the zeny column is `crates/ro-persistence/sql/01_schema.sql:34` (`zeny INT NOT NULL DEFAULT 0`). Nothing reads it.
- **Importers:** skill data is `tools/skill-import` → `crates/ro-db/src/skill.rs` (`SkillData` `:74`, `sp()`/`range()` accessors). Item data is `tools/item-import` → `crates/ro-db/src/item.rs`.

### Pitfalls
- **`AttackContext::NORMAL` must not change the roll order.** Every MECH-CMB / ENG-CMB test must pass untouched. Skills draw fewer rolls (no lucky dodge, no crit).
- **Swing vs cast in one tick:** combat timers run before intents (DESIGN §5.1). So a cast in the tick of a due swing sees the swing's raised `canact_at` and is dropped. That matches the capture (9 drops). Don't "fix" it.
- **`canact_at` arithmetic:** cast begin `max(old, T + amotion + 100, T + max(cast, 100)) + 100`, castend `max(T + delay, canact − 100)`. The net with no cast time and no delay is `T + amotion + 100`. Heal gets `max(T + 500, T + amotion + 100)`.
- **Requirements:** check SP, then zeny, then weapon at cast begin. Re-check only HP cost, weapon and items at castend. Consume SP before zeny.
- **Hidden skills:** use `visible` slots. Level 0 is a silent drop. Clamp the requested level to the learned one.
- **Heals don't mark dirty** (vitals only). Push `SP_HP` to the **target's** session, not the caster's.
- **Item use:** delete without `0x07fa`, but push `SP_WEIGHT`. The failure ack is SELF with amount 0, the success ack AREA with the remaining amount.
- **Determinism:** reuse one `EffectList` buffer per world, dispatch by `HookId`, keep `UnitView` `Copy`. Document the roll order in the tests.
- **Licensing:** no rAthena number in Rust or Lua definitions except formula facts. Costs, delays and potion values come from `crates/ro-db/data/` at runtime.
- **Lua build (M9):** `verify.sh` doesn't pass `--features`, so run `cargo clippy -p ro-scripting --features lua --all-targets` and `cargo test -p ro-scripting --features lua` by hand. `mlua` vendored needs a C compiler. Keep `LuaHooks` out of `Registries` (it isn't `Send` without mlua's `send`). The `lua` feature must not be default.

## Session A results (M1-M3, 2026-11-01)
Uncommitted, per the commit-once rule. `./verify.sh -p ro-db -p skill-import -p item-import -p ro-mechanics -p ro-engine -p ro-scripting -p map-server` is green (586 tests). The inline `SKILL_DATA` headers in the engine and map-server tests were bumped to version 2.

**What landed:**
- **M1 (skill data):** `SKILL_DATA` v2 and regenerated data. `SkillData` gains these per-level lists, each with an accessor by level: `cast_time`, `fixed_cast_time`, `after_cast_act_delay`, `after_cast_walk_delay`, `cooldown`, `hit_count`, `element` (`SkillElement`), `hp_cost`, `zeny_cost`. Plus `damage_flags`, `cast_time_flags`, `weapons` (`Vec<WeaponType>`, empty = any; `allows_weapon`), `item_cost` (`SkillItemCost`, aegis name; `item_cost(lv)` applies the level-dependent rule) and `state` (`Option<String>`). Tests: DB-EFF-01 and `casting_fields_are_imported`.
- **M1 (item data):** `ITEM_DATA` v3. `ItemData.on_use: Option<ro_db::ItemUse>` (YAML key `Use`). 288 lifted (212/76). Tests: DB-EFF-02, DB-EFF-03, and `tool_item_05` checks the real counts.
- **M2 (mechanics):**
  - `AttackContext { ratio, hit_pct, can_crit, can_lucky_dodge }`, with `NORMAL` and `skill(ratio, hit_pct)`, is the 3rd argument of `calc_weapon_attack`.
  - `cast_time(CastTimeData, Caster, &BattleConfig)` and `delayfix(delay, amotion)`.
  - `BattleConfig` gains `vcast_stat_scale` 530, `default_fixed_castrate` 20 and `item_use_interval` 100, also added to `config/battle.yml`.
  - Tests: MECH-EFF-02/03 in `ro-mechanics/tests/effects_tests.rs`. MECH-EFF-01 is the existing combat vectors passing `NORMAL`.
- **M3 (`ro-scripting`):**
  - `Effect`, `EffectList` (a SmallVec newtype that derefs to `[Effect]`), `EffectSpec`, `LevelValue`, `Who`, `HealShow::{Skill, Silent}`.
  - `UnitView`, `CastSource`, `SkillLevels`, `Roll`, `HookCtx`.
  - `HookBackend`, `HookId`, `HookError`, `RustHooks`.
  - `SkillDef { id, effects, hook, target: TargetRule }`, `ItemUse::{ItemHeal, PercentHeal}`, `ItemUseDef`, `Definitions`, `MissingHook`.
  - Tests: SCR-EFF-01..05 in `ro-scripting/tests/effects_tests.rs`.

**Rulings (my calls; each says what it costs if wrong):**
1. **FixedCastTime absent = 0, not −1** (explorer finding). Spec §0.1 and MECH-EFF-03 were corrected. If wrong, only the one NPC skill changes.
2. **Element data keeps `Weapon` 12.** `0x0b1a` sends the resolved attack element, Neutral until weapon elements exist (spec §2.3 note). If wrong, the M6 element field changes.
3. **`calc_weapon_attack` gained a parameter.** The combat tests pass `&AttackContext::NORMAL` and keep their expected values unchanged. The engine call site in `combat.rs` passes `NORMAL`.
4. **No mastery ATK exists yet.** The ratio applies after P.ATK, where mastery would also sit. Sword Mastery rows in the capture are therefore not reproducible (the spec already only asks for the unarmed rows).
5. **`Element` per-level lists:** the listed levels are set, gaps stay Neutral, and levels above the highest listed one repeat it. This is not the numeric series rule. It affects 4 skills, none in the slice.
6. **`Requires.Weapon`** is `Weapons: [..]` with our `WeaponType` names. The unused dual-wield names are dropped with a warning.
7. **`ItemCost` keeps aegis names.** M6 resolves them with `ItemDb::by_aegis_name`. No slice skill has one.
8. **Lifting is restricted to `Healing`/`Usable`/`Cash` items.** Equipment scripts are bonuses.
9. **A constant `itemheal` side draws no roll.** Lua must match this (M9 equality test).
10. **Heal amounts are `u32`.** A negative `LevelValue` heals 0.
11. **`resolve` clears the whole list on any error**, including the `Spend*` effects the engine pushed first. **M6/M7 must resolve completely before applying anything.** At castend: push the `Spend*`, call `def.resolve`, and only then apply in order, sending `0x0983` after the spends and before the first non-spend effect. Then a hook error sends no packet (spec §2.1).
12. **`Definitions::builtin(&dyn HookBackend)` binds hook names to one backend's ids.** `Registries.effects` and `Registries.hooks` must come from the same backend.
13. **`TargetRule { reject_undead }` lives on `SkillDef`.** The engine checks it at the `0x0438` request against the mob's `ro_db::Element::Undead`.
14. **`UnitView.matk_base`** is filled by the engine as `status.status_matk + 5 * spl` (`Status::status_matk` has no SPL term; check `crates/ro-mechanics/src/status.rs:341`). Weapon overrefine is left out of Heal's MATK range (deferred).
15. **Potion deletion is engine plumbing, not an `Effect`.** The order is ack → delete → heal, from the used slot. `Effect::ConsumeItem { item, amount }` is for skill item costs. An item-use `HookCtx` has `skill` 0, `level` 0, `source: ItemUse(item id)` and `target` = the caster.
16. **`WeaponDamage.div` comes from the definition (1).** `SkillData.hit_count` is imported but not read yet.

**The user's cast-time capture** (they chose to record one; until then MECH-EFF-03 uses the catalog examples). Float vs double `sqrt` never changes the result for DEX/INT ≤ 130, so this checks the formula end to end, plus the cast-time packet flow for ENG-EFF-12. Steps:
1. Run `./run_rathena_servers.sh` and connect the client through `tools/proxy`, with an observer if possible.
2. Set up a Mage: `@job 2`, `@blevel 60` (relative), `@allskill`. Leave the stats at 1, then go to `prt_fild08`.
3. Cast Fire Bolt (19) on a Poring at lv 1, 5 and 10, at each DEX/INT pair below. `@dex`/`@int` are relative (`@dex 9` takes DEX 1 to 10). `@heal` between casts.
4. Expected `0x0b1a` delay, from `cast_time` (lv 1 / 5 / 10 = cast 500/1700/3200 + fixed 300/700/1200):

   | DEX / INT | lv 1 | lv 5 | lv 10 |
   | :--- | :--- | :--- | :--- |
   | 1 / 1 | 762 | 2272 | 4159 |
   | 10 / 1 | 700 | 2061 | 3763 |
   | 30 / 20 | 605 | 1739 | 3156 |
   | 50 / 50 | 534 | 1495 | 2697 |
   | 99 / 99 | 425 | 1127 | 2004 |
5. The capture also shows, for ENG-EFF-12: whether SP is spent at castend (after the delay), and where `0x0983` POSTDELAY (1400) lands relative to the damage packet. Add the frames to MECH-EFF-03 and to ENG-EFF-12's expectations when it arrives.

## Session B results (M4-M6 + capture 2, 2026-11-01)
Uncommitted, per the commit-once rule. The early `reviewer` ran over M3 + M6; its fixes are applied (below).

**Capture `scripting_2.txt`** (user, 2026-11-01): redacted, written up as catalog `scripting/skills.md` §15, README/INDEX rows. Spec updated: no `0x00b1` at map entry (zeny rides in the char list `money` @12); wrong weapon = cause 6 at the request; attack skill on a player = `0x0b1a` then cause 0 at castend; `0x0b1a` element 0 for `Weapon`/`Endowed`/`Random` (ruling 2 replaced: no attack-element lookup ever); `0x0088` before a cast while walking. New Heal vectors (1342/289) in SCR-EFF-03. It also shows the `itemskill` flow (Fire Converter: `0x01c8` with the amount unchanged, `0x0147`, the client's own `0x0438`, then `0x07fa`), for the plan that does `ItemSkill`.

**What landed:**
- **M4:** `ClientIntent::UseSkill { level, skill_id, target }`, `UseItem { index }`; `ServerEvent::{SkillCasting (0x0b1a), SkillDamage (0x01de), SkillNoDamage (0x09cb), ItemUseAck (0x01c8), LongParameterChanged (0x00b1)}` (0x0983 is the existing `StatusIconStarted`). Encoders in `ro-packets/src/outgoing/skill.rs`. PKT-EFF-01/02 in `ro-packets/tests/effects_codec_tests.rs` (frames by file line of `scripting_1.txt`; the catalog's `#N` there are off by 10 in places, so the test cites file lines).
- **M5:** `CharacterStatus.zeny` and `CharacterSelectStats.zeny`; `ro_core::MAX_ZENY` = 1e9; load/save with the play state; the char list's `money`. `@zeny`, `@heal` in `ro-engine/src/vitals.rs` (with `heal_player`, `spend_sp/hp`, `set_zeny`). PER-EFF-01 (repo + char codec), ENG-EFF-16.
- **M6:** `ro-engine/src/cast.rs`. `Registries.scripts: SkillScripts` (**not** `effects`/`hooks` as planned: one type holds both, so the hook ids always match the backend; `SkillScripts::builtin()` / `with_backend(Arc<dyn HookBackend + Send + Sync>)`; `Default` = nothing castable). The map-server already wires `SkillScripts::builtin()?` (`bins/map-server/src/service.rs`), so M8 only adds the TCP test. `combat.rs`: `AttackState` gained `cast`, `cast_gen`, `step_skill`; `CombatTimer::CastEnd`; shared `attacker_view` + `queue_hit` (swings and weapon skills); `set_step_skill`, `clear_step_action`. ENG-EFF-01..12, 17 in `ro-engine/tests/effects_tests.rs`.

**Rulings and fixes (session B):**
17. **Request order** (source-checked after review): silent checks → target → **requirements** (weight ≥ 90 % cause 9, HP 2, weapon 6, SP 1, zeny 5; item-cost skills dropped) → clear any step action → step store or range → cast begin. Castend re-check: target (silent), HP cause 2, weapon cause 6.
18. **Cast costs** push HP before SP (`status_zap`), then zeny.
19. A failed castend (attack skill on a player) keeps cast begin's `canact = T + amotion + 200`.
20. **Test fixtures:** a first job with fewer than 9 skill points spent sees only the Novice tree, so give `NV_BASIC 9` when the slice skill alone is below 9 levels. A click walk is limited (20 cells fails), keep test walks around 10 cells.

**For session C (M7/M8):**
- `SkillScripts::get()`, `MapRoll` and `MapInstance::unit_view` are `pub(crate)` in `cast.rs` for `item_use.rs`. `heal_player` (vitals) pushes `SP_HP` then `SP_SP` only when changed, which is the potion order after the ack and `SP_WEIGHT`.
- `ro_db::ItemUse` and `ro_scripting::ItemUse` share a name: alias one. `HookCtx.args` is `[i32; 4]` and `ItemHeal` ranges are `u32`: convert with `try_from` and fail the use (warn) on overflow.
- Reuse `self.effect_buf` (take, clear, resolve, apply, put back), as `cast_end` does.
- Potion use must not stop a walk or the attack loop, and works while sitting (spec §2.7; user-confirmed from gameplay). Unlike a cast, don't call `stop_walking`/`stop_attack`.
- Optional captures offered to the user for M7 (not recorded yet): a level-restricted potion's failure ack (High Red Potion 11621 below base level 60), percent heals (607/608) with both HP and SP down (only the HP-then-SP push order is still open: the user confirmed the rate is of the max), the last potion of a stack. If none arrive, follow the source (catalog `mechanics/inventory.md` §7.2-§7.3).
- Disk: on 2026-11-01 the disk filled up (`target/` 52 GB); `target/debug/incremental` was cleared. If `verify.sh` fails with "No space left on device", that's the cause.

## Session C results (M7-M8, 2026-11-02)
Uncommitted, per the commit-once rule. The full `./verify.sh` is green (925 tests). No new capture arrived for potions; nothing needed one (the source path in catalog `mechanics/inventory.md` §7.2-§7.4 plus `scripting_1.txt` §7.5 covered every scenario).

**What landed:**
- **M7:** `crates/ro-engine/src/item_use.rs`: `MapInstance::use_item` (dispatched from `ClientIntent::UseItem`).
  - Gates, in order: dead (silent), slot (silent), then the failure `0x01c8` SELF for a type other than Healing/Usable/Cash, `now < canuseitem_at`, the user restrictions, and no lifted `Use`.
  - Then resolve (through `Definitions::item_use` + the hooks, `CastSource::ItemUse(id)`, skill/level 0, target = caster, the reused `effect_buf`). Then `canuseitem_at`, the AREA ack with the remaining amount, `consume_one` (`SP_WEIGHT`, no `0x07fa`), then `apply_effect` for each effect.
  - `PlayerInfo.canuseitem_at` (new field). `equipment::usable_by` (gender, level, job, class) shares `level_allows`/`gender_allows` with `check_equip`. `cast.rs` `apply_effect` is now `pub(crate)`.
  - Tests: ENG-EFF-13 (two tests: reduced HP; walking, sitting and the attack loop are not stopped), ENG-EFF-14, ENG-EFF-15 in `ro-engine/tests/effects_tests.rs`, with our own hand-written potion items (ids 9001-9006).
- **M8:** the map-server already wired `SkillScripts::builtin()` (session B). `bins/map-server/tests/effects_tests.rs` BIN-EFF-01: a base-50 Swordman at 10 HP casts First Aid, then Bash 10 on a shipped Poring, then drinks a Red Potion; the caster and an observer get the §2 orders. Note: `Client::collect(Duration::ZERO)` never reads the socket, so the observer reads with its own 100 ms window.

**Rulings (session C; each says what it costs if wrong):**
21. **A hook error (or no scripts loaded) on an item use sends the failure `0x01c8`** instead of nothing. rAthena always answers a refused use, "or the client gets stuck" (catalog §7). Spec §2.1 updated. Cost if wrong: one SELF packet on a data error.
22. **Rental items** (`expire_time != 0`): the failure-form ack, the item is kept, the effects still apply (source, catalog §7). Nothing creates rentals yet. Cost if wrong: unreachable today.
23. **A dead player's use is silent.** rAthena sends a dead-state refresh with no ack; player death doesn't exist yet. Deferred with player death.
24. **`canuseitem_at` lives on `PlayerInfo`**, which a map change rebuilds, so the 100 ms gate resets on a warp. Cost if wrong: one extra use within 100 ms across a map change.
25. **Gate order** among the failure-ack gates (type, interval, restrictions, `Use`) isn't observable: they all send the same ack. The `Use` lift goes last, as the deviation.

**For session D (M9 + Finish):**
- Ruling 9 (a constant `itemheal` side draws no roll) still applies to the Lua equality test.
- The manual client check list under Finish already includes potions. Optional potion captures (level-restricted failure ack, percent heals with both HP and SP down, last potion) were never recorded; the engine follows the source for them.

## Session D results (M9 + Finish, 2026-11-03)
No potion capture arrived, and none was needed (session C already covered potions from the source).

**What landed (M9):**
- `mlua` 0.12 (`lua54`, `vendored`) is a workspace dependency, optional in `ro-scripting` behind `lua = ["dep:mlua"]`. Its dependency tree is MIT/Apache.
- `crates/ro-scripting/src/lua.rs` (`#[cfg(feature = "lua")]`, re-exported at the root): `LuaHooks` (`new`, `load_file`, `load_source`, `set_ctx_mode`, `HookBackend`), `CtxMode::{Table, Userdata}`, `LuaLoadError`.
  - A script returns `{ skills = { { id, effects, hook, reject_undead } }, hooks = { name = function(ctx) } }`. A hook returns a list of effect tables.
  - `ctx` has `skill`, `level`, `source`, `item`, `args`, `caster`/`target` fields, plus `ctx:roll(n)` and `ctx:skill_level(id)`. Both live in a `Lua::scope` for the call.
  - A failed load keeps nothing. Hook ids are sorted by name per file.
- `crates/ro-scripting/lua/sm_bash.lua` (Bash as specs and as the `sm_bash` hook) and `lua/item_heal.lua`. Both are MIT and read from disk.
- SCR-LUA-01 in `crates/ro-scripting/tests/lua_tests.rs` (5 tests). Mutating the Lua (Bash ratio, or rolling on a constant side) fails them.
- `examples/hook_bench.rs` (`required-features = ["lua"]`). Results are in catalog `scripting/lua_notes.md` §3.1: bare call ~35 ns, `LuaHooks` ~2.5 µs (table ctx) / ~6.5 µs (scoped userdata), Rust 4-20 ns. `ro-scripting/DESIGN.md` §5 has the production advice.

**Finish (docs):** spec §5 moved into `DEFERRED.md` "effects" (plus "still to verify", the deviations, the casting gaps, the Lua findings). DESIGN status lines updated (`ro-scripting` header and §5, `ro-engine` "Casting", §11 notes, `AttackContext`).

**Rulings (session D; each says what it costs if wrong):**
26. **`sm_bash.lua` holds Bash twice**: the declarative definition (equal to the Rust one) and a `sm_bash` hook that builds the same effect. The spec asks for "effect specs plus a hook function that returns the same effects", and both together would double the effect, so the test checks each form on its own. Cost if wrong: a test reshuffle.
27. **`lua/item_heal.lua` was added** (not in the spec), so "same seed" actually exercises rolls (Bash draws none) and ruling 9. Cost if wrong: one extra MIT file.
28. **Lua hook results are strict.** A non-integer or out-of-range number, or a negative heal, is a hook error, unlike `LevelValue`'s "negative heals 0". Cost if wrong: a spike-only divergence.
29. **`mlua` isn't in the README**: it has no dependency list, and Acknowledgements credit reference repos and data sources. Cost if wrong: one line.
30. **`./verify.sh --smoke` couldn't run at first:** rAthena's docker containers held port 6900, and they weren't stopped without asking. After the commit, the user stopped them and the smoke run passed (2026-11-05, on `f206617`).

**Final `reviewer` run (whole diff): no Critical. Fixes applied:**
- **Important:** a loaded stack of amount 0 underflowed `amount - 1` in `use_item` (a debug panic on the map actor; in release, a free heal every 100 ms). It is now silent like an empty slot (catalog §7.2). `consume_one` returns false on a failed removal, and then no effect applies. Test: `eng_eff_15_zero_amount_stack_is_silent` (RED: overflow panic).
- Loaded zeny is capped at `MAX_ZENY` (play state and char list). Test: `per_eff_01_loaded_zeny_is_capped`.
- `LuaHooks::call` truncates `out` back on an error, like a Rust hook (`EffectList::truncate` added). `LuaHooks` doc warns it isn't sandboxed.
- Tests: the entry-sequence filter in `settings_tests.rs` now includes `LongParameterChanged`, which pins PER-EFF-01's "no `0x00b1` at entry". Removed dead setup in ENG-EFF-04. ENG-EFF-15 covers the gender, job and class gates (items 9007-9009).
- Not changed: BIN-EFF-01 wall-clock windows (the same pattern as the other bin tests).

## Session split (recommended)
The M0 planning session is too large to continue. Start fresh:
- **A:** M1-M3, the leaf crates: data, mechanics, `ro-scripting`. Low risk; `./verify.sh -p` per crate.
- **B:** M4-M6, plus the early `reviewer` run on M3 + M6. This covers the crate API and the shared map-server state.
- **C:** M7-M8.
- **D:** M9 + Finish.

Each session reads this file, the spec, and the catalog sections the milestone names. Record "Session X results" here before ending (the combat plan has examples).

## Milestones

### M0: catalog, capture, spec, plan (done 2026-11-01)
- [x] Explorer write-ups: catalog `scripting/skills.md`, `scripting/items.md`, `scripting/units.md`, `scripting/lua_notes.md`, `scripting/rust-ro-script-architecture.md`, `mechanics/inventory.md` §7.1-§7.5, and the atcommands/stats capture notes. `INDEX.md` rows added.
- [x] Proxy rows (`82f8d4d`, plus `0x0111`/`0x08e2` in the follow-up proxy commit). Capture `scripting_1.txt` redacted into `reference/catalog/captures/`, with a README row.
- [x] `DEFERRED.md` "effects" section; `crates/ro-scripting/DESIGN.md` rewritten; `ro-engine/DESIGN.md` notes; the `CLAUDE.md` dependency rule.
- [x] `spec.md`, `plan.md`. **User checkpoint**: review the spec and plan before M1.

### M1: data (DB-EFF)
- `tools/skill-import`: the spec §0.1 fields. `tools/item-import`: the spec §0.2 `Use:` lift, with counts. Use our own small fixtures (never rAthena excerpts) for importer tests.
- `ro-db`: `SkillData` accessors, `ItemData.on_use`, `ItemUse`. Data version bumps, regenerated files, `NOTICE.md`, header tests.
- Watch the `deny_unknown_fields` structs on both the raw and the data side.

### M2: mechanics (MECH-EFF)
- `AttackContext` into `calc_weapon_attack` (spec §2.6). Renewal `cast_time(...)` and `delayfix(...)` as pure functions (catalog `scripting/skills.md` §2, §4).
- Add `item_use_interval` to `BattleConfig` (default 100).

### M3: `ro-scripting` (SCR-EFF)
- The crate gets `[lints] workspace = true`, private modules and root re-exports. It depends on `ro-core` and `smallvec`. The `lua` feature is declared but empty until M9.
- Types from DESIGN §2: `EffectSpec`, `Effect`, `EffectList`, `Who`, `HealShow`, `LevelValue`, `HookId`, `HookCtx`, `UnitView`, `CastSource`, `SkillLevels`, `Roll`, `HookBackend`, `HookError`, `SkillDef`, `ItemUse` (the scripting side), `Definitions`, `RustHooks`.
- Test doubles: a fixed `Roll` and a `SkillLevels` map.

### M4: core and packets (PKT-EFF) (done, session B)
- `ClientIntent::UseSkill { level, skill_id, target }` (`0x0438`) and `UseItem { index }` (`0x0439`).
- `ServerEvent` variants for `0x0b1a`, `0x01de`, `0x09cb`, `0x0983` (efst/flag/timers), `0x01c8`, `0x00b1`, and the serializers. Encode tests against the capture frames.

### M5: zeny and GM commands (PER-EFF, ENG-EFF-16) (done, session B)
- Settled by `scripting_2.txt` (session B): **no `0x00b1` at map entry**; the char-server's char list fills `CHARACTER_INFO.money` from `characters.zeny`.
- `CharacterStatus.zeny` (ro-core), load and save in `ro-persistence`, the char-list `money` field, `@zeny`, `@heal` (spec §2.8-§2.9).

### M6: cast pipeline and effect applier (ENG-EFF-01..12, 17) (done, session B; early review applied)
- `crates/ro-engine/src/cast.rs`: the `0x0438` flow (spec §2.2-§2.4), `CombatTimer::CastEnd`, the step-action extension for skills.
- `effect.rs`: apply `Spend*`, `Heal` and `WeaponDamage` (via the shared `weapon_hit` split out of `swing()`), with the packets.
- **Early `reviewer` run** over M3 + M6 (crate API, shared tick state). Apply its fixes before M7.

### M7: potion use (ENG-EFF-13..15) (done, session C)
- `0x0439` flow (spec §2.7) in `inventory.rs` or a new `item_use.rs`, through `Definitions` and the hooks. Map `ro_db::ItemUse` to the scripting `ItemUse` + `ctx.args`.

### M8: bin (BIN-EFF-01) (done, session C)
- Wire `Registries.effects`/`hooks` in the map-server. TCP test: First Aid, Bash on a spawned mob, a potion, with an observer.

### M9: Lua spike (SCR-LUA-01) (done, session D)
- Feature `lua`: `mlua` (vendored, `lua54`). `crates/ro-scripting/lua/sm_bash.lua` is MIT, our own, and read from disk. Build the `LuaHooks` loader, `examples/hook_bench.rs`, and the equality test.
- Record the call cost and the context-passing comparison in catalog `scripting/lua_notes.md`. Add `mlua` to the README if a dependency list is kept there.

### Finish
- `./verify.sh` green, then `./verify.sh --smoke` (db up). Run the `reviewer` once over `git diff HEAD` plus untracked files, and apply its fixes.
- Move spec §5 into `DEFERRED.md` "effects" (mark what landed). Update DESIGN status lines. One commit.
- **Manual client check (user):** on `prt_fild08` with an observer:
  - First Aid (after `@heal -50 0`), including at full HP;
  - Bash lv 10/lv 1 on Porings, including a kill and a mid-loop cast;
  - Mammonite with `@zeny`, including running out of zeny;
  - Heal on self, the observer and a Poring;
  - Red/Blue Potions, including at full HP.

  Then a metaling capture compared against `scripting_1.txt`.
