# AGENTS.md: `crates/persistence` Subagent (PostgreSQL & Repositories)

## 1. Strict Boundary Rules
* **Read Scope:** `./crates/persistence/**`, `./crates/contracts/**` (including `./crates/contracts/src/**`), and `./reference/**`.
* **Direct Contract Inspection (No `cargo doc`):** Read the source files in `./crates/contracts/src/` directly via file viewing tools. Avoid running `cargo doc` or background documentation builds to inspect contracts, because reading files directly eliminates compilation overhead.
* **Write Scope:** `./crates/persistence/src/**`, `./crates/persistence/migrations/**`, and `./crates/persistence/Cargo.toml` ONLY.
* **Prohibition:** NEVER write to `./crates/contracts/**`, `./bins/*`, or another domain crate (`./crates/<other_domain>/**`). NEVER read implementation files in other domain crates (`./crates/<other_domain>/src/**`). If you need a new shared type or trait in `crates/contracts`, STOP and request a Contract Amendment from the Main Agent.

---

## 2. Domain Architecture & Repository Mechanics

`crates/persistence` (`ro-persistence`) owns durable and in-memory stub data storage for accounts, authentication sessions, character slots, and saved character state across `login-server`, `char-server`, and `map-server`.

### 2.1 Dual Repository Implementation Strategy
1. **`InMemoryStubRepo` (Phases 0 to 3):**
   * Provides a thread-safe (`Send + Sync`) in-memory implementation of the repository traits in `crates/contracts`.
   * Pre-seeds a default test account (`AccountId(2000000)`, username `"testuser"`, `Sex::Male`) and a default character (`CharId(150001)`, slot `0`, name `"Novice"`, job `0`, `last_map: "prontera.gat"`, `last_pos: (156, 191, 0)`).
   * Supports in-memory updates to character position (`last_map`, `last_pos`) during Phase 3 map warps and character creation/deletion in tests without requiring a running PostgreSQL instance.
2. **`PostgresRepo` (Phase 4+):**
   * Uses `sqlx::PgPool` to execute asynchronous queries and manage SQL schema migrations adapted from `reference/rathena/sql-files/main.sql` and `reference/rust-ro/server/src/repository/`.
   * Manages relational tables for:
     * `accounts`: `account_id`, `userid`, `user_pass`, `sex`, `email`, `lastlogin`, `state`.
     * `characters`: `char_id`, `account_id`, `char_num` (`slot`), `name`, `class` (`job_class`), `base_level`, `job_level`, `base_exp`, `job_exp`, `zeny`, `str`, `agi`, `vit`, `int`, `dex`, `luk`, `max_hp`, `hp`, `max_sp`, `sp`, `status_point`, `skill_point`, `hair`, `hair_color`, `clothes_color`, `sex`, `last_map`, `last_x`, `last_y`.
   * Enforces uniqueness constraints on `(account_id, char_num)` and character `name`, and uses modern 32-bit `u32` (`i32`/`i64` in SQL) for item IDs in inventory tables.

---

## 3. Cross-Domain Interface Topology (`crates/contracts`)

* **Traits Implemented by `crates/persistence`:**
  * `AccountRepository`: Credential authentication, account lookup, and active `SessionTicket` validation.
  * `CharacterRepository`: Listing characters by `AccountId`, loading a single `CharacterSummary` by `CharId` or `(AccountId, slot)`, creating/deleting characters, and flushing updated `(last_map, last_pos)` state.
* **Upstream Callers:**
  * `bins/login-server` (`LoginService`): Calls `AccountRepository` during `CA_LOGIN` and `/api/v1/sessions/validate`.
  * `bins/char-server` (`CharService`): Calls `CharacterRepository` during `CH_ENTER`, `CH_SELECT_CHAR`, `CH_MAKE_CHAR`, and `CH_DELETE_CHAR`.
  * `bins/map-server` (`MapService`): Calls `CharacterRepository` to load character spawn state on `CZ_ENTER` and persist updated coordinates on map transfer or disconnect.
* **Isolation Boundary:** `crates/persistence` never imports `ro-packets` or `ro-engine`. It communicates strictly using domain DTOs (`AccountId`, `CharId`, `CharacterSummary`, `MapPosition`, `Sex`, `SessionTicket`) from `crates/contracts`.
