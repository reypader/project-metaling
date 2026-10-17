# Project Metaling — Persistence Architecture & Database Design (`ro-db`)

Note that this design was written before certain names were standardized. There may be small details that do no directly map to the current state of the project. Take the current state of the project as the source of truth and adapt the design to it. Call out to the user that you will be adjusting certain terms in the design for their review and awareness.

## 1. Workload Analysis & Contention Profile
In the *Ragnarok Online* server architecture, `char-server` (and `ro-db`) is the sole persistence gateway for character records, while `map-server` (`ro-engine`) holds the authoritative in-memory state (`World`) while a character is online[cite: 1, 2].

### 1.1 Single-Owner Entities Have Near-Zero Lock Contention
Because a character can only exist in one active `World` at any given millisecond and `char-server` serializes all periodic state dumps from `map-server`, **individual `char_id` records experience virtually zero concurrent row-level lock contention**[cite: 1, 2].
* **Read Profile:** Cold/warm read once on character login (`CH_ENTER` -> `CZ_ENTER`)[cite: 1].
* **Write Profile:** Blind periodic checkpoint dumps (autosave every `N` seconds with per-entity jitter), map-server handoffs, and graceful disconnects.
* **Handoff Race Mitigation:** During inter-server map transfers, `char-server` updates its in-memory session cache before queueing the asynchronous PostgreSQL flush, ensuring the destination `map-server` reads fresh state without waiting on disk I/O.

### 1.2 The Three Real Database Bottlenecks
Although single-character rows do not suffer from lock contention, naive relational schemas fail under load due to[cite: 1]:
1. **Multi-Table Write Amplification:** Flushing 100 inventory rows, 50 skills, 20 active statuses, and 50 script variables via row-by-row `INSERT/UPDATE/DELETE` queries generates massive index churn and MVCC dead tuples.
2. **Synchronized Checkpoint "Thundering Herds":** Global autosave timers flushing all online players on the same tick spike WAL and disk IOPS. Autosaves MUST be staggered per entity and gated by dirty flags (`PersistenceDirtyFlags`).
3. **True High-Contention Shared Ledgers:** Unlike `char_id`, shared economic state—**Account Storage (`storage`), Guild Storage (`guild_storage`), Vending/Trades, and Mail (Rodex)**—is concurrently accessed by multiple characters/sessions and requires strict ACID transactions to prevent item or Zeny duplication.

---

## 2. Vertically Partitioned PostgreSQL Schema Design
To maximize PostgreSQL MVCC performance and eliminate write amplification, `ro-db` splits character persistence into **three distinct storage patterns**[cite: 1]:

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
│    • Relational rows with unique `item_uid` (UUID / BIGSERIAL)          │
│    • Atomic multi-player transfers (Trades, Vending, Storage, Mail)     │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Pattern 1: Fixed-Width Scalar Table (`characters`) & HOT Optimization
PostgreSQL's MVCC writes a new tuple version on every `UPDATE`. A **Heap-Only Tuple (HOT)** update allows PostgreSQL to write the new row version on the **exact same 8KB disk page** without updating any B-Tree indexes, and allows instant **single-page opportunistic pruning** to recycle dead tuple space on subsequent reads/writes without waiting for `autovacuum`.

For HOT updates to succeed continuously across thousands of saves, two strict invariants must hold on the `characters` table:
1. **No Index on Any Mutable Gameplay Column:** Only immutable identity columns (`char_id`, `account_id`, `name`) may be indexed[cite: 1]. Never add indexes to `last_map`, `last_x`, `last_y`, `base_level`, or `zeny`[cite: 1].
2. **Keep Rows Small (~160 Bytes) & Set `fillfactor = 75` at Table Creation:** Never store variable-length `JSONB` or large `TEXT` fields in `characters` (which would trigger out-of-line TOAST storage and exhaust the 8KB page). By keeping `characters` fixed-width and reserving 25% free space per 8KB page at creation time, each page holds ~2 KB of reusable headroom for continuous in-place HOT chains:

```sql
CREATE TABLE characters (
    char_id       INT GENERATED BY DEFAULT AS IDENTITY PRIMARY KEY,
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
    last_map      VARCHAR(16) NOT NULL DEFAULT 'prontera',
    last_x        SMALLINT NOT NULL DEFAULT 156,
    last_y        SMALLINT NOT NULL DEFAULT 191,
    save_map      VARCHAR(16) NOT NULL DEFAULT 'prontera',
    save_x        SMALLINT NOT NULL DEFAULT 156,
    save_y        SMALLINT NOT NULL DEFAULT 191,
    UNIQUE (account_id, slot)
) WITH (fillfactor = 75);

CREATE INDEX idx_characters_account_id ON characters(account_id);
```

### 2.2 Pattern 2: Single-Owner Sub-Component Table (`character_blobs`)
Transient and semi-static collections owned exclusively by one character (`skills`, `active_statuses`, `script_vars`) are never queried relationally across characters[cite: 1, 2]. Storing them in a separate 1:1 `JSONB` table prevents multi-table `JOIN` overhead on login and protects the `characters` table from TOAST bloat:

```sql
CREATE TABLE character_blobs (
    char_id         INT PRIMARY KEY REFERENCES characters(char_id) ON DELETE CASCADE,
    skills          JSONB NOT NULL DEFAULT '[]'::jsonb,
    active_statuses JSONB NOT NULL DEFAULT '[]'::jsonb,
    script_vars     JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
) WITH (fillfactor = 80);
```

### 2.3 Pattern 3: Relational Economic Ledgers (`inventory_items`, `storage_items`)
All items use **32-bit Item IDs (`INT` / `u32`)** per our modern `PACKETVER` invariant (`ItemId(pub u32)`) and carry a unique 64-bit `item_uid` (`BIGINT`) for anti-dupe auditing[cite: 1].
* **Atomic Cross-Entity Transactions:** Player-to-player trades, vending purchases, and storage transfers MUST execute inside a single `sqlx::Transaction` (`BEGIN ... COMMIT`) that atomically moves `item_uid` ownership and updates `characters.zeny`.
* **Single-Round-Trip Batch Upserts (`UNNEST`):** Never loop over inventory slots issuing individual `INSERT` or `UPDATE` queries in Rust. Use PostgreSQL `UNNEST($1::bigint[], $2::int[], ...)` with `ON CONFLICT (item_uid) DO UPDATE` to flush an entire inventory delta in a single query.

---

## 3. Dirty-Flagged Checkpoint Protocol (`ro-contracts` Integration)
When `map-server` flushes a character state snapshot to `char-server` / `ro-db`, the flush payload must carry a dirty bitmask so unchanged tables are completely skipped[cite: 1]:

* **`DIRTY_SCALARS` (`1 << 0`):** Executes the single-row HOT `UPDATE` on `characters` (position, HP/SP, exp, look)[cite: 1].
* **`DIRTY_BLOBS` (`1 << 1`):** Executes `UPDATE character_blobs` (only set when skills are leveled, persistent statuses change, or NPC script variables mutate).
* **`DIRTY_INVENTORY` (`1 << 2`):** Executes the batched `UNNEST` upsert/delete on `inventory_items` (only set when items are looted, consumed, equipped, or traded).