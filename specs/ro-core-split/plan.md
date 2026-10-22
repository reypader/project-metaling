# Replace `ro-contracts` with a slim `ro-core` + crate-owned types

## Context

`ro-contracts` (3.3k lines) started as a frozen interface hub so parallel agents could work on separate crates
("Owned exclusively by the Main Orchestrator Agent"). Today every library crate depends only on it, and its
cycle-prevention role is not needed. Without it, dependencies run one way naturally:
engine → db, and bins wire persistence to the engine.

**Assessment: drop the hub, keep a small shared-vocabulary crate.** This matches how Rust workspaces are
usually laid out: a small `*-core` crate for types that more than one crate shares; each crate owns its own
errors, traits and implementation details; a trait lives with its consumer or implementor and exists only
when there is a real second implementation (including test doubles).

Evidence from the code:
- **About 2/3 of the crate belongs to one other crate.** The opcode table (`packet/opcode.rs`, 2,000 lines),
  `RawPacket`, `PacketLength` and `PacketCodecError` are used only by ro-packets, the proxy and bins.
  `CellType`, `MapDataError` and `DbLoadError` belong to ro-db. `EngineError` belongs to ro-engine.
  Any edit to these rebuilds the whole workspace.
- **Most traits are unused as abstractions.** `MapEngine` has one implementation, and map-server holds
  `MapInstance` directly. `EventEncoder`, `PacketIntentParser` and `PacketLengthTable` are always named by
  their concrete types. The real swap points are only the repository traits (test doubles in bins) and
  `MapWalkability` (test grids in ro-engine).
- **`OutboundPacket`/`OutboundTarget` are dead code** (used only in a contracts test).
- **Dependency hygiene.** login-server and char-server depend on all 7 crates but use only core, packets
  and persistence. ro-mechanics and ro-scripting are stubs that just `pub use ro_contracts`.

Decisions (confirmed with user): optimize for idiomatic Rust over the agent hub; name the crate `ro-core`;
move repository traits into ro-persistence; delete the `MapEngine` trait.

## Target dependency graph

```
ro-core            ids, domain DTOs, intent/event vocabulary, internal HTTP DTOs, TARGET_PACKETVER
├── ro-packets     opcodes, framing, length table, intent parser, event encoder
├── ro-db          CellType, MapWalkability, grids, load errors
├── ro-persistence repo traits + errors, PostgresRepo
├── ro-mechanics   (stub)
├── ro-scripting   (stub)
└── ro-engine  ──► ro-db          (later: ro-mechanics, ro-scripting)
bins: login → core, packets, persistence
      char  → core, packets, persistence
      map   → core, packets, persistence, db, engine
tools/proxy → core, packets
```
Rule: library crates never depend on bins or tools. A crate may depend on a peer only in the
direction shown. Future engine↔scripting needs are met by a trait owned by ro-scripting
(e.g. a world-query trait) that ro-engine implements, so no cycle forms.

**Outcome:** after step 7, ro-db no longer needs ro-core at all, so today it has no metaling
dependencies (it may later use ro-core IDs). ro-mechanics and ro-scripting have no dependencies until they get code.

## Where each item goes

| Item (current `ro-contracts` file) | New home |
| :--- | :--- |
| `ids.rs` (AccountId, CharId, SessionId, Sex, MapPosition, CharacterSummary, CharServerInfo; unused ItemId and SessionTicket deleted later) | `ro-core::ids` |
| `character.rs`: CharacterSpawn, DEFAULT_WALK_SPEED, CreateCharacterRequest | `ro-core::character` |
| `character.rs`: CharacterRepository, CreateCharacterError, DeleteReserveOutcome, CancelDeleteOutcome | `ro-persistence` (e.g. `src/repository.rs`) |
| `account.rs`: Account, AccountState | `ro-core::account` (DTO shared with login-server) |
| `account.rs`: AccountRepository, AuthError | `ro-persistence` |
| `repository.rs`: RepositoryError | `ro-persistence` |
| `http.rs` (all internal HTTP DTOs) | `ro-core::http` |
| `packet.rs`: ClientIntent (+ redacting Debug), SessionIntent, ServerEvent, ScopedServerEvent, BroadcastScope, UnitAppearance | `ro-core::message` |
| `packet.rs`: RawPacket, PacketLength, MAX_PACKET_SIZE, PacketCodecError, PacketLengthTable, PacketIntentParser, EventEncoder; `packet/opcode.rs` + its unit tests | `ro-packets` |
| `packet.rs`: OutboundPacket, OutboundTarget, `From<BroadcastScope>` | delete (dead) |
| `map.rs`: CellType, MapWalkability, MapDataError, DbLoadError | `ro-db` |
| `engine.rs`: EngineError | `ro-engine` |
| `engine.rs`: MapEngine | delete; move its doc comments onto `MapInstance`'s methods (`crates/ro-engine/src/engine.rs:611`) |
| `TARGET_PACKETVER` | `ro-core` root |

