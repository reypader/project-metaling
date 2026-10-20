# `crates/ro-mechanics` Module Rules (Pure Combat & Stat Math)

## 1. Module Scope & Operational Guidelines
* **Role & Scope:** Governs pure stateless Aegis and Renewal stat derivations, elemental/size/race modifier tables, and combat damage formulas under `./crates/ro-mechanics/**`.
* **Direct Inspection:** Agents working on `./crates/ro-mechanics/**` inspect engine callers (`ro-engine`), contracts, and reference catalogs directly from disk without messaging barriers.
* **Engine Synergy:** Coordinate data types with `ro-engine` to ensure stat blocks and combat context structs align with ECS component layout, maximizing cache locality and minimizing pointer chasing.
* **Contract Compliance:** Inspect `./crates/ro-contracts/src/**` directly from disk. If a shared contract requires modification, update it according to task requirements and verify with `cargo check -p ro-contracts`.
* **Reference Access:** Consult `./reference/catalog/` or the `reference-explorer` subagent whenever Aegis or Renewal stat formulas, ASPD tables, modifier tables, or skill ratios are required.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-mechanics` requires invoking `code-reviewer` to audit integer truncation order and arithmetic edge cases before completion.
* **Verification Command:** Run `cargo test -p ro-mechanics` to verify compilation and test passage.
* **Scripting Delegation:** Delegate one-off formula verification or test vector generation scripts to `utility-scripter`. Utility scripting is exempt from code and architectural reviews.

---

## 2. Stateless Calculation Specifications

`crates/ro-mechanics` is a pure, stateless functional math library implementing Ragnarok Online (Renewal and Aegis) status derivations, elemental/race/size modifier tables, and combat damage formulas.

### 2.1 Stateless Calculation Principles
1. **Deterministic Integer Arithmetic:**
   * Aegis and rAthena rely on intermediate integer truncation at specific stages of status and damage calculation.
   * Preserve exact integer division order before applying percentage modifiers (`val * ratio / 100`).
2. **Core Formula Domains:**
   * **Derived Stats:** Base and equipment `ATK`, `MATK`, hard/soft `DEF`, hard/soft `MDEF`, `HIT`, `FLEE`, `PERFECT_DODGE`, `CRIT`, `MAX_HP`, `MAX_SP`, and job/weapon `ASPD` (including delay `amotion = 2000 - aspd * 10`).
   * **Modifier Tables:** 10x10 Elemental attribute table (levels 1 to 4), 3-size weapon penalty table (Small, Medium, Large), and race/card bonus multipliers.
   * **Combat & Skill Pipeline:** Physical short-range and long-range damage, magical damage, critical hit bypass rules, hit/miss roll evaluation, and per-skill damage ratios.
3. **Mandatory Unit Test Coverage:**
   * Every stat derivation and damage formula must include unit tests asserting exact numerical outputs against known rAthena and Aegis test vectors.

---

## 3. Contract Boundaries (`crates/ro-contracts`)
* **Upstream Caller:** `crates/ro-engine` invokes `ro-mechanics` functions and traits during entity status recalculation and Phase 4 (`Combat Read Phase`) of the deterministic map tick loop.
* **Contract Interface:** All inputs (attacker and defender stat snapshots, weapon and element attributes, skill parameters) and outputs (derived stat blocks, `DamageResult`) pass through pure data structures and traits defined in `crates/ro-contracts`.
* **Isolation Invariant:** `crates/ro-mechanics` performs zero I/O, spawns zero async tasks, holds zero global mutable state, and has zero dependency on `ro-engine`, `ro-packets`, `ro-db`, or `ro-persistence`.
