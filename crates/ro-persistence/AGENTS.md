# `crates/ro-persistence` Module Rules (PostgreSQL & Repositories)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-persistence.md` (`ro-persistence` custom subagent).
* **Write Scope:** `./crates/ro-persistence/**` exclusively.
* **Design Reference:** Read `./crates/ro-persistence/DESIGN.md` for an initial architectural design to follow. This document serves as a rough directional sketch of where the persistence domain is headed and allows flexibility as implementation details evolve.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever SQL table schemas, column types, or repository patterns from `./reference/**` are needed. Never read `./reference/**` directly.
* **Peer Consultation:** Coordinate with `ro-bins` via subagent messaging when integrating `InMemoryStubRepo` and `PostgresRepo` into `login-server`, `char-server`, and `map-server`.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-persistence` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit SQL constraints, thread safety, and edge cases before completion.
* **Red-Green Testing & Spec Collaboration:** Collaborate with `spec-writer` to define repository CRUD, authentication query, character persistence, and schema migration test scenarios supporting high-level bin-observable scenarios. Author or update persistence unit and integration tests upfront (red phase) before writing production code changes.

---

## 2. Repository Architecture & Data Models

`crates/ro-persistence` manages durable and in-memory stub data storage for accounts, authentication sessions, character slots, and saved character state across `login-server`, `char-server`, and `map-server`.

### 2.1 Dual Repository Implementation Strategy
1. **`InMemoryStubRepo` (Phases 0 to 3):**
   * Provides a thread-safe (`Send + Sync`) in-memory implementation of the repository traits in `crates/ro-contracts`.
   * Pre-seeds a default test account (`AccountId(2000000)`, username `"test"`, password `"password"`, `Sex::Male`) and a default character (`CharId(150000)`, slot `0`, name `"Novice"`, job `0`, `last_map: "prontera.gat"`, `last_pos: (156, 191, 0)`).
   * Supports in-memory updates to character position (`last_map`, `last_pos`) during Phase 3 map warps and character creation or deletion in tests without requiring a running PostgreSQL instance.
2. **`PostgresRepo` (Phase 4+):**
   * Uses `sqlx::PgPool` to execute asynchronous queries and manage SQL schema migrations.
   * Manages relational tables for:
     * `accounts`: `account_id`, `userid`, `user_pass`, `sex`, `email`, `lastlogin`, `state`.
     * `characters`: `char_id`, `account_id`, `char_num` (`slot`), `name`, `class` (`job_class`), `base_level`, `job_level`, `base_exp`, `job_exp`, `zeny`, `str`, `agi`, `vit`, `int`, `dex`, `luk`, `max_hp`, `hp`, `max_sp`, `sp`, `status_point`, `skill_point`, `hair`, `hair_color`, `clothes_color`, `sex`, `last_map`, `last_x`, `last_y`.
   * Enforces uniqueness constraints on `(account_id, char_num)` and character `name`, and uses modern 32-bit `u32` (`i32` or `i64` in SQL) for item IDs in inventory tables.

---

## 3. Contract Boundaries (`crates/ro-contracts`)
* **Traits Implemented:**
  * `AccountRepository`: Credential authentication, account lookup, and active `SessionTicket` validation.
  * `CharacterRepository`: Listing characters by `AccountId`, loading a single `CharacterSummary` by `CharId` or `(AccountId, slot)`, creating and deleting characters, and flushing updated `(last_map, last_pos)` state.
* **Upstream Callers:**
  * `bins/login-server` (`LoginService`): Calls `AccountRepository` during `CA_LOGIN` and `/api/v1/sessions/validate`.
  * `bins/char-server` (`CharService`): Calls `CharacterRepository` during `CH_ENTER`, `CH_SELECT_CHAR`, `CH_MAKE_CHAR`, and `CH_DELETE_CHAR`.
  * `bins/map-server` (`MapService`): Calls `CharacterRepository` to load character spawn state on `CZ_ENTER` and persist updated coordinates on map transfer or disconnect.
* **Isolation Invariant:** `crates/ro-persistence` has zero dependency on `ro-packets` or `ro-engine`. It communicates exclusively using domain DTOs (`AccountId`, `CharId`, `CharacterSummary`, `MapPosition`, `Sex`, `SessionTicket`) from `crates/ro-contracts`.
