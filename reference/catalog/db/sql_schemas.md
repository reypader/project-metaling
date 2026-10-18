# Relational Persistence Schemas

Target client version: `PACKETVER = 20260219`.

## Relational Persistence Schemas

* **Provenance:** Workspace Architecture Policy (`./AGENTS.md`, Section 5) & `rathena` (`sql-files/main.sql`) & `rust-ro` (`server/src/repository/`), `PACKETVER = 20260219`.
* **Core Phase 4 Entities:**
  * `accounts`: Persists `account_id`, credentials, `sex`, `email`, and login metadata.
  * `characters`: Persists `char_id`, `account_id`, `slot`, `name`, `sex`, `hair_style`, `hair_color`, `last_map`, `last_x`, and `last_y`, along with base/job levels and primary attributes.
  * `inventory`: Persists character item slots using 32-bit `item_id` (`u32` / `INTEGER`), amount, equip bitmask, refine level, and card slots.
* **Catalog Status:** Initialized overview. Detailed column definitions and PostgreSQL migration mappings are populated by `reference-curator` as `reference-explorer` extracts specifications from `reference/rathena/sql-files/main.sql`.
