# File structure refactor: plan

## Context

At M0 (2026-11-05, after commit `f206617`) the codebase is about 77k lines of Rust in 218 files.

Problems:
- **Files with several types:** 55 non-test files declare more than one non-private type. The worst are `ro-packets/src/incoming/map.rs` (22 types), `tools/skill-import/src/lib.rs` (20), `ro-core/src/http.rs` (13) and `bins/map-server/src/service.rs` (12).
- **Inline tests:** 51 files carry inline test modules, the biggest being `ro-engine/src/equipment.rs` with 667 test lines.
- **Mixed ro-engine feature files:** in ro-engine, 18 feature files each add an `impl MapInstance` block and also define their own helper types.
- **Copied test helpers:** integration tests repeat their helpers in every file. `char_id` is defined 16 times across ro-engine's 17 test files.

Goal: a reader can open any file and understand it on its own.

**This is a refactor only. Behavior does not change. Test names, scenario IDs and the public API stay the same.**

Decisions confirmed with the user: feature folders for ro-engine, private helper types may stay beside their owner, both inline and integration tests move, the scope is `crates/` + `bins/` + `tools/`, one test binary per crate, the big tables stay as they are, DTO files use a `_dto.rs` suffix, and **one commit at the end** (CLAUDE.md). The M0 docs may be committed on their own.

## Rules

1. **One main type per file.** A file holds one struct, enum or trait plus its impls (inherent and trait impls). Small *private* helper types used only by that type may stay in the same file, for example `Departure` next to `MapWorldActor`, or serde shims next to the type they deserialize.
2. **DTO groups.** Data-only types (derives, a trivial `new`/`Default`, small value mappings like `wire_code()`) are grouped by theme in one file named `<theme>_dto.rs`. Each such file starts with a one-line `//!` doc saying what the group is.
3. **Other kinds of file.**
   - A file of free functions covers one purpose and is named for it (`skill/tree.rs`, `job/rules.rs`).
   - A file of only constants is named `<theme>_consts.rs`.
   - A `mod.rs` holds module declarations, re-exports and, where the folder is named for it, the folder's main type.
4. **ro-engine feature folders.** Each feature is a folder. The feature's `impl MapInstance { .. }` block goes in its `systems.rs`. If that file is still large, split it into `systems/<responsibility>.rs`. `MapInstance` itself lives in `map_instance/`, split by facet. Rust allows `impl MapInstance` blocks in any module of the crate.
5. **The big single-type tables stay as they are.** These are `ro-packets` `opcode.rs`, `table.rs` and `outgoing/encoder.rs` (`ModernEventEncoder` and its thin `Phase0EventEncoder` delegate), and the `ServerEvent` enum. Only their inline tests move out.
6. **Tests leave the source files.**
   - An inline `#[cfg(test)] mod tests { .. }` becomes `#[cfg(test)] mod tests;` with its body in `<module>/tests.rs`. A `mod tests;` declared in `foo.rs` resolves to `foo/tests.rs`, and `use super::*` keeps working.
   - Integration tests become one test binary per crate (M8).
7. **The public API does not change.** Library crates keep private modules and root re-exports (CLAUDE.md "one import path per item"). Bins and tools keep the same names re-exported from `lib.rs`. Only intra-crate `crate::…` paths change. Nothing outside a crate imports through a module path; at M0, `grep -rnE '(map_server|login_server|char_server|ro_proxy|npc_import)::[a-z_]+::'` finds nothing.
8. **Allowed edits other than moves.**
   - Widening visibility to `pub(crate)` or `pub(super)` where a sibling module now needs access. Never widen to `pub`.
   - Merging *identical* duplicate constants or helpers into one place (listed per milestone).
   - Moving shared test fixtures into `#[cfg(test)]` `test_support` modules.
   - Turning redundant `pub(crate) mod` into `mod` at a crate root.
   - **Nothing else is allowed:** no renames of public items, no logic edits, no new abstractions, no `Redacted` newtype for `ClientIntent`.
9. **Keep git history useful.** When splitting a file, `git mv` it to the new file that keeps the most of its code, then create the others. This preserves rename detection and blame.

## Baseline and checks (recorded at M0)

| Check | Baseline | How |
| :--- | :--- | :--- |
| REF-01 tests | **927 passed**, 0 failed, 0 ignored (full `./verify.sh`, db up) | `./verify.sh`. Per-package counts are in the table below. |
| REF-02 public API | 496 `<lib.rs> <name>` rows in `public_api.txt` | `specs/file-structure/public_api.sh \| diff specs/file-structure/public_api.txt -` |
| REF-03 structure | 106 findings (55 multi-type, 51 inline-tests) | `specs/file-structure/structure_check.sh` must exit 0 at the end. The only allowlisted file is `ro-packets/src/outgoing/encoder.rs` (rule 5). |
| REF-04 generated data | the committed `crates/ro-db/data/` | With `RATHENA_PATH` set, rerun each importer (commands in `crates/ro-db/data/NOTICE.md`), then `git diff --exit-code crates/ro-db/data/`. No golden test covers this. |

Notes on the checks:
- `public_api.sh` expands glob re-exports (ro-core) by listing every `pub` item under `src/`, recursively. A sub-folder `mod.rs` that forgets to re-export an item still shows up as present. Downstream compile errors catch the items that are actually used.
- `structure_check.sh` counts only top-level `pub`/`pub(crate)`/`pub(super)` struct/enum/trait declarations. It skips private helpers, `_dto.rs`, `tests.rs` and `test_support` files.

Per-package test counts (`cargo test -q -p <pkg> -- --list | grep -c ': test$'`; includes doctests):

| Package | Tests | Package | Tests |
| :--- | ---: | :--- | ---: |
| ro-core | 6 | login-server | 20 |
| ro-packets | 137 | char-server | 9 |
| ro-engine | 343 | map-server | 95 |
| ro-db | 98 | ro-proxy | 54 |
| ro-mechanics | 37 | skill-import | 12 |
| ro-persistence | 51 | item-import | 11 |
| ro-scripting | 11 (16 with `--features lua`) | job-import | 9 |
| | | mob-import | 4 |
| | | npc-import | 28 |
| | | refine-import | 2 |
| **total** | **927** (lua not included) | | |

## Milestones

