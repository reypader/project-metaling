# Implementation Plan: Mob spawning

Spec: `specs/mob-spawning/spec.md`.

## Decisions (settled with the user)
| Topic | Decision |
| :--- | :--- |
| Scope | Permanent spawn lines only. Placeholder stats (1 HP, level 1 unless the line gives one, speed 150). No movement, AI, death or respawn. Movement/AI findings go to `DEFERRED.md`. |
| Data layout | A new dataset `crates/ro-db/data/mob_spawn/<map>.yml` (`MOB_SPAWN_DB` v1), written by the existing `tools/npc-import`. It has its own `MOB_SPAWN_DATA_DIR`. NPC files are unchanged. |
| Placement | rAthena's `mob_spawn` placement at default config, using the instance's existing `SplitMix64`. No `rand` crate. |
| Mob gids | Shared per-instance counter with NPCs, from `START_NPC_ID`. |
| `MobId` home | `ro-db`, like `NpcState`. The engine already depends on `ro-db`, and the wire uses a plain `u16` `job`. |
| `0x0adf` for mobs | Reuse `ServerEvent::AckNpcName`. |

## Notes for the implementer
- **Patterns to copy:**
  - `ro-db`: `crates/ro-db/src/npc.rs`. It uses private `Raw*` serde structs with `deny_unknown_fields`
    and `TryFrom` validation, and writes YAML 1.2 (`yaml_12 = true`).
  - Tests: `crates/ro-db/tests/npc_tests.rs`. `db_npc_03` finds the data via
    `env!("CARGO_MANIFEST_DIR")`.
- **Importer:**
  - The lexer splits a header with `splitn(4, '\t')`, so a trailing `\t//comment` ends up inside w4. The
    w4 parse must stop at the first tab.
  - The mob_db reader in `tools/npc-import/src/sprite.rs` needs `DuplicateKeyPolicy::LastWins`, because
    rAthena's mob_db repeats keys. Keep that when adding `Name` / `JapaneseName`.
  - `tool_npc_03_scripts_and_shops` asserts monster skips; update it.
  - `tool_npc_07_counts_match_catalog` (835 files, 838 NPC maps) must still pass.
- **Engine:**
  - Follow `MapInstance::spawn_npc` (`crates/ro-engine/src/engine.rs`). Bump `next_npc_id` only after a
    successful spawn, and call `world.show(e)` to put the unit in the grid.
  - Observers are already filtered with `players.contains` in `step` and `aoi_recipients`, so mobs get no
    events without extra code.
  - `gid()`, `appearance()` and `name_request` try players first, then NPCs. Add mobs as a third branch.
  - `SplitMix64` is seeded from the map name and shared with warp random-cell resolution. Mob placement
    consumes values at spawn, which changes the random warp cells that follow. That is fine, but check
    whether any warp test pins exact random cells.
- **Map-server:** `bins/map-server/tests/common/mod.rs` has `data_loader(walls, npcs)` and
  `TestServer::start_with_loader`. Add a mobs argument and keep the NPC tests unchanged.

## Milestones
Write tests first, named with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 research, spec, plan:** catalog `mechanics/mobs.md` §1.5 fix, §1.6 and §1.7, plus an `INDEX.md`
  row. This spec and plan.
- [x] **M1 ro-db**
  - New `mob_spawn.rs`:
    - `MobId(pub u32)`, `MobSpawn`, `MapMobSpawns { spawns }`;
    - `from_yaml_str`, `load_yaml`, `to_yaml_string`;
    - the `MOB_SPAWN_DB_TYPE` / `MOB_SPAWN_DB_VERSION` constants.
    - Re-export them from the root.
  - Move the header check and the "missing file is `Ok(None)`" I/O out of `npc.rs` into one private
    helper that both datasets use.
  - Tests DB-MOB-01/02 in `crates/ro-db/tests/mob_spawn_tests.rs`.
  - `DESIGN.md` §2.4 and §3: the mob spawn pipeline; replace the "Future: MobSpawnBlueprint" comment and
    the "MobId goes in ro-core" note.
- [x] **M2 tools/npc-import**
  - `header.rs`: `Header::Mob(MobHeader)` replaces `SkipReason::Monster` / `BossMonster`, and parses
    w1, w3 and w4 (TOOL-MOB-01..04).
  - `sprite.rs`:
    - read the mob_db `Name` / `JapaneseName`;
    - `resolve_mob(token) -> Option<u32>`;
    - name lookup for `--en--` / `--ja--`.
  - `convert.rs`: `finish` also returns `BTreeMap<String, MapMobSpawns>`. Report the counts, the skips
    and the largest map total.
  - `lib.rs` / `main.rs`: make `render_map_file` / `write_output` work for each dataset, and add
    `--mob-out`.
  - Tests TOOL-MOB-01..05 in `tests/import_tests.rs` (inline samples). Update
    `tool_npc_03_scripts_and_shops`, which expects monster skips.
  - Generate the data in its own commit. Pass an expanded path to `--rathena`, because `$RATHENA_PATH`
    holds a literal `~`.
  - `NOTICE.md`: add a "Mob spawn lists" section and update the NPC section.
  - Add the DB-MOB-03 test.
  - Check that the largest map total plus that map's NPCs stays well under `DEFAULT_ENTITY_CAPACITY`
    (4096).
- [x] **M3 ro-core + ro-packets:** add `UnitObjectType::Mob`, mapped to `0x05` in `object_type()`
  (`crates/ro-packets/src/outgoing/map.rs`). PKT-MOB-01 in `tests/map_unit_codec_tests.rs`.
- [ ] **M4 ro-engine**
  - Move `SplitMix64` from `warp.rs` into `rng.rs` and add `below(n)` / `range_inclusive(a, b)`.
  - `EntityKind::Mob`, `MobInfo { gid, mob_id, name, level, hp, max_hp, spawn_group }`,
    `World.mobs: SparseSet<MobInfo>`, `World::spawn_mob`. Clean up mobs in `despawn`.
  - New `mob.rs`: `SpawnGroup` (the normalized line), placement (ENG-MOB-02/03), and `spawn_mobs`.
  - A mob branch in `gid()`, `appearance()` and `name_request`.
  - Placeholder constants: `PLACEHOLDER_MOB_HP`, `PLACEHOLDER_MOB_LEVEL`, `DEFAULT_MOB_SPEED`.
  - Tests ENG-MOB-01..05 in `crates/ro-engine/tests/mob_tests.rs`.
  - `DESIGN.md` §1a.
- [ ] **M5 map-server**
  - `MapConfig.mob_spawn_data_dir` (`--mob-spawn-data-dir` / `MOB_SPAWN_DATA_DIR`), `MapData.mobs`,
    `load_mob_spawns`, and the `map_data_loader` argument.
  - `MapWorldActor::new` calls `spawn_mobs` after `spawn_npcs` and logs both counts.
  - `main.rs` warns when the directory is unset. Export it in `run_metaling_servers.sh`.
  - `tests/common` `data_loader` takes mobs. BIN-MOB-01 in `bins/map-server/tests/mob_tests.rs`.
- [ ] **Finish**
  - Run the `reviewer` over the full diff.
  - `DEFERRED.md`: add a mob-spawning section (spec §4 and catalog §1.7), and remove "mob spawns" from the
    npc-spawning list. The section must include the event labels: they are stored but never run, and
    they need death and scripting (spec §4).
  - `./verify.sh --smoke`.
  - Optional real-client check: walk `prt_fild08` against metaling and rAthena.
