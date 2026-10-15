# AGENTS.md — `crates/contracts` (Main Agent Exclusive)

* **Owner:** Main Agent (Root Orchestrator) ONLY.
* **Purpose:** Pure interface crate containing shared ID newtypes (`AccountId`, `CharId`, `ItemId`), DTO structs, `ClientIntent` / `OutboundPacket` enums, and cross-domain traits (`AccountRepository`, `CharacterRepository`, `MapWalkabilityProvider`).
* **Rule:** Zero business logic, zero database drivers, and zero heavy dependencies. Domain subagents may read this crate but MUST NEVER modify it directly.
