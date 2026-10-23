# Implementation Plan: Warp map change

Spec: `specs/warp-map-change/spec.md`. Design and decisions: `brief.md`.

## Decisions (settled with the user)
| Topic | Decision |
| :--- | :--- |
| Destination `.gat` missing | Inert when `MAP_DATA_DIR` is set (trigger cleared at load, one warning per warp). Without it, use the fallback grid. |
| `@`/instance destinations | Inert. Out of scope. |
| World cap | Warps don't check `MAX_HOSTED_WORLDS`. |
| RNG | Private SplitMix64 per instance, seeded from the map name. |
| Map property / extra packets | None needed. The captures show nothing between `0x0091` and `0x007d` (spec §0). A real client already gets past loading with metaling's current entry burst, which has no `0x099b` (user, 2026-10-23). |
| Disconnect in transit | Persist `from_pos` on the source map. The arrival-cell check re-fires the warp on re-login (spec §1, Deviation). |
| Claim release | An RAII `Claim` owns release and the last position. It moves with the session from A to B, and dropping it is the only release (early review, H1/H2). |
| Position writes | One persistence writer task, so writes land in order (L7). |
| Unframed packets in captures | Backlog in [PF-21](../../PACKET_FINDINGS.md#pf-21). Not part of this feature. |

## Milestones
Write tests first and name them with spec IDs. Iterate with `./verify.sh -p <crate>`.

- [x] **M0 research, spec, early review**
  - Captures diffed. The catalog (`map_entry_actions.md`, `npcs.md`, `packets/map.md`) and `PACKET_FINDINGS.md` (PF-01, PF-20) are updated.
  - Spec and plan written.
  - Captures cited by the catalog are committed in `reference/catalog/captures/` as `.txt`, with redactions.
  - Early `reviewer` pass on the handoff is done, and all its findings are applied (spec §1 "Early review").
- [ ] **M1 ro-core:** `gat_map_name`, `bare_map_name`, re-exported at the root. CORE-MAP-01. Switch `load_npcs` to them.
- [ ] **M2 ro-engine, trigger and same map**
  - Reshape `WarpTrigger` (resolved `.gat`, `same_map`).
  - Add `warp_cells`, `World::hide`, `PlayerInfo.rewarp_count`, the SplitMix64 RNG and cell resolution.
  - Add `warp_player` (same-map branch), the step hook in `advance_walkers` and the `actor_init` tail hook.
  - Tests in `crates/ro-engine/tests/warp_tests.rs`: ENG-WARP-01, 02, 04, 05, 06, 07.
- [ ] **M3 ro-engine, cross map**
  - `MapTransfer`, the outbox, `drain_transfers`, and the cross-map branch of `warp_player`, sharing despawn code with `remove_character`.
  - `enter_by_warp`.
  - ENG-WARP-03, ENG-WARP-08. ENG-WARP-06 across a transfer.
  - Update `DESIGN.md` §1a and §7.
- [ ] **M4 map-server**
  - Do this first, in its own commit, with no warp code:
    - `Claim` (RAII);
    - `SharedState::release` returning `bool` with an error for an unheld release;
    - the single persistence writer with sequence numbers;
    - `resolve_entry` claiming first;
    - all existing release sites moved onto `Claim`.
    - Tests: BIN-WARP-07, and the existing map-server tests still pass.
  - `SessionControl`/`MapHandoff`, the `JoinSession` reshape (`spawn`, `control_tx`, `Entry`), and B's `Entry::Warp` path with its failure branch.
  - The transfer step around each tick's `flush` in actor A: `set_last` before the flush, the handoff after it.
  - B encoding the `0x0091` for `Entry::Warp`.
  - The session: `control_rx` in `select!`, applying `ChangeMap`, and dropping `control_tx`/`control_rx`/`outbound_tx` before awaiting the writer on exit.
  - Inert-warp filtering in `map_data_loader`.
  - Tests in `bins/map-server/tests/warp_tests.rs` (`tests/common` `data_loader` with two maps; needs the db): BIN-WARP-01..06.
- [ ] **Finish**
  - Run the `reviewer` over the full diff.
  - `DEFERRED.md`: remove the landed warp prerequisites, and add spec §3.
  - `./verify.sh --smoke`.
  - Repeat a capture route against metaling (one cross-map and one same-map warp with an observer) and diff it with the rAthena captures.
