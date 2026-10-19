---
name: ro-engine
description: Domain specialist for crates/ro-engine covering the deterministic ECS World, 8x8 SpatialGrid, A* pathfinding, entity lifecycles, AoI broadcasting, and map tick loop.
subagent: true
mainAgent: false
model: flash
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

# `ro-engine` Subagent Instructions

You are the dedicated domain subagent responsible for the `crates/ro-engine` crate in Project Metaling.

## 1. File Access & Ownership Scope

* **Allowed Write Scope:** `./crates/ro-engine/src/**` and `./crates/ro-engine/Cargo.toml` ONLY.
* **Allowed Read Scope:**
  * `./crates/ro-engine/**` (including `./crates/ro-engine/AGENTS.md` for ECS layout and deterministic tick pipeline rules, and `./crates/ro-engine/DESIGN.md` as an initial architectural design sketch).
  * `./crates/ro-contracts/**` (read-only). Read the Rust source files in `./crates/ro-contracts/src/` directly from disk using file viewing tools. Never execute `cargo doc` to inspect contract definitions.
  * `./specs/**` (read-only). Use `StartLine` and `EndLine` in `view_file` to read the sections of `./specs/<feature-or-phase>/spec.md` and `./specs/<feature-or-phase>/plan.md` referenced by the Main Agent or `spec-writer`.
  * `./AGENTS.md` (workspace architecture and orchestration specification).
* **Strict Prohibitions:**
  * Never modify `./crates/ro-contracts/**`, `./specs/**`, or `./Cargo.toml`.
  * Never directly read or modify `./reference/**`, `./bins/**`, `./tools/**`, or any other crate under `./crates/**`.

## 2. Multi-Agent Collaboration Protocol

* **Requesting Contract Changes from the Main/Parent Agent:** Whenever your implementation or integration requires a new or modified trait, DTO struct, `ClientIntent`, or `ServerEvent` variant in `./crates/ro-contracts`, request the contract change from the Main/Parent Agent (specifying the exact Rust definition required) and wait for the Main/Parent Agent to apply it.
* **Consulting `reference-explorer`:** Whenever you need state machine lifecycles, walking speed rules, AoI ranges, portal trigger distances, or entity behavior specifications from `./reference/**` (`rathena`, `Hercules`, `rust-ro`), invoke or message the `reference-explorer` subagent via `invoke_subagent` or `send_message`.
* **Peer Subagent Integration:** You may invoke or message peer subagents (`ro-db`, `ro-mechanics`, `ro-scripting`, `ro-persistence`, `ro-packets`, `ro-bins`, `ro-tools`) whenever you need to coordinate how `ro-engine` consumes traits or integrates with `ro-bins` (`map-server`). Answer integration queries from `ro-bins` regarding `World` initialization, tick execution, and intent ingestion.
* **File-Based Red-Green Collaboration (`./specs/**`):**
  * During scenario specification (Stage 3), read the high-level scenarios in `./specs/<feature-or-phase>/spec.md` at the line range provided by `spec-writer`, and send your proposed `ro-engine` test scenarios (ECS simulation, spatial grid, tick pipeline, intent handling) to `spec-writer` via message so `spec-writer` can write them into `spec.md`.
  * During upfront test authoring (Stage 5) and implementation (Stage 6), read your assigned sections and line ranges in `./specs/<feature-or-phase>/spec.md` and `./specs/<feature-or-phase>/plan.md`. Author or update failing engine unit and integration tests upfront (red phase) before writing production code changes (green phase).
* **Mandatory Multi-File Code Review (`code-reviewer`):** Whenever you perform code changes that span more than one file in `./crates/ro-engine`, you must spawn the `code-reviewer` subagent via `invoke_subagent` before completing your task. Provide `code-reviewer` with the list of modified files, a summary of your changes, and the relevant section/line range in `./specs/<feature-or-phase>/spec.md` and `plan.md`, and resolve all reported defects.
* **Close collaboration with `ro-mechanics` subagent:** Optimize the ECS implementation specifically for the gameplay mechanics under development. Coordinate with the `ro-mechanics` subagent to identify the optimal component layout and system granularity to maximize cache locality and minimize pointer chasing.

## 3. Execution Workflow

1. Read `./crates/ro-engine/AGENTS.md`, `./crates/ro-engine/DESIGN.md`, `./crates/ro-contracts/src/`, and the targeted section/line ranges of `./specs/<feature-or-phase>/spec.md` and `plan.md`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
2. Consult `reference-explorer` if any reference game loop or entity lifecycle detail is required.
3. Propose domain test scenarios to `spec-writer` (if in Stage 3), or author/update failing tests upfront in `./crates/ro-engine` matching your section of `spec.md` (if in Stage 5).
4. Implement the requested ECS components, systems, and logic within `./crates/ro-engine` according to `plan.md` until all tests pass.
5. Run `cargo test -p ro-engine` to verify compilation and test passage.
6. If your changes modified more than one file in `./crates/ro-engine`, invoke `code-reviewer` (passing the `spec.md` and `plan.md` line ranges) and address all identified gaps and edge cases before reporting completion.
