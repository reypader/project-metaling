# `crates/ro-persistence` Module Rules (PostgreSQL & Repositories)

## 1. Module Scope & Operational Guidelines
* **Role & Scope:** Governs database persistence, `InMemoryStubRepo`, PostgreSQL `sqlx` repository implementations (`PostgresRepo`), SQL migrations, and account/character CRUD operations under `./crates/ro-persistence/**`.
* **Design Reference:** Read `./crates/ro-persistence/DESIGN.md` for architectural design direction.
* **Direct Inspection:** Agents working on `./crates/ro-persistence/**` inspect server callers (`bins/*`), docker configs (`tools/docker`), contracts, and reference catalogs directly from disk without messaging barriers.
* **Contract Compliance:** Inspect `./crates/ro-contracts/src/**` directly from disk. If a shared contract requires modification, update it according to task requirements and verify with `cargo check -p ro-contracts`.
* **Reference Access:** Consult `./reference/catalog/` or the `reference-explorer` subagent whenever relational table schemas, column types, or repository patterns are required.
* **Multi-File Review Gate:** Any change spanning more than one file in `./crates/ro-persistence` requires invoking `code-reviewer` to audit SQL constraints, thread safety, and edge cases before completion.
* **Verification Command:** Run `cargo test -p ro-persistence` to verify compilation and test passage.
* **Scripting Delegation:** Delegate one-off migration scripts, seed data parsers, or SQL conversion scripts to `utility-scripter`. Utility scripting is exempt from code and architectural reviews.

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
