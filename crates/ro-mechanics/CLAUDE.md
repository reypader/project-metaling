# ro-mechanics

Stateless formula functions only: no ECS state and no I/O.

- **Integer truncation order matters.** rAthena/Aegis truncate intermediate values at specific steps. Port each expression in rAthena's exact operation order (e.g. `val * ratio / 100`, not `val * (ratio / 100)`). Never use floats where rAthena uses integers.
- Every formula needs unit tests with exact expected values taken from rAthena/Aegis. Record where each test vector came from in a comment.