Each milestone leaves `./verify.sh -p <touched crates>` green, with the same per-package test count. Production code moves first, in M1–M7, while integration tests stay untouched. They import only root re-exports, so they act as the safety net. Tests are restructured in M8 against code that no longer moves.

Work stays uncommitted until M9 (CLAUDE.md "commit once per plan"). If a session ends mid-plan, the working tree carries the progress; tick the milestone boxes in this file as you go.

### M0: Spec and baseline
- [x] `spec.md` and `plan.md` written.
- [x] Baselines recorded: `public_api.txt`, the counts above.
- [x] Check scripts written.
- [x] `CLAUDE.md` gains a "File structure" rule.
- **User checkpoint.**

### M1: ro-core
`message.rs` (890 lines) splits into:

| New file | Contents |
| :--- | :--- |
| `client_intent.rs` | `ClientIntent` and its hand-written `Debug` impl (prints `[REDACTED]` for the password); about 290 lines |
| `server_event.rs` | `ServerEvent`, kept whole (rule 5) |
| `envelope_dto.rs` | `BroadcastScope`, `SessionIntent`, `ScopedServerEvent` |
| `unit_dto.rs` | `UnitObjectType`, `UnitLooks`, `UnitAppearance` |
| `client_ui_dto.rs` | `SkillInfo`, `StatusWindow`, `AchievementSummary` |

Other files:
- `ids.rs` keeps `AccountId`, `CharId`, `SessionId`, `Sex` and `MapPosition`. `CharacterSummary`, `CharacterSelectStats`, `CharacterLooks` and `CharServerInfo` move to `char_select_dto.rs`.
- `http.rs` becomes `http/mod.rs` (re-exports) plus:
  - `http/server_registry_dto.rs`: `ServerStatus`, `ServerType`, `HealthResponse`, `ServerHeartbeat(+Response)`, `RegisterCharServer*`, `RegisterMapServer*`
  - `http/session_dto.rs`: `ValidateSession*`, `PrepareMapTransfer*`
- `character.rs` becomes `character_dto.rs`. Move the stray `impl CharacterSpawn { fn progress }` (lines 96–108, before the struct) to sit next to the struct.
- `item.rs` becomes `item_dto.rs` and `account.rs` becomes `account_dto.rs`. `map_name.rs` stays.

ro-core-specific notes:
- `lib.rs` keeps `pub use <module>::*` for each module.
- `account.rs`, `http.rs`, `character.rs` and `message.rs` import siblings via `crate::ids::`/`crate::item::`; fix those paths.
- [x] Done. Deviation: `ids.rs` became `ids_dto.rs`; its five data-only ID types would otherwise fail REF-03.

### M2: ro-db, ro-mechanics, ro-scripting, ro-persistence
**ro-db.** Its `lib.rs` uses explicit re-export lists; keep every name.

| Old file | New files |
| :--- | :--- |
| `skill.rs` (695) | `skill_flags_dto.rs` (`SkillFlags`, `DamageFlags`, `CastTimeFlags`, `SkillItemCost`)<br>`skill_element.rs`<br>`skill_data.rs` (DTO plus per-level getters)<br>`skill_db.rs`<br>`skill_tree_dto.rs` (`SkillRequirement`, `TreeEntry`, `JobTree`)<br>`skill_tree_db.rs` |
| `mob.rs` (518) | `mob_enum_dto.rs` (`MobClass`, `MobSize`, `MobRace`)<br>`element.rs` (`Element`, which skill code also uses)<br>`mob_mode.rs` (`MobMode` and `MobModes`)<br>`mob_data.rs`<br>`mob_db.rs` (with `check_mob`) |
| `job.rs` (535) | `job_dto.rs` (`JobBonus`, `JobBonusEntry`, `JobData`, `StatPointRow`, `ExpKind`, `ExpGroup`)<br>`job.rs` (`Job`)<br>`job_db.rs` (`JobDb`, `build_job`, constants) |
| `item.rs` (471) | `item_type_dto.rs` (`ItemType`, `WeaponType`, `AmmoType`, `Gender`, `ItemUse`)<br>`item_data.rs` (`ItemData` and its serde default helpers)<br>`item_db.rs` |
| `npc.rs` | `npc_dto.rs` (`NpcState`, `WarpDest`, `NpcKind`, `NpcSpawn`)<br>`map_npcs.rs`<br>The private `Raw*` serde shims stay beside `NpcSpawn` |
| `cell.rs` | `cell_type.rs`<br>`error.rs` (`MapDataError`, `DbLoadError`) |
| `refine.rs` | `refine_db.rs`<br>`refine_dto.rs` |
| `mob_spawn.rs` | `MobId` moves to `mob_enum_dto.rs` (or `ids_dto.rs`); the rest stays |

- Merge the duplicate helpers into `yaml.rs`: `schema_error` (4 copies: item/job/skill/refine), `load_required` (2: job/skill), and `flow_list` and the `is_zero*` variants (3–4 copies). Merge only copies that are byte-identical.

**ro-mechanics:**
- `combat.rs` becomes:
  - `combat_dto.rs`: `WeaponAttack`, `AttrRates`, `AttackerView`, `DefenderView`, `AttackContext`, `HitKind`, `DamageResult`
  - `combat.rs`: `calc_weapon_attack`, its private helpers and the tuning constants
  - `distance.rs`: `client_distance`, `check_distance_client`
- `status.rs` becomes:
  - `status_input_dto.rs`: `JobRow`, `WeaponClass`, `RefineInput`, `HandInput`, `GearInput`, `StatusInputs`
  - `status_dto.rs`: `Status`, `Primary`, `Traits`
  - `status.rs`: `calc_status`, helpers, constants
- The other files are already fine.

**ro-scripting:**

| Old file | New files |
| :--- | :--- |
| `effect.rs` | `effect_dto.rs` (`Who`, `HealShow`, `Effect`)<br>`effect_list.rs`<br>`effect_spec.rs` (with `LevelValue`) |
| `definition.rs` | `definition_dto.rs` (`TargetRule`, `ItemUse`, `SkillDef`, `ItemUseDef`, private `resolve`)<br>`missing_hook.rs`<br>`definitions.rs` (`Definitions` and its skill-id constants) |
| `hook.rs` | `hook_backend.rs` (trait, `HookId`, `HookFn`)<br>`hook_error.rs`<br>`rust_hooks.rs`<br>`builtin_hooks.rs` (`al_heal`, `item_heal`, `percent_heal`, math helpers, skill-id constants) |
| `lua.rs` | `lua/mod.rs` (`LuaHooks`, `CtxMode`)<br>`lua/error.rs` (`LuaLoadError`)<br>`lua/marshal.rs` (table builders, `ScopedCtx`, `LuaUnit`)<br>`lua/parse.rs` (Lua→effect parsers) |
| `context.rs` | `context_dto.rs` (`UnitView`, `CastSource`, `HookCtx`)<br>`traits.rs` (`SkillLevels`, `Roll`) |

