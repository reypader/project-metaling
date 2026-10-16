# AGENTS.md: `crates/mechanics` Subagent (Pure Combat & Stat Math)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/mechanics/**`, `./crates/contracts/**` (including `./crates/contracts/src/**`), and `./reference/**`.
* **Direct Contract Inspection (No `cargo doc`):** Read the source files in `./crates/contracts/src/` directly via file viewing tools. Avoid running `cargo doc` or background documentation builds to inspect contracts, because reading files directly eliminates compilation overhead.
* **Write Scope:** `./crates/mechanics/src/**` and `./crates/mechanics/Cargo.toml` ONLY.
* **Prohibition:** NEVER write to `./crates/contracts/**`, `./bins/*`, or another domain crate (`./crates/<other_domain>/**`). NEVER read implementation files in other domain crates (`./crates/<other_domain>/src/**`). If you need a new shared type or trait in `crates/contracts`, STOP and request a Contract Amendment from the Main Agent.

---

## 2. Domain Architecture & Aegis Math Rules

`crates/mechanics` (`ro-mechanics`) is a pure, stateless functional math library implementing Ragnarok Online (Renewal / Aegis) status derivations, elemental/race/size modifier tables, and combat damage formulas.

### 2.1 Stateless Calculation Principles
1. **Deterministic Integer Arithmetic:**
   * Aegis and rAthena rely on intermediate integer truncation at specific stages of status and damage calculation.
   * Reference `reference/rathena/src/map/status.cpp`, `reference/rathena/src/map/battle.cpp`, `reference/rathena/src/map/skills/<job>/*.cpp`, and `reference/rust-ro/tools/stat-calc/`.
   * Preserve exact integer division order before applying percentage modifiers (`val * ratio / 100`).
2. **Core Formula Domains:**
   * **Derived Stats:** Base and equipment `ATK`, `MATK`, hard/soft `DEF`, hard/soft `MDEF`, `HIT`, `FLEE`, `PERFECT_DODGE`, `CRIT`, `MAX_HP`, `MAX_SP`, and job/weapon `ASPD` (including delay `amotion = 2000 - aspd * 10`).
   * **Modifier Tables:** 10x10 Elemental attribute table (levels 1 to 4), 3-size weapon penalty table (Small, Medium, Large), and race/card bonus multipliers.
   * **Combat & Skill Pipeline:** Physical short/long-range damage, magical damage, critical hit bypass rules, hit/miss roll evaluation, and per-skill damage ratios.
3. **Mandatory Unit Test Coverage:**
   * Every stat derivation and damage formula must include unit tests asserting exact numerical outputs against known rAthena/Aegis test vectors.

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Upstream Caller:** `crates/engine` invokes `ro-mechanics` functions or traits during entity status recalculation and Phase 4 (`Combat Read Phase`) of the deterministic map tick loop.
* **Contract Boundary:** All inputs (attacker/defender stat snapshots, weapon/element attributes, skill parameters) and outputs (derived stat blocks, `DamageResult`) pass through pure data structures and traits defined in `crates/contracts`.
* **Isolation Boundary:** `crates/mechanics` performs zero I/O, spawns zero async tasks, holds zero global mutable state, and has no dependency on `ro-engine` ECS storage, `ro-packets`, `ro-db`, or `ro-persistence`.