Notes:
- `ServerEvent` carries only `u8`/`u32` wire codes, so moving `AuthError`/`CreateCharacterError` out of
  core leaves no back-reference. Their `wire_error_code()` and `wire_fields()` methods move with them as-is.
- Check whether ro-packets' trait set is still worth keeping as traits. The proxy uses
  `&impl PacketLengthTable` (`tools/proxy/src/logger/resync.rs:46`) and `PacketDecoder<T>` is generic, so
  keep `PacketLengthTable`. `EventEncoder` has two implementations (Phase0/Modern), so keep it.
  Keep `PacketIntentParser` too for now; only moving it is in scope.
- Keep `pub use module::*` re-exports at the `ro-core` root so most imports become a plain
  `ro_contracts` → `ro_core` swap.

## Steps (each leaves `./verify.sh` green; one commit per step)

0. Copy this plan to `specs/ro-core-split/plan.md` (CLAUDE.md asks for specs before feature work). This
   is a refactor with no behavior change, so the spec is one paragraph and the moved tests keep their IDs.
1. **Rename the crate.** `git mv crates/ro-contracts crates/ro-core`. Update the root `Cargo.toml` members
   and `[workspace.dependencies]`, every `Cargo.toml` (`ro-core.workspace = true`), and all
   `ro_contracts::` → `ro_core::` (sed over `crates bins tools`). Update the crate doc in `lib.rs`.
2. **Packets wire types → ro-packets.** Move `packet/opcode.rs` and the wire items listed above into ro-packets
   (e.g. `src/wire.rs` + `src/opcode.rs`), re-exported at the crate root. Fix imports in ro-packets,
   bins and the proxy. Move `bytes`/`io` deps accordingly. Delete `OutboundPacket`/`OutboundTarget`.
   Rename the rest of `packet.rs` to `ro-core/src/message.rs`.
3. **Map types → ro-db, engine depends on ro-db.** Move `map.rs` contents into ro-db (e.g. `src/cell.rs`). Add
   `ro-db.workspace = true` to ro-engine. Repoint imports in ro-engine (`path.rs`, `engine.rs`, tests),
   ro-db and map-server.
4. **Engine owns its error; delete MapEngine.** Move `EngineError` into ro-engine (e.g. `src/error.rs`). Delete
   the trait, make its methods inherent `pub fn` on `MapInstance`, and drop `use ro_core::MapEngine`
   from map-server.
5. **Repo traits → ro-persistence.** Move `AccountRepository`, `CharacterRepository`, `RepositoryError`, `AuthError`,
   `CreateCharacterError` and the delete outcomes into ro-persistence; move `async-trait` and `thiserror` deps
   as needed. Repoint `bins/*/src/service.rs`, bin tests (`bins/map-server/tests/common/mod.rs:84`,
   login/char handshake tests) and ro-persistence's own impls and tests.
6. **Split `ro-core/tests/engine_contract_tests.rs` by owner.** CellType tests (`db_02_*`) → `crates/ro-db/tests/`.
   `engine_error_new_variants_display` → ro-engine. `opcode_names_fixed_per_spec` → ro-packets.
   `character_spawn_from_summary_*` and `unit_appearance_and_new_server_events_construct` stay in ro-core.
   Delete `broadcast_aoi_carries_exclude` (dead type) and `map_engine_is_object_safe` (trait gone).
7. **Trim dependencies.** login/char-server drop ro-engine, ro-db, ro-mechanics and ro-scripting. map-server drops
   ro-mechanics and ro-scripting. ro-mechanics and ro-scripting drop the `pub use ro_contracts` re-export
   and the dependency until they have code. Remove unused workspace deps from ro-core (`async-trait`,
   probably `bytes`). Run `cargo machete` or check by hand.
8. **Docs.**
   - `CLAUDE.md`: the `TARGET_PACKETVER` path, the layout row (`crates/ro-core`: "Shared IDs, DTOs, intent/event
     vocabulary"), and a short rule stating the dependency graph above.
   - `bins/CLAUDE.md`: the http DTO path.
   - `.claude/agents/reviewer.md:16`: replace "cross-crate communication goes through `ro-contracts`" with
     "dependencies follow the graph in CLAUDE.md; types live in the crate that owns them".
   - DESIGN.md mentions in ro-db, ro-engine, ro-persistence and ro-scripting. `ScriptWorldCommand` should
     live in ro-scripting, not core.
   - Leave `specs/engine-foundation/*` alone (historical).

## Verification

- After each step: `./verify.sh -p <touched crates>`. After the last step: run the full `./verify.sh`
  with the db up (`docker compose -f tools/docker/docker-compose.yml up -d db`), then `./verify.sh --smoke`
  to boot all three servers and the proxy.
- Run `cargo tree -p login-server --depth 1` (and for char/map) to confirm the trimmed graph matches the
  diagram, and `cargo tree -i ro-db` to show only ro-engine and map-server depend on it.
- `grep -rn ro_contracts` across the repo returns nothing (outside `specs/engine-foundation`).
- Test count before and after matches, minus the two deleted tests (compare the `verify.sh` summaries).
- Feature complete: run the `reviewer` agent over the full diff, per CLAUDE.md.
