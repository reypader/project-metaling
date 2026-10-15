# AGENTS.md — `crates/mechanics` Subagent (Pure Combat & Stat Math)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/mechanics/**`, `./crates/contracts/src/**`, and `./reference/**`.
* **Write Scope:** `./crates/mechanics/**` ONLY.
* **Prohibition:** NEVER read or modify any other crate in `./crates/*` or `./bins/*`.

## 2. Responsibilities
1. Implement pure, stateless mathematical functions for derived stats (ATK, MATK, DEF, MDEF, HIT, FLEE, ASPD) and damage calculation (`DamageContext` modifiers).
2. Reference `reference/rathena/src/map/battle.cpp`, `status.cpp`, `src/map/skills/<job>/*.cpp`, and `reference/rust-ro/tools/stat-calc/` for exact Aegis integer rounding and elemental/size/race tables.
3. Every formula must be accompanied by unit tests.
