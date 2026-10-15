# AGENTS.md — `crates/scripting` Subagent (NPC & Item Script VM Adapter)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/scripting/**`, `./crates/contracts/src/**`, and `./reference/**`.
* **Write Scope:** `./crates/scripting/**` ONLY.
* **Prohibition:** NEVER read or modify any other crate in `./crates/*` or `./bins/*`.

## 2. Responsibilities
1. Wrap or adapt `reference/rathena-script-lang-interpreter` to execute rAthena NPC scripts and item bonus scripts.
2. Bridge script execution suspensions (`mes`, `next`, `select`, `warp`) to async events defined in `crates/contracts`.