- **`lua` is behind `#[cfg(feature = "lua")]`, and `./verify.sh` never builds it.** Keep the cfg on the
  `mod lua;` and `pub use lua::…` lines. After the split, run
  `cargo clippy -p ro-scripting --features lua --all-targets` and `cargo test -p ro-scripting --features lua`,
  and compare against the lua baseline in the table above.
- `examples/hook_bench.rs` stays as is; it imports only root items.

**ro-persistence:**
- Move the `pub(crate)` mapping fns from `postgres/mod.rs` into `postgres/mapping.rs`, so `mod.rs` holds only declarations.
- Move the numeric conversion helpers from `postgres/inventory.rs` (`bad`, `narrow`, `big`, `int`, `small`, `to_*`) into `postgres/convert.rs`.
- Move `RepositoryError` from `repository/mod.rs` into `repository/error.rs`.
- Move `CreateCharacterError`, `DeleteReserveOutcome` and `CancelDeleteOutcome` into `repository/character_outcome.rs`. `AuthError` stays with `AccountRepository`, as part of its contract.
- Move the inline tests out (`postgres/mod.rs`, `postgres/character.rs`).

- [x] Done. Deviations, all so that REF-03 passes (the check counts every `pub` type in a non-`_dto` file):
  - ro-db: `mob_mode.rs` holds only `MobMode`; `MobModes` is in `mob_modes.rs`. `cell.rs`'s errors went to `map_data_error.rs` and `db_load_error.rs`. `mob_enum_dto.rs` is named `mob_dto.rs` because it also holds `MobDrop` and `MobId`. `mob_spawn.rs` was split: `MapMobSpawns` and its constants are in `map_mob_spawns.rs`.
  - ro-db helpers: `schema_error` and `load_required` were byte-identical. The `flow_list` and `is_zero*` copies differed only in their signatures (generic parameter order, or concrete `u8`/`u16`/`u32`/`i16`) and had the same bodies, so each became one generic helper in `yaml.rs`. Item's `is_default` stays where it is.
  - ro-mechanics (not in the plan; the check flagged them): `bonus.rs` became `bonus_slot.rs` + `bonus_totals.rs`, and `cast.rs` moved its DTOs into `cast_dto.rs`.
  - ro-scripting: `LevelValue` is in `level_value.rs` (not `effect_spec.rs`), `HookId` is in `hook_id.rs`, and the private `HookFn` alias is in `rust_hooks.rs`. `context.rs`'s traits went to `skill_levels.rs` and `roll.rs`, and `CtxMode` went to `lua/ctx_mode.rs`.
  - ro-persistence: `AuthError` moved to `repository/auth_error.rs` and `PlayState` to `repository/play_state.rs`. The outcome file is `character_outcome_dto.rs`. Callers import the mapping fns from `super::mapping::`.

### M3: ro-packets
- **`incoming/map.rs` (672 lines, 22 `Cz*Packet` structs)** becomes `incoming/map/<packet>.rs`, one file per packet (e.g. `request_move.rs`, `use_skill_to_id.rs`).
  - Each packet has `decode`/`to_intent`, so each is behavior and gets its own file.
  - `MAX_HEAD_DIR`/`MAX_BODY_DIR` go with `change_direction.rs`.
  - **Code continues after the test module: tests are at lines 317–426, and 11 more structs follow at 427–672.**
  - Move the tests to `incoming/map/tests.rs`.
- `incoming/char.rs` (7 packets) becomes `incoming/char/<packet>.rs` in the same way.
- `incoming/login.rs` and `incoming/frame.rs` stay; only their tests move out.
- `incoming/mod.rs` and `outgoing/mod.rs` are `pub mod` children of private parents. Keep their re-exports.
- `wire.rs` splits into:
  - `wire_dto.rs`: `PacketLength`, `RawPacket`, `MAX_PACKET_SIZE`
  - `error.rs`: `PacketCodecError`
  - `packet_length_table.rs`, `packet_intent_parser.rs`, `event_encoder.rs`: one trait each
- `codec.rs` becomes `packet_decoder.rs` and `packet_encoder.rs`.
- Move tests out of `coords.rs`, `opcode.rs`, `table.rs`, `outgoing/encoder.rs` (403 test lines), `outgoing/{login,char,map,settings,status}.rs`, `incoming/{login,frame}.rs`.
- The `outgoing/*.rs` encoder-function files are already one-purpose; leave them.
- [x] Done. Deviations:
  - `incoming/frame.rs` held two types: `Phase0IntentParser` and its two aliases moved to `incoming/intent_parser.rs`. `frame.rs` keeps `DecodedFrame` and `decode_frame`.
  - File names drop the `Cz`/`Ch` prefix and the `Packet` suffix (`PingPacket` → `char/ping.rs`).
  - The codec tests (decoder only) live in `packet_decoder/tests.rs`.
  - Moved test modules import what they used to get from the parent's `use` lines (e.g. `crate::{PacketCodecError, RawPacket}`).
- **Helper scripts used for M1–M3** (not in the repo; recreate them if useful): one moves an inline `#[cfg(test)] mod tests { .. }` into `<module>/tests.rs` without re-indenting, and `cargo fmt` fixes the indentation, so string literals stay exact. Strip leading blank lines, or `cargo fmt --check` fails. A second compares the significant-line multiset of a `HEAD` file against its split files, to catch lines dropped or duplicated by off-by-one `sed -n` ranges. **Use `cat -n` for line ranges:** tool output hides a leading blank line, which caused off-by-one ranges twice.

