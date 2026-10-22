# Implementation Plan: Engine Foundation (movement first)

Spec: `specs/engine-foundation/spec.md`. This plan is self-contained so a fresh session can execute
it from M1. M0 (research + spec rewrite) is done.

## Current state (as of M0)
- `ro-engine` and `ro-db`: empty crates (only `pub use ro_contracts`).
- `ro-contracts`: **none** of the spec §1 changes exist yet. (An earlier version of this plan claimed
  they did. It was wrong.)
- `ro-packets`: CZ decoders for move/dir/act/reqname exist and are wired in
  `incoming/frame.rs` (`Phase0IntentParser`, aliased `ModernIntentParser`). The ZC encoders in
  `outgoing/map.rs` hardcode speed/job/sex, put char_id in AID, and miss `0x009c`, `0x0a30`, and `0x0088`.
  `table.rs` has `0x0091` wrong (18 → 22) and `0x0af7` wrong (Dynamic → Fixed(32)), and lacks `0x009c`, `0x0a30`, and `0x0088`.
- `bins/map-server/src/service.rs`: `MapWorldActor` keeps stale positions in `ConnectedPlayer.pos`,
  echoes the client tick on `RequestTime` (bug), and filters AoI itself. The existing tests in
  `bins/map-server/tests/` assert some old behaviour (for example `EntityAppeared` to self on ACTORINIT)
  and must be updated to the spec.

## Decisions (settled with the user; don't reopen)
| Topic | Decision | Why |
| :--- | :--- | :--- |
| Wire truth | rAthena 20260219 via `reference/catalog/` | Captures and rAthena agree; the old spec had wrong packets |
| Entry | Sync `MapEngine::spawn_character(CharacterSpawn, now_ms)`; `EnterMap` never reaches the engine | `EnterMap` has no appearance data, and duplicate-entry errors can't surface from inside `step_tick` |
| Visibility | Entity created at spawn, put in the grid and announced at `NotifyActorInit` | rAthena's `map_addblock` runs in LoadEndAck |
| Redirect | rAthena style (§0.3); sub-cell always (8,8) | The client expects it, and it's simpler |
| AoI routing | Engine resolves AoI recipients at emission into `Characters(list)` and per-observer `SelfOnly` (changed in M5: resolving after the tick double-sent entries) | Positions live only in the engine; matches rAthena `clif_send` |
| Map data | `MAP_DATA_DIR`; missing `.gat` → warn + 512x512 all-walkable `MemoryMapGrid` | Smoke tests run without client data |
| Fixture | `MemoryMapGrid` is a plain public ro-db type | Shared by tests and the runtime fallback |
| Clock | Actor-owned monotonic `now_ms`; wire tick = `now_ms as u32` | One time source |
| Bad direction values | Decoder returns `InvalidField`; session closes on `InvalidField` | CLAUDE.md: drop the session on bad input |
| Engine scope | Single-threaded subset of `DESIGN.md` (intents → path → step → commit + AoI → emit). No rayon, status, or combat | Foundation only |
| Deferred | See spec §6 | User chose scope |

## Working rules for every milestone
1. Write the tests first. Name them with spec IDs (for example `eng_05_wall_destination_rejected`).
2. Iterate with `./verify.sh -p <crate>`. Read `target/verify/<step>.log` on failure.
3. When done, run the `reviewer` agent on the milestone diff and the spec path. After M4, M5, and M6, also
   run `/code-review`. If `reviewer`/`metaling-explorer` don't load as agent types (seen in the M0 session), run a
   `general-purpose` agent and tell it to follow `.claude/agents/<name>.md`; use `model: sonnet` for metaling-explorer.
4. A wire fact not in the catalog → metaling-explorer first, then code.
5. Commit per milestone on a branch (ask the user before committing).

## M1 — `ro-contracts` (spec §1)
- `src/engine.rs`: the `MapEngine` trait and the new `EngineError` variants.
- `src/character.rs`: `CharacterSpawn` (+ `From<(&CharacterSummary, AccountId)>` or similar).
- `src/map.rs`: fix the `CellType` variants and walkability (unknown values are not walkable).
- `src/packet.rs`:
  - add `exclude` to `AreaOfInterest`, and update `OutboundPacket::broadcast_aoi`
  - add `UnitAppearance`
  - the new `ServerEvent` variants
  - remove `EntityMoved`/`EntityAppeared`/`EntityVanished`
- `src/packet/opcode.rs`: rename/add per spec §1.4. Fix the `name()` table and its tests.
- Fix any compile fallout in `ro-packets` and `map-server` minimally: stub the encoder arms with
  `UnsupportedEvent` if needed, and leave the real work to M3/M6.
- Verify: `./verify.sh` (the whole workspace must still build).

## M2 — `ro-db` (DB-01..04)
- `src/map.rs`: `GatMapGrid` (`from_bytes(label, &[u8])`, `load(path)`, flat row-major `Vec<u8>` or
  `Vec<CellType>`) and `MemoryMapGrid` (`all_walkable`, `set`).
