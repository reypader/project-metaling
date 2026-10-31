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
| Capture | `scripting_1.txt`, recorded by the user 2026-10-31, written up in M0. |
| Deferred list | Everything postponed is in `DEFERRED.md` "effects" (user request). |
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
- **Registries:** `crates/ro-engine/src/engine.rs:136`. Add `effects: Arc<ro_scripting::Definitions>` and `hooks: Arc<dyn HookBackend + Send + Sync>`; the map-server builds them where it builds the other registries. The wire gid of an entity is `gid(e)` (`engine.rs:240`).
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

### M4: core and packets (PKT-EFF)
- `ClientIntent::UseSkill { level, skill_id, target }` (`0x0438`) and `UseItem { index }` (`0x0439`).
- `ServerEvent` variants for `0x0b1a`, `0x01de`, `0x09cb`, `0x0983` (efst/flag/timers), `0x01c8`, `0x00b1`, and the serializers. Encode tests against the capture frames.

### M5: zeny and GM commands (PER-EFF, ENG-EFF-16)
- First a quick `metaling-explorer` lookup: where rAthena sends `SP_ZENY` at map entry for a character with zeny. Record it in the catalog (`atcommands.md` zeny notes or `packets/status.md`, the latter is near its 400-line limit).
- `CharacterStatus.zeny` (ro-core), load and save in `ro-persistence`, `0x00b1` at entry per the lookup, `@zeny`, `@heal` (spec §2.8-§2.9).

### M6: cast pipeline and effect applier (ENG-EFF-01..12)
- `crates/ro-engine/src/cast.rs`: the `0x0438` flow (spec §2.2-§2.4), `CombatTimer::CastEnd`, the step-action extension for skills.
- `effect.rs`: apply `Spend*`, `Heal` and `WeaponDamage` (via the shared `weapon_hit` split out of `swing()`), with the packets.
- **Early `reviewer` run** over M3 + M6 (crate API, shared tick state). Apply its fixes before M7.

### M7: potion use (ENG-EFF-13..15)
- `0x0439` flow (spec §2.7) in `inventory.rs` or a new `item_use.rs`, through `Definitions` and the hooks. Map `ro_db::ItemUse` to the scripting `ItemUse` + `ctx.args`.

### M8: bin (BIN-EFF-01)
- Wire `Registries.effects`/`hooks` in the map-server. TCP test: First Aid, Bash on a spawned mob, a potion, with an observer.

### M9: Lua spike (SCR-LUA-01)
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
