# `tools/` Module Rules (Packet Proxy & Diagnostic Utilities)

## 1. Module Scope & Operational Guidelines
* **Role & Scope:** Governs auxiliary development, packet forwarding, inspection proxies (`tools/proxy`), and Docker Compose container infrastructure (`tools/docker`) under `./tools/**`.
* **Direct Inspection:** Agents working on `./tools/**` inspect packet codecs (`ro-packets`), SQL initialization scripts (`ro-persistence`), server entrypoints (`bins/*`), contracts, and reference catalogs directly from disk without messaging barriers.
* **Contract Compliance:** Inspect `./crates/ro-contracts/src/**` directly from disk. If a shared contract requires modification, update it according to task requirements and verify with `cargo check -p ro-contracts`.
* **Reference Access:** Consult `./reference/catalog/` or the `reference-explorer` subagent whenever wire framing, Docker conventions, or opcode specifications are required.
* **Multi-File Review Gate:** Any change spanning more than one file in `./tools/**` requires invoking `code-reviewer` to audit stream forwarding, framing synchronization, and edge cases before completion.
* **Verification Command:** Run `cargo test -p ro-proxy` (or package-specific tests under `./tools/**`) to verify compilation and test passage.
* **Scripting Delegation:** Delegate one-off testing scripts or packet dump formatters to `utility-scripter`. Utility scripting is exempt from code and architectural reviews.

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