- Tests go in `src/map.rs` unit tests or `tests/gat_tests.rs`. Build `.gat` bytes in the test.
- Verify: `./verify.sh -p ro-db`.

## M3 — `ro-packets` (PKT-01..13)
- First, run metaling-explorer on `0x0088` ZC_STOPMOVE layout/length at 20260219 and on the `0x00b0` var ids for
  weight / max weight (`SP_WEIGHT`, `SP_MAXWEIGHT`). Add both to `reference/catalog/packets/map_units.md`.
- `table.rs`: `0x0091` → 22, `0x0af7` → Fixed(32); add `0x009c`, `0x0a30`, and `0x0088`; fix comments. Update the
  table tests, including the unmapped list (`0x009c` leaves it; `0x0086` stays).
- `outgoing/map.rs`:
  - build the entry packets from `UnitAppearance` (AID = gid, GID = char_id or 0, with real speed/job/sex/hair/head dir/body state/clevel/name)
  - add encoders for `0x009c`, `0x0a30`, `0x0088`, and `0x0091`
  - reuse `coords.rs` and `helpers::write_fixed_string`
- `outgoing/encoder.rs`: dispatch every new `ServerEvent`. Update `is_implemented_outbound_opcode`.
- `incoming/map.rs`: `CzChangeDirectionPacket` validates the ranges → `InvalidField`.
- Verify: `./verify.sh -p ro-packets`. Check the byte offsets against `map_units.md`.

## M4 — `ro-engine` core (ENG-01..06)
- New modules, all `pub(crate)` unless tests need them:
  - `entity.rs`: `Entity`, `EntityAllocator`
  - `sparse_set.rs`
  - `world.rs`: dense `positions`/`kinds`/`body_state`/`head_dir`/`speed`/`appearance`, `walking_paths`, `char_to_entity`, `gid_to_entity`
  - `spatial.rs`: `SpatialGrid8x8`, `query_radius`
  - `path.rs`: `AStarScratchBuffer` with rAthena costs/heuristic/corner rule, the last-row/col rule, and `MAX_WALKPATH`; scratch reused across calls
  - `movement.rs`: `WalkingPath`, `step_duration_ms`
- Keep the hot path cache-friendly: no per-tick allocations, and reuse event and scratch buffers.
- Update `crates/ro-engine/DESIGN.md`: add a "Foundation subset (implemented)" section, and mark Stage 1/3c,
  rayon, and the combat tiers as future. Fix its `0x0086`/`ZC_NOTIFY_MOVE` mention per the catalog.
- Verify: `./verify.sh -p ro-engine`.

## M5 — `ro-engine` `MapInstance` (ENG-07..12)
- `engine.rs`: `MapInstance { world, walkability: Arc<dyn MapWalkability>, intents: Vec<SessionIntent>, scratch, events }` implementing `MapEngine`.
- `step_tick` order (changed in M5 so a click sees steps that ended before it):
  1. advance steps (loop while `now >= next_step_at`); per completed step: move in the grid, run the
     visibility diff (§0.4), and do the pending-redirect repath
  2. drain intents FIFO (last move wins per tick)
  3. validate and plan paths (`max_walk_path` 17)
  4. append events to the caller's buffer
- Also: sit/stand, direction, name (range 14), time sync, and `remove_character`.
- Verify: `./verify.sh -p ro-engine`.

## M6 — `bins/map-server` (BIN-01..09, BIN-SRV-01..04/06, BIN-CFG-01)
- `config.rs`: `MAP_DATA_DIR` (optional).
- `service.rs`:
  - `MapServiceHandle::new` takes a walkability loader so tests can inject a `MemoryMapGrid`.
  - The actor owns a `MapInstance` and an `Instant`.
  - `JoinSession` → `char_repo.find_by_id` → `spawn_character`. On error, close the session, and send no `ZC_AID`.
  - Tick → `push_intent`s → `step_tick(now_ms)` → dispatch (spec BIN-SRV-03).
  - `LeaveSession`/eviction → `remove_character` + persist `character_position`.
  - Remove `ConnectedPlayer.pos` and the actor-side AoI filter.
  - `RequestTime` is handled by the engine.
- `session.rs`: close on `InvalidField`. Keep the `EnterMap` handshake path and send nothing before join succeeds.
- Tests: update `tests/map_entry_handshake_tests.rs` / `map_handshake_tests.rs`, and add multi-client tests
  for BIN-02..09. Use the in-memory repo and the injected grid. DB-backed persistence tests stay as they are.
- Verify: `docker compose -f tools/docker/docker-compose.yml up -d db`, then `./verify.sh` and `./verify.sh --smoke`.

## Final checks
- Every spec ID appears in at least one test name (`grep -rn "eng_\|pkt_\|bin_\|db_"`).
- Manual check: run real client sessions through `tools/proxy` against metaling and rAthena (two clients walking
  in and out of view, sit, turn, name), and diff the captures in `tools/proxy/packet_captures/`.
