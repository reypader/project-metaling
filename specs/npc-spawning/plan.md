# Implementation Plan: NPC spawning

Spec: `specs/npc-spawning/spec.md`. M0 (research, catalog, spec) is done.

## Decisions (settled with the user)
| Topic | Decision |
| :--- | :--- |
| Convert to YAML now | Yes. The rAthena grammar lives only in `tools/npc-import`; runtime reads YAML via serde. |
| Output | Committed, `crates/ro-db/data/npc/<map>.yml`, GPL-3.0 with LICENSE + NOTICE; repo is mixed-license. |
| Scope | Positioned warp/warp2/script/duplicate/shop-family. No behavior, no mobs, no mapflags. |
| YAML crate | `serde-saphyr` 1.3 (pure Rust, panic-free parsing, active; `serde_yaml` archived, `serde_yaml_ng` idle since 2024). |
| NPC gids | Per-instance counter from 110000000. |
| Names on wire | `Arc<str>` in `UnitAppearance` (was a DEFERRED perf item). |

## Milestones
Write tests first and name them with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [ ] **M1 ro-db**: `npc.rs` (types with Serialize + Deserialize, `MapNpcs::load_yaml`, `from_yaml_str`), `NpcDataError`, DB-NPC-01/02 tests, `DESIGN.md` §2.4/§3 update.
- [ ] **M2 tools/npc-import**: TOOL-NPC-01..07 with `.txt` fixtures. Then generate the data and add the licensing files, README, root LICENSE, Cargo `exclude` and the CLAUDE.md rule. Add the DB-NPC-03 test. The data commit is separate from the code commit.
- [ ] **M3 ro-core + ro-packets**: `ObjectType` and `effect_state` on `UnitAppearance`, `name: Arc<str>`, the NPC name event plus `0x0adf` encoder/opcode/table. PKT-NPC-01/02.
- [ ] **M4 ro-engine**: `EntityKind::Npc`, `NpcInfo`, `WarpTrigger`, `spawn_npcs`, and kind dispatch in `gid`/`appearance`/`name_request`. ENG-NPC-01..05.
- [ ] **M5 map-server**: `npc_data_dir` config, `MapDataLoader`, `run_metaling_servers.sh`. BIN-NPC-01.
- [ ] **Finish**: `reviewer` over the full diff, `DEFERRED.md` (spec §4), `./verify.sh --smoke`.