### M4: ro-engine foundation
At M0 the engine has 30 files and 14,259 lines, about 4,350 of them inline tests. All modules are private.
- **`lib.rs`**: six modules are written `pub(crate) mod`, which is redundant; make them `mod`. Root re-exports: `SkillScripts`, `roll_mob_drops`, `MobDropRequest`, `MapInstance`, `Registries`, `DEFAULT_ENTITY_CAPACITY`, `START_NPC_ID`, `EngineError`, `MobView`, `MapTransfer`, `PlaySnapshot`.
- **`protocol_consts.rs`** holds the wire constants shared across features. Feature-private constants stay in their folder.

  | Constants | Comes from | Used by |
  | :--- | :--- | :--- |
  | `SP_*` var ids | status.rs | 6 files |
  | `SP_WEIGHT`, `SP_MAXWEIGHT` | engine.rs | |
  | `SP_ZENY` | vitals.rs | |
  | `VANISH_*` | engine.rs | |
  | `STANDING`, `SITTING`, `DEAD = 1` | engine.rs; `DEAD` is defined 4 times (floor_item, warp, wear, stat_up tests) | |
  | `ACTION_*`, `LOOK_*`, `EFFECT_*` | | |
  | `UNKNOWN_ITEM_TYPE` | defined twice (inventory, floor_item) | |

  Merge these duplicates too: `STAT_COUNT` (status, level_command), `EDGE_GAP` (mob, floor_item), and the move-cost constants (path.rs `MOVE_COST`/`MOVE_DIAGONAL_COST`, movement.rs, combat.rs `DIAGONAL_COST`/`STRAIGHT_COST`). Merge only where the values are identical. If they differ, leave them and note why.
- **`ecs/`**: `entity.rs` (`Entity`), `entity_allocator.rs`, `sparse_set.rs`, `spatial.rs` (`SpatialGrid8x8`, `step_visibility`, `AREA_SIZE`), `rng.rs` (`SplitMix64`). `error.rs` stays at the root.
- **`world/`**: `mod.rs` holds `World`; `components_dto.rs` holds `EntityKind`, `PlayerInfo`, `NpcInfo`, `MobInfo`, `FloorItem`, `WarpTrigger`. `world` imports types from 8 feature modules; that is fine.
- **`map_instance/`** is today's `engine.rs`, 1,299 lines: impl block A at lines 170–976 and the public impl block B at 977–1204.

  | File | Contents |
  | :--- | :--- |
  | `mod.rs` | `MapInstance` struct, `new`/`with_capacity`, `set_registries` |
  | `registries.rs` | `Registries` |
  | `intents.rs` | `handle_intent` (the ~84-line routing match), `push_intent` |
  | `tick.rs` | `step_tick` (~20 lines; it is already just the schedule) |
  | `queries.rs` | `gid`, `pos`, `cell`, `char_of`, `appearance`, `dead_sit`, `is_walking`, `map_name`, revisions, `play_snapshot`, `query_aoi_characters`, `character_position`, `weight`, `drain_save_requests` |
  | `emit.rs` | `aoi_recipients`, `recipients_*`, `emit_self`/`emit_aoi`/`emit_to`, `emit_weight`, `emit_action` |
  | `player.rs` | `spawn_character`, `spawn_player`, `despawn_player`, `remove_character`, `actor_init`, `send_inventory`, sit/stand, `give_item`/`give_item_to`/`add_item`, and `is_dead`/`refuse_dead` (moved from wear.rs; used by combat, cast, item_use, experience, stat_up) |
  | `npc.rs` | `spawn_npcs`/`spawn_npc`, `name_request`, `entry_event`, `set_cell` |

  `emit_self` is called from 15 files and `gid` from 10, so they must be `pub(crate)`.
- **`movement/`**:
  - `walking_path.rs`: `WalkingPath`, `step_duration_ms`
  - `path.rs`: `AStarScratchBuffer`, the private `Passable`, `MAX_WALKPATH*`
  - `line_of_sight.rs`: `line_of_sight`, `battle_check_range`
  - `systems.rs`: `find_path`, `start_walk`, `plan_moves`/`plan_move`, `advance_walkers`, `step` (~92 lines), `repath`, `stop_walking`, `walk_dest`, `query_into_after`, `step_dir`, `PathCells`

- [x] Done. Deviations:
  - `Entity`'s two fields are `pub(super)` so `EntityAllocator` (sibling file) can read them.
  - `protocol_consts.rs` takes every `SP_*` and `LOOK_*` value, including ones only one feature uses, so each wire enum stays in one place. This supersedes M5's `LOOK_WEAPON` in `equipment/systems/looks.rs`. It also takes `EDGE_GAP`.
  - Move costs: movement.rs and combat.rs (both `u64`, same values) merged into `movement/walking_path.rs` as `pub(crate)` `MOVE_COST`/`MOVE_DIAGONAL_COST`. `path.rs` keeps its own `u32` pair: the A* costs are `u32`, and merging would need casts (a logic edit).
  - The test-only duplicates `SP_MAXHP`/`SP_MAXSP` (stats tests) and `DEAD` (stat_up, wear tests) now import from `protocol_consts`.
  - `movement/test_support.rs` holds `grid`, shared by the `path` and `line_of_sight` tests. engine.rs's `npc_tests` became `map_instance/npc/tests.rs`.
  - `MapInstance` fields `moves`, `walkers`, `step_candidates`, `aoi_after`, `left`, `entered`, `scratch` are now `pub(crate)` (`movement/systems.rs` uses them).

### M5: ro-engine features
- [x] **`combat/`**:
  - `damage_log.rs`: `DamageLog`, `DamageEntry`
  - `state_dto.rs`: `AttackState`, `StepSkill`, `PendingCast`, `PendingHit`
  - `timers.rs`: `CombatTimer`, `CombatTimers`, the private `TimerEntry` and its `Ord`
  - `rules.rs`: `attacker_view`, `needed_ammo`, `chebyshev`
  - `systems/attack.rs`: request, cancel, stop, range
  - `systems/swing.rs`: `swing_check`, `swing`
  - `systems/hit.rs`: `queue_hit`, `spend_ammo`, `apply_hit`, `send_hp_bar`
  - `systems/death.rs`: `kill_mob`, `vanish_dead_mob`
  - `systems/step_action.rs`
  - `systems/timers.rs`: `run_combat_timers`, including the `CastEnd` arm that calls into cast
  - `FAIL_SKILL_INTERVAL`/`HIDE_SKILL_INTERVAL` are used by cast, so make them `pub(crate)`.
