# AGENTS.md: `crates/contracts` (Main Agent Exclusive)

* **Owner:** Main Agent (Root Orchestrator) ONLY.
* **Purpose:** Pure interface crate containing shared ID newtypes (`AccountId`, `CharId`, `ItemId`), DTO structs, `ClientIntent` / `ServerEvent` / `OutboundPacket` enums, and cross-domain traits (`AccountRepository`, `CharacterRepository`, `MapWalkabilityProvider`, `PacketIntentParser`, `EventEncoder`).
* **Target Version:** Pinned exclusively to `TARGET_PACKETVER = 20260219`. Omit backwards compatibility for older packet versions.
* **Rule:** Zero business logic, zero database drivers, and zero heavy dependencies. Domain subagents may read all files in this crate (`./crates/contracts/**`) directly from disk. Domain subagents MUST NOT execute `cargo doc` to read contract interfaces, because reading files directly avoids unnecessary build overhead. Domain subagents MUST NEVER modify or write to this crate.
