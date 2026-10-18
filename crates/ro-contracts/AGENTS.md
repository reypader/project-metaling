# `crates/ro-contracts` Module Rules (Shared Domain Contracts)

## 1. Module Ownership & Access Policy
* **Owner:** Main Agent (Root Orchestrator) exclusively.
* **Subagent Access:** All custom subagents (`ro-packets`, `ro-engine`, `ro-db`, `ro-mechanics`, `ro-persistence`, `ro-scripting`, `ro-bins`, `ro-tools`, `reference-explorer`, `code-reviewer`, `architectural-code-reviewer`, and `spec-writer`) have read-only access to `./crates/ro-contracts/**`.
* **Direct Source Inspection:** Agents must read the Rust files in `./crates/ro-contracts/src/` directly from disk using file viewing tools. Running `cargo doc` to inspect contract definitions is prohibited because reading source files directly avoids compilation overhead.
* **Contract Change Requests from Subagents:** Subagents must never edit or write files inside `./crates/ro-contracts/**`. When a subagent needs a new or modified trait method, DTO field, error variant, or enum in `ro-contracts`, it requests the contract change from the Main/Parent Agent.

---

## 2. Architectural Invariants
* **Strict Interface Purity:** `ro-contracts` defines shared ID newtypes (`AccountId`, `CharId`, `SessionId`, `EntityId`, `ItemId`), DTO structs, `ClientIntent` / `ServerEvent` / `ScopedServerEvent` / `OutboundPacket` enums, HTTP API payload types, and cross-domain traits (`AccountRepository`, `CharacterRepository`, `MapWalkability`, `PacketLengthTable`, `PacketIntentParser`, `EventEncoder`).
* **Zero Implementation Logic:** `ro-contracts` contains zero business logic, zero concrete wire codecs, zero database drivers, zero ECS storage, and zero network socket loops.
* **Pinned Client Version:** All protocol contracts target `TARGET_PACKETVER = 20260219` exclusively. Legacy packet version compatibility is omitted.
* **32-bit Item IDs:** Every item identifier uses `ItemId(pub u32)` to support modern `PACKETVER` item ranges (`MAX_ITEMID > UINT16_MAX`).