- [x] **`cast/`**:
  - `skill_scripts.rs`: `SkillScripts`, re-exported at the root
  - `adapters.rs`: `MapRoll` (implements `Roll`; item_use uses it) and `impl SkillLevels for SkillSet` (the trait is from ro-scripting, the type from skill/)
  - `systems/request.rs`: `use_skill` and its requirement checks
  - `systems/pipeline.rs`: `cast_begin`, `cast_end`, `send_postdelay`
  - `systems/effects.rs`: `apply_effect`, `apply_heal`, `weapon_skill_hit`, `unit_view`, `casting_element`
- [x] **`skill/`**:
  - `skill_slot.rs`, `skill_set.rs`
  - `tree.rs`: the private `TreeData`/`TreeOwner`/`TreeChange` plus `points_spent`, `max_job_level`, `tree_job`, `reduce`, `hidden_at_zero`, `compute_visible`, `range2`
  - `wire.rs`: `skill_info`, `skill_list`
  - `systems.rs`: refresh, send list, upgrade, `skill_update`, tree diff
  - `commands.rs`: `reset_skills`, `command_reset_*`, `command_all_skills`
  - `chat()` here and `level_command::chat_line` look alike. Merge them only if they are identical.
- [x] **`item/`**:
  - `inventory/player_inventory.rs`: `PlayerInventory`, the private `Added`, `ADD_*` codes
  - `inventory/wire.rs`: `wire_index`, `slot_of`, `wire_cards`, `list_entry`, `item_weight`, `is_stackable`
  - `floor_item/mod.rs`: `FloorItemSpec`, `loot_tick`, `MIN_FLOOR_ID`
  - `floor_item/systems.rs`: add/remove/expire, pick up, drop, free-cell search
  - `drops/mod.rs`: `MobDropRequest`, `roll_mob_drops`, `identified_drop`
  - `drops/systems.rs`
  - `item_use.rs`: the MI block plus the private `Use` and `overflow`
  - **`item/test_support.rs` (`#[cfg(test)]`):** today `inventory.rs:407` is `pub(crate) mod tests`, and the floor_item and wear tests import `test_items` from it. Create test_support first, then split.
- [x] **`equipment/`** (rules from equipment.rs plus flows from wear.rs):
  - `eqp_consts.rs`: `EQP_*`, `EQI_ORDER`, `MSI_*`, `EQUIP_FAIL*`, `W_DOUBLE_*`
  - `worn_slots.rs`
  - `gear.rs`: `Gear<'a>` and both of its impl blocks, plus the `WeaponGear`/`GearStatus` DTOs (or `gear_dto.rs`)
  - `rules.rs`: `Wearer`, `Refusal`, `equip_point`, `check_equip`, `usable_by`, `ammo_*`, `is_ranged`, `resolve_position`, `prepare_equip`, `equip_view`
  - `systems/equip.rs`: equip and ammo
  - `systems/unequip.rs`: unequip, unequip-all and unequip-slot, with the private `Unequip`
  - `systems/recheck.rs`: level/job re-checks
  - `systems/looks.rs`: `equip_look_events`, `refresh_looks`, `send_*_looks`, `ammo_icon`, `LOOK_WEAPON`
  - wear.rs's `is_ranged_weapon` duplicates the idea of `equipment::is_ranged`. Merge them only if they are identical.
- [x] **`job/`**: `mod.rs` (the ~13 pure fns used by 9 files, plus `JOBL_*`/`MAPID_*`/`ITEMJ_*`), `table.rs` (the 154-row `JOB_MAPIDS`).
- [x] **`stats/`**:
  - `mod.rs`: `FollowUp`, dirty flags `DIRTY`/`STALE_GEAR`/`SKILLS`
  - `recalc.rs`: `weapon_class`, `aspd_base`, `hand_input`, `gear_input`, `job_row`
  - `systems.rs`: the stat pass
  - `status_push.rs`: today's status.rs `Persisted` plus the push builders `param`, `couple`, `status_diff`, `status_window`, `trait_costs`, `battle_*`, `exp`
  - `login_push.rs`: today's status.rs MI block
  - `raise.rs`: today's stat_up.rs, `stat_mut`, `raise_stat`, `raise_trait`
- [x] **`progression/`**:
  - `mod.rs`: `LivePush` (from experience.rs) and `points` (from level_command.rs)
  - `experience.rs`: `level_up`, `add_exp`, `gain_exp`, the bursts
  - `level_commands/{blvl, jlvl, job, reset, points}.rs`
  - This breaks the near-cycle stats ↔ status ↔ experience ↔ level_command. `EFFECT_*` goes to `protocol_consts.rs`.
- [x] **`mob/`**: `mob_stats.rs`, `spawn_group.rs`, `view.rs` (`MobView`, re-exported at the root), `placement.rs` (`search`, `walkable_cell`), `systems.rs` (spawn, `mobs()`).
- [x] **`warp/`**: `cell_bits.rs`, `transfer_dto.rs` (`MapTransfer`, `PlaySnapshot`), `systems.rs` (warp cells, touch, same-map and cross-map warps, rewarp, `drain_transfers`, `enter_by_warp`, `area_contains`).
- [x] **`session/`**:
  - `entry_settings.rs`: today's settings.rs, `Arrival` plus the login burst
  - `chat.rs`: today's command.rs, chat/tell/`run_command`/`@item`, `GM_GROUP`, `parse_item_params`
  - `vitals.rs`: heal, `change_vitals`, `spend_sp`/`spend_hp`, zeny
  - `vitals_commands.rs`: `@zeny`/`@heal`
