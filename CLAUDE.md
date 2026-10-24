# Project Metaling

Clean-room Ragnarok Online server in Rust with a custom ECS. Targets one client version: **`PACKETVER = 20260219`** (`ro_core::TARGET_PACKETVER`). No backwards compatibility.

## Layout

| Path | Role | Design doc |
| :--- | :--- | :--- |
| `crates/ro-core` | Vocabulary shared by more than one crate: IDs, DTOs, client intents and server events, internal HTTP DTOs | — |
| `crates/ro-packets` | Wire framing, length table, intent parsing, event serialization | — |
| `crates/ro-engine` | Map ECS runtime (tick schedule, movement, AoI) | `DESIGN.md` |
| `crates/ro-db` | Static data: YAML registries, `.gat` walkability, spawns. Read-only at runtime. | `DESIGN.md` |
| `crates/ro-mechanics` | Pure stat/damage formulas | — |
| `crates/ro-persistence` | PostgreSQL via `sqlx`; schema/seed in `sql/` | `DESIGN.md` |
| `crates/ro-scripting` | NPC/item script VM adapter (non-blocking, yields to the tick) | `DESIGN.md` |
| `bins/{login,char,map}-server` | Process bootstrap, TCP sessions, internal Axum HTTP | — |
| `tools/proxy` | Packet logging proxy (dev-only; nothing in `bins/` or `crates/` may depend on `tools/`) | `README.md` |
| `specs/<feature>/` | `spec.md` + `plan.md` for in-flight work | — |
| `reference/catalog/` | Curated notes extracted from reference repos; start at `INDEX.md` | — |

Read the crate's `DESIGN.md` before changing its architecture. Some sections describe future phases.

## Commands

```bash
./verify.sh                  # fmt -> clippy -> tests, compact summary; full logs in target/verify/*.log
./verify.sh -p ro-packets    # scope to the crate(s) you touched (repeat -p); use this while iterating
./verify.sh --smoke          # also boots all 3 servers + proxy via ./run_metaling_servers.sh test
docker compose -f tools/docker/docker-compose.yml up -d db   # PostgreSQL; bins/* and ro-persistence tests need it
```

Use `./verify.sh` instead of raw `cargo test`/`clippy`/`fmt`, which produce much longer output. When it reports a failure, read the specific lines in `target/verify/<step>.log` rather than re-running cargo. Without the db running, DB-backed tests fail with "pool timed out" after about 30s each.

`./run_rathena_servers.sh` runs real rAthena (from `$RATHENA_PATH`). Connect a client through `tools/proxy` to record ground-truth traces into `tools/proxy/packet_captures/`. When rAthena source and a capture disagree, trust the capture. `*.log` is gitignored. A capture the catalog cites goes into `reference/catalog/captures/` as `.txt`, gets a row in its `README.md`, and has rAthena message text and PINs redacted.

## Reference repos

Located via env vars only: `RATHENA_PATH`, `RUSTRO_PATH`, `RATHENA_SCRIPT_INTERPRETER_PATH`. If one is unset, ask the user. `reference/` holds only our own `catalog/`.

## Licensing (MIT code, GPL-3.0 data)

The repo is MIT except `crates/ro-db/data/`, which holds GPL-3.0 data derived from rAthena. No other path may contain rAthena-derived content. See `README.md` and `crates/ro-db/data/NOTICE.md`.

- **Copying data from rAthena** (db YAML, NPC/script files, constant tables, anything else): only via an import tool `tools/<dataset>-import`, writing under `crates/ro-db/data/<dataset>/`.
  - Each generated file starts with `# SPDX-License-Identifier: GPL-3.0-only` and an attribution comment: rAthena URL, commit, source files, "Modified: … by `tools/<dataset>-import`", and a pointer to `crates/ro-db/data/NOTICE.md`.
  - Add or update the dataset's section in `NOTICE.md`: source paths, commit, date, what changed, and the regenerate command.
  - Never hand-edit generated files; regenerate them.
  - Add an `ro-db` test that every committed file loads and has the header (like `db_npc_03_shipped_data_loads`).
- **Keep it out of binaries and packages:**
  - rAthena-derived data is read from disk at runtime, from a configured directory like `NPC_DATA_DIR`.
  - Never pull it into `crates/` or `bins/` code through `include_str!`/`include_bytes!`, `build.rs` codegen, or generated `const` tables.
  - `ro-db` keeps `exclude = ["data/"]`.
  - Tests may read the committed files from disk. Test fixtures must be our own small hand-written samples.
