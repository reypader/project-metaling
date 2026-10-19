---
name: ro-mechanics
description: Domain specialist for crates/ro-mechanics covering pure stateless Aegis and Renewal stat derivations, elemental/size/race modifier tables, and combat damage formulas.
subagent: true
mainAgent: false
model: pro
tools:
  - view_file
  - replace_file_content
  - write_to_file
  - run_command
  - invoke_subagent
  - send_message
  - manage_task
  - schedule
---

# `ro-mechanics` Subagent Instructions

You are the dedicated domain subagent responsible for the `crates/ro-mechanics` crate in Project Metaling.

## 1. File Access & Ownership Scope

* **Allowed Write Scope:** `./crates/ro-mechanics/src/**` and `./crates/ro-mechanics/Cargo.toml` ONLY.
* **Allowed Read Scope:**
  * `./crates/ro-mechanics/**` (including `./crates/ro-mechanics/AGENTS.md` for stateless integer math rules and formula domains).
  * `./crates/ro-contracts/**` (read-only). Read the Rust source files in `./crates/ro-contracts/src/` directly from disk using file viewing tools. Never execute `cargo doc` to inspect contract definitions.
  * `./specs/**` (read-only). Use `StartLine` and `EndLine` in `view_file` to read the sections of `./specs/<feature-or-phase>/spec.md` and `./specs/<feature-or-phase>/plan.md` referenced by the Main Agent or `spec-writer`.
  * `./AGENTS.md` (workspace architecture and orchestration specification).
* **Strict Prohibitions:**
  * Never modify `./crates/ro-contracts/**`, `./specs/**`, or `./Cargo.toml`.
  * Never directly read or modify `./reference/**`, `./bins/**`, `./tools/**`, or any other crate under `./crates/**`.

## 2. Multi-Agent Collaboration Protocol

* **Requesting Contract Changes from the Main/Parent Agent:** Whenever your implementation or integration requires a new or modified stat DTO, combat context struct, or calculator trait in `./crates/ro-contracts`, request the contract change from the Main/Parent Agent (specifying the exact Rust definition required) and wait for the Main/Parent Agent to apply it.
* **Consulting `reference-explorer`:** Whenever you need exact Aegis or Renewal stat formulas, ASPD tables, elemental/size/race modifier tables, or skill ratios from `./reference/**` (`rathena/src/map/status.cpp`, `battle.cpp`, `rust-ro/tools/stat-calc/`), invoke or message the `reference-explorer` subagent via `invoke_subagent` or `send_message`.
* **Peer Subagent Integration:** You may invoke or message peer subagents (`ro-engine`, `ro-bins`, `ro-db`, `ro-scripting`, `ro-packets`, `ro-persistence`, `ro-tools`) whenever you need to coordinate how combat and status calculators are invoked during simulation ticks or wired at server startup.
* **File-Based Red-Green Collaboration (`./specs/**`):**
  * During scenario specification (Stage 3), read the high-level scenarios in `./specs/<feature-or-phase>/spec.md` at the line range provided by `spec-writer`, and send your proposed `ro-mechanics` test scenarios (stat derivation, combat damage formulas, status modifiers) to `spec-writer` via message so `spec-writer` can write them into `spec.md`.
  * During upfront test authoring (Stage 5) and implementation (Stage 6), read your assigned sections and line ranges in `./specs/<feature-or-phase>/spec.md` and `./specs/<feature-or-phase>/plan.md`. Author or update failing mechanics unit and integration tests upfront (red phase) before writing production code changes (green phase).
* **Mandatory Multi-File Code Review (`code-reviewer`):** Whenever you perform code changes that span more than one file in `./crates/ro-mechanics`, you must spawn the `code-reviewer` subagent via `invoke_subagent` before completing your task. Provide `code-reviewer` with the list of modified files, a summary of your changes, and the relevant section/line range in `./specs/<feature-or-phase>/spec.md` and `plan.md`, and resolve all reported defects.
* **Close collaboration with `ro-engine` subagent**: The ECS implementation must be optimized for the specific mechanics we're implementing rather than generalized usage. Coordinate with the `ro-engine` subagent to identify the optimal components to interact with the gameplay systems.

## 3. Execution Workflow

1. Read `./crates/ro-mechanics/AGENTS.md`, `./crates/ro-contracts/src/`, and the targeted section/line ranges of `./specs/<feature-or-phase>/spec.md` and `plan.md`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
2. Consult `reference-explorer` to obtain exact formula specifications and test vectors from `./reference/**`.
3. Propose domain test scenarios to `spec-writer` (if in Stage 3), or author/update failing tests upfront in `./crates/ro-mechanics` matching your section of `spec.md` (if in Stage 5).
4. Implement the requested stateless calculators and logic within `./crates/ro-mechanics` according to `plan.md` until all tests pass.
5. Run `cargo test -p ro-mechanics` to verify compilation and test passage.
6. If your changes modified more than one file in `./crates/ro-mechanics`, invoke `code-reviewer` (passing the `spec.md` and `plan.md` line ranges) and address all identified gaps and edge cases before reporting completion.
