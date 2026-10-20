# `crates/ro-contracts` Module Rules (Shared Domain Contracts)

## 1. Module Scope & Access Policy
* **Role & Scope:** Governs pure cross-crate traits, shared IDs, client intents, server events, error types, and DTOs in `./crates/ro-contracts/**`.
* **Direct Inspection:** All agents inspect the Rust files in `./crates/ro-contracts/src/` directly from disk. Running `cargo doc` is prohibited because reading source files directly avoids compilation overhead.
* **Task-Based Modification:** The Main Agent and the `implementer` subagent can add or update contracts as necessary to fulfill task requirements and implementation plans. After modifying contracts, verify compilation with `cargo check -p ro-contracts`.
* **Reviewers & Spec Writer:** Reviewers (`code-reviewer`, `architectural-code-reviewer`) and `spec-writer` have read-only access to inspect contracts and never write implementation code.

---

## 2. Architectural Invariants
* **Strict Interface Purity:** `ro-contracts` defines shared ID newtypes (`AccountId`, `CharId`, `SessionId`, `EntityId`, `ItemId`), DTO structs, `ClientIntent` / `ServerEvent` / `ScopedServerEvent` / `OutboundPacket` enums, HTTP API payload types, and cross-domain traits (`AccountRepository`, `CharacterRepository`, `MapWalkability`, `PacketLengthTable`, `PacketIntentParser`, `EventEncoder`).
* **Zero Implementation Logic:** `ro-contracts` contains zero business logic, zero concrete wire codecs, zero database drivers, zero ECS storage, and zero network socket loops.
* **Pinned Client Version:** All protocol contracts target `TARGET_PACKETVER = 20260219` exclusively. Legacy packet version compatibility is omitted.
* **32-bit Item IDs:** Every item identifier uses `ItemId(pub u32)` to support modern `PACKETVER` item ranges (`MAX_ITEMID > UINT16_MAX`).
