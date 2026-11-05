# File structure: one concept per file

Refactor only; no behavior change. Reorganize `crates/`, `bins/` and `tools/` so that every file can be
read on its own: one main type (plus its impls) per file, data-only types grouped by theme in `_dto.rs`
files, ro-engine split into feature folders, inline tests moved out of source files, and integration tests
merged into one test binary per crate with shared helpers. The public API, test names and scenario IDs stay
the same. Rules and the per-file mapping are in `plan.md`.

| ID | Scenario | Check |
| :--- | :--- | :--- |
| REF-01 | Every test still exists and passes | `./verify.sh` reports the M0 baseline count (927 passed) |
| REF-02 | Root re-exports unchanged | `specs/file-structure/public_api.sh` output equals `public_api.txt` |
| REF-03 | Structure rules hold | `specs/file-structure/structure_check.sh` exits 0 |
| REF-04 | Generated GPL data unchanged | rerun each `tools/*-import`, then `git diff --exit-code crates/ro-db/data/` (needs `RATHENA_PATH`) |

Done when all four pass, `./verify.sh --smoke` passes, and the live docs (`crates/ro-engine/DESIGN.md`,
`bins/CLAUDE.md`, `crates/ro-packets/CLAUDE.md`, `PACKET_FINDINGS.md`, `reference/catalog`) point at the
new paths.
