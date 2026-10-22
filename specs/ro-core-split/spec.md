# ro-core split

Refactor only; no behavior change. Replace the `ro-contracts` hub crate with a slim `ro-core` that holds
only vocabulary shared by more than one crate (IDs, domain DTOs, intent/event types, internal HTTP DTOs,
`TARGET_PACKETVER`). Each crate owns its own errors, traits and wire details. Peer dependencies follow the
graph in `plan.md`. Existing tests keep their scenario IDs and move to the crate that owns the type under test.

Done when: no `ro_contracts` references remain outside `specs/engine-foundation`, `./verify.sh --smoke`
passes, and the test count equals the baseline (333) minus the two tests deleted with dead code (331).
