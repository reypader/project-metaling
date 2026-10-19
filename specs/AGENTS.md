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
3. **`## 2. High-Level Bin-Observable Scenarios (ro-bins)`**
   * End-to-end client-server and inter-server scenarios observable at `login-server`, `char-server`, and `map-server`.
4. **Domain-Scoped Sections (included for each affected crate/package):**
   * `## 3. Domain Scenarios: ro-packets`
   * `## 4. Domain Scenarios: ro-engine`
   * `## 5. Domain Scenarios: ro-db`
   * `## 6. Domain Scenarios: ro-mechanics`
   * `## 7. Domain Scenarios: ro-persistence`
   * `## 8. Domain Scenarios: ro-scripting`
   * `## 9. Domain Scenarios: ro-tools`

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

Once `architectural-code-reviewer` approves `./specs/<feature-or-phase>/spec.md`, the Main Agent creates `./specs/<feature-or-phase>/plan.md` structured by target subagent and execution stage:

1. **`# Implementation Plan: <Feature or Phase Name>`**
   * Link to `./specs/<feature-or-phase>/spec.md`.
2. **`## 1. Contract Updates (crates/ro-contracts)`**
   * Status and summary of contract definitions applied by the Main Agent.
3. **`## 2. Upfront Test Authoring (Red Phase)`**
   * Subsections per subagent (`### 2.1 ro-packets`, `### 2.2 ro-engine`, etc.) listing the exact test files to create/update, the scenario IDs (`PKT-01`, `ENG-01`), and the line ranges in `spec.md` to reference.
4. **`## 3. Domain & Binary Implementation (Green Phase)`**
   * Subsections per subagent (`### 3.1 ro-packets`, `### 3.2 ro-engine`, `### 3.3 ro-bins`, etc.) detailing the concrete implementation tasks, integration touchpoints, and verification commands required to make the upfront tests pass.
5. **`## 4. Verification & Review Checklist`**
   * Crate-level `code-reviewer` checkpoints and final `architectural-code-reviewer` vertical-slice audit criteria.
