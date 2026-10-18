# `crates/ro-persistence` Module Rules (PostgreSQL & Repositories)

## 1. Module Ownership & Collaboration Boundaries
* **Designated Owner:** `.agents/agents/ro-persistence.md` (`ro-persistence` custom subagent).
* **Write Scope:** `./crates/ro-persistence/**` exclusively.
* **Design Reference:** Read `./crates/ro-persistence/DESIGN.md` for an initial architectural design to follow. This document serves as a rough directional sketch of where the persistence domain is headed and allows flexibility as implementation details evolve.
* **Contract Inspection & Change Requests:** Read `./crates/ro-contracts/src/**` directly from disk. Never modify `./crates/ro-contracts/**` and never run `cargo doc`. Request any needed contract changes in `ro-contracts` from the Main/Parent Agent.
* **Reference Lookups:** Consult `.agents/agents/reference-explorer.md` (`reference-explorer`) whenever SQL table schemas, column types, or repository patterns from `./reference/**` are needed. Never read `./reference/**` directly.
* **Peer Consultation:** Coordinate with `ro-bins` via subagent messaging when integrating `PostgresRepo` into `login-server`, `char-server`, and `map-server`.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-persistence` requires spawning `.agents/agents/code-reviewer.md` (`code-reviewer`) to audit SQL constraints, thread safety, and edge cases before completion.
* **Red-Green Testing & Spec Collaboration:** Collaborate with `spec-writer` to define repository CRUD, authentication query, character persistence, and schema migration test scenarios supporting high-level bin-observable scenarios. Author or update persistence unit and integration tests upfront (red phase) before writing production code changes.

---

## 2. Repository Architecture & Data Models

`crates/ro-persistence` manages durable and in-memory stub data storage for accounts, authentication sessions, character slots, and saved character state across `login-server`, `char-server`, and `map-server`.

### 2.1 Repository Implementation Strategy
`crates/ro-persistence` standardizes on PostgreSQL persistence via `PostgresRepo`:
* Uses `sqlx::PgPool` to execute asynchronous queries and manage database operations.
* Manages relational tables for:
  * `accounts`: `account_id`, `username`, `password_hash`, `sex`, `email`, `group_id`, `state`, `unban_time`, `expiration_time`.
  * `characters`: `char_id`, `account_id`, `slot`, `name`, `sex`, `job_class`, `base_level`, `job_level`, `base_exp`, `job_exp`, `zeny`, `str`, `agi`, `vit`, `int_stat`, `dex`, `luk`, `max_hp`, `hp`, `max_sp`, `sp`, `status_point`, `skill_point`, `hair_style`, `hair_color`, `clothes_color`, `weapon_view`, `shield_view`, `head_top_view`, `head_mid_view`, `head_bot_view`, `last_map`, `last_x`, `last_y`, `last_dir`, `save_map`, `save_x`, `save_y`, `delete_date`.
* Enforces uniqueness constraints on `(account_id, slot)` and character `name`. Uses modern 32-bit `u32` for item IDs in future inventory tables.

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
