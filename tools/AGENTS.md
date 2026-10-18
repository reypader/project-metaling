# `tools/` Module Rules (Packet Proxy & Diagnostic Utilities)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-tools.md` (`ro-tools` custom subagent).
* **Write Scope:** `./tools/**` (including `./tools/proxy/**` and `./tools/docker/**`) exclusively.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Peer Subagent Integration:** Never read or modify implementation files inside `./crates/ro-packets/**`, `./crates/ro-engine/**`, `./crates/ro-db/**`, `./crates/ro-mechanics/**`, `./crates/ro-persistence/**`, `./crates/ro-scripting/**`, or `./bins/**`. Invoke or message `ro-packets`, `ro-persistence`, and `ro-bins` via `invoke_subagent` or `send_message` whenever you need codec constructors, SQL initialization paths, framing behavior, or server port/handshake details.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever wire framing, Docker conventions, or opcode specifications from `./reference/**` are needed. Never read `./reference/**` directly.
* **Multi-File Review Gate:** Any change spanning more than one file in `./tools/**` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit stream forwarding, framing synchronization, and edge cases before completion.
* **Red-Green Testing & Spec Collaboration:** Collaborate with `spec-writer` to define packet forwarding, proxy inspection, and telemetry test scenarios supporting high-level bin-observable scenarios. Author or update proxy integration tests upfront (red phase) before writing production code changes.

---

## 2. Proxy, Docker, & Diagnostic Tooling Architecture
1. **Packet Logging & Forwarding Proxy (`tools/proxy`):**
   * Intercepts and forwards TCP streams across the three Ragnarok Online service ports (`:6900` login, `:6121` char, `:5121` map).
   * Consumes `PacketLengthTable` and framing codecs from `crates/ro-packets` to log structured opcode, length, direction, and hex dump telemetry without corrupting client-server byte streams.
   * Accounts for the raw 4-byte `AccountId` server response immediately following `CH_ENTER` on `:6121` and `CZ_ENTER` on `:5121` before resuming standard 2-byte opcode framing on the downstream server-to-client half of the stream.
2. **Container & Database Infrastructure (`tools/docker`):**
   * Provides `tools/docker/docker-compose.yml` to run the PostgreSQL database (`db`) initialized with schema (`crates/ro-persistence/sql/01_schema.sql`) and seed (`crates/ro-persistence/sql/02_seed.sql`) scripts mounted into `/docker-entrypoint-initdb.d/`.
   * Structured to expand into building and orchestrating the `login-server`, `char-server`, and `map-server` containers alongside the database.
3. **Isolation Invariant:**
   * Packages and configurations under `./tools/**` are standalone developer utilities. Production server binaries (`./bins/*`) and domain crates (`./crates/*`) must never depend on `./tools/*`.
