# AGENTS.md — `crates/db` Subagent (Static Data & Map Cache)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/db/**`, `./crates/contracts/src/**`, and `./reference/**`.
* **Write Scope:** `./crates/db/**` ONLY.
* **Prohibition:** NEVER read or modify any other crate in `./crates/*` or `./bins/*`.

## 2. Responsibilities
1. Parse `.gat` / `.rsw` map cache files (reference: `reference/rust-ro/tools/map-cache/` and `reference/rathena/src/map/map.cpp`) into fast walkability grids implementing the map collision traits in `crates/contracts`.
2. Deserialize `reference/rathena/db/re/mob_db.yml`, `item_db.yml`, and `skill_db.yml` using `serde_yaml` into blueprints exposed via `crates/contracts`.
3. Parse monster and warp portal spawn definitions from `reference/rathena/npc/re/`.
