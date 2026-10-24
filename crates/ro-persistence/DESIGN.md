# Project Metaling: Persistence Architecture & Database Design (`ro-persistence`)

## 1. Domain Scope and Architecture Overview

The `ro-persistence` crate manages durable PostgreSQL repository storage for accounts, credentials, character slots, and character gameplay state across the server cluster. Static game data assets belong to `ro-db`, which loads static YAML databases (`item_db`, `mob_db`, `skill_db`) and `.gat`/`.rsw` map walkability grids. Relational database tables, SQL initialization scripts, and repository implementations belong to `ro-persistence`.

### 1.1 Single Authoritative Repository Architecture (`PostgresRepo`)

The `ro-persistence` crate standardizes on `PostgresRepo` as the single authoritative repository implementation backed by an asynchronous connection pool (`sqlx::PgPool`). All account authentication, credential verification, character slot queries, character creation, deletion reservations, deletion cancellations, and coordinate updates execute directly against PostgreSQL database tables.

### 1.2 Integration with Server Binaries and Contracts

The `PostgresRepo` repository implements the repository traits defined in this crate (`src/repository/`) and is injected into server services:

* `bins/login-server` (`LoginService`): Consumes `AccountRepository` to authenticate credentials during client login (`CA_LOGIN`) and retrieve account records during session ticket validation or account queries.
* `bins/char-server` (`CharService`): Consumes `CharacterRepository` to retrieve character summaries (`list_by_account` for `CH_ENTER`), select active characters (`find_by_slot` for `CH_SELECT_CHAR`), create new characters (`create_character` for `CH_MAKE_CHAR`), and manage character deletion reservations (`reserve_delete` for `CH_DELETE_CHAR3_RESERVED`, `cancel_delete` for `CH_DELETE_CHAR3_CANCEL`).
* `bins/map-server` (`MapService`): Consumes `CharacterRepository` to load character spawn coordinates (`find_by_id` for `CZ_ENTER`) and flush updated coordinates (`last_map`, `last_pos`) upon map transfers, checkpoint saves, or disconnects. Consumes `InventoryRepository` to load the play state (STR, account `group_id`, inventory) at `CZ_ENTER` and to save whole-inventory snapshots on logout, map change and autosave.

Types involved:
* From `ro-core` (shared DTOs): `AccountId`, `CharId`, `Sex`, `MapPosition`, `Account`, `AccountState`, `CharacterSummary`, and `CreateCharacterRequest`.
* Owned by `ro-persistence`: `AccountRepository`, `AuthError`, `CharacterRepository`, `CreateCharacterError`, `DeleteReserveOutcome`, `CancelDeleteOutcome`, `InventoryRepository`, `PlayState`, and `RepositoryError`.

---

## 2. Workload Analysis & Contention Profile

In the server architecture, `char-server` coordinates persistence queries for character records, and `map-server` maintains the authoritative state in its ECS `World` during an active player session.

### 2.1 Single-Owner Entities Have Low Lock Contention

A character exists in only one active `World` at any single millisecond. The `char-server` coordinates periodic state updates from `map-server`. As a result, individual `char_id` records experience near-zero concurrent row-level lock contention.
* Read Profile: Initial load once on character login (`CH_ENTER` -> `CZ_ENTER`).
* Write Profile: Blind periodic checkpoint saves every `N` seconds with per-entity jitter, map transfers, and disconnect flushes.
* Handoff Race Mitigation: During inter-server map transfers, `char-server` updates its active session cache before queueing the asynchronous database flush. The destination `map-server` reads the fresh state directly from the cache.

### 2.2 Relational Database Bottlenecks

Relational schemas encounter performance degradation under load through three distinct mechanisms:
1. Multi-Table Write Amplification: Flushing inventory rows, skills, active statuses, and script variables through individual row queries creates heavy index churn and MVCC dead tuples.
2. Synchronized Checkpoint Spikes: Global autosave timers flushing all online players on the exact same tick create WAL and disk IOPS spikes. Checkpoint saves require per-entity jitter and dirty flag gating.
3. Shared Economic Ledgers: Shared state, such as Account Storage (`storage`), Guild Storage (`guild_storage`), Vending, Trades, and Mail, experiences concurrent access across multiple characters and sessions. These interactions require strict ACID transactions to prevent item or currency duplication.

---