- [x] Root re-exports unchanged (REF-02).
- [x] Integration tests untouched and green.
- [x] Done. Deviations, mostly so that REF-03 passes (two `pub(crate)` types in one non-`_dto` file fail it):
  - `combat/`: `DamageEntry` and `CombatTimer` live in `state_dto.rs`. `FAIL_SKILL_INTERVAL`/`HIDE_SKILL_INTERVAL` moved to `protocol_consts.rs` (wire values shared by combat and cast).
  - `cast/`: `casting_element` and `MAX_PLAIN_ELEMENT` are in `systems/pipeline.rs`, next to their only caller `cast_begin`. `INF_*`/`FAIL_*` are in `cast/cast_consts.rs` (request and pipeline share them).
  - `skill/`: `TreeData`/`TreeOwner`/`TreeChange` are in `tree_dto.rs`. `NV_BASIC`/`NV_TRICKDEAD`/`SG_DEVIL` are in `skill_consts.rs`. `chat()` and `level_command::chat_line` were identical and are now one `pub(crate) fn chat_line` in `map_instance/emit.rs`.
  - `item/`: `Added` is returned by `pub(crate) fn add`, so it stays `pub(crate)`, in `inventory/added_dto.rs`. The inventory tests' `data`/`test_items` moved to `item/test_support.rs`.
  - `equipment/`: `WeaponGear`/`GearStatus` are in `gear_dto.rs`, `Wearer`/`Refusal` in `wearer_dto.rs`. Every `eqp_consts.rs` value is `pub(crate)` (glob re-export). `is_ranged_weapon` (takes `&ItemData`, checks the item type) only wraps `is_ranged`; they are not identical, so both stay.
  - `progression/level_commands/`: added `args.rs` (`level_argument`, used by blvl and jlvl) and `msg_consts.rs` (every reply line). `gets_transcendent_points` is in `points.rs` (reset and job share it).
  - `job/`: `JOB_MAPIDS` is in `table.rs`, `pub(super)`.
  - A split module keeps one test file per old test module: e.g. `skill/tests.rs`, `stats/status_push/tests.rs`, `equipment/systems/tests.rs` (wear.rs's tests).
  - Hazard for later milestones: a `match` that uses constants as patterns keeps compiling when the constants fall out of scope (they become catch-all bindings), and `cargo fix` then renames them `_NAME`. It happened once in `equipment/rules.rs` and was caught by the tests. Check for "unreachable pattern" or case warnings before running `cargo fix`.

### M6: bins
- [x] Done (124 bin tests pass; REF-02 diff empty; no REF-03 findings in `bins/`). Deviations, mostly so that REF-03 passes:
  - map-server `service/`: `MapData`, `MapDataLoader` and `RegistryPaths` are in `map_data_dto.rs`; `map_data.rs` holds only the loader functions plus `FALLBACK_MAP_SIZE`/`MAX_WIRE_MAP_NAME`. `MapServiceOptions` and `AUTOSAVE_INTERVAL` are in `options.rs`. `handle.rs` keeps `MapServiceHandle`, `PendingTransfers`, `MAX_*` and `is_valid_map_name`.
  - map-server `world_actor/`: methods called across its files are `pub(super)`; `join` and `player_intent` are `pub(crate)` because `service/tests.rs` calls them. `MapServiceHandle::shared` is `pub(super)` for the same tests.
  - map-server `claim/`: `Claim` is in `handle.rs`. `Write`, `WriteKind`, `RecentPlay`, `Recent` and `RecentMap` are in `write_dto.rs`; `RecentWrites` has its own `recent_writes.rs`. `writer.rs` holds only `apply`, `run_writer` and `RETRY_*`. `lock` is re-exported from `claim/mod.rs` under `#[cfg(test)]` only (`test_support.rs` uses it).
  - char-server: the actor reads `CharSharedState` and `RegisteredMapServer`, so they are `pub(super)` and live in `service/state_dto.rs`.
  - login-server `service/tests.rs` and the map-server `claim/tests.rs`, `service/tests.rs` declare their own `std`/`ro_core` imports (they used to come through `super::*`).
- **map-server `service.rs` (1,486 lines)** becomes the `service/` folder. Keep `pub mod service` so `lib.rs`'s re-export list stays the same.

  | File | Contents |
  | :--- | :--- |
  | `service/map_data.rs` | `MapData`, `MapDataLoader`, `RegistryPaths`, `load_*`, `fallback_grid`, `map_data_loader`, `disable_unreachable_warps`, `MAX_WIRE_MAP_NAME`, `FALLBACK_MAP_SIZE` |
  | `service/messages_dto.rs` | `Entry`, `JoinSession`, `SessionControl`, `MapHandoff`, `WorldCommand` |
  | `service/handle.rs` | `MapServiceHandle`, `MapServiceOptions`, `PendingTransfers`, `is_valid_map_name`, `MAX_HOSTED_WORLDS`, `MAX_PENDING_TRANSFERS` |
  | `service/world_actor/mod.rs` | `MapWorldActor`, its private `Session` and `Departure`, `new`/`run`/`tick`/`now_ms`, plus `TICK_INTERVAL`, `AUTOSAVE_*`, `MAX_COMMANDS_PER_WAKE`, `CHAT_SEPARATOR` |
  | `service/world_actor/save.rs` | `autosave`, `save_session`, `save_requested`, `hand_off` |
  | `service/world_actor/join.rs` | `handle_command`, `join`, `spawn_login`, `player_intent`, `leave`, `remove_session` |
  | `service/world_actor/routing.rs` | `flush`, `send_batches`, `encode`, `route`, `send_to_*` |

  Child modules can see the parent's private fields.
- **map-server `claim.rs` (743 lines)** becomes the `claim/` folder:
  - `claim/mod.rs`: re-exports
  - `claim/claim.rs`: `Claim` plus its `Drop`
  - `claim/shared_state.rs`: `SharedState`, `RecentWrites`, `Recent`, `RecentMap`, `lock`
  - `claim/writer.rs`: `Write`, `WriteKind`, `RecentPlay`, `apply`, `run_writer`, `RETRY_*`
  - `claim/map_repository.rs`: the trait and its blanket impl

  `claim/claim.rs` trips the `clippy::module_inception` lint. Name it `claim/handle.rs` or put `Claim` in `claim/mod.rs`.
- **map-server test fixtures:** today `claim.rs:337` is `pub(crate) mod tests`, and the `service` tests (BIN-WARP-07) import `claim::tests::{at, eventually, FakeRepo, ACCOUNT, CHAR, potions}`. Move `FakeRepo` (~160 lines), `at`, `potions`, `snapshot`, `eventually`, `CHAR` and `ACCOUNT` to `src/test_support.rs` (`#[cfg(test)] mod test_support;` in `lib.rs`) **before** splitting.
- **login-server:** `service.rs` becomes the `service/` folder:
  - `login_service.rs`: `LoginService`, `LoginServiceHandle`, `MAX_CHAR_SERVERS`
  - `session_actor.rs`: `LoginSessionActor`
  - `registry_dto.rs`: `ActiveSessionTicket`, `RegisteredCharServer`
  - `tests.rs`
- **char-server:** `service.rs` becomes the `service/` folder:
  - `service_handle.rs`: `CharServiceHandle`, plus its private `RegisteredMapServer` and `CharSharedState`
  - `session_actor.rs`: `CharSessionActor`
- In every bin, move the `config.rs` tests out. `session.rs`, `lib.rs` and `main.rs` stay.

### M7: tools
- [x] Done (120 tool tests pass; REF-02 diff empty; `structure_check.sh` exits 0; REF-04: every importer regenerates `crates/ro-db/data/` byte-identical, and its console report is unchanged). Deviations, mostly so that REF-03 passes:
  - skill, item, job and mob importers: `Converted` and `Report` are in `output_dto.rs` (two public types in `lib.rs` fail REF-03); `lib.rs` re-exports them.
  - skill-import: `JobAcc`, `EntryAcc` and `Jobs` are in `tree_acc_dto.rs` (`lib.rs` reads `Jobs::list` and `JobAcc::name`, so they are `pub(crate)`); `tree.rs` holds `merge_jobs` and `resolve_tree`. `RathenaFile` is in `raw_skill_dto.rs`; `parse_body` and `SLOTS` stay in `lib.rs`.
  - item-import: the build-only constants (`MAX_SLOTS` … `WEDDING_RING_*`) and `kept`/`small` are in `build.rs`; `take` is in `raw_dto.rs`; `jobs`/`classes` are in `names.rs`.
  - job-import: every constant (they are all `Acc` defaults) is in `acc.rs`.
  - mob-import: the constants, `same_name`/`named` and the mode/drop helpers are in `build.rs`.
  - npc-import: beyond the plan, `header.rs`'s 8 parse-result types are in `header_dto.rs`, `lex.rs`'s `Record`/`LexError` in `lex_dto.rs`, and `sprite.rs`'s `Sprite` in `sprite_dto.rs`. `Report::count` is `pub(crate)` (the importer calls it).
  - proxy: `args` and `logger::sequence_dto` are private modules (review fix); `lib.rs` re-exports `ProxyArgs` and `logger` re-exports the sequence DTOs. `main.rs` now imports `ProxyArgs` from the root. `config/tests.rs` imports `ProxyArgs` (it tests `build_config`).
  - Field types of `pub(crate)` raw structs are `pub(crate)` too (the `private_interfaces` lint).
- **The generated output must stay byte-identical (REF-04).**
- `skill-import/src/lib.rs` (939 lines) splits into:
  - `lib.rs`: the pipeline (`import`, `convert`, `render_*`, `write_output`, `attribution`) and the `Converted`/`Report` DTOs. Its pub items must keep being reachable at the root (REF-02).
  - `raw_skill_dto.rs`: `RathenaFile`, `RawSkill`, `RawRequires`, `PerLevel`, `RawElement*`, `RawItemCost`, the `Entry` trait plus the `entry!` macro (a macro must be defined before it is used; keep `#[macro_use]` or local ordering in mind)
  - `convert_skill.rs`: `convert_skill` and its helpers
  - `level_expand.rs`: `BadLevel`, `expand`, `MaxLevels`
  - `raw_tree_dto.rs`: `RawJob`, `RawTreeEntry`, `RawRequirement`, `Ordered`
  - `tree.rs`: `JobAcc`, `EntryAcc`, `Jobs`, `merge_jobs`, `resolve_tree`
- `item-import`:
  - `raw_dto.rs`: `RathenaFile`, `RawItem` plus `merge`, `Switches`, `RawFlags`, `RawStack`, `RawTrade`
  - `names.rs`: the ~15 name→enum mappers
  - `build.rs`: `build`, `is_consumable`
  - Move tests out of `script.rs`.
- `job-import`: `raw_dto.rs` and `acc.rs` (`Acc` and the table helpers).
- `mob-import`: `raw_dto.rs` and `build.rs`.
- `npc-import`: move `Report`, `MobReport` and `Imported` from `convert.rs` into `report_dto.rs`. Move tests out of `sprite.rs`.
- `refine-import`: fine as is.
- **proxy:**
  - `config.rs` becomes `args.rs` (`ProxyArgs`) and `config.rs` (`ProxyConfig`, `format_host_port`, `resolve_target`, port constants).
  - Move tests out of `logger/packet_logger.rs` (423 test lines, with its `reqname_frame` helper), `logger/recorder.rs`, `logger/session.rs`, `logger/resync.rs`, `forwarder.rs` and `config.rs`.
  - `logger/session.rs` has `PacketDirection`, `PacketSequenceEntry` and `SessionSequenceTracker`. Move the first two to `logger/sequence_dto.rs`.
  - `logger/recorder.rs`: move `GlobalSequenceEntry` to that same DTO file.
- Every tool's `main.rs` has the same duplicated `rathena_commit()`. Leave it; `bins`/`tools` can't share code without a new crate, which is out of scope.

### M8: integration tests, one binary per crate
- [x] Done. Every package's test-function names were listed before and after (`cargo test -p <pkg> -- --list`, last path segment); the lists are identical, 927 in all (943 with `--features lua`). Notes:
  - Module names drop the `_tests` suffix (`combat_tests.rs` → `engine/combat.rs`); `trait_tests` became `traits` (`trait` is a keyword).
  - Helpers were merged only when their text is identical (comments aside) **and** every local helper they call is merged for the same files. ro-engine: 78 helpers (mostly ids and protocol constants, plus `char_id`, `gid`, `tick`, `seen_by`, `intent`, `enter`, `walk`, ...) in `engine/common/{mod,consts}.rs`; 164 copies removed. `instance`, `items`, `send` and most `spawn_data` differ per file and stay in their modules, as do differing versions of merged names (a local definition shadows the import).
  - map-server: the old `tests/common/mod.rs` is split into `common/{client,consts,ids,loaders,packets,repo,server,wait}.rs`; the 14 helpers shared by its test files joined them. `#![allow(dead_code)]` is gone: in one binary every helper is used.
  - Split modules keep their helpers in `<module>/mod.rs`; children start with `use super::*;`. ro-persistence, npc-import and item-import (one big file each) became `common.rs` plus one module per group.
  - Left whole: `char_handshake_tests.rs` (plan), ro-engine `stats` (635) and `settings` (590), ro-persistence `char` (647, 16 tests in one area). Single-file crates (login, char, job, mob, refine, skill) keep `tests/<file>.rs`.
In each crate, turn `tests/*.rs` into `tests/<area>/main.rs`, where `<area>` is the crate name without `ro-`, e.g. `crates/ro-engine/tests/engine/main.rs` and `bins/map-server/tests/map_server/main.rs`. Cargo discovers `tests/<dir>/main.rs` automatically; no manifest sets `autotests` or `[[test]]`. Neither `verify.sh` nor any doc names a `--test` target, so the target rename breaks nothing.
- `main.rs` starts with `#![allow(clippy::unwrap_used, clippy::expect_used)]` and declares `mod common;` and one module per feature. Each former file becomes a module, as a folder if it gets split.
- **Deduplicate the helpers into `common/`.** In ro-engine, `char_id` exists ×16, `instance` ×8, `spawn_data` ×7, `items` ×7, `gid` ×7, `tick` ×6, `seen_by` ×6, and `intent`, `send`, `jobs`, `enter` ×5 each. Each file starts with 90–520 lines of helpers.
  - Merge only helpers that are identical.
  - Helpers that differ get distinct, descriptive names in `common/`, or stay in their module.
  - **Do not change what a test does.**
- `bins/map-server/tests/common/mod.rs` (808 lines, `#![allow(dead_code)]`) splits by purpose: `repo.rs` (`InMemoryRepo`), `loaders.rs`, `server.rs` (`TestServer`), `client.rs` (`Client`, frame readers), `packets.rs` (`cz_*` builders), `wait.rs`. `map_entry_handshake_tests.rs` is the only one of its 14 test files that does not use `common`.
- **Splits for files over ~600 lines:**

  | File | Lines | Split |
  | :--- | ---: | :--- |
  | ro-persistence `postgres_repo_tests` | 1,633 | auth, char, schema, inventory+equip, skill+trait+effects. Needs the db. |
  | ro-engine `effects_tests` | 1,589 | by ENG-EFF sub-range: cast, item use, heal. 519 lines of helpers before the first test. |
  | ro-engine `combat_tests` | 1,362 | by ENG-CMB group |
  | ro-engine `equipment_tests` | 1,169 | equip, unequip+ammo, gear-stat (ENG-STAT-01..03) |
  | ro-engine `map_instance_tests` | 1,141 | ENG-04, 07..12 by area |
  | ro-engine `skill_tests` | 949 | |
  | ro-packets `framing_tests` | 949 | login/char framing, map framing, table lengths |
  | ro-engine `trait_tests` | 928 | |
  | ro-engine `warp_tests` | 778 | |
  | ro-engine `level_command_tests` | 760 | |
  | npc-import `import_tests` | 748 | npc (TOOL-NPC) and mob (TOOL-MOB) |
  | item-import `import_tests` | 729 | |
  | ro-packets `map_unit_codec_tests` | 723 | |
  | map-server `bin_engine_tests` | 697 | entry+movement (BIN-01..09), server/config (BIN-SRV, BIN-CFG) |

  - Files between 500 and 600 lines may stay whole.
  - `char_handshake_tests` (700 lines, 4 tests) stays whole.
- Crates with a single small test file keep it as `tests/<file>.rs`. There is no benefit in nesting it.
- Test function names and scenario IDs do not change. REF-01 must match exactly after this milestone.
- The tests that only run with `RATHENA_PATH` set (`*_counts_match_catalog`: TOOL-ITEM-05, TOOL-MOBDB-03, TOOL-MOB-05) keep that gating.

### M9: Docs, review, commit
- [x] Done. Docs updated: ro-engine and ro-scripting `DESIGN.md`, `bins/CLAUDE.md`, `PACKET_FINDINGS.md`, `DEFERRED.md`, catalog `mechanics/job_change.md` and `scripting/lua_notes.md` (the ro-packets `CLAUDE.md` paths still exist). Older `specs/*/spec.md` and `brief.md` keep their paths as history. REF-01 927 passed, REF-02 empty diff, REF-03 exit 0, REF-04 identical, `./verify.sh --smoke` ok. The reviewer found no Critical or Important issues; its three Minor fixes are applied (proxy modules private again, three needless widenings reverted, one lost doc restored).
- Update the live docs:
  - `crates/ro-engine/DESIGN.md` §1a (lines ~16–32) and its other path references (~196, 649, 659, 664; about 15 in all)
  - `bins/CLAUDE.md` (`session.rs`, `crates/ro-core/src/http.rs`)
  - `crates/ro-packets/CLAUDE.md` (`src/table.rs`, `src/coords.rs`)
  - `PACKET_FINDINGS.md`
  - the `reference/catalog` entries that cite moved files (e.g. `db/exp_tables.md` → `job_ids.rs`)
- Leave `specs/*/plan.md` alone; they are history.
- Run REF-01 to REF-04, `./verify.sh` with the db up, and `./verify.sh --smoke`.
- Run the `reviewer` agent over `git diff HEAD -M` plus untracked files, with this plan as the spec. Apply its fixes, and re-run the checks.
- Make one commit.

## Notes for executing sessions
- **Sibling visibility.** Child modules see their parent's private items; siblings don't. A feature's `systems.rs` touching `MapInstance` fields needs those fields to be `pub(crate)` (most already are). Helpers called across facets become `pub(crate)`. Widen only as far as the compiler requires.
- **Order inside a split:** create the new files, then move code, then fix `use` lines, then run `./verify.sh -p <crate>`. cargo fmt reflows code, so keep Edit anchors short. For large mechanical moves a line-range script (`sed -n 'a,bp'`) is fine. Re-read line numbers after each move, because they shift.
- **Shared test fixtures** (`inventory::tests`, `claim::tests`) move to `test_support` modules first, or the dependent test modules break mid-split.
- **The DB** must be up for ro-persistence and bins tests: `docker compose -f tools/docker/docker-compose.yml up -d db`.
- **Session split (recommended):** M1–M3 in one session (small crates; this sets the pattern), M4–M5 in one (ro-engine, the largest and riskiest), and M6–M9 in one. Each session starts from this file and ticks off what it finished.
