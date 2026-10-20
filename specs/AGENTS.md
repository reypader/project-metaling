# AGENTS.md: `./specs/` (Shared Specifications & Implementation Plans)

## 1. Purpose & Ownership

The `./specs/` directory stores version-controlled test scenario specifications (`spec.md`) and implementation plans (`plan.md`) organized by milestone phase or feature directory (`./specs/<feature-or-phase>/`). These files act as the shared source of truth across all agents and subagents to eliminate context window duplication and transcription drift during handovers.

* **Write Ownership:**
  * `./specs/<feature-or-phase>/spec.md`: Exclusively authored and updated by the `spec-writer` subagent.
  * `./specs/<feature-or-phase>/plan.md`: Exclusively authored and updated by the Main Agent.
  * `./specs/AGENTS.md`: Exclusively maintained by the Main Agent.
* **Read Access:**
  * All agents and subagents have read-only access to `./specs/**`.
  * When delegating work or requesting reviews, parent agents provide the target file path and specific section headings or line ranges (`StartLine` to `EndLine`) so subagents read only the sections relevant to their scope.

---

## 2. Standard Structure for `spec.md` (`spec-writer`)

Every `./specs/<feature-or-phase>/spec.md` file must use clear top-level Markdown sections so parent agents can direct subagents to exact line ranges:

1. **`# Specification: <Feature or Phase Name>`**
   * Brief summary of the target behavior and scope.
2. **`## 1. Contract Touchpoints (ro-contracts)`**
   * Exact traits, DTOs, `ClientIntent` variants, `ServerEvent` variants, and IDs involved in this specification.
3. **`## 2. High-Level Observable Scenarios`**
   * End-to-end client-server and inter-server scenarios observable at `login-server`, `char-server`, and `map-server`.
4. **Component & Module Scenarios (included for each affected crate or binary package):**
   * `## 3. Wire Protocol Scenarios (ro-packets)`
   * `## 4. Engine & Simulation Scenarios (ro-engine)`
   * `## 5. Static Data & Walkability Scenarios (ro-db)`
   * `## 6. Mechanics & Math Scenarios (ro-mechanics)`
   * `## 7. Persistence & Database Scenarios (ro-persistence)`
   * `## 8. Scripting VM Scenarios (ro-scripting)`
   * `## 9. Tooling & Proxy Scenarios (tools)`

Each individual scenario within a section must define:
* **Scenario ID & Title** (for example, `BIN-01`, `PKT-01`, `ENG-01`)
* **Supports High-Level Scenario(s)** (for domain scenarios, referencing the parent `BIN-*` ID)
* **Observable Target** (binary, crate, module, or trait)
* **Preconditions & Initial State**
* **Input Stimulus**
* **Expected Observable Behavior**
* **Failure Modes & Edge Cases**
* **Assertion Criteria**

---

## 3. Standard Structure for `plan.md` (Main Agent)

Once `architectural-code-reviewer` approves `./specs/<feature-or-phase>/spec.md` and recommends a milestone breakdown, the Main Agent creates `./specs/<feature-or-phase>/plan.md`. The plan organizes tasks into incremental milestones to prevent overly large deliverables. Milestones can be motivated by architectural refactoring for emergent patterns, abstraction for extendability, or independent behaviors. Stubs are used to maintain compilation across milestone boundaries.

The plan structure follows:

1. **`# Implementation Plan: <Feature or Phase Name>`**
   * Link to `./specs/<feature-or-phase>/spec.md`.
2. **`## 1. Contract Updates (crates/ro-contracts)`**
   * Status and summary of contract definitions applied in `ro-contracts`.
3. **`## 2. Upfront Test Authoring (Red Phase)`**
   * Tasks for the `implementer` listing the exact test files to create or update across modules, referencing scenario IDs (`BIN-01`, `PKT-01`, `ENG-01`) and line ranges in `spec.md`.
4. **`## 3. Milestone Execution (Green Phase)`**
   * Incremental milestones (`### 3.1 Milestone 1: <Name>`, `### 3.2 Milestone 2: <Name>`) detailing concrete implementation steps, boundary stubs, module integration touchpoints, and verification commands (`cargo test -p <pkg>`, `cargo test --bins`).
5. **`## 4. Verification & Review Checklist`**
   * Multi-file `code-reviewer` checkpoints and final `architectural-code-reviewer` vertical-slice audit criteria. One-off scripts executed by `utility-scripter` are exempt from review.