## 3. Vertically Partitioned PostgreSQL Schema Design

To maximize PostgreSQL MVCC performance and eliminate write amplification, `ro-persistence` organizes character persistence into three distinct storage patterns:

```text
┌─────────────────────────────────────────────────────────────────────────┐
│ 1. HOT Scalar Table: `characters` (Fixed ~160B row, fillfactor = 75)    │
│    • Frequent checkpoint updates (pos, HP/SP, exp, zeny, look)          │
│    • Zero mutable columns indexed ──► 100% Heap-Only Tuple (HOT) updates│
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ 1:1 (Flushed ONLY when dirty)
┌────────────────────────────────────▼────────────────────────────────────┐
│ 2. Warm Aggregate Table: `character_blobs` (JSONB)                      │
│    • Single-owner collections never queried cross-character             │
│    • `skills`, `active_statuses`, `script_vars`                         │
└────────────────────────────────────┬────────────────────────────────────┘
                                     │ 1:N (Batch UNNEST + ACID Ledger)
┌────────────────────────────────────▼────────────────────────────────────┐
│ 3. Economic Ledger Tables: `inventory_items`, `storage_items`           │
│    • Relational rows with unique `item_uid` (BIGINT)                    │
│    • Atomic multi-player transfers (Trades, Vending, Storage, Mail)     │
└─────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Pattern 1: Accounts Table (`accounts`)

The `accounts` table stores user authentication records and administrative states, matching `ro_core::Account`. The primary key starts at `2000000` to conform with rAthena conventions:

```sql
CREATE TABLE accounts (
    account_id      INT GENERATED BY DEFAULT AS IDENTITY (START WITH 2000000) PRIMARY KEY,
    username        VARCHAR(23) NOT NULL UNIQUE,
    password_hash   VARCHAR(128) NOT NULL,
    sex             SMALLINT NOT NULL DEFAULT 1,
    email           VARCHAR(39) NOT NULL DEFAULT 'a@a.com',
    group_id        INT NOT NULL DEFAULT 0,
    state           SMALLINT NOT NULL DEFAULT 0,
    unban_time      BIGINT,
    expiration_time BIGINT
);
```

The columns map to domain definitions:
* `state`: Represents `AccountState` (`0 = Active`, `1 = Banned`, `2 = Expired`, `3 = Locked`).
* `sex`: Represents `Sex` (`0 = Female`, `1 = Male`, `2 = Server`).
* `unban_time` and `expiration_time`: Nullable UNIX epoch timestamps in seconds.

### 3.2 Pattern 2: Fixed-Width Scalar Table (`characters`) & HOT Optimization

PostgreSQL writes a new tuple version on every row update. A Heap-Only Tuple (HOT) update allows PostgreSQL to write the new row version on the exact same 8KB disk page without updating any B-Tree indexes. This allows instant single-page opportunistic pruning to recycle dead tuple space on subsequent operations.

For continuous HOT updates, two invariants govern the `characters` table:
1. No Index on Any Mutable Column: Only immutable identity columns (`char_id`, `account_id`, `name`) receive indexes. The mutable columns `last_map`, `last_x`, `last_y`, `last_dir`, `base_level`, and `zeny` remain unindexed.
2. Fixed-Width Rows (~160 Bytes) and `fillfactor = 75`: Variable-length JSONB and unbounded TEXT fields reside in separate aggregate tables to prevent out-of-line TOAST allocation. Reserving 25% free space per page at creation time provides approximately 2 KB of reusable space for continuous in-place updates.

The primary key starts at `150000` to conform with rAthena conventions:

```sql
CREATE TABLE characters (
    char_id       INT GENERATED BY DEFAULT AS IDENTITY (START WITH 150000) PRIMARY KEY,
    account_id    INT NOT NULL REFERENCES accounts(account_id) ON DELETE CASCADE,
    slot          SMALLINT NOT NULL,
    name          VARCHAR(24) NOT NULL UNIQUE,
    sex           SMALLINT NOT NULL,
    job_class     SMALLINT NOT NULL DEFAULT 0,
    base_level    SMALLINT NOT NULL DEFAULT 1,
    job_level     SMALLINT NOT NULL DEFAULT 1,
    base_exp      BIGINT NOT NULL DEFAULT 0,
    job_exp       BIGINT NOT NULL DEFAULT 0,
    zeny          INT NOT NULL DEFAULT 0,
    str           SMALLINT NOT NULL DEFAULT 1,
    agi           SMALLINT NOT NULL DEFAULT 1,
    vit           SMALLINT NOT NULL DEFAULT 1,
    int_stat      SMALLINT NOT NULL DEFAULT 1,
    dex           SMALLINT NOT NULL DEFAULT 1,
    luk           SMALLINT NOT NULL DEFAULT 1,
    max_hp        INT NOT NULL DEFAULT 40,
    hp            INT NOT NULL DEFAULT 40,
    max_sp        INT NOT NULL DEFAULT 11,
    sp            INT NOT NULL DEFAULT 11,
    status_point  SMALLINT NOT NULL DEFAULT 48,
    skill_point   SMALLINT NOT NULL DEFAULT 0,
    hair_style    SMALLINT NOT NULL DEFAULT 1,
    hair_color    SMALLINT NOT NULL DEFAULT 0,
    clothes_color SMALLINT NOT NULL DEFAULT 0,
    weapon_view   INT NOT NULL DEFAULT 0,
    shield_view   INT NOT NULL DEFAULT 0,
    head_top_view INT NOT NULL DEFAULT 0,
    head_mid_view INT NOT NULL DEFAULT 0,
    head_bot_view INT NOT NULL DEFAULT 0,
    last_map      VARCHAR(16) NOT NULL DEFAULT 'prontera.gat',
    last_x        SMALLINT NOT NULL DEFAULT 156,
    last_y        SMALLINT NOT NULL DEFAULT 191,
    last_dir      SMALLINT NOT NULL DEFAULT 0,
    save_map      VARCHAR(16) NOT NULL DEFAULT 'prontera.gat',
    save_x        SMALLINT NOT NULL DEFAULT 156,
    save_y        SMALLINT NOT NULL DEFAULT 191,
    delete_date   BIGINT,
    UNIQUE (account_id, slot)
) WITH (fillfactor = 75);