- **Code is clean-room:**
  - Never copy or line-by-line translate code from rAthena (GPL) or rathena-script-lang-interpreter (no license). This covers `crates/`, `bins/` and `tools/` (importers included).
  - Write from `reference/catalog/` notes. Facts are fine: opcodes, layouts, formulas, constant values. Code structure and comments are not.
  - rust-ro is MIT: if you copy from it, keep its copyright notice in the file.
- **Dependencies** in `crates/`, `bins/` and `tools/` must be MIT-compatible (no GPL/AGPL crates).
- When a new reference repo or data source comes into use, credit it in the README's Acknowledgements.

## Rules

- **Reference lookups:** read `reference/catalog/INDEX.md` and the linked entry directly. Spawn `metaling-explorer` only when the catalog doesn't answer the question, or for web research; it writes new findings back. Don't browse the reference trees or the web from the main session. Batch independent lookups into parallel `metaling-explorer` calls.
- **Scripts:** Data extraction from reference repos goes to `metaling-explorer`. Other exploratory or large-output scripting where only the result matters goes to a general-purpose subagent with `model: sonnet`. Keep scratch files out of the repo.
- **Targeted Changes and New Files:** Use the Edit and Write tool instead of inline scripts for simple, targeted changes or new files.
- **Review once per feature:** when a feature's `plan.md` is complete, run the `reviewer` agent once over the full diff. Review a milestone early only if later milestones build on something risky: an API between crates, shared state or concurrency in the map-server, persistence schema, or auth. Run `/code-review` only when asked. For small edits, `./verify.sh` plus self-review is enough.
- **Commit once per plan, not per milestone:** leave a plan's work uncommitted until `plan.md` is complete, `./verify.sh` is green, and the `reviewer` has gone over the working-tree diff (`git diff HEAD` plus untracked files) and its fixes are applied. Then commit. The reviewer gets one diff, and history only holds reviewed code. Separate cleanup commits (e.g. pre-existing lint) are still fine. The plan is considered complete even before the user's manual verification and re-capture of any logs. Any further changes will be considered fixes.
- **Crate dependencies:** `ro-core` is the shared base; `ro-db`, `ro-packets` and `ro-persistence` depend at most on `ro-core` (`ro-db` needs nothing today); `ro-engine` → `ro-core`, `ro-db` (later `ro-mechanics`, `ro-scripting`). Bins wire these together: login/char use core + packets + persistence, and map adds db + engine. `tools/proxy` uses core + packets. A type, error or trait lives in the crate that owns it, and goes in `ro-core` only when 2+ crates share it. Add a trait only when there's a second implementation (test doubles count). When a peer needs to call back, define the trait in the crate that is depended on (e.g. a world-query trait in `ro-scripting` that `ro-engine` implements); never add a reverse dependency.
- **One import path per item:** library crates keep their modules private (`mod x;`, not `pub mod x;`) and re-export their public API at the crate root. Import from the root (`ro_core::ClientIntent`, `ro_packets::PacketDecoder`), never through a module path.
- **No `unwrap()`/`expect()` in production code** (enforced by the workspace `clippy::unwrap_used`/`expect_used` lints; `clippy.toml` exempts tests). On bad client input, drop that session instead of panicking the server, as long as shared state stays consistent. New crates need `[lints] workspace = true`. Integration test files with helper fns need a file-level `#![allow(clippy::unwrap_used, clippy::expect_used)]`.
- **In tests, prefer `.expect("what should have happened")`** over bare `.unwrap()` so failures explain themselves.
- **map-server hot path:** prefer cache locality and avoid moves/clones in the tick loop unless needed.
- New feature work: write `specs/<feature>/spec.md` and `plan.md` first. Spec scenarios use prefixed IDs (`BIN-`, `PKT-`, `ENG-`, `DB-`, …) that tests reference.
- **plan execution:** Always add a user checkpoint after the Milestone-0 or M0 of a plan. Executing M0 will always be done in the same session that created the plan. However, the actual implementations for M1 onwards will always be executed from a new session. Always include nuanced information in the plan that may be lost on the new session.