CREATE INDEX idx_characters_account_id ON characters(account_id);
```

The columns support current contract requirements:
* `last_map`, `last_x`, `last_y`, and `last_dir`: Reflect `MapPosition { x, y, dir }` and map name from `CharacterSummary`.
* `delete_date`: Stores the nullable UNIX epoch timestamp in seconds for deletion reservations created by `reserve_delete` and cleared by `cancel_delete`.

### 3.3 Pattern 3: Single-Owner Aggregate Table (`character_blobs`) (Future Phases)

Semi-static collections owned exclusively by a single character (`skills`, `active_statuses`, `script_vars`) are never queried across characters. Storing them in a 1:1 `JSONB` table eliminates multi-table join overhead on login and protects the `characters` table from storage bloat:

```sql
CREATE TABLE character_blobs (
    char_id         INT PRIMARY KEY REFERENCES characters(char_id) ON DELETE CASCADE,
    skills          JSONB NOT NULL DEFAULT '[]'::jsonb,
    active_statuses JSONB NOT NULL DEFAULT '[]'::jsonb,
    script_vars     JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
) WITH (fillfactor = 80);
```

### 3.4 Pattern 4: Inventory Table (`inventory`)

Built by `specs/items` (spec §2.8). One row per stored item, keyed by an identity `id`, with `char_id` → `characters` (`ON DELETE CASCADE`, indexed on `(char_id, id)`). Columns mirror rAthena's `struct item`: `nameid`, `amount`, `equip`, `identify`, `refine`, `attribute`, `card0..3`, `option_id/val/parm0..4`, `expire_time`, `favorite`, `bound`, `unique_id`, `equip_switch`, `enchantgrade`. `u32` fields are `BIGINT`; the `u64` `unique_id` is stored bit-cast into `BIGINT`. `characters.unique_item_counter` holds the per-character counter for `unique_id = (char_id << 32) | counter`.

* **Load (`load_play_state`):** one query for the `CharacterStatus` columns (STR..LUK, status and skill points, base and job exp), `unique_item_counter` and the account's `group_id`, then the rows `ORDER BY id`. Both reads run in one `REPEATABLE READ READ ONLY` transaction, so a concurrent save can't pair the old counter with the new items. Rows fill slots `0..n`, so runtime holes compact on relog (rAthena does the same). More than 100 rows is an error (the entry is refused) rather than a silent truncation the next save would make permanent; a save with more than 100 slots is refused too.
* **Save (`save_inventory`):** one transaction: `UPDATE characters SET unique_item_counter` (0 rows = `NotFound`), `DELETE` every row of the character, then a single multi-row `INSERT ... SELECT FROM UNNEST(...) WITH ORDINALITY ORDER BY ord`. Ids rise in slot order, so the next load keeps the slot order. The `UPDATE` must run first: its row lock serializes concurrent saves of one character. No per-row diffing: an inventory is at most 100 rows and is written on logout, map change and a 300 s autosave only.
* **Ordering:** the map-server sends inventory and position writes for one character through one ordered writer, so they never reorder (see `bins/map-server`).
* **Future:** storage and cart tables follow the same pattern. Trades and vending will need item moves inside one transaction across two characters.

---

## 4. Checkpoint Protocol & Dirty Bitmasks (Future Phases)

When `map-server` flushes a character state snapshot to `char-server`, the flush payload carries dirty bitmask flags to limit disk updates:
* `DIRTY_SCALARS` (`1 << 0`): Executes the single-row HOT `UPDATE` on `characters` for position, health, spell points, experience, and look values.
* `DIRTY_BLOBS` (`1 << 1`): Executes `UPDATE character_blobs` when skill levels, persistent statuses, or NPC script variables mutate.
* `DIRTY_INVENTORY` (`1 << 2`): Executes the batched `UNNEST` upsert or delete on `inventory_items` when items are gathered, consumed, equipped, or transferred.

---

## 5. SQL Initialization, Seeding, and Phased Implementation Scope

### 5.1 Initialization and Seed Scripts

Database bootstrapping uses SQL scripts located in `crates/ro-persistence/sql/`:
* `crates/ro-persistence/sql/01_schema.sql`: Contains DDL creating the `accounts`, `characters` and `inventory` tables, identity sequences, foreign key constraints, indexes, and storage fillfactors. `PostgresRepo::init_schema` runs it at every server start, so it must stay idempotent and safe to run concurrently: it takes a transaction-scoped advisory lock first, tables use `CREATE TABLE IF NOT EXISTS`, and a column added to an existing table goes in a `DO` block that runs `ALTER TABLE` only when the column is missing (an unconditional `ALTER` takes an exclusive lock and deadlocks with concurrent character writes).
* `crates/ro-persistence/sql/02_seed.sql`: Inserts the baseline development account (`account_id = 2000000`, username `"test"`, password hash `"password"`, `sex = 1`, `email = "test@example.com"`, `state = 0`) with zero initial characters, then sets its `group_id` to 99 (GM) with an `UPDATE`, so older databases get it too.

The local development container configuration in `tools/docker/docker-compose.yml` mounts the `crates/ro-persistence/sql/` directory directly into `/docker-entrypoint-initdb.d/` of the PostgreSQL container. PostgreSQL executes these scripts alphabetically on initial volume creation, establishing tables and seed data automatically.

### 5.2 Phased Scope Matrix

Persistence functionality progresses across planned milestones:

| Component / Feature | Phase 4 Scope | Future Phases Scope |
| :--- | :--- | :--- |
| `accounts` Table | DDL, constraints, identity sequence at 2000000 | Administrative logging and multi-factor flags |
| `characters` Table | DDL, constraints, identity sequence at 150000, HOT config | Extended quest and achievement columns |
| `AccountRepository` | Credential validation, account lookup by ID and username | Session token revocation lists, password hashing upgrades |
| `CharacterRepository` | Listing, slot lookup, creation, delete reservation/cancellation, coordinate updates | Permanent deletion execution, party and guild validation |
| Coordinate Persistence | Flushing `(last_map, last_pos)` on map transfer or logout | Resilient offline reconnect queuing |
| `PostgresRepo` | Authoritative repository backed by `sqlx::PgPool` | Read replica query routing |
| `character_blobs` Table | Deferred | DDL, JSONB serializers, skill/status persistence |
| `inventory` Table / `InventoryRepository` | Whole-inventory snapshot load/save (`specs/items`) | Storage, cart, trade transactions |
| Dirty Checkpoint Protocol | Deferred | Dirty bitmask flags (`DIRTY_SCALARS`, `DIRTY_BLOBS`, `DIRTY_INVENTORY`) |